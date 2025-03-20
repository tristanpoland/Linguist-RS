//! Repository analysis functionality.
//!
//! This module provides structures for analyzing entire repositories
//! and gathering language statistics.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use git2::{Repository as GitRepository, Tree, Oid, ObjectType, FileMode};

use crate::blob::{BlobHelper, LazyBlob, FileBlob};
use crate::{Error, Result};

// Maximum repository tree size to consider for analysis
const MAX_TREE_SIZE: usize = 100_000;

/// Type alias for the cache mapping of filename to (language, size)
type FileStatsCache = HashMap<String, (String, usize)>;

/// Repository analysis results
#[derive(Debug, Clone)]
pub struct LanguageStats {
    /// Breakdown of languages by byte size
    pub language_breakdown: HashMap<String, usize>,
    
    /// Total size in bytes
    pub total_size: usize,
    
    /// Primary language
    pub language: Option<String>,
    
    /// Breakdown of files by language
    pub file_breakdown: HashMap<String, Vec<String>>,
}

/// Repository analysis functionality
pub struct Repository {
    /// The Git repository
    repo: Arc<GitRepository>,
    
    /// The commit ID to analyze
    commit_oid: Oid,
    
    /// Maximum tree size to consider
    max_tree_size: usize,
    
    /// Previous commit ID for incremental analysis
    old_commit_oid: Option<Oid>,
    
    /// Previous analysis results
    old_stats: Option<FileStatsCache>,
    
    /// Analysis cache
    cache: Option<FileStatsCache>,
}

impl Repository {
    /// Create a new Repository for analysis
    ///
    /// # Arguments
    ///
    /// * `repo` - The Git repository
    /// * `commit_oid_str` - The commit ID to analyze
    /// * `max_tree_size` - Maximum tree size to consider
    ///
    /// # Returns
    ///
    /// * `Result<Repository>` - The repository analysis instance
    pub fn new<P: AsRef<Path>>(repo_path: P, commit_oid_str: &str, max_tree_size: Option<usize>) -> Result<Self> {
        let repo = GitRepository::open(repo_path)?;
        let commit_oid = Oid::from_str(commit_oid_str)?;
        
        Ok(Self {
            repo: Arc::new(repo),
            commit_oid,
            max_tree_size: max_tree_size.unwrap_or(MAX_TREE_SIZE),
            old_commit_oid: None,
            old_stats: None,
            cache: None,
        })
    }
    
    /// Create a new Repository for incremental analysis
    ///
    /// # Arguments
    ///
    /// * `repo` - The Git repository
    /// * `commit_oid_str` - The commit ID to analyze
    /// * `old_commit_oid_str` - The previous commit ID
    /// * `old_stats` - The previous analysis results
    /// * `max_tree_size` - Maximum tree size to consider
    ///
    /// # Returns
    ///
    /// * `Result<Repository>` - The repository analysis instance
    pub fn incremental<P: AsRef<Path>>(
        repo_path: P, 
        commit_oid_str: &str, 
        old_commit_oid_str: &str, 
        old_stats: FileStatsCache, 
        max_tree_size: Option<usize>
    ) -> Result<Self> {
        let repo = GitRepository::open(repo_path)?;
        let commit_oid = Oid::from_str(commit_oid_str)?;
        let old_commit_oid = Oid::from_str(old_commit_oid_str)?;
        
        Ok(Self {
            repo: Arc::new(repo),
            commit_oid,
            max_tree_size: max_tree_size.unwrap_or(MAX_TREE_SIZE),
            old_commit_oid: Some(old_commit_oid),
            old_stats: Some(old_stats),
            cache: None,
        })
    }
    
    /// Load existing analysis results
    ///
    /// # Arguments
    ///
    /// * `old_commit_oid_str` - The previous commit ID
    /// * `old_stats` - The previous analysis results
    pub fn load_existing_stats(&mut self, old_commit_oid_str: &str, old_stats: FileStatsCache) -> Result<()> {
        let old_commit_oid = Oid::from_str(old_commit_oid_str)?;
        self.old_commit_oid = Some(old_commit_oid);
        self.old_stats = Some(old_stats);
        Ok(())
    }
    
