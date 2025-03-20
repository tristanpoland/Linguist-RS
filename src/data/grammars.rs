//! TextMate grammar utilities.
//!
//! This module handles TextMate grammar information for syntax highlighting.

use std::path::Path;

/// Get the path to the directory containing language grammar JSON files
///
/// # Returns
///
/// * `&str` - The path to the grammars directory
pub fn path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/grammars")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_path() {
        let grammar_path = path();
        assert!(!grammar_path.is_empty());
    }
}