//! Language definitions and utilities.
//!
//! This module defines the Language struct and related functions for
//! looking up languages by name, extension, or filename.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Once;

use serde::{Deserialize, Serialize};

use crate::data::languages;
use crate::Result;

static INIT: Once = Once::new();
static mut LANGUAGES: Option<Vec<Language>> = None;
static mut LANGUAGE_INDEX: Option<HashMap<String, usize>> = None;
static mut NAME_INDEX: Option<HashMap<String, usize>> = None;
static mut ALIAS_INDEX: Option<HashMap<String, usize>> = None;
static mut LANGUAGE_ID_INDEX: Option<HashMap<usize, usize>> = None;
static mut EXTENSION_INDEX: Option<HashMap<String, Vec<usize>>> = None;
static mut INTERPRETER_INDEX: Option<HashMap<String, Vec<usize>>> = None;
static mut FILENAME_INDEX: Option<HashMap<String, Vec<usize>>> = None;

/// Language type enumerations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum LanguageType {
    /// Data languages (JSON, YAML, etc.)
    Data,
    /// Programming languages (Rust, Python, etc.)
    Programming,
    /// Markup languages (HTML, Markdown, etc.)
    Markup,
    /// Prose languages (Text, AsciiDoc, etc.)
    Prose,
    /// Other/unclassified languages
    Other,
}

impl Default for LanguageType {
    fn default() -> Self {
        LanguageType::Other
    }
}

/// Represents a programming or markup language.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Language {
    /// The human-readable name of the language
    pub name: String,
    
    /// The name used in filesystem paths
    pub fs_name: Option<String>,
    
    /// The type of language
    #[serde(default)]
    pub language_type: LanguageType,
    
    /// The color associated with the language (hex code)
    pub color: Option<String>,
    
    /// Alternate names or aliases for the language
    #[serde(default)]
    pub aliases: Vec<String>,
    
    /// TextMate scope for syntax highlighting
    pub tm_scope: Option<String>,
    
    /// Ace editor mode
    pub ace_mode: Option<String>,
    
    /// CodeMirror mode
    pub codemirror_mode: Option<String>,
    
    /// CodeMirror MIME type
    pub codemirror_mime_type: Option<String>,
    
    /// Whether to wrap text when displaying
    #[serde(default)]
    pub wrap: bool,
    
    /// File extensions associated with the language
    #[serde(default)]
    pub extensions: Vec<String>,
    
    /// Filenames associated with the language
    #[serde(default)]
    pub filenames: Vec<String>,
    
    /// Interpreters associated with the language
    #[serde(default)]
    pub interpreters: Vec<String>,
    
    /// Unique identifier for the language
    pub language_id: usize,
    
    /// Whether the language is popular
    #[serde(default)]
    pub popular: bool,
    
    /// The parent language group name
    pub group_name: Option<String>,
    
    /// Cached reference to the group language
    #[serde(skip)]
    pub group: Option<usize>,
}

impl Language {
    /// Initialize the language data.
    fn init() {
        INIT.call_once(|| {
            unsafe {
                let (langs, name_idx, alias_idx, lang_idx, lang_id_idx, ext_idx, interp_idx, file_idx) = 
                    languages::load_language_data();
                
                LANGUAGES = Some(langs);
                LANGUAGE_INDEX = Some(lang_idx);
                NAME_INDEX = Some(name_idx);
                ALIAS_INDEX = Some(alias_idx);
                LANGUAGE_ID_INDEX = Some(lang_id_idx);
                EXTENSION_INDEX = Some(ext_idx);
                INTERPRETER_INDEX = Some(interp_idx);
                FILENAME_INDEX = Some(file_idx);
            }
        });
    }