    /// Get the breakdown of languages in the repository
    ///
    /// # Returns
    ///
    /// * `HashMap<String, usize>` - Mapping of language names to byte sizes
    pub fn languages(&mut self) -> Result<HashMap<String, usize>> {
        let cache = self.get_cache()?;
        
        let mut sizes = HashMap::new();
        for (_, (language, size)) in cache {
            *sizes.entry(language.to_string()).or_insert(0) += size;
        }
        
        Ok(sizes)
    }
    
    /// Get the primary language of the repository
    ///
    /// # Returns
    ///
    /// * `Option<String>` - The primary language name, if determined
    pub fn language(&mut self) -> Result<Option<String>> {
        let languages = self.languages()?;
        
        if languages.is_empty() {
            return Ok(None);
        }
        
        let primary = languages.iter()
            .max_by_key(|&(_, size)| size)
            .map(|(lang, _)| lang.clone());
            
        Ok(primary)
    }
    
    /// Get the total size of the repository
    ///
    /// # Returns
    ///
    /// * `usize` - The total size in bytes
    pub fn size(&mut self) -> Result<usize> {
        let languages = self.languages()?;
        
        let total = languages.values().sum();
        
        Ok(total)
    }
    
    /// Get a breakdown of files by language
    ///
    /// # Returns
    ///
    /// * `HashMap<String, Vec<String>>` - Mapping of language names to file lists
    pub fn breakdown_by_file(&mut self) -> Result<HashMap<String, Vec<String>>> {
        let cache = self.get_cache()?;
        
        let mut breakdown = HashMap::new();
        for (filename, (language, _)) in cache {
            breakdown.entry(language.to_string())
                .or_insert_with(Vec::new)
                .push(filename.to_string());
        }
        
        // Sort filenames for consistent output
        for files in breakdown.values_mut() {
            files.sort();
        }
        
        Ok(breakdown)
    }
    
    /// Get the complete language statistics
    ///
    /// # Returns
    ///
    /// * `Result<LanguageStats>` - The language statistics
    pub fn stats(&mut self) -> Result<LanguageStats> {
        let language_breakdown = self.languages()?;
        let total_size = self.size()?;
        let language = self.language()?;
        let file_breakdown = self.breakdown_by_file()?;
        
        Ok(LanguageStats {
            language_breakdown,
            total_size,
            language,
            file_breakdown,
        })
    }
    
    /// Get the analysis cache
    ///
    /// # Returns
    ///
    /// * `Result<&FileStatsCache>` - The analysis cache
    fn get_cache(&mut self) -> Result<&FileStatsCache> {
        if self.cache.is_none() {
            // Use old stats if commit hasn't changed
            if let Some(old_commit_oid) = self.old_commit_oid {
                if old_commit_oid == self.commit_oid {
                    self.cache = self.old_stats.clone();
                } else {
                    self.cache = Some(self.compute_stats()?);
                }
            } else {
                self.cache = Some(self.compute_stats()?);
            }
        }
        
        Ok(self.cache.as_ref().unwrap())
    }
    
