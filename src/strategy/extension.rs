//! Extension-based language detection strategy.
//!
//! This strategy detects languages based on file extensions.

use std::collections::HashSet;
use std::path::Path;

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;

lazy_static::lazy_static! {
    // Generic extensions that should not be considered reliable for language detection
    static ref GENERIC_EXTENSIONS: HashSet<String> = {
        let exts = vec![
            ".1", ".2", ".3", ".4", ".5", ".6", ".7", ".8", ".9",
            ".app", ".cmp", ".msg", ".resource", ".sol", ".stl", ".tag", ".url"
            // Add more generic extensions from generic.yml
        ];
        exts.into_iter().map(String::from).collect()
    };
}

/// Extension-based language detection strategy
#[derive(Debug)]
pub struct Extension;

impl Extension {
    /// Check if a filename has a generic extension
    ///
    /// # Arguments
    ///
    /// * `filename` - The filename to check
    ///
    /// # Returns
    ///
    /// * `bool` - True if the filename has a generic extension
    fn is_generic(filename: &str) -> bool {
        let path = Path::new(filename);
        
        if let Some(ext) = path.extension() {
            let ext_str = format!(".{}", ext.to_string_lossy().to_lowercase());
            return GENERIC_EXTENSIONS.contains(&ext_str);
        }
        
        false
    }
}

impl Strategy for Extension {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        // Skip files with generic extensions
        if Self::is_generic(blob.name()) {
            return candidates.to_vec();
        }
        
        // Find languages by extension
        let languages = Language::find_by_extension(blob.name());
        
        // Filter by candidates if provided
        if !candidates.is_empty() {
            let candidate_set: HashSet<_> = candidates.iter().collect();
            languages.into_iter()
                .filter(|lang| candidate_set.contains(lang))
                .cloned()
                .collect()
        } else {
            languages.into_iter().cloned().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blob::FileBlob;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    
    #[test]
    fn test_extension_strategy() -> crate::Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.rs");
        
        {
            let mut file = File::create(&file_path)?;
            file.write_all(b"fn main() { println!(\"Hello, world!\"); }")?;
        }
        
        let blob = FileBlob::new(&file_path)?;
        let strategy = Extension;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert!(languages.iter().any(|lang| lang.name == "Rust"));
        
        Ok(())
    }
    
    #[test]
    fn test_extension_strategy_with_candidates() -> crate::Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.rs");
        
        {
            let mut file = File::create(&file_path)?;
            file.write_all(b"fn main() { println!(\"Hello, world!\"); }")?;
        }
        
        let blob = FileBlob::new(&file_path)?;
        let strategy = Extension;
        
        // With Rust in candidates
        let rust = Language::find_by_name("Rust").unwrap();
        let python = Language::find_by_name("Python").unwrap();
        
        let languages = strategy.call(&blob, &[rust.clone(), python.clone()]);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "Rust");
        
        // With only Python in candidates (no match)
        let languages = strategy.call(&blob, &[python.clone()]);
        assert!(languages.is_empty());
        
        Ok(())
    }
    
    #[test]
    fn test_generic_extensions() {
        assert!(Extension::is_generic("file.app"));
        assert!(Extension::is_generic("file.resource"));
        assert!(!Extension::is_generic("file.rs"));
        assert!(!Extension::is_generic("file.py"));
    }
}