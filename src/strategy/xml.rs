//! XML detection strategy.
//!
//! This strategy detects XML files based on the XML declaration
//! at the beginning of the file.

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;

/// Number of lines to check at the beginning of the file
const SEARCH_SCOPE: usize = 2;

/// XML detection strategy
#[derive(Debug)]
pub struct Xml;

impl Strategy for Xml {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        // If candidates is not empty, just return them as is
        if !candidates.is_empty() {
            return candidates.to_vec();
        }
        
        // Get the first few lines of the file
        let header = blob.first_lines(SEARCH_SCOPE).join("\n");
        
        // Check for XML declaration
        if header.contains("<?xml version=") {
            if let Some(xml) = Language::find_by_name("XML") {
                return vec![xml.clone()];
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
    fn test_xml_strategy() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Test with XML file
        let xml_path = dir.path().join("data.xml");
        {
            let mut file = File::create(&xml_path)?;
            file.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root></root>")?;
        }
        
        let blob = FileBlob::new(&xml_path)?;
        let strategy = Xml;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "XML");
        
        // Test with non-XML file
        let non_xml_path = dir.path().join("data.txt");
        {
            let mut file = File::create(&non_xml_path)?;
            file.write_all(b"This is not XML content")?;
        }
        
        let blob = FileBlob::new(&non_xml_path)?;
        let languages = strategy.call(&blob, &[]);
        assert!(languages.is_empty());
        
        Ok(())
    }
    
    #[test]
    fn test_xml_strategy_with_candidates() -> crate::Result<()> {
        let dir = tempdir()?;
        let xml_path = dir.path().join("data.xml");
        
        {
            let mut file = File::create(&xml_path)?;
            file.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root></root>")?;
        }
        
        let blob = FileBlob::new(&xml_path)?;
        let strategy = Xml;
        
        // Python in candidates - should just return Python
        let python = Language::find_by_name("Python").unwrap();
        
        let languages = strategy.call(&blob, &[python.clone()]);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "Python");
        
        // Empty candidates - should detect XML
        let languages = strategy.call(&blob, &[]);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "XML");
        
        Ok(())
    }
}