    /// Compute the file stats for the repository
    ///
    /// # Returns
    ///
    /// * `Result<FileStatsCache>` - The computed file stats
    fn compute_stats(&self) -> Result<FileStatsCache> {
        // Check if tree is too large
        let tree_size = self.get_tree_size(self.commit_oid)?;
        if tree_size >= self.max_tree_size {
            return Ok(HashMap::new());
        }
        
        // Set up attribute source for .gitattributes
        self.set_attribute_source(self.commit_oid)?;
        
        let mut file_map = if let Some(old_stats) = &self.old_stats {
            old_stats.clone()
        } else {
            HashMap::new()
        };
        
        // Compute the diff if we have old stats
        if let Some(old_commit_oid) = self.old_commit_oid {
            let old_tree = self.get_tree(old_commit_oid)?;
            let new_tree = self.get_tree(self.commit_oid)?;
            
            let diff = self.repo.diff_tree_to_tree(
                Some(&old_tree),
                Some(&new_tree),
                None
            )?;
            
            // Check if any .gitattributes files were changed
            let mut gitattributes_changed = false;
            for delta in diff.deltas() {
                let new_path = delta.new_file().path().unwrap_or_else(|| Path::new(""));
                if new_path.file_name() == Some(std::ffi::OsStr::new(".gitattributes")) {
                    gitattributes_changed = true;
                    break;
                }
            }
            
            // If gitattributes changed, we need to do a full scan
            if gitattributes_changed {
                file_map.clear();
                
                // Full scan
                let tree = self.get_tree(self.commit_oid)?;
                self.process_tree(&tree, "", &mut file_map)?;
            } else {
                // Process only changed files
                for delta in diff.deltas() {
                    let old_path = delta.old_file().path()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    
                    let new_path = delta.new_file().path()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    
                    // Remove old file from map
                    file_map.remove(&old_path);
                    
                    // Skip if binary or deleted
                    if delta.status() == git2::Delta::Deleted {
                        continue;
                    }
                    
                    // Check if the file is binary by looking at the content
                    let is_binary = if let Ok(blob) = self.repo.find_blob(delta.new_file().id()) {
                        // Quick check for null bytes which indicate binary content
                        blob.content().contains(&0)
                    } else {
                        false
                    };
                    
                    if is_binary {
                        continue;
                    }
                    
                    // Process new/modified file
                    if delta.status() == git2::Delta::Added || delta.status() == git2::Delta::Modified {
                        // Skip submodules and symlinks
                        let mode = delta.new_file().mode();
                        if mode == FileMode::Link || mode == FileMode::Commit {
                            continue;
                        }
                        
                        // Get the blob
                        let oid = delta.new_file().id();
                        let mode_str = format!("{:o}", mode as u32);
                        let blob = LazyBlob::new(
                            self.repo.clone(), 
                            oid, 
                            new_path.clone(), 
                            Some(mode_str)
                        );
                        
                        // Update file map if included in language stats
                        if blob.include_in_language_stats() {
                            if let Some(language) = blob.language() {
                                file_map.insert(new_path, (language.group().unwrap().name.clone(), blob.size()));
                            }
                        }
                    }
                }
            }
        } else {
            // Full scan if no previous stats
            let tree = self.get_tree(self.commit_oid)?;
            self.process_tree(&tree, "", &mut file_map)?;
        }
        
        Ok(file_map)
    }
    
    /// Process a tree recursively
    ///
    /// # Arguments
    ///
    /// * `tree` - The Git tree
    /// * `prefix` - Path prefix for entries
    /// * `file_map` - Map to store results
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error
    fn process_tree(&self, tree: &Tree, prefix: &str, file_map: &mut FileStatsCache) -> Result<()> {
        for entry in tree.iter() {
            let name = entry.name().unwrap_or_default();
            let path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", prefix, name)
            };
            
            match entry.kind() {
                Some(ObjectType::Tree) => {
                    let subtree = self.repo.find_tree(entry.id())?;
                    self.process_tree(&subtree, &path, file_map)?;
                },
                Some(ObjectType::Blob) => {
                    // Skip submodules and symlinks
                    let mode = entry.filemode();
                    if mode == FileMode::Link as i32 || mode == FileMode::Commit as i32 {
                        continue;
                    }
                    
                    // Get the blob
                    let mode_str = format!("{:o}", mode as u32);
                    let blob = LazyBlob::new(
                        self.repo.clone(), 
                        entry.id(), 
                        path.clone(), 
                        Some(mode_str)
                    );
                    
                    // Update file map if included in language stats
                    if blob.include_in_language_stats() {
                        if let Some(language) = blob.language() {
                            file_map.insert(path, (language.group().unwrap().name.clone(), blob.size()));
                        }
                    }
                },
                _ => (), // Skip other types
            }
        }
        
