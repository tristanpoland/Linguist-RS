//! Language definitions and data loading functionality.
//!
//! This module handles loading language definitions from the languages.yml file
//! and preparing the necessary indices for fast language lookups.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Once;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::language::Language;
use crate::Result;

// Path to the included languages.yml file
const LANGUAGES_DATA_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/languages.yml");

// Path to the included popular.yml file
const POPULAR_DATA_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/popular.yml");

// Static initialization for the language data
static INIT: Once = Once::new();
static mut LANGUAGES_DATA: Option<String> = None;
static mut POPULAR_DATA: Option<Vec<String>> = None;

/// Load the language data from the embedded languages.yml file
fn load_languages_yml() -> Result<String> {
    unsafe {
        INIT.call_once(|| {
            // Load the languages.yml file
            let mut file = File::open(LANGUAGES_DATA_PATH).expect("Failed to open languages.yml");
            let mut contents = String::new();
            file.read_to_string(&mut contents).expect("Failed to read languages.yml");
            LANGUAGES_DATA = Some(contents);
            
            // Load the popular.yml file
            let mut file = File::open(POPULAR_DATA_PATH).expect("Failed to open popular.yml");
            let mut contents = String::new();
            file.read_to_string(&mut contents).expect("Failed to read popular.yml");
            
            // Parse the YAML data
            let popular: Vec<String> = serde_yaml::from_str(&contents).expect("Failed to parse popular.yml");
            POPULAR_DATA = Some(popular);
        });
        
        Ok(LANGUAGES_DATA.as_ref().unwrap().clone())
    }
}

/// Get the list of popular language names
fn get_popular_languages() -> Result<Vec<String>> {
    unsafe {
        if POPULAR_DATA.is_none() {
            // Ensure languages.yml is loaded, which also loads popular.yml
            load_languages_yml()?;
        }
        
        Ok(POPULAR_DATA.as_ref().unwrap().clone())
    }
}