    /// Get a reference to all known languages.
    pub fn all() -> &'static [Language] {
        Self::init();
        unsafe { LANGUAGES.as_ref().unwrap() }
    }
    
    /// Look up a language by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the language to look up
    ///
    /// # Returns
    ///
    /// * `Option<&Language>` - The language if found, None otherwise
    pub fn find_by_name(name: &str) -> Option<&'static Language> {
        Self::init();
        
        let name = name.to_lowercase();
        
        unsafe {
            if let Some(idx) = NAME_INDEX.as_ref().unwrap().get(&name) {
                return Some(&LANGUAGES.as_ref().unwrap()[*idx]);
            }
            
            // Try looking up by the first part of a comma-separated name
            if name.contains(',') {
                let first_part = name.split(',').next().unwrap().trim().to_lowercase();
                if let Some(idx) = NAME_INDEX.as_ref().unwrap().get(&first_part) {
                    return Some(&LANGUAGES.as_ref().unwrap()[*idx]);
                }
            }
            
            None
        }
    }
    
    /// Look up a language by alias.
    ///
    /// # Arguments
    ///
    /// * `alias` - The alias of the language to look up
    ///
    /// # Returns
    ///
    /// * `Option<&Language>` - The language if found, None otherwise
    pub fn find_by_alias(alias: &str) -> Option<&'static Language> {
        Self::init();
        
        let alias = alias.to_lowercase();
        
        unsafe {
            if let Some(idx) = ALIAS_INDEX.as_ref().unwrap().get(&alias) {
                return Some(&LANGUAGES.as_ref().unwrap()[*idx]);
            }
            
            // Try looking up by the first part of a comma-separated alias
            if alias.contains(',') {
                let first_part = alias.split(',').next().unwrap().trim().to_lowercase();
                if let Some(idx) = ALIAS_INDEX.as_ref().unwrap().get(&first_part) {
                    return Some(&LANGUAGES.as_ref().unwrap()[*idx]);
                }
            }
            
            None
        }
    }
    
    /// Look up languages by filename.
    ///
    /// # Arguments
    ///
    /// * `filename` - The filename to look up
    ///
    /// # Returns
    ///
    /// * `Vec<&Language>` - The languages matching the filename
    pub fn find_by_filename(filename: &str) -> Vec<&'static Language> {
        Self::init();
        
        let basename = std::path::Path::new(filename)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        
        unsafe {
            FILENAME_INDEX
                .as_ref()
                .unwrap()
                .get(&basename)
                .map(|idxs| idxs.iter().map(|&idx| &LANGUAGES.as_ref().unwrap()[idx]).collect())
                .unwrap_or_default()
        }
    }
    
    /// Look up languages by file extension.
    ///
    /// # Arguments
    ///
    /// * `filename` - The filename to extract extension from
    ///
    /// # Returns
    ///
    /// * `Vec<&Language>` - The languages matching the extension
    pub fn find_by_extension(filename: &str) -> Vec<&'static Language> {
        Self::init();
        
        let lowercase_filename = filename.to_lowercase();
        let path = std::path::Path::new(&lowercase_filename);
        
        // Extract all extensions (e.g., ".tar.gz" gives [".tar.gz", ".gz"])
        let mut extensions = Vec::new();
        let mut current_path = path;
        
        while let Some(ext) = current_path.extension() {
            let full_ext = format!(".{}", ext.to_string_lossy());
            extensions.push(full_ext);
            
            current_path = match current_path.file_stem() {
                Some(stem) => std::path::Path::new(stem),
                None => break,
            };
        }
        
        // Find the first extension with language definitions
        for ext in extensions {
            unsafe {
                if let Some(idxs) = EXTENSION_INDEX.as_ref().unwrap().get(&ext) {
                    if !idxs.is_empty() {
                        return idxs.iter().map(|&idx| &LANGUAGES.as_ref().unwrap()[idx]).collect();
                    }
                }
            }
        }
        
        Vec::new()
    }
    
    /// Look up languages by interpreter.
    ///
    /// # Arguments
    ///
    /// * `interpreter` - The interpreter name
    ///
    /// # Returns
    ///
    /// * `Vec<&Language>` - The languages matching the interpreter
    pub fn find_by_interpreter(interpreter: &str) -> Vec<&'static Language> {
        Self::init();
        
        unsafe {
            INTERPRETER_INDEX
                .as_ref()
                .unwrap()
                .get(interpreter)
                .map(|idxs| idxs.iter().map(|&idx| &LANGUAGES.as_ref().unwrap()[idx]).collect())
                .unwrap_or_default()
        }
    }
    
    /// Get a language by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The language ID
    ///
    /// # Returns
    ///
    /// * `Option<&Language>` - The language if found, None otherwise
    pub fn find_by_id(id: usize) -> Option<&'static Language> {
        Self::init();
        
        unsafe {
            LANGUAGE_ID_INDEX
                .as_ref()
                .unwrap()
                .get(&id)
                .map(|&idx| &LANGUAGES.as_ref().unwrap()[idx])
        }
    }
    
    /// Language lookup by name or alias.
    ///
    /// # Arguments
    ///
    /// * `name` - The name or alias to look up
    ///
    /// # Returns
    ///
    /// * `Option<&Language>` - The language if found, None otherwise
    pub fn lookup(name: &str) -> Option<&'static Language> {
        if name.is_empty() {
            return None;
        }
        
        let result = Self::find_by_name(name);
        if result.is_some() {
            return result;
        }
        
        Self::find_by_alias(name)
    }
    
    /// Get a list of popular languages.
    ///
    /// # Returns
    ///
    /// * `Vec<&Language>` - The popular languages
    pub fn popular() -> Vec<&'static Language> {
        Self::init();
        
        let mut popular = Self::all()
            .iter()
            .filter(|lang| lang.popular)
            .collect::<Vec<_>>();
        
        popular.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        popular
    }
    
    /// Get a list of non-popular languages.
    ///
    /// # Returns
    ///
    /// * `Vec<&Language>` - The unpopular languages
    pub fn unpopular() -> Vec<&'static Language> {
        Self::init();
        
        let mut unpopular = Self::all()
            .iter()
            .filter(|lang| !lang.popular)
            .collect::<Vec<_>>();
        
        unpopular.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        unpopular
    }
    
    /// Get a list of languages with assigned colors.
    ///
    /// # Returns
    ///
    /// * `Vec<&Language>` - The languages with colors
    pub fn colors() -> Vec<&'static Language> {
        Self::init();
        
        let mut colors = Self::all()
            .iter()
            .filter(|lang| lang.color.is_some())
            .collect::<Vec<_>>();
        
        colors.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        colors
    }
    
    /// Get the default alias for a language.
    ///
    /// # Returns
    ///
    /// * `String` - The default alias
    pub fn default_alias(&self) -> String {
        self.name.to_lowercase().replace(" ", "-")
    }
    
    /// Get the language's group.
    ///
    /// # Returns
    ///
    /// * `Option<&Language>` - The group language if defined
    pub fn group(&self) -> Option<&'static Language> {
        Self::init();
        
        let group_name = match &self.group_name {
            Some(name) => name,
            None => &self.name,
        };
        
        Self::find_by_name(group_name)
    }
    
    /// Check if the language is popular.
    ///
    /// # Returns
    ///
    /// * `bool` - True if the language is popular
    pub fn is_popular(&self) -> bool {
        self.popular
    }
    
    /// Check if the language is not popular.
    ///
    /// # Returns
    ///
    /// * `bool` - True if the language is not popular
    pub fn is_unpopular(&self) -> bool {
        !self.popular
    }
}

