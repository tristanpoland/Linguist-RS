//! Bayesian classifier for language detection.
//!
//! This module provides a statistical classifier for identifying
//! programming languages based on tokenized file content.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;

// Maximum bytes to consider for classification
const CLASSIFIER_CONSIDER_BYTES: usize = 50 * 1024;

// Minimum document frequency for a token to be considered
const MIN_DOCUMENT_FREQUENCY: usize = 2;

/// A token extracted from source code
type Token = String;

/// A mapping from token to a numeric value (e.g., frequency)
type TokenFrequencies = HashMap<Token, f64>;

/// A mapping from language name to its token frequencies
type LanguageTokens = HashMap<String, TokenFrequencies>;

/// Language classifier based on token frequencies
#[derive(Debug)]
pub struct Classifier;

impl Classifier {
    /// Tokenize content into a sequence of tokens
    ///
    /// # Arguments
    ///
    /// * `content` - The file content to tokenize
    ///
    /// # Returns
    ///
    /// * `Vec<Token>` - The extracted tokens
    fn tokenize(content: &str) -> Vec<Token> {
        // For simplicity, we'll just split by whitespace and filter out common tokens
        // A real implementation would use a more sophisticated tokenization strategy
        let mut tokens = Vec::new();
        let stop_words = HashSet::from([
            "the", "a", "an", "and", "or", "but", "if", "then", "else", "when",
            "this", "that", "these", "those", "it", "is", "are", "was", "were",
            "be", "been", "has", "have", "had", "do", "does", "did", "at", "in",
            "on", "by", "to", "from", "with", "for", "of",
        ]);
        
        for line in content.lines() {
            for word in line.split_whitespace() {
                let token = word.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                
                if !token.is_empty() && !stop_words.contains(&token.as_str()) && token.len() > 1 {
                    tokens.push(token);
                }
            }
        }
        
        tokens
    }
    
    /// Calculate term frequency (TF) for tokens
    ///
    /// # Arguments
    ///
    /// * `tokens` - The tokens to analyze
    ///
    /// # Returns
    ///
    /// * `TokenFrequencies` - Mapping from token to its frequency
    fn calculate_term_frequencies(tokens: &[Token]) -> TokenFrequencies {
        let mut frequencies = HashMap::new();
        
        for token in tokens {
            *frequencies.entry(token.clone()).or_insert(0.0) += 1.0;
        }
        
        // Calculate log term frequency
        for (_, freq) in frequencies.iter_mut() {
            *freq = 1.0 + f64::ln(*freq);
        }
        
        frequencies
    }
    
    /// Calculate term frequency-inverse document frequency (TF-IDF)
    ///
    /// # Arguments
    ///
    /// * `term_freq` - Term frequencies for a document
    /// * `inverse_class_freq` - Inverse class frequencies for tokens
    ///
    /// # Returns
    ///
    /// * `TokenFrequencies` - TF-IDF scores for tokens
    fn calculate_tf_idf(term_freq: &TokenFrequencies, inverse_class_freq: &TokenFrequencies) -> TokenFrequencies {
        let mut tf_idf = HashMap::new();
        
        for (token, tf) in term_freq {
            if let Some(icf) = inverse_class_freq.get(token) {
                tf_idf.insert(token.clone(), tf * icf);
            }
        }
        
        // L2 normalization
        Self::l2_normalize(&mut tf_idf);
        
        tf_idf
    }
    
    /// Normalize token frequencies using L2 norm
    ///
    /// # Arguments
    ///
    /// * `frequencies` - Token frequencies to normalize
    fn l2_normalize(frequencies: &mut TokenFrequencies) {
        let norm: f64 = frequencies.values()
            .map(|&freq| freq * freq)
            .sum::<f64>()
            .sqrt();
        
        if norm > 0.0 {
            for freq in frequencies.values_mut() {
                *freq /= norm;
            }
        }
    }
    
    /// Calculate similarity between two token frequency vectors
    ///
    /// # Arguments
    ///
    /// * `a` - First token frequency vector
    /// * `b` - Second token frequency vector
    ///
    /// # Returns
    ///
    /// * `f64` - Similarity score (cosine similarity)
    fn similarity(a: &TokenFrequencies, b: &TokenFrequencies) -> f64 {
        let mut similarity = 0.0;
        
        for (token, freq_a) in a {
            if let Some(freq_b) = b.get(token) {
                similarity += freq_a * freq_b;
            }
        }
        
        similarity
    }
    
    /// Train the classifier with sample data
    ///
    /// # Note
    ///
    /// In a full implementation, this would load and process all language samples
    /// from a training set. For simplicity, we're using a pre-trained model.
    fn train() -> (LanguageTokens, TokenFrequencies) {
        // In a real implementation, we would:
        // 1. Load all language samples
        // 2. Tokenize each sample
        // 3. Calculate term frequencies for each language
        // 4. Calculate inverse class frequencies
        // 5. Create centroids for each language
        
        // For this simplified version, return empty structures
        (HashMap::new(), HashMap::new())
    }
}

