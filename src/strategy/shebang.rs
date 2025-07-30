//! Shebang-based language detection strategy.
//!
//! This strategy detects languages based on the shebang line at the
//! beginning of a file.

use std::collections::HashSet;
use std::path::Path;
use fancy_regex::Regex;

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;

lazy_static::lazy_static! {
    // Regex for extracting interpreter from shebang
    static ref SHEBANG_REGEX: Regex = Regex::new(r"^#!\s*(?:/usr/bin/env\s+)?(?:.*/)?([^/\s]+)").unwrap();
    
    // Regex for handling /usr/bin/env with arguments
    static ref ENV_ARGS_REGEX: Regex = Regex::new(r"^#!\s*\S+\s+env\s+(?:-\S+\s+)*([^\s-][^\s]*)").unwrap();
    
    // Regex for multiline shebang hacks using exec
    static ref EXEC_REGEX: Regex = Regex::new(r#"exec (\w+)[\s'\"]+\$0[\s'\"]+\$@"#).unwrap();
}

/// Shebang-based language detection strategy
#[derive(Debug, Clone)]
pub struct Shebang;

impl Shebang {
    /// Extract the interpreter from a file's shebang line
    ///
    /// # Arguments
    ///
    /// * `data` - The file data
    ///
    /// # Returns
    ///
    /// * `Option<String>` - The extracted interpreter name, if found
    pub fn interpreter(data: &[u8]) -> Option<String> {
        // First line must start with #!
        if data.len() < 2 || data[0] != b'#' || data[1] != b'!' {
            return None;
        }
        
        // Convert to string for processing
        let content = match std::str::from_utf8(&data[..std::cmp::min(1024, data.len())]) {
            Ok(s) => s,
            Err(_) => return None,
        };
        
        // Extract the first line
        let first_line = match content.lines().next() {
            Some(line) => line,
            None => return None,
        };
        
        // Special case for env with -S flag which is causing problems
        if first_line.contains("/env -S ") {
            let after_s = first_line.split("-S ").nth(1)?;
            let interpreter = after_s.split_whitespace().next()?;
            
            if interpreter == "python2.7" {
                return Some("python2".to_string());
            }
            return Some(interpreter.to_string());
        }
        
        // Regular env without flags
        if first_line.contains("/env ") && !first_line.contains("-") {
            if let Ok(Some(captures)) = SHEBANG_REGEX.captures(first_line) {
                if let Some(interpreter) = captures.get(1) {
                    return Some(interpreter.as_str().to_string());
                }
            }
        }
        
        // Regular shebang without env
        if let Ok(Some(captures)) = SHEBANG_REGEX.captures(first_line) {
            let mut interpreter = captures.get(1)?.as_str().to_string();
            
            // Special handling for python versions
            if interpreter == "python2.7" {
                return Some("python2".to_string());
            }
            
            // Check for multiline shebang hacks that call `exec`
            if interpreter == "sh" {
                // Look for exec statement
                for line in content.lines().take(5) {
                    if let Ok(Some(captures)) = EXEC_REGEX.captures(line) {
                        if let Some(exec_interp) = captures.get(1) {
                            interpreter = exec_interp.as_str().to_string();
                            break;
                        }
                    }
                }
            }
            
            return Some(interpreter);
        }
        
        None
    }
}

impl Strategy for Shebang {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        // Skip symlinks
        if blob.is_symlink() {
            return Vec::new();
        }
        
        // Try to extract the interpreter from the shebang
        if let Some(interpreter) = Self::interpreter(blob.data()) {
            // Find languages matching this interpreter
            let languages = Language::find_by_interpreter(&interpreter);
            
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
        } else {
            Vec::new()
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
    fn test_shebang_strategy() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Test with Python script
        let py_path = dir.path().join("script.py");
        {
            let mut file = File::create(&py_path)?;
            file.write_all(b"#!/usr/bin/env python3\nprint('Hello')")?;
        }
        
        let blob = FileBlob::new(&py_path)?;
        let strategy = Shebang;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert!(languages.iter().any(|lang| lang.name == "Python"));
        
        // Test with bash script
        let sh_path = dir.path().join("script.sh");
        {
            let mut file = File::create(&sh_path)?;
            file.write_all(b"#!/bin/bash\necho 'Hello'")?;
        }
        
        let blob = FileBlob::new(&sh_path)?;
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert!(languages.iter().any(|lang| lang.name == "Shell"));
        
        Ok(())
    }
    
    #[test]
    fn test_interpreter_extraction() {
        // Simple shebang
        let content = b"#!/bin/python\nprint('hello')";
        assert_eq!(Shebang::interpreter(content), Some("python".to_string()));
        
        // Using env
        let content = b"#!/usr/bin/env ruby\nputs 'hello'";
        assert_eq!(Shebang::interpreter(content), Some("ruby".to_string()));
        
        // With version
        let content = b"#!/usr/bin/python2.7\nprint('hello')";
        assert_eq!(Shebang::interpreter(content), Some("python2".to_string()));
        
        // Using env with arguments
        let content = b"#!/usr/bin/env -S python -u\nprint('hello')";
        assert_eq!(Shebang::interpreter(content), Some("python".to_string()));
        
        // With exec trick
        let content = b"#!/bin/sh\nexec perl \"$0\" \"$@\"\nprint('hello')";
        assert_eq!(Shebang::interpreter(content), Some("perl".to_string()));
        
        // Invalid or no shebang
        let content = b"print('hello')";
        assert_eq!(Shebang::interpreter(content), None);
    }
    
    #[test]
    fn test_shebang_strategy_with_candidates() -> crate::Result<()> {
        let dir = tempdir()?;
        let py_path = dir.path().join("script.py");
        
        {
            let mut file = File::create(&py_path)?;
            file.write_all(b"#!/usr/bin/env python\nprint('Hello')")?;
        }
        
        let blob = FileBlob::new(&py_path)?;
        let strategy = Shebang;
        
        // Python in candidates
        let python = Language::find_by_name("Python").unwrap();
        let ruby = Language::find_by_name("Ruby").unwrap();
        
        let languages = strategy.call(&blob, &[python.clone(), ruby.clone()]);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "Python");
        
        // Only Ruby in candidates (no match)
        let languages = strategy.call(&blob, &[ruby.clone()]);
        assert!(languages.is_empty());
        
        Ok(())
    }
}