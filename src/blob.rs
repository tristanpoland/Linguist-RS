//! Blob functionality for analyzing file contents.
//!
//! This module provides traits and implementations for accessing and
//! analyzing file contents, both from the filesystem and from git repositories.

use std::cell::UnsafeCell;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use encoding_rs::Encoding;
use encoding_rs_io::DecodeReaderBytesBuilder;
use memmap2::Mmap;
use fancy_regex::Regex;

use crate::generated::Generated;
use crate::language::Language;
use crate::{Error, Result};

// Maximum size to consider for full analysis
const MEGABYTE: usize = 1024 * 1024;

lazy_static::lazy_static! {
    // Regular expression patterns for vendored paths (from vendor.yml)
    static ref VENDORED_REGEXP: Regex = {
        let patterns = vec![
            r"(^|/)cache/",
            r"^[Dd]ependencies/",
            r"(^|/)dist/",
            // Add more patterns from vendor.yml here
        ];
        Regex::new(&patterns.join("|")).unwrap()
    };

    // Regular expression patterns for documentation paths (from documentation.yml)
    static ref DOCUMENTATION_REGEXP: Regex = {
        let patterns = vec![
            r"^[Dd]ocs?/",
            r"(^|/)[Dd]ocumentation/",
            r"(^|/)[Gg]roovydoc/",
            // Add more patterns from documentation.yml here
        ];
        Regex::new(&patterns.join("|")).unwrap()
    };
}

/// Trait for objects that provide blob-like functionality

pub trait BlobHelper {
    /// Get the name/path of the blob
    fn name(&self) -> &str;
    
    /// Get the file extension
    fn extension(&self) -> Option<String>;
    
    /// Get all extensions in a multi-extension filename
    fn extensions(&self) -> Vec<String>;
    
    /// Get the file data
    fn data(&self) -> &[u8];
    
    /// Get the size of the blob in bytes
    fn size(&self) -> usize;
    
    /// Check if the blob is a symlink
    fn is_symlink(&self) -> bool;
    
    /// Check if the file is binary
    fn is_binary(&self) -> bool;
    
    /// Check if the file is likely binary based on its MIME type
    fn likely_binary(&self) -> bool;
    
    /// Check if the file is empty
    fn is_empty(&self) -> bool {
        self.size() == 0 || self.data().is_empty()
    }
    
    /// Check if the file is a text file
    fn is_text(&self) -> bool {
        !self.is_binary()
    }
    
    /// Check if the file is an image
    fn is_image(&self) -> bool {
        match self.extension() {
            Some(ext) => {
                let ext = ext.to_lowercase();
                [".png", ".jpg", ".jpeg", ".gif"].contains(&ext.as_str())
            }
            None => false,
        }
    }
    
    /// Check if the file is vendored
    fn is_vendored(&self) -> bool {
        VENDORED_REGEXP.is_match(self.name()).unwrap_or(false)
    }
    
    /// Check if the file is documentation
    fn is_documentation(&self) -> bool {
        DOCUMENTATION_REGEXP.is_match(self.name()).unwrap_or(false)
    }
    
    /// Check if the file is generated
    fn is_generated(&self) -> bool {
        Generated::is_generated(self.name(), self.data())
    }
    
    /// Get the lines of the file
    fn lines(&self) -> Vec<String> {
        if !self.is_text() || self.is_empty() {
            return Vec::new();
        }
        
        // Convert to UTF-8 string
        let content = match std::str::from_utf8(self.data()) {
            Ok(s) => s.to_string(),
            Err(_) => {
                // Try to detect encoding and convert
                match self.encoding() {
                    Some((encoding, _)) => {
                        let (cow, _, _) = encoding.decode(self.data());
                        cow.into_owned()
                    }
                    None => return Vec::new(), // Cannot decode
                }
            }
        };
        
        content.lines().map(String::from).collect()
    }
    
    /// Get the first n lines
    fn first_lines(&self, n: usize) -> Vec<String> {
        self.lines().into_iter().take(n).collect()
    }
    
    /// Get the last n lines
    fn last_lines(&self, n: usize) -> Vec<String> {
        let lines = self.lines();
        if n >= lines.len() {
            lines
        } else {
            let skip_count = lines.len() - n;
            lines.into_iter().skip(skip_count).collect()
        }
    }
    
    /// Get the number of lines
    fn loc(&self) -> usize {
        self.lines().len()
    }
    
    /// Get the number of non-empty lines
    fn sloc(&self) -> usize {
        self.lines().iter().filter(|line| !line.trim().is_empty()).count()
    }
    
