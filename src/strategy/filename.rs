//! Filename-based language detection strategy.
//!
//! This strategy detects languages based on exact filenames.

use std::collections::HashSet;
use std::path::Path;

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;

/// Filename-based language detection strategy
#[derive(Debug, Clone)]
pub struct Filename;

impl Strategy for Filename {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        // Extract the basename from the path
        let path = Path::new(blob.name());
        let filename = path.file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        
        // Find languages by filename
        let languages = Language::find_by_filename(filename);
        
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
    fn test_filename_strategy() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Test with Dockerfile
        let dockerfile_path = dir.path().join("Dockerfile");
        {
            let mut file = File::create(&dockerfile_path)?;
            file.write_all(b"FROM ubuntu:20.04")?;
        }
        
        let blob = FileBlob::new(&dockerfile_path)?;
        let strategy = Filename;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert!(languages.iter().any(|lang| lang.name == "Dockerfile"));
        
        // Test with Makefile
        let makefile_path = dir.path().join("Makefile");
        {
            let mut file = File::create(&makefile_path)?;
            file.write_all(b"all:\n\techo \"Hello\"")?;
        }
        
        let blob = FileBlob::new(&makefile_path)?;
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert!(languages.iter().any(|lang| lang.name == "Makefile"));
        
        Ok(())
    }
    
    #[test]
    fn test_filename_strategy_with_candidates() -> crate::Result<()> {
        let dir = tempdir()?;
        let dockerfile_path = dir.path().join("Dockerfile");
        
        {
            let mut file = File::create(&dockerfile_path)?;
            file.write_all(b"FROM ubuntu:20.04")?;
        }
        
        let blob = FileBlob::new(&dockerfile_path)?;
        let strategy = Filename;
        
        // Dockerfile in candidates
        let dockerfile = Language::find_by_name("Dockerfile").unwrap();
        let python = Language::find_by_name("Python").unwrap();
        
        let languages = strategy.call(&blob, &[dockerfile.clone(), python.clone()]);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "Dockerfile");
        
        // Only Python in candidates (no match)
        let languages = strategy.call(&blob, &[python.clone()]);
        assert!(languages.is_empty());
        
        Ok(())
    }
}