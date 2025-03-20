//! Manpage detection strategy.
//!
//! This strategy detects man pages based on file extensions.

use fancy_regex::Regex;

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;

lazy_static::lazy_static! {
    // Regular expression for matching conventional manpage extensions
    static ref MANPAGE_EXTS: Regex = Regex::new(r"\.(?:[1-9](?![0-9])[a-z_0-9]*|0p|n|man|mdoc)(?:\.in)?$").unwrap();
}

/// Manpage detection strategy
#[derive(Debug)]
pub struct Manpage;

impl Strategy for Manpage {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        // If candidates is not empty, just return them as is
        if !candidates.is_empty() {
            return candidates.to_vec();
        }
        
        // Check if the filename has a manpage extension
        if MANPAGE_EXTS.is_match(blob.name()).unwrap_or(false) {
            let mut result = Vec::new();
            
            // Add Roff Manpage as the first choice
            if let Some(manpage) = Language::find_by_name("Roff Manpage") {
                result.push(manpage.clone());
            }
            
            // Add Roff as the second choice
            if let Some(roff) = Language::find_by_name("Roff") {
                result.push(roff.clone());
            }
            
            return result;
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
    fn test_manpage_regex() {
        assert!(MANPAGE_EXTS.is_match("file.1").unwrap_or(false));
        assert!(MANPAGE_EXTS.is_match("file.3").unwrap_or(false));
        assert!(MANPAGE_EXTS.is_match("file.man").unwrap_or(false));
        assert!(MANPAGE_EXTS.is_match("file.mdoc").unwrap_or(false));
        assert!(MANPAGE_EXTS.is_match("file.1.in").unwrap_or(false));
        
        assert!(!MANPAGE_EXTS.is_match("file.txt").unwrap_or(false));
        assert!(!MANPAGE_EXTS.is_match("file.10").unwrap_or(false));
        assert!(!MANPAGE_EXTS.is_match("file.c").unwrap_or(false));
    }
    
    #[test]
    fn test_manpage_strategy() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Test with manpage
        let man_path = dir.path().join("test.1");
        {
            let mut file = File::create(&man_path)?;
            file.write_all(b".TH TEST 1\n.SH NAME\ntest - a test command")?;
        }
        
        let blob = FileBlob::new(&man_path)?;
        let strategy = Manpage;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "Roff Manpage");
        assert_eq!(languages[1].name, "Roff");
        
        // Test with non-manpage
        let non_man_path = dir.path().join("test.txt");
        {
            let mut file = File::create(&non_man_path)?;
            file.write_all(b"This is not a manpage")?;
        }
        
        let blob = FileBlob::new(&non_man_path)?;
        let languages = strategy.call(&blob, &[]);
        assert!(languages.is_empty());
        
        Ok(())
    }
    
    #[test]
    fn test_manpage_strategy_with_candidates() -> crate::Result<()> {
        let dir = tempdir()?;
        let man_path = dir.path().join("test.1");
        
        {
            let mut file = File::create(&man_path)?;
            file.write_all(b".TH TEST 1\n.SH NAME\ntest - a test command")?;
        }
        
        let blob = FileBlob::new(&man_path)?;
        let strategy = Manpage;
        
        // With candidates - just return them
        let python = Language::find_by_name("Python").unwrap();
        
        let languages = strategy.call(&blob, &[python.clone()]);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "Python");
        
        Ok(())
    }
}