/// Load language data from the embedded YAML files
///
/// This function returns the language definitions and various indices for fast lookups.
///
/// # Returns
///
/// * `(Vec<Language>, HashMap<String, usize>, HashMap<String, usize>, HashMap<String, usize>, HashMap<usize, usize>, HashMap<String, Vec<usize>>, HashMap<String, Vec<usize>>, HashMap<String, Vec<usize>>)` -
///   A tuple containing:
///   - Vec<Language>: The language definitions
///   - HashMap<String, usize>: Name index mapping lowercase language name to index
///   - HashMap<String, usize>: Alias index mapping lowercase alias to index
///   - HashMap<String, usize>: Language index mapping lowercase name or alias to index
///   - HashMap<usize, usize>: Language ID index mapping language_id to index
///   - HashMap<String, Vec<usize>>: Extension index mapping extensions to indices
///   - HashMap<String, Vec<usize>>: Interpreter index mapping interpreters to indices
///   - HashMap<String, Vec<usize>>: Filename index mapping filenames to indices
pub fn load_language_data() -> (
    Vec<Language>,
    HashMap<String, usize>,
    HashMap<String, usize>,
    HashMap<String, usize>,
    HashMap<usize, usize>,
    HashMap<String, Vec<usize>>,
    HashMap<String, Vec<usize>>,
    HashMap<String, Vec<usize>>,
) {
    // Load YAML data
    let languages_yaml = load_languages_yml().expect("Failed to load languages.yml");
    let popular_languages = get_popular_languages().expect("Failed to load popular.yml");
    
    // Parse YAML into a map
    let lang_map: HashMap<String, Value> = serde_yaml::from_str(&languages_yaml)
        .expect("Failed to parse languages.yml");
    
    // Create languages and indices
    let mut languages = Vec::new();
    let mut name_index = HashMap::new();
    let mut alias_index = HashMap::new();
    let mut language_index = HashMap::new();
    let mut language_id_index = HashMap::new();
    let mut extension_index: HashMap<String, Vec<usize>> = HashMap::new();
    let mut interpreter_index: HashMap<String, Vec<usize>> = HashMap::new();
    let mut filename_index: HashMap<String, Vec<usize>> = HashMap::new();
    
    // Convert each language entry to a Language struct
    for (name, attrs) in lang_map {
        let popular = popular_languages.contains(&name);
        
        // Start with default values
        let mut language = Language {
            name: name.clone(),
            fs_name: None,
            language_type: crate::language::LanguageType::Other,
            color: None,
            aliases: Vec::new(),
            tm_scope: None,
            ace_mode: None,
            codemirror_mode: None,
            codemirror_mime_type: None,
            wrap: false,
            extensions: Vec::new(),
            filenames: Vec::new(),
            interpreters: Vec::new(),
            language_id: 0,
            popular,
            group_name: None,
            group: None,
        };
        
        // Fill in values from the YAML
        if let Value::Mapping(map) = attrs {
            for (key, value) in map {
                if let Value::String(key_str) = key {
                    match key_str.as_str() {
                        "fs_name" => {
                            if let Value::String(fs_name) = value {
                                language.fs_name = Some(fs_name);
                            }
                        },
                        "type" => {
                            if let Value::String(type_str) = value {
                                language.language_type = match type_str.as_str() {
                                    "data" => crate::language::LanguageType::Data,
                                    "programming" => crate::language::LanguageType::Programming,
                                    "markup" => crate::language::LanguageType::Markup,
                                    "prose" => crate::language::LanguageType::Prose,
                                    _ => crate::language::LanguageType::Other,
                                };
                            }
                        },
                        "color" => {
                            if let Value::String(color) = value {
                                language.color = Some(color);
                            }
                        },
                        "aliases" => {
                            if let Value::Sequence(aliases) = value {
                                for alias in aliases {
                                    if let Value::String(alias_str) = alias {
                                        language.aliases.push(alias_str);
                                    }
                                }
                            }
                        },
                        "tm_scope" => {
                            if let Value::String(tm_scope) = value {
                                language.tm_scope = Some(tm_scope);
                            }
                        },
                        "ace_mode" => {
                            if let Value::String(ace_mode) = value {
                                language.ace_mode = Some(ace_mode);
                            }
                        },
                        "codemirror_mode" => {
                            if let Value::String(codemirror_mode) = value {
                                language.codemirror_mode = Some(codemirror_mode);
                            }
                        },
                        "codemirror_mime_type" => {
                            if let Value::String(codemirror_mime_type) = value {
                                language.codemirror_mime_type = Some(codemirror_mime_type);
                            }
                        },
                        "wrap" => {
                            if let Value::Bool(wrap) = value {
                                language.wrap = wrap;
                            }
                        },
                        "extensions" => {
                            if let Value::Sequence(extensions) = value {
                                for ext in extensions {
                                    if let Value::String(ext_str) = ext {
                                        language.extensions.push(ext_str);
                                    }
                                }
                            }
                        },
                        "filenames" => {
                            if let Value::Sequence(filenames) = value {
                                for filename in filenames {
                                    if let Value::String(filename_str) = filename {
                                        language.filenames.push(filename_str);
                                    }
                                }
                            }
                        },
                        "interpreters" => {
                            if let Value::Sequence(interpreters) = value {
                                for interpreter in interpreters {
                                    if let Value::String(interpreter_str) = interpreter {
                                        language.interpreters.push(interpreter_str);
                                    }
                                }
                            }
                        },
                        "language_id" => {
                            if let Value::Number(language_id) = value {
                                if let Some(id) = language_id.as_u64() {
                                    language.language_id = id as usize;
                                }
                            }
                        },
                        "group" => {
                            if let Value::String(group_name) = value {
                                language.group_name = Some(group_name);
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
        
        // If no aliases, add default alias
        if language.aliases.is_empty() {
            language.aliases.push(language.default_alias());
        }
        
        // Add to languages and build indices
        let index = languages.len();
        
        // Add name to indices
        let name_lower = language.name.to_lowercase();
        name_index.insert(name_lower.clone(), index);
        language_index.insert(name_lower, index);
        
        // Add aliases to indices
        for alias in &language.aliases {
            let alias_lower = alias.to_lowercase();
            alias_index.insert(alias_lower.clone(), index);
            language_index.insert(alias_lower, index);
        }
        
        // Add language_id to index
        language_id_index.insert(language.language_id, index);
        
        // Add extensions to index
        for ext in &language.extensions {
            let ext_lower = ext.to_lowercase();
            extension_index.entry(ext_lower)
                .or_insert_with(Vec::new)
                .push(index);
        }
        
        // Add interpreters to index
        for interpreter in &language.interpreters {
            interpreter_index.entry(interpreter.clone())
                .or_insert_with(Vec::new)
                .push(index);
        }
        
        // Add filenames to index
        for filename in &language.filenames {
            filename_index.entry(filename.clone())
                .or_insert_with(Vec::new)
                .push(index);
        }
        
        languages.push(language);
    }
    
    // Sort indices for consistency
    for indices in extension_index.values_mut() {
        indices.sort();
    }
    
    for indices in interpreter_index.values_mut() {
        indices.sort();
    }
    
    for indices in filename_index.values_mut() {
        indices.sort();
    }
    
    (languages, name_index, alias_index, language_index, language_id_index, extension_index, interpreter_index, filename_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_language_data() {
        let (
            languages,
            name_index,
            alias_index,
            language_index,
            language_id_index,
            extension_index,
            interpreter_index,
            filename_index,
        ) = load_language_data();
        
        // Check that we have languages
        assert!(!languages.is_empty());
        
        // Check that indices are populated
        assert!(!name_index.is_empty());
        assert!(!alias_index.is_empty());
        assert!(!language_index.is_empty());
        assert!(!language_id_index.is_empty());
        assert!(!extension_index.is_empty());
        
        // Verify some common languages
        assert!(name_index.contains_key("rust"));
        assert!(name_index.contains_key("javascript"));
        assert!(name_index.contains_key("python"));
        
        // Verify extensions
        assert!(extension_index.contains_key(".rs"));
        assert!(extension_index.contains_key(".js"));
        assert!(extension_index.contains_key(".py"));
        
        // Verify interpreters
        assert!(interpreter_index.contains_key("python"));
        assert!(interpreter_index.contains_key("node"));
        
        // Verify filenames
        assert!(filename_index.contains_key("Makefile"));
        assert!(filename_index.contains_key("Dockerfile"));
    }
    
    #[test]
    fn test_popular_languages() {
        let popular = get_popular_languages().unwrap();
        
        // Check that we have popular languages
        assert!(!popular.is_empty());
        
        // Verify some common popular languages
        assert!(popular.contains(&"JavaScript".to_string()));
        assert!(popular.contains(&"Python".to_string()));
        assert!(popular.contains(&"Ruby".to_string()));
    }
}