        Ok(())
    }
    
    /// Get the tree for a commit
    ///
    /// # Arguments
    ///
    /// * `oid` - The commit ID
    ///
    /// # Returns
    ///
    /// * `Result<Tree>` - The commit's tree
    fn get_tree(&self, oid: Oid) -> Result<Tree> {
        let commit = self.repo.find_commit(oid)?;
        Ok(commit.tree()?)
    }
    
    /// Get the size of a tree
    ///
    /// # Arguments
    ///
    /// * `oid` - The commit ID
    ///
    /// # Returns
    ///
    /// * `Result<usize>` - The tree size
    fn get_tree_size(&self, oid: Oid) -> Result<usize> {
        let tree = self.get_tree(oid)?;
        let mut count = 0;
        
        // Count recursively up to max tree size
        self.count_tree_entries(&tree, &mut count)?;
        
        Ok(count)
    }
    
    /// Count entries in a tree recursively
    ///
    /// # Arguments
    ///
    /// * `tree` - The tree
    /// * `count` - Running count of entries
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error
    fn count_tree_entries(&self, tree: &Tree, count: &mut usize) -> Result<()> {
        for entry in tree.iter() {
            *count += 1;
            
            // Stop if we reached max tree size
            if *count >= self.max_tree_size {
                return Ok(());
            }
            
            // Recurse into subtrees
            if let Some(ObjectType::Tree) = entry.kind() {
                let subtree = self.repo.find_tree(entry.id())?;
                self.count_tree_entries(&subtree, count)?;
            }
        }
        
        Ok(())
    }
    
    /// Set up attribute source for GitAttributes
    ///
    /// # Arguments
    ///
    /// * `oid` - The commit ID
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error
    fn set_attribute_source(&self, _oid: Oid) -> Result<()> {
        // This is a simplified placeholder
        // In a real implementation, we would set up a real attribute source
        // based on .gitattributes files in the repository
        
        Ok(())
    }
}

/// Analyze a directory on the filesystem
pub struct DirectoryAnalyzer {
    /// Root directory path
    root: PathBuf,
    
    /// Analysis cache
    cache: Option<FileStatsCache>,
}