    /// Try to detect the encoding of the file
    fn encoding(&self) -> Option<(&'static Encoding, u32)> {
        if self.is_binary() || self.is_empty() {
            return None;
        }
        
        let (encoding, confidence) = encoding_rs::Encoding::for_bom(self.data())
            .or_else(|| {
                // Try charset detection with a limited sample
                let sample_size = std::cmp::min(self.data().len(), 4096);
                let sample = &self.data()[..sample_size];
                
                // Here we would use an encoding detector similar to CharlockHolmes
                // For simplicity, we'll just default to UTF-8 with medium confidence
                Some((encoding_rs::UTF_8, 60))
            })
            ?;
            
        Some((encoding, confidence.try_into().unwrap()))
    }
    
    /// Get the language of the blob
    fn language(&self) -> Option<Language> {
        crate::detect(self, false)
    }
    
    /// Check if the blob should be included in language statistics
    fn include_in_language_stats(&self) -> bool {
        if self.is_vendored() || self.is_documentation() || self.is_generated() {
            return false;
        }
        
        if let Some(language) = self.language() {
            // Only include programming and markup languages
            matches!(language.language_type, 
                crate::language::LanguageType::Programming | 
                crate::language::LanguageType::Markup)
        } else {
            false
        }
    }
}

/// A blob implementation for files on disk
pub struct FileBlob {
    path: PathBuf,
    name: String,
    data: Vec<u8>,
    symlink: bool,
}

impl FileBlob {
    /// Create a new FileBlob from a path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let name = path.to_string_lossy().to_string();
        
        // Check if it's a symlink
        let symlink = path.symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        
        // Read the file
        let data = if symlink {
            Vec::new()
        } else {
            let mut file = File::open(path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            buffer
        };
        
        Ok(Self {
            path: path.to_path_buf(),
            name,
            data,
            symlink,
        })
    }
    
    /// Create a new FileBlob with in-memory data
    pub fn from_data<P: AsRef<Path>>(path: P, data: Vec<u8>) -> Self {
        let path = path.as_ref();
        let name = path.to_string_lossy().to_string();
        
        Self {
            path: path.to_path_buf(),
            name,
            data,
            symlink: false,
        }
    }
}

impl BlobHelper for FileBlob {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn extension(&self) -> std::option::Option<String> {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))
    }
    
    fn extensions(&self) -> Vec<String> {
        let name = self.path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
            
        let parts: Vec<&str> = name.split('.').collect();
        
        if parts.len() <= 1 {
            return Vec::new();
        }
        
        // Generate extensions like [".html.erb", ".erb"]
        parts[1..].iter()
            .enumerate()
            .map(|(i, _)| {
                let extension = parts[1 + i..].join(".");
                format!(".{}", extension)
            })
            .collect()
    }
    
    fn data(&self) -> &[u8] {
        &self.data
    }
    
    fn size(&self) -> usize {
        self.data.len()
    }
    
    fn is_symlink(&self) -> bool {
        self.symlink
    }
    
    fn is_binary(&self) -> bool {
        // Check for null bytes or non-UTF-8 sequences
        if self.data.is_empty() {
            return false; // Empty files are not binary
        }
        
        // Quick check for null bytes which indicate binary content
        if self.data.contains(&0) {
            return true;
        }
        
        // Try to interpret as UTF-8
        match std::str::from_utf8(&self.data) {
            Ok(_) => false, // Valid UTF-8 is considered text
            Err(_) => true,  // Invalid UTF-8 is considered binary
        }
    }
    
    fn likely_binary(&self) -> bool {
        // Check MIME type based on extension
        if let Some(ext) = self.extension() {
            let ext = ext.to_lowercase();
            
            // Common binary extensions
            if [".png", ".jpg", ".jpeg", ".gif", ".pdf", ".zip", ".gz", 
                ".tar", ".tgz", ".exe", ".dll", ".so", ".o"].contains(&ext.as_str()) {
                return true;
            }
        }
        
        false
    }
}

/// A blob implementation for lazy-loaded git blobs
pub struct LazyBlob {
    repo: Arc<git2::Repository>,
    oid: git2::Oid,
    path: String,
    mode: Option<String>,
    data: UnsafeCell<Option<Vec<u8>>>,
    size: UnsafeCell<Option<usize>>,
}

impl LazyBlob {
    /// Create a new LazyBlob from a git repository
    pub fn new(repo: Arc<git2::Repository>, oid: git2::Oid, path: String, mode: Option<String>) -> Self {
        Self {
            repo,
            oid,
            path,
            mode,
            data: UnsafeCell::new(None),
            size: UnsafeCell::new(None),
        }
    }
    
