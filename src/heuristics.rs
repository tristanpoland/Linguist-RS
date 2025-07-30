//! Heuristics for language detection.
//!
//! This module provides heuristics for disambiguating languages
//! with the same file extension.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use fancy_regex::Regex;

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;

// Maximum bytes to consider for heuristic analysis
const HEURISTICS_CONSIDER_BYTES: usize = 50 * 1024;

/// A heuristic rule that can match on file content
#[derive(Debug)]
enum Rule {
    /// Matches when the pattern is found in the content
    Pattern(Regex),
    
    /// Matches when the pattern is NOT found in the content
    NegativePattern(Regex),
    
    /// Matches when all of the sub-rules match
    And(Vec<Rule>),
    
    /// Always matches
    AlwaysMatch,
}

impl Rule {
    /// Check if the rule matches the given content
    fn matches(&self, content: &str) -> bool {
        match self {
            Rule::Pattern(regex) => regex.is_match(content).unwrap_or(false),
            Rule::NegativePattern(regex) => !regex.is_match(content).unwrap_or(false),
            Rule::And(rules) => rules.iter().all(|rule| rule.matches(content)),
            Rule::AlwaysMatch => true,
        }
    }
}

/// A disambiguation rule for a set of file extensions
#[derive(Debug)]
struct Disambiguation {
    /// File extensions this rule applies to
    extensions: Vec<String>,
    
    /// The rules to apply, mapped to their corresponding languages
    rules: Vec<(Rule, Vec<Language>)>,
}

impl Disambiguation {
    /// Check if this disambiguation applies to the given file
    fn matches_extension(&self, filename: &str) -> bool {
        let path = Path::new(filename.to_lowercase().as_str());
        
        for ext in &self.extensions {
            if filename.to_lowercase().ends_with(ext) {
                return true;
            }
        }
        
        false
    }
    
    /// Apply the disambiguation rules to the file content
    fn disambiguate(&self, content: &str, candidates: &[Language]) -> Vec<Language> {
        let candidate_set: HashSet<_> = candidates.iter().collect();
        
        for (rule, languages) in &self.rules {
            if rule.matches(content) {
                // Filter languages by candidates if provided
                if !candidates.is_empty() {
                    return languages.iter()
                        .filter(|lang| candidate_set.contains(lang))
                        .cloned()
                        .collect();
                } else {
                    return languages.clone();
                }
            }
        }
        
        Vec::new()
    }
}

