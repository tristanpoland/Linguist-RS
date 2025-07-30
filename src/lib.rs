//! Linguist library for language detection.
//!
//! This is a Rust port of GitHub's Linguist, which is used to detect programming languages
//! in repositories based on file extensions, filenames, and content analysis.

pub mod blob;
pub mod classifier;
pub mod generated;
pub mod heuristics;
pub mod language;
pub mod repository;
pub mod strategy;
pub mod vendor;
pub mod data;

use std::sync::Arc;
use language::Language;
use strategy::{Strategy, StrategyType};

// Public re-exports
pub use blob::BlobHelper;
pub use language::Language as LanguageType;
pub use repository::Repository;

/// Error type for Linguist operations
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    
    #[error("Yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    
    #[error("Fancy regex error: {0}")]
    FancyRegex(#[from] fancy_regex::Error),
    
    #[error("Encoding error: {0}")]
    Encoding(#[from] std::string::FromUtf8Error),
    
    #[error("Unknown language: {0}")]
    UnknownLanguage(String),
    
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

// Strategies used to detect languages, in order of priority
lazy_static::lazy_static! {
    static ref STRATEGIES: Vec<StrategyType> = vec![
        StrategyType::Modeline(strategy::modeline::Modeline),
        StrategyType::Filename(strategy::filename::Filename),
        StrategyType::Shebang(strategy::shebang::Shebang),
        StrategyType::Extension(strategy::extension::Extension),
        StrategyType::Xml(strategy::xml::Xml),
        StrategyType::Manpage(strategy::manpage::Manpage),
        StrategyType::Heuristics(heuristics::Heuristics),
        StrategyType::Classifier(classifier::Classifier),
    ];
}

/// Detects the language of a blob.
///
/// # Arguments
///
/// * `blob` - A blob object implementing the BlobHelper trait
/// * `allow_empty` - Whether to allow empty files
///
/// # Returns
///
/// * `Option<Language>` - The detected language or None if undetermined
pub fn detect<B: BlobHelper + ?Sized>(blob: &B, allow_empty: bool) -> Option<Language> {
    // Bail early if the blob is binary or empty
    if blob.likely_binary() || blob.is_binary() || (!allow_empty && blob.is_empty()) {
        return None;
    }

    let mut candidates = Vec::new();
    
    // Try each strategy until one returns a single candidate
    for strategy in STRATEGIES.iter() {
        let result = strategy.call(blob, &candidates);
        
        if result.len() == 1 {
            return result.into_iter().next();
        } else if !result.is_empty() {
            candidates = result;
        }
    }
    
    // If we have exactly one candidate at the end, return it
    if candidates.len() == 1 {
        candidates.into_iter().next()
    } else {
        None
    }
}

/// Detects the language of a blob (simplified from parallel version).
///
/// # Arguments
///
/// * `blob` - A blob object implementing the BlobHelper trait
/// * `allow_empty` - Whether to allow empty files
///
/// # Returns
///
/// * `Option<Language>` - The detected language or None if undetermined
pub fn detect_parallel<B: BlobHelper + Send + Sync + 'static>(blob: Arc<B>, allow_empty: bool) -> Option<Language> {
    // Simplified to use the regular detect function
    detect(blob.as_ref(), allow_empty)
}

/// Batch detect languages for multiple blobs in parallel
///
/// # Arguments
///
/// * `blobs` - Vector of blobs to analyze
/// * `allow_empty` - Whether to allow empty files
///
/// # Returns
///
/// * `Vec<Option<Language>>` - Detected languages for each blob
pub fn detect_batch_parallel<B: BlobHelper + Send + Sync + 'static>(
    blobs: Vec<Arc<B>>, 
    allow_empty: bool
) -> Vec<Option<Language>> {
    use rayon::prelude::*;
    
    blobs.par_iter()
        .map(|blob| detect_parallel(blob.clone(), allow_empty))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blob::FileBlob;
    use std::path::Path;
    
    #[test]
    fn test_detect_ruby() {
        // Create a simple Ruby file in memory
        let content = "#!/usr/bin/env ruby\nputs 'Hello, world!'";
        let blob = FileBlob::from_data(Path::new("test.rb"), content.as_bytes().to_vec());
        
        let language = detect(&blob, false).unwrap();
        assert_eq!(language.name, "Ruby");
    }
    
    
    // Add more tests for different language detection scenarios
}