impl DirectoryAnalyzer {
    /// Create a new DirectoryAnalyzer
    ///
    /// # Arguments
    ///
    /// * `root` - Root directory to analyze
    ///
    /// # Returns
    ///
    /// * `DirectoryAnalyzer` - The analyzer
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            cache: None,
        }
    }
    
    /// Analyze the directory
    ///
    /// # Returns
    ///
    /// * `Result<LanguageStats>` - The language statistics
    pub fn analyze(&mut self) -> Result<LanguageStats> {
        let mut file_map = HashMap::new();
        
        // Traverse the directory
        self.process_directory(&self.root, &mut file_map)?;
        
        self.cache = Some(file_map);
        
        let language_breakdown = self.languages()?;
        let total_size = self.size()?;
        let language = self.language()?;
        let file_breakdown = self.breakdown_by_file()?;
        
        Ok(LanguageStats {
            language_breakdown,
            total_size,
            language,
            file_breakdown,
        })
    }
    
    /// Process a directory recursively
    ///
    /// # Arguments
    ///
    /// * `dir` - Directory to process
    /// * `file_map` - Map to store results
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error
    fn process_directory(&self, dir: &Path, file_map: &mut FileStatsCache) -> Result<()> {
        for entry_result in walkdir::WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
        {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            
            // Skip directories
            if entry.file_type().is_dir() {
                continue;
            }
            
            // Get relative path
            let path = entry.path().strip_prefix(&self.root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .to_string();
                
            // Skip if path is empty
            if path.is_empty() {
                continue;
            }
                
            // Create blob
            let blob = FileBlob::new(entry.path())?;
            
            // Update file map if included in language stats
            if blob.include_in_language_stats() {
                if let Some(language) = blob.language() {
                    file_map.insert(path, (language.group().unwrap().name.clone(), blob.size()));
                }
            }
        }
        
        Ok(())
    }
    
    /// Get the breakdown of languages
    ///
    /// # Returns
    ///
    /// * `Result<HashMap<String, usize>>` - Mapping of language names to byte sizes
    fn languages(&self) -> Result<HashMap<String, usize>> {
        let cache = self.get_cache()?;
        
        let mut sizes = HashMap::new();
        for (_, (language, size)) in cache {
            *sizes.entry(language.to_string()).or_insert(0) += size;
        }
        
        Ok(sizes)
    }
    
    /// Get the primary language
    ///
    /// # Returns
    ///
    /// * `Result<Option<String>>` - The primary language name, if determined
    fn language(&self) -> Result<Option<String>> {
        let languages = self.languages()?;
        
        if languages.is_empty() {
            return Ok(None);
        }
        
        let primary = languages.iter()
            .max_by_key(|&(_, size)| size)
            .map(|(lang, _)| lang.clone());
            
        Ok(primary)
    }
    
    /// Get the total size
    ///
    /// # Returns
    ///
    /// * `Result<usize>` - The total size in bytes
    fn size(&self) -> Result<usize> {
        let languages = self.languages()?;
        
        let total = languages.values().sum();
        
        Ok(total)
    }
    
    /// Get a breakdown of files by language
    ///
    /// # Returns
    ///
    /// * `Result<HashMap<String, Vec<String>>>` - Mapping of language names to file lists
    fn breakdown_by_file(&self) -> Result<HashMap<String, Vec<String>>> {
        let cache = self.get_cache()?;
        
        let mut breakdown = HashMap::new();
        for (filename, (language, _)) in cache {
            breakdown.entry(language.to_string())
                .or_insert_with(Vec::new)
                .push(filename.to_string());
        }
        
        // Sort filenames for consistent output
        for files in breakdown.values_mut() {
            files.sort();
        }
        
        Ok(breakdown)
    }
    
    /// Get the cache
    ///
    /// # Returns
    ///
    /// * `Result<&FileStatsCache>` - The analysis cache
    fn get_cache(&self) -> Result<&FileStatsCache> {
        self.cache.as_ref().ok_or_else(|| Error::Other("Cache not initialized".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    
    #[test]
    fn test_directory_analyzer() -> Result<()> {
        let dir = tempdir()?;
        
        // Create some test files
        let rust_path = dir.path().join("main.rs");
        fs::write(&rust_path, "fn main() { println!(\"Hello, world!\"); }")?;
        
        let js_path = dir.path().join("script.js");
        fs::write(&js_path, "console.log('Hello, world!');")?;
        
        let py_path = dir.path().join("hello.py");
        fs::write(&py_path, "print('Hello, world!')")?;
        
        // Create a subdirectory with more files
        let subdir = dir.path().join("src");
        fs::create_dir(&subdir)?;
        
        let rust2_path = subdir.join("lib.rs");
        fs::write(&rust2_path, "pub fn hello() -> &'static str { \"Hello, world!\" }")?;
        
        // Analyze the directory
        let mut analyzer = DirectoryAnalyzer::new(dir.path());
        let stats = analyzer.analyze()?;
        
        // Verify stats
        assert!(!stats.language_breakdown.is_empty());
        assert!(stats.total_size > 0);
        assert!(stats.language.is_some());
        assert!(!stats.file_breakdown.is_empty());
        
        // Check that Rust files are detected
        assert!(stats.file_breakdown.contains_key("Rust"));
        let rust_files = &stats.file_breakdown["Rust"];
        assert!(rust_files.contains(&"main.rs".to_string()) || rust_files.contains(&"src/lib.rs".to_string()));
        
        // Check that JavaScript files are detected
        assert!(stats.file_breakdown.contains_key("JavaScript"));
        let js_files = &stats.file_breakdown["JavaScript"];
        assert!(js_files.contains(&"script.js".to_string()));
        
        // Check that Python files are detected
        assert!(stats.file_breakdown.contains_key("Python"));
        let py_files = &stats.file_breakdown["Python"];
        assert!(py_files.contains(&"hello.py".to_string()));
        
        Ok(())
    }
}