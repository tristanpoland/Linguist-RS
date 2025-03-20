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
    static ref SHEBANG_REGEX: Regex = Regex::new(r"^#!\s*(?:/usr/bin/env\s+)?([^/\s]+)").unwrap();
    
    // Regex for handling /usr/bin/env with arguments
    static ref ENV_ARGS_REGEX: Regex = Regex::new(r"^#!\s*\S+\s+env\s+((?:-[i0uCSv]*\s+)?|(?:--\S+\s+)?|(?:\S+=\S+\s+)?)(.+)").unwrap();
    
    // Regex for multiline shebang hacks using exec
    static ref EXEC_REGEX: Regex = Regex::new(r#"exec (\w+)[\s'\"]+\$0[\s'\"]+\$@"#).unwrap();
}

/// Shebang-based language detection strategy
#[derive(Debug)]
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
        if !data.starts_with(b"#!") {
            return None;
        }
        
        // Convert to string for regex processing
        let content = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => return None,
        };
        
        // Extract the first line
        let first_line = content.lines().next()?;
        
        // Try to extract the interpreter from the shebang
        if let Ok(Some(captures)) = SHEBANG_REGEX.captures(first_line) {
            let mut interpreter = captures.get(1)?.as_str().to_string();
            
            // If using env with arguments
            if interpreter == "env" {
                if let Ok(Some(captures)) = ENV_ARGS_REGEX.captures(first_line) {
                    interpreter = captures.get(2)?.as_str().to_string();
                }
            }
            
            // Remove version numbers (e.g., "python2.7" -> "python2")
            if let Some(idx) = interpreter.rfind(|c| c == '.') {
                if interpreter[idx+1..].chars().all(|c| c.is_ascii_digit()) {
                    interpreter = interpreter[..idx].to_string();
                }
            }
            
            // Check for multiline shebang hacks that call `exec`
            if interpreter == "sh" {
                // Look at the first few lines for an exec statement
                for line in content.lines().take(5) {
                    if let Ok(Some(captures)) = EXEC_REGEX.captures(line) {
                        interpreter = captures.get(1)?.as_str().to_string();
                        break;
                    }
                }
            }
            
            // Special handling for osascript with -l argument
            if interpreter == "osascript" && first_line.contains("-l") {
                return None;
            }
            
            // Remove path components
            if let Some(idx) = interpreter.rfind('/') {
                interpreter = interpreter[idx+1..].to_string();
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