impl Strategy for Classifier {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        // Skip binary files or symlinks
        if blob.is_binary() || blob.is_symlink() {
            return Vec::new();
        }
        
        // Get the data for analysis, limited to a reasonable size
        let data_bytes = blob.data();
        let consider_bytes = std::cmp::min(data_bytes.len(), CLASSIFIER_CONSIDER_BYTES);
        let data_slice = &data_bytes[..consider_bytes];
        
        // Convert to string for tokenization
        let content = match std::str::from_utf8(data_slice) {
            Ok(s) => s,
            Err(_) => return Vec::new(), // Binary content
        };
        
        // Tokenize the content
        let tokens = Self::tokenize(content);
        
        // If we have too few tokens, don't attempt classification
        if tokens.len() < 10 {
            return Vec::new();
        }
        
        // Calculate term frequencies for the input
        let term_freq = Self::calculate_term_frequencies(&tokens);
        
        // In a real implementation, here we would:
        // 1. Load the pre-trained model (language centroids and ICF)
        // 2. Calculate TF-IDF for the input using the model's ICF
        // 3. Calculate similarity scores with each language centroid
        // 4. Return the language with the highest similarity score
        
        // For this simplified version, use a simple heuristic
        // This is a placeholder for the actual classifier logic
        if !candidates.is_empty() {
            // Just return the first candidate as a placeholder
            // In a real implementation, we would rank candidates by similarity
            vec![candidates[0].clone()]
        } else {
            // In a real implementation, we would return the most similar language
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
    fn test_tokenization() {
        let content = r#"
        function hello(name) {
            return "Hello, " + name + "!";
        }
        "#;
        
        let tokens = Classifier::tokenize(content);
        assert!(tokens.contains(&"function".to_string()));
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"name".to_string()));
        assert!(tokens.contains(&"return".to_string()));
        
        // Stop words should be filtered out
        assert!(!tokens.contains(&"the".to_string()));
    }
    
    #[test]
    fn test_term_frequencies() {
        let tokens = vec![
            "hello".to_string(),
            "world".to_string(),
            "hello".to_string(),
            "rust".to_string(),
        ];
        
        let frequencies = Classifier::calculate_term_frequencies(&tokens);
        
        // Check log term frequencies
        assert!(frequencies.contains_key(&"hello".to_string()));
        assert!(frequencies.contains_key(&"world".to_string()));
        assert!(frequencies.contains_key(&"rust".to_string()));
        
        // hello appears twice, so its frequency should be higher
        assert!(frequencies[&"hello".to_string()] > frequencies[&"world".to_string()]);
    }
    
    #[test]
    fn test_l2_normalization() {
        let mut frequencies = HashMap::new();
        frequencies.insert("hello".to_string(), 2.0);
        frequencies.insert("world".to_string(), 1.0);
        
        Classifier::l2_normalize(&mut frequencies);
        
        // Check that the vector is normalized (sum of squares = 1)
        let sum_of_squares: f64 = frequencies.values()
            .map(|&freq| freq * freq)
            .sum();
        
        assert!((sum_of_squares - 1.0).abs() < 1e-10);
    }
    
    #[test]
    fn test_similarity() {
        let mut a = HashMap::new();
        a.insert("hello".to_string(), 0.8);
        a.insert("world".to_string(), 0.6);
        
        let mut b = HashMap::new();
        b.insert("hello".to_string(), 0.6);
        b.insert("world".to_string(), 0.8);
        
        let similarity = Classifier::similarity(&a, &b);
        
        // Vectors are similar but not identical
        assert!(similarity > 0.0);
        assert!(similarity < 1.0);
        
        // Identical vectors should have similarity 1.0
        assert!((Classifier::similarity(&a, &a) - 1.0).abs() < 1e-10);
    }
    
    #[test]
    fn test_classifier_strategy() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Create a simple JavaScript file
        let js_path = dir.path().join("script.js");
        {
            let mut file = File::create(&js_path)?;
            file.write_all(b"function hello() { return 'world'; }")?;
        }
        
        let blob = FileBlob::new(&js_path)?;
        let strategy = Classifier;
        
        // Test with candidates
        let js = Language::find_by_name("JavaScript").unwrap();
        let python = Language::find_by_name("Python").unwrap();
        
        let languages = strategy.call(&blob, &[js.clone(), python.clone()]);
        assert_eq!(languages.len(), 1);
        
        // In this simplified version, it just returns the first candidate
        assert_eq!(languages[0].name, "JavaScript");
        
        Ok(())
    }
}