impl PartialEq for Language {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Language {}

impl Hash for Language {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_find_by_name() {
        let rust = Language::find_by_name("Rust").unwrap();
        assert_eq!(rust.name, "Rust");
        assert_eq!(rust.ace_mode.as_deref(), Some("rust"));
        
        // Case insensitive
        let rust = Language::find_by_name("rust").unwrap();
        assert_eq!(rust.name, "Rust");
    }
    
    #[test]
    fn test_find_by_extension() {
        let rust_langs = Language::find_by_extension("hello.rs");
        assert_eq!(rust_langs.len(), 1);
        assert_eq!(rust_langs[0].name, "Rust");
        
        let js_langs = Language::find_by_extension("script.js");
        assert_eq!(js_langs.len(), 1);
        assert_eq!(js_langs[0].name, "JavaScript");
    }
    
    #[test]
    fn test_find_by_filename() {
        let docker_langs = Language::find_by_filename("Dockerfile");
        assert!(!docker_langs.is_empty());
        assert_eq!(docker_langs[0].name, "Dockerfile");
    }
    
    #[test]
    fn test_popular_languages() {
        let popular = Language::popular();
        assert!(!popular.is_empty());
        assert!(popular.iter().any(|l| l.name == "JavaScript"));
        assert!(popular.iter().any(|l| l.name == "Python"));
    }
}