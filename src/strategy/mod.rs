//! Language detection strategies.
//!
//! This module contains various strategies for detecting the language
//! of a file based on different criteria.

pub mod extension;
pub mod filename;
pub mod manpage;
pub mod modeline;
pub mod shebang;
pub mod xml;

use crate::blob::BlobHelper;
use crate::language::Language;

/// Enum-based language detection strategy
#[derive(Debug)]
pub enum StrategyType {
    /// Modeline-based strategy
    Modeline(modeline::Modeline),
    /// Filename-based strategy
    Filename(filename::Filename),
    /// Shebang-based strategy
    Shebang(shebang::Shebang),
    /// Extension-based strategy
    Extension(extension::Extension),
    /// XML detection strategy
    Xml(xml::Xml),
    /// Manpage detection strategy
    Manpage(manpage::Manpage),
    /// Heuristics-based strategy
    Heuristics(crate::heuristics::Heuristics),
    /// Classifier-based strategy
    Classifier(crate::classifier::Classifier),
}

/// Trait for language detection strategies
pub trait Strategy: Send + Sync {
    /// Try to detect languages for a blob using this strategy.
    ///
    /// # Arguments
    ///
    /// * `blob` - The blob to analyze
    /// * `candidates` - Optional list of candidate languages from previous strategies
    ///
    /// # Returns
    ///
    /// * `Vec<Language>` - Languages that match the blob according to this strategy
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language>;
}

impl Strategy for StrategyType {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        match self {
            StrategyType::Modeline(strategy) => strategy.call(blob, candidates),
            StrategyType::Filename(strategy) => strategy.call(blob, candidates),
            StrategyType::Shebang(strategy) => strategy.call(blob, candidates),
            StrategyType::Extension(strategy) => strategy.call(blob, candidates),
            StrategyType::Xml(strategy) => strategy.call(blob, candidates),
            StrategyType::Manpage(strategy) => strategy.call(blob, candidates),
            StrategyType::Heuristics(strategy) => strategy.call(blob, candidates),
            StrategyType::Classifier(strategy) => strategy.call(blob, candidates),
        }
    }
}