    /// Load the blob data if not already loaded
    fn load_blob(&self) -> Result<()> {
        // Safety: We're ensuring internal mutability in a controlled way
        // This is safe because we're only modifying the internal state when needed,
        // and the modification is not visible to the outside world other than
        // through the APIs we control
        unsafe {
            let data_ptr = self.data.get();
            let size_ptr = self.size.get();
            
            if (*data_ptr).is_none() {
                let blob = self.repo.find_blob(self.oid)?;
                let blob_data = blob.content().to_vec();
                *size_ptr = Some(blob_data.len());
                *data_ptr = Some(blob_data);
            }
        }
        Ok(())
    }
}

impl BlobHelper for LazyBlob {
    fn name(&self) -> &str {
        &self.path
    }
    
    fn extension(&self) -> Option<String> {
        Path::new(&self.path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))
    }
    
    fn extensions(&self) -> Vec<String> {
        // Implementation unchanged
        let name = Path::new(&self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
            
        let parts: Vec<&str> = name.split('.').collect();
        
        if parts.len() <= 1 {
            return Vec::new();
        }
        
        // Generate extensions like [".html.erb", ".erb"]
        parts[1..].iter()
            .enumerate()
            .map(|(i, _)| {
                let extension = parts[1 + i..].join(".");
                format!(".{}", extension)
            })
            .collect()
    }
    
    fn data(&self) -> &[u8] {
        // First, ensure the data is loaded
        if let Err(_) = self.load_blob() {
            return &[];
        }
        
        // Safety: We know the data exists because we just loaded it,
        // and we're only returning an immutable reference to it
        unsafe {
            if let Some(ref data) = *self.data.get() {
                data
            } else {
                &[]
            }
        }
    }
    
    fn size(&self) -> usize {
        // If size is already calculated, return it
        unsafe {
            if let Some(size) = *self.size.get() {
                return size;
            }
        }
        
        // Otherwise, ensure data is loaded and return its length
        self.data().len()
    }
    
    // Other methods remain unchanged
    fn is_symlink(&self) -> bool {
        // Check if the mode is a symlink (120000 in octal)
        if let Some(ref mode) = self.mode {
            if let Ok(mode_int) = u32::from_str_radix(mode, 8) {
                return (mode_int & 0o170000) == 0o120000;
            }
        }
        false
    }
    
    fn is_binary(&self) -> bool {
        // Implementation unchanged
        let data = self.data();
        
        // Check for null bytes or non-UTF-8 sequences
        if data.is_empty() {
            return false; // Empty files are not binary
        }
        
        // Quick check for null bytes which indicate binary content
        if data.contains(&0) {
            return true;
        }
        
        // Try to interpret as UTF-8
        match std::str::from_utf8(data) {
            Ok(_) => false, // Valid UTF-8 is considered text
            Err(_) => true,  // Invalid UTF-8 is considered binary
        }
    }
    
    fn likely_binary(&self) -> bool {
        // Implementation unchanged
        // Check MIME type based on extension
        if let Some(ext) = self.extension() {
            let ext = ext.to_lowercase();
            
            // Common binary extensions
            if [".png", ".jpg", ".jpeg", ".gif", ".pdf", ".zip", ".gz", 
                ".tar", ".tgz", ".exe", ".dll", ".so", ".o"].contains(&ext.as_str()) {
                return true;
            }
        }
        
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    
    #[test]
    fn test_file_blob() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");
        
        {
            let mut file = File::create(&file_path)?;
            file.write_all(b"This is a test")?;
        }
        
        let blob = FileBlob::new(&file_path)?;
        
        assert_eq!(blob.name(), file_path.to_string_lossy());
        assert_eq!(blob.extension(), Some(".txt".to_string()));
        assert_eq!(blob.data(), b"This is a test");
        assert_eq!(blob.size(), 14);
        assert!(!blob.is_binary());
        assert!(!blob.is_symlink());
        assert!(!blob.is_empty());
        assert!(blob.is_text());
        
        Ok(())
    }
    
    #[test]
    fn test_file_blob_extensions() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.html.erb");
        
        {
            let mut file = File::create(&file_path)?;
            file.write_all(b"<% puts 'Hello' %>")?;
        }
        
        let blob = FileBlob::new(&file_path)?;
        
        let extensions = blob.extensions();
        assert_eq!(extensions.len(), 2);
        assert!(extensions.contains(&".html.erb".to_string()));
        assert!(extensions.contains(&".erb".to_string()));
        
        Ok(())
    }
    
    #[test]
    fn test_binary_detection() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("binary.bin");
        
        {
            let mut file = File::create(&file_path)?;
            file.write_all(&[0, 1, 2, 3, 0, 5])?;
        }
        
        let blob = FileBlob::new(&file_path)?;
        
        assert!(blob.is_binary());
        assert!(!blob.is_text());
        
        Ok(())
    }
}