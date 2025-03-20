//! Modeline-based language detection strategy.
//!
//! This strategy detects languages based on Vim and Emacs modelines
//! embedded in the file.

use std::collections::HashSet;
use fancy_regex::Regex;

use crate::blob::BlobHelper;
use crate::language::Language;
use crate::strategy::Strategy;

lazy_static::lazy_static! {
    // Emacs modeline regex
    static ref EMACS_MODELINE: Regex = Regex::new(r#"(?im)
        # Opening delimiter
        -\*-

        (?:
          # Short form: `-*- ruby -*-`
          [ \t]*
          (?=
            [^:;\s]+  # Name of mode
            [ \t]*    # Optional whitespace
            -\*-      # Closing delimiter
          )
          |

          # Longer form: `-*- foo:bar; mode: ruby; -*-`
          (?:
            .*?[ \t;] # Preceding variables: `-*- foo:bar bar:baz;`
            |
            (?<=-\*-) # Not preceded by anything: `-*-mode:ruby-*-`
          )

          # Explicitly-named variable: `mode: ruby` or `mode  : ruby`
          [ \t]* mode [ \t]* : [ \t]*
        )

        # Name of major-mode, which corresponds to syntax or filetype
        ([^:;\s]+)

        # Ensure the name is terminated correctly
        (?=
          # Followed by semicolon or whitespace
          [ \t;]
          |
          # Touching the ending sequence: `ruby-*-`
          (?<![-*])   # Don't allow stuff like `ruby--*-` to match; it'll invalidate the mode
          -\*-        # Emacs has no problems reading `ruby --*-`, however.
        )

        # If we've gotten this far, it means the modeline is valid.
        # We gleefully skip past everything up until reaching \"-*-\"
        .*?

        # Closing delimiter
        -\*-"#).unwrap();
    
    // Vim modeline regex
    static ref VIM_MODELINE: Regex = Regex::new(r#"(?im)
        # Start of modeline (syntax documented in E520)
        (?:
          # `vi:`, `vim:` or `Vim:`
          (?:^|[ \t]) (?:vi|Vi(?=m))

          # Check if specific Vim version(s) are requested (won't work in vi/ex)
          (?:
            # Versioned modeline. `vim<700:` targets Vim versions older than 7.0
            m
            [<=>]?    # If comparison operator is omitted, *only* this version is targeted
            [0-9]+    # Version argument = (MINOR_VERSION_NUMBER * 100) + MINOR_VERSION_NUMBER
            |

            # Unversioned modeline. `vim:` targets any version of Vim.
            m
          )?
          |

          # `ex:`, which requires leading whitespace to avoid matching stuff like \"lex:\"
          [ \t] ex
        )

        # If the option-list begins with `set ` or `se `, it indicates an alternative
        # modeline syntax partly-compatible with older versions of Vi. Here, the colon
        # serves as a terminator for an option sequence, delimited by whitespace.
        (?=
          # So we have to ensure the modeline ends with a colon
          : (?=[ \t]* set? [ \t] [^\r\n:]+ :) |

          # Otherwise, it isn't valid syntax and should be ignored
          : (?![ \t]* set? [ \t])
        )

        # Possible (unrelated) `option=value` pairs to skip past
        (?:
          # Option separator, either
          (?:
            # 1. A colon (possibly surrounded by whitespace)
            [ \t]* : [ \t]*     # vim: noai :  ft=sh:noexpandtab
            |

            # 2. At least one (horizontal) whitespace character
            [ \t]               # vim: noai ft=sh noexpandtab
          )

          # Option's name. All recognised Vim options have an alphanumeric form.
          \w*

          # Possible value. Not every option takes an argument.
          (?:
            # Whitespace between name and value is allowed: `vim: ft   =sh`
            [ \t]*=

            # Option's value. Might be blank; `vim: ft= ` means \"use no filetype\".
            (?:
              [^\\\s]    # Beware of escaped characters: titlestring=\ ft=sh
              |          # will be read by Vim as { titlestring: \" ft=sh\" }.
              \\.
            )*
          )?
        )*

        # The actual filetype declaration
        [ \t:] (?:filetype|ft|syntax) [ \t]*=

        # Language's name
        (\w+)

        # Ensure it's followed by a legal separator (including EOL)
        (?=$|\s|:)"#).unwrap();
    
    // Search scope (number of lines to check at beginning and end of file)
    static ref SEARCH_SCOPE: usize = 5;
}

/// Modeline-based language detection strategy
#[derive(Debug)]
pub struct Modeline;

impl Modeline {
    /// Extract modeline from content
    ///
    /// # Arguments
    ///
    /// * `content` - The file content
    ///
    /// # Returns
    ///
    /// * `Option<String>` - The detected language name, if found
    fn modeline(content: &str) -> Option<String> {
        // First try Emacs modeline
        if let Ok(Some(captures)) = EMACS_MODELINE.captures(content) {
            if let Some(mode) = captures.get(1) {
                return Some(mode.as_str().to_string());
            }
        }
        
        // Then try Vim modeline
        if let Ok(Some(captures)) = VIM_MODELINE.captures(content) {
            if let Some(mode) = captures.get(1) {
                return Some(mode.as_str().to_string());
            }
        }
        
        None
    }
}

impl Strategy for Modeline {
    fn call<B: BlobHelper + ?Sized>(&self, blob: &B, candidates: &[Language]) -> Vec<Language> {
        // Skip symlinks
        if blob.is_symlink() {
            return Vec::new();
        }
        
        // Get the first and last few lines
        let lines = blob.first_lines(*SEARCH_SCOPE);
        let header = lines.join("\n");
        
        // Return early for Vimball files
        if header.contains("UseVimball") {
            return Vec::new();
        }
        
        let last_lines = blob.last_lines(*SEARCH_SCOPE);
        let footer = last_lines.join("\n");
        
        // Combine header and footer for modeline detection
        let content = format!("{}\n{}", header, footer);
        
        if let Some(mode) = Self::modeline(&content) {
            if let Some(language) = Language::find_by_alias(&mode) {
                return if !candidates.is_empty() {
                    if candidates.contains(language) {
                        vec![language.clone()]
                    } else {
                        Vec::new()
                    }
                } else {
                    vec![language.clone()]
                };
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
    fn test_emacs_modeline() {
        let content = "-*- mode: ruby -*-\nputs 'hello'";
        assert_eq!(Modeline::modeline(content), Some("ruby".to_string()));
        
        let content = "-*-ruby-*-\nputs 'hello'";
        assert_eq!(Modeline::modeline(content), Some("ruby".to_string()));
        
        let content = "-*- foo:bar; mode: python; -*-\nprint('hello')";
        assert_eq!(Modeline::modeline(content), Some("python".to_string()));
    }
    
    #[test]
    fn test_vim_modeline() {
        let content = "#!/bin/sh\n# vim: ft=ruby\nputs 'hello'";
        assert_eq!(Modeline::modeline(content), Some("ruby".to_string()));
        
        let content = "// vim: set syntax=javascript:\nconsole.log('hello')";
        assert_eq!(Modeline::modeline(content), Some("javascript".to_string()));
        
        let content = "/* vim: set filetype=c: */\n#include <stdio.h>";
        assert_eq!(Modeline::modeline(content), Some("c".to_string()));
    }
    
    #[test]
    fn test_modeline_strategy() -> crate::Result<()> {
        let dir = tempdir()?;
        
        // Test with Ruby modeline
        let ruby_path = dir.path().join("script");
        {
            let mut file = File::create(&ruby_path)?;
            file.write_all(b"#!/bin/sh\n# vim: ft=ruby\nputs 'hello'")?;
        }
        
        let blob = FileBlob::new(&ruby_path)?;
        let strategy = Modeline;
        
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "Ruby");
        
        // Test with Python modeline
        let py_path = dir.path().join("script");
        {
            let mut file = File::create(&py_path)?;
            file.write_all(b"-*- mode: python -*-\nprint('hello')")?;
        }
        
        let blob = FileBlob::new(&py_path)?;
        let languages = strategy.call(&blob, &[]);
        assert!(!languages.is_empty());
        assert_eq!(languages[0].name, "Python");
        
        Ok(())
    }
    
    #[test]
    fn test_modeline_strategy_with_candidates() -> crate::Result<()> {
        let dir = tempdir()?;
        let ruby_path = dir.path().join("script");
        
        {
            let mut file = File::create(&ruby_path)?;
            file.write_all(b"# vim: ft=ruby\nputs 'hello'")?;
        }
        
        let blob = FileBlob::new(&ruby_path)?;
        let strategy = Modeline;
        
        // Ruby in candidates
        let ruby = Language::find_by_name("Ruby").unwrap();
        let python = Language::find_by_name("Python").unwrap();
        
        let languages = strategy.call(&blob, &[ruby.clone(), python.clone()]);
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "Ruby");
        
        // Only Python in candidates (no match)
        let languages = strategy.call(&blob, &[python.clone()]);
        assert!(languages.is_empty());
        
        Ok(())
    }
}