lazy_static::lazy_static! {
    static ref DISAMBIGUATIONS: Vec<Disambiguation> = {
        // Manually define disambiguation rules
        // These are based on the rules in heuristics.yml
        
        let mut disambiguations = Vec::new();
        
        // C/C++ Header disambiguation
        let mut cpp_extensions = vec![".h".to_string()];
        
        let cpp_rule = Rule::Pattern(Regex::new(r#"^\s*#\s*include <(cstdint|string|vector|map|list|array|bitset|queue|stack|forward_list|unordered_map|unordered_set|(i|o|io)stream)>"#).unwrap());
        let objective_c_rule = Rule::Pattern(Regex::new(r#"^\s*(@(interface|class|protocol|property|end|synchronised|selector|implementation)\b|#import\s+.+\.h[">])"#).unwrap());
        
        let cpp_langs = Language::find_by_name("C++")
            .map(|lang| vec![lang.clone()])
            .unwrap_or_default();
        let objc_langs = Language::find_by_name("Objective-C")
            .map(|lang| vec![lang.clone()])
            .unwrap_or_default();
        let c_langs = Language::find_by_name("C")
            .map(|lang| vec![lang.clone()])
            .unwrap_or_default();
        
        disambiguations.push(Disambiguation {
            extensions: cpp_extensions,
            rules: vec![
                (objective_c_rule, objc_langs),
                (cpp_rule, cpp_langs.clone()),
                (Rule::AlwaysMatch, c_langs),
            ],
        });
        
        // JavaScript/JSX disambiguation
        let js_extensions = vec![".js".to_string()];
        
        let jsx_rule = Rule::Pattern(Regex::new(r"import\s+React|\bReact\.|<[A-Z][A-Za-z]+>|<\/[A-Z][A-Za-z]+>|<[A-Z][A-Za-z]+\s").unwrap());
        
        let js_langs = vec![Language::find_by_name("JavaScript").unwrap().clone()];
        let jsx_langs = if let Some(jsx) = Language::find_by_name("JSX") {
            vec![jsx.clone()]
        } else {
            js_langs.clone()
        };
        
        disambiguations.push(Disambiguation {
            extensions: js_extensions,
            rules: vec![
                (jsx_rule, jsx_langs),
                (Rule::AlwaysMatch, js_langs),
            ],
        });
        
        // Add more disambiguations here...
        
        disambiguations
    };
}

/// Heuristics language detection strategy
#[derive(Debug, Clone)]
pub struct Heuristics;

impl Strategy for Heuristics {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        // Return early if the blob is binary
        if blob.is_binary() || blob.is_symlink() {
            return Vec::new();
        }
        
        // Get the data for analysis, limited to a reasonable size
        let data_bytes = blob.data();
        let consider_bytes = std::cmp::min(data_bytes.len(), HEURISTICS_CONSIDER_BYTES);
        let data_slice = &data_bytes[..consider_bytes];
        
        // Convert to string for pattern matching
        let content = match std::str::from_utf8(data_slice) {
            Ok(s) => s,
            Err(_) => return Vec::new(), // Binary content
        };
        
        // Find a disambiguation that matches the file extension
        for disambiguation in DISAMBIGUATIONS.iter() {
            if disambiguation.matches_extension(blob.name()) {
                let result = disambiguation.disambiguate(content, candidates);
                if !result.is_empty() {
                    return result;
                }
            }
        }
        
        // No matches found, return empty
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
    fn test_cpp_header_heuristic() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Test C++ header
        let cpp_path = dir.path().join("vector.h");
        {
            let mut file = File::create(&cpp_path)?;
            file.write_all(b"#include <vector>\n#include <string>\n")?;
        }
        
        let blob = FileBlob::new(&cpp_path)?;
        let strategy = Heuristics;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "C++");
        
        // Test C header
        let c_path = dir.path().join("stdio.h");
        {
            let mut file = File::create(&c_path)?;
            file.write_all(b"#include <stdio.h>\n#include <stdlib.h>\n")?;
        }
        
        let blob = FileBlob::new(&c_path)?;
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "C");
        
        Ok(())
    }
    
    #[test]
    fn test_objective_c_heuristic() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Test Objective-C header
        let objc_path = dir.path().join("view.h");
        {
            let mut file = File::create(&objc_path)?;
            file.write_all(b"#import <UIKit/UIKit.h>\n@interface MyView : UIView\n@end")?;
        }
        
        let blob = FileBlob::new(&objc_path)?;
        let strategy = Heuristics;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "Objective-C");
        
        Ok(())
    }
    
    #[test]
    fn test_jsx_heuristic() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Skip this test if JSX language isn't available
        if Language::find_by_name("JSX").is_none() {
            return Ok(());
        }
        
        // Test JSX file
        let jsx_path = dir.path().join("component.js");
        {
            let mut file = File::create(&jsx_path)?;
            file.write_all(b"import React from 'react';\nexport default () => <div>Hello</div>;")?;
        }
        
        let blob = FileBlob::new(&jsx_path)?;
        let strategy = Heuristics;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "JSX");
        
        // Test plain JavaScript
        let js_path = dir.path().join("script.js");
        {
            let mut file = File::create(&js_path)?;
            file.write_all(b"function hello() { return 'world'; }")?;
        }
        
        let blob = FileBlob::new(&js_path)?;
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "JavaScript");
        
        Ok(())
    }
    
    #[test]
    fn test_heuristics_with_candidates() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Test C++ header with candidates
        let cpp_path = dir.path().join("vector.h");
        {
            let mut file = File::create(&cpp_path)?;
            file.write_all(b"#include <vector>\n#include <string>\n")?;
        }
        
        let blob = FileBlob::new(&cpp_path)?;
        let strategy = Heuristics;
        
        // With C and C++ in candidates
        let c = Language::find_by_name("C").unwrap();
        let cpp = Language::find_by_name("C++").unwrap();
        
        let languages = strategy.call(&blob, &[c.clone(), cpp.clone()]);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "C++");
        
        // With only C in candidates (no match from heuristic rule)
        let languages = strategy.call(&blob, &[c.clone()]);
        assert!(languages.is_empty());
        
        Ok(())
    }
}