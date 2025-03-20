//! Sample code utilities.
//!
//! This module provides functionality for accessing sample code files
//! used in training the classifier.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::Result;

// Path to the samples directory
const SAMPLES_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples");

/// Sample information structure
#[derive(Debug, Clone)]
pub struct Sample {
    /// Path to the sample file
    pub path: PathBuf,
    
    /// Language of the sample
    pub language: String,
    
    /// Filename of the sample (for filename samples)
    pub filename: Option<String>,
    
    /// Interpreter of the sample (for interpreter samples)
    pub interpreter: Option<String>,
    
    /// Extension of the sample
    pub extension: Option<String>,
}

/// Load sample data from the samples directory
///
/// # Returns
///
/// * `Result<HashMap<String, Vec<Sample>>>` - Mapping of language names to samples
pub fn load_samples() -> Result<HashMap<String, Vec<Sample>>> {
    let mut samples = HashMap::new();
    
    // Check if samples directory exists
    if !Path::new(SAMPLES_ROOT).exists() {
        return Ok(samples);
    }
    
    // Iterate through language directories
    for entry in fs::read_dir(SAMPLES_ROOT)? {
        let entry = entry?;
        let language_path = entry.path();
        
        // Skip non-directories
        if !language_path.is_dir() {
            continue;
        }
        
        let language_name = language_path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
            
        if language_name == "." || language_name == ".." {
            continue;
        }
        
        let mut language_samples = Vec::new();
        
        // Iterate through sample files
        for sample_entry in fs::read_dir(&language_path)? {
            let sample_entry = sample_entry?;
            let sample_path = sample_entry.path();
            
            let sample_name = sample_path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
                
            if sample_name == "." || sample_name == ".." {
                continue;
            }
            
            if sample_name == "filenames" {
                // Process filename samples
                if sample_path.is_dir() {
                    for filename_entry in fs::read_dir(&sample_path)? {
                        let filename_entry = filename_entry?;
                        let filename_path = filename_entry.path();
                        
                        let filename = filename_path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default()
                            .to_string();
                            
                        if filename == "." || filename == ".." {
                            continue;
                        }
                        
                        language_samples.push(Sample {
                            path: filename_path.clone(),
                            language: language_name.clone(),
                            filename: Some(filename),
                            interpreter: None,
                            extension: None,
                        });
                    }
                }
            } else {
                // Process regular samples
                let extension = sample_path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| format!(".{}", ext));
                    
                // Try to detect interpreter from shebang
                let mut interpreter = None;
                if let Ok(mut file) = File::open(&sample_path) {
                    let mut content = vec![0; 1024]; // Read first 1KB
                    if let Ok(bytes_read) = file.read(&mut content) {
                        content.truncate(bytes_read);
                        
                        if bytes_read > 2 && content[0] == b'#' && content[1] == b'!' {
                            // Extract interpreter from shebang
                            if let Ok(text) = String::from_utf8(content.clone()) {
                                if let Some(first_line) = text.lines().next() {
                                    if first_line.starts_with("#!") {
                                        interpreter = crate::strategy::shebang::Shebang::interpreter(content.as_slice());
                                    }
                                }
                            }
                        }
                    }
                }
                
                language_samples.push(Sample {
                    path: sample_path.clone(),
                    language: language_name.clone(),
                    filename: None,
                    interpreter,
                    extension,
                });
            }
        }
        
        if !language_samples.is_empty() {
            samples.insert(language_name, language_samples);
        }
    }
    
    Ok(samples)
}

/// Extract file extensions and interpreters from samples
///
/// # Returns
///
/// * `HashMap<String, HashMap<String, Vec<String>>>` - Map of languages to extension and interpreter data
pub fn extract_sample_data() -> Result<HashMap<String, HashMap<String, Vec<String>>>> {
    let samples = load_samples()?;
    
    let mut data = HashMap::new();
    
    for (language, samples) in samples {
        let mut language_data = HashMap::new();
        let mut extensions = Vec::new();
        let mut interpreters = Vec::new();
        let mut filenames = Vec::new();
        
        for sample in samples {
            if let Some(ext) = sample.extension {
                if !extensions.contains(&ext) {
                    extensions.push(ext);
                }
            }
            
            if let Some(interpreter) = sample.interpreter {
                if !interpreters.contains(&interpreter) {
                    interpreters.push(interpreter);
                }
            }
            
            if let Some(filename) = sample.filename {
                if !filenames.contains(&filename) {
                    filenames.push(filename);
                }
            }
        }
        
        if !extensions.is_empty() {
            language_data.insert("extensions".to_string(), extensions);
        }
        
        if !interpreters.is_empty() {
            language_data.insert("interpreters".to_string(), interpreters);
        }
        
        if !filenames.is_empty() {
            language_data.insert("filenames".to_string(), filenames);
        }
        
        if !language_data.is_empty() {
            data.insert(language, language_data);
        }
    }
    
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_samples() {
        // This test will be skipped if the samples directory doesn't exist
        if !Path::new(SAMPLES_ROOT).exists() {
            return;
        }
        
        let samples = load_samples().unwrap();
        
        // Check that we have samples
        assert!(!samples.is_empty());
        
        // Check that we have samples for common languages
        assert!(samples.contains_key("JavaScript") || 
                samples.contains_key("Python") || 
                samples.contains_key("Ruby"));
    }
    
    #[test]
    fn test_extract_sample_data() {
        // This test will be skipped if the samples directory doesn't exist
        if !Path::new(SAMPLES_ROOT).exists() {
            return;
        }
        
        let data = extract_sample_data().unwrap();
        
        // Check that we have data
        assert!(!data.is_empty());
        
        // Check that we have data for common languages
        for lang in &["JavaScript", "Python", "Ruby"] {
            if data.contains_key(*lang) {
                let lang_data = &data[*lang];
                
                // Check that we have extensions or interpreters
                assert!(lang_data.contains_key("extensions") || 
                        lang_data.contains_key("interpreters") ||
                        lang_data.contains_key("filenames"));
                
                // If we have extensions, check they're not empty
                if let Some(extensions) = lang_data.get("extensions") {
                    assert!(!extensions.is_empty());
                }
                
                // If we have interpreters, check they're not empty
                if let Some(interpreters) = lang_data.get("interpreters") {
                    assert!(!interpreters.is_empty());
                }
                
                // If we have filenames, check they're not empty
                if let Some(filenames) = lang_data.get("filenames") {
                    assert!(!filenames.is_empty());
                }
            }
        }
    }
}