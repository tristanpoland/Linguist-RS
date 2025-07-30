// Modeline-based language detection strategy.
//
// This strategy detects languages based on Vim and Emacs modelines
// embedded in the file.

use std::collections::HashSet;
use fancy_regex::Regex;

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;

lazy_static::lazy_static! {
    // Updated Emacs modeline regex to handle both formats:
    // -*- mode: ruby -*-  and -*-ruby-*-
    static ref EMACS_MODELINE: Regex = Regex::new(r"(?i)-\*-(?:\s*(?:mode:\s*)?([^:;\s]+)(?:;|(?:\s*-\*-))|\s*(?:[^:]*?:\s*[^;]*?;)*?\s*mode\s*:\s*([^;]+?)(?:;|\s*-\*-))").unwrap();
    
    // Simplified Vim modeline regex
    static ref VIM_MODELINE: Regex = Regex::new(r"(?i)(?:vi|vim|ex)(?:m)?:.+(?:ft|filetype|syntax)\s*=\s*([a-z0-9]+)").unwrap();
    
    // Search scope (number of lines to check at beginning and end of file)
    static ref SEARCH_SCOPE: usize = 5;
}

/// Modeline-based language detection strategy
#[derive(Debug, Clone)]
pub struct Modeline;

impl Modeline {
    /// Extract modeline from content
    ///
    /// # Arguments
    ///
    /// * `content` - The file content
    ///
    /// # Returns
    ///
    /// * `Option<String>` - The detected language name, if found
    fn modeline(content: &str) -> Option<String> {
        // Updated to handle both capture groups in the regex
        if let Ok(Some(captures)) = EMACS_MODELINE.captures(content) {
            // Check first capture group (for -*-ruby-*- format)
            if let Some(mode) = captures.get(1) {
                let mode_str = mode.as_str().trim();
                return Some(mode_str.to_string());
            }
            
            // Check second capture group (for -*- mode: ruby -*- format)
            if let Some(mode) = captures.get(2) {
                let mode_str = mode.as_str().trim();
                return Some(mode_str.to_string());
            }
        }
        
        // Then try Vim modeline
        if let Ok(Some(captures)) = VIM_MODELINE.captures(content) {
            if let Some(mode) = captures.get(1) {
                return Some(mode.as_str().to_string());
            }
        }
        
        None
    }
}

impl Strategy for Modeline {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        // Skip symlinks and binary files
        if blob.is_symlink() || blob.is_binary() {
            return Vec::new();
        }
        
        // Get the first and last few lines
        let lines = blob.first_lines(*SEARCH_SCOPE);
        let header = lines.join("\n");
        
        let last_lines = blob.last_lines(*SEARCH_SCOPE);
        let footer = last_lines.join("\n");
        
        // Combine header and footer for modeline detection
        let content = format!("{}\n{}", header, footer);
        
        if let Some(mode) = Self::modeline(&content) {
            // Try direct language lookup
            if let Some(language) = Language::find_by_name(&mode) {
                // Check if language is in candidates
                if !candidates.is_empty() {
                    if candidates.iter().any(|c| c.name == language.name) {
                        return vec![language.clone()];
                    } else {
                        return Vec::new();
                    }
                } else {
                    return vec![language.clone()];
                }
            }
            
            // Try alias lookup
            if let Some(language) = Language::find_by_alias(&mode) {
                // Check if language is in candidates
                if !candidates.is_empty() {
                    if candidates.iter().any(|c| c.name == language.name) {
                        return vec![language.clone()];
                    } else {
                        return Vec::new();
                    }
                } else {
                    return vec![language.clone()];
                }
            }
            
            // Special case for ruby
            if mode.to_lowercase() == "ruby" {
                if let Some(ruby) = Language::find_by_name("Ruby") {
                    // Check if language is in candidates
                    if !candidates.is_empty() {
                        if candidates.iter().any(|c| c.name == ruby.name) {
                            return vec![ruby.clone()];
                        } else {
                            return Vec::new();
                        }
                    } else {
                        return vec![ruby.clone()];
                    }
                }
            }
        }
        
        Vec::new()
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
    fn test_emacs_modeline() {
        let content = "-*- mode: ruby -*-\nputs 'hello'";
        assert_eq!(Modeline::modeline(content), Some("ruby".to_string()));
        
        let content = "-*-ruby-*-\nputs 'hello'";
        assert_eq!(Modeline::modeline(content), Some("ruby".to_string()));
        
        let content = "-*- foo:bar; mode: python; -*-\nprint('hello')";
        assert_eq!(Modeline::modeline(content), Some("python".to_string()));
    }
    
    #[test]
    fn test_vim_modeline() {
        let content = "#!/bin/sh\n# vim: ft=ruby\nputs 'hello'";
        assert_eq!(Modeline::modeline(content), Some("ruby".to_string()));
        
        let content = "// vim: set syntax=javascript:\nconsole.log('hello')";
        assert_eq!(Modeline::modeline(content), Some("javascript".to_string()));
        
        let content = "/* vim: set filetype=c: */\n#include <stdio.h>";
        assert_eq!(Modeline::modeline(content), Some("c".to_string()));
    }
    
    #[test]
    fn test_modeline_strategy() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Test with Ruby modeline
        let ruby_path = dir.path().join("script");
        {
            let mut file = File::create(&ruby_path)?;
            file.write_all(b"#!/bin/sh\n# vim: ft=ruby\nputs 'hello'")?;
        }
        
        let blob = FileBlob::new(&ruby_path)?;
        let strategy = Modeline;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "Ruby");
        
        // Test with Python modeline
        let py_path = dir.path().join("script");
        {
            let mut file = File::create(&py_path)?;
            file.write_all(b"-*- mode: python -*-\nprint('hello')")?;
        }
        
        let blob = FileBlob::new(&py_path)?;
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "Python");
        
        Ok(())
    }
    
    #[test]
    fn test_modeline_strategy_with_candidates() -> crate::Result<()> {
        let dir = tempdir()?;
        let ruby_path = dir.path().join("script");
        
        {
            let mut file = File::create(&ruby_path)?;
            file.write_all(b"# vim: ft=ruby\nputs 'hello'")?;
        }
        
        let blob = FileBlob::new(&ruby_path)?;
        let strategy = Modeline;
        
        // Ruby in candidates
        let ruby = Language::find_by_name("Ruby").unwrap();
        let python = Language::find_by_name("Python").unwrap();
        
        let languages = strategy.call(&blob, &[ruby.clone(), python.clone()]);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "Ruby");
        
        // Only Python in candidates (no match)
        let languages = strategy.call(&blob, &[python.clone()]);
        assert!(languages.is_empty());
        
        Ok(())
    }
}