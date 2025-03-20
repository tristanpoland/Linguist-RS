//! Vendor detection functionality.
//!
//! This module provides functionality to identify vendored files,
//! which are typically third-party libraries or dependencies.

use fancy_regex::Regex;
use std::path::Path;

lazy_static::lazy_static! {
    // Regular expression patterns for vendored paths (from vendor.yml)
    pub static ref VENDOR_REGEX: Regex = {
        let patterns = vec![
            // Vendor Conventions
            r"(^|/)cache/",
            r"^[Dd]ependencies/",
            r"(^|/)dist/",
            r"^deps/",
            r"(^|/)configure$",
            r"(^|/)config\.guess$",
            r"(^|/)config\.sub$",
            
            // Autoconf generated files
            r"(^|/)aclocal\.m4",
            r"(^|/)libtool\.m4",
            r"(^|/)ltoptions\.m4",
            r"(^|/)ltsugar\.m4",
            r"(^|/)ltversion\.m4",
            r"(^|/)lt~obsolete\.m4",
            
            // .NET Core Install Scripts
            r"(^|/)dotnet-install\.(ps1|sh)$",
            
            // Node dependencies
            r"(^|/)node_modules/",
            
            // Yarn 2
            r"(^|/)\.yarn/releases/",
            r"(^|/)\.yarn/plugins/",
            r"(^|/)\.yarn/sdks/",
            r"(^|/)\.yarn/versions/",
            r"(^|/)\.yarn/unplugged/",
            
            // Bower Components
            r"(^|/)bower_components/",
            
            // Minified JavaScript and CSS
            r"(\.|-)min\.(js|css)$",
            
            // Bootstrap css and js
            r"(^|/)bootstrap([^/.]*)(\..*)?\.(js|css|less|scss|styl)$",
            
            // jQuery
            r"(^|/)jquery([^.]*)\.js$",
            r"(^|/)jquery\-\d\.\d+(\.\d+)?\.js$",
            
            // jQuery UI
            r"(^|/)jquery\-ui(\-\d\.\d+(\.\d+)?)?(\.\w+)?\.(js|css)$",
            
            // Vendor directories
            r"(3rd|[Tt]hird)[-_]?[Pp]arty/",
            r"(^|/)vendors?/",
            r"(^|/)[Ee]xtern(als?)?/",
            r"(^|/)[Vv]+endor/",
            
            // Add more patterns from vendor.yml as needed
        ];
        Regex::new(&patterns.join("|")).unwrap()
    };
}

/// Check if a path is a vendored file
///
/// # Arguments
///
/// * `path` - The path to check
///
/// # Returns
///
/// * `bool` - True if the path is a vendored file
pub fn is_vendored(path: &str) -> bool {
    VENDOR_REGEX.is_match(path).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vendored_paths() {
        assert!(is_vendored("vendor/jquery.min.js"));
        assert!(is_vendored("node_modules/react/index.js"));
        assert!(is_vendored("third-party/library.js"));
        assert!(is_vendored("deps/openssl/crypto/md5/md5.c"));
        assert!(is_vendored("path/to/cache/file.js"));
        assert!(is_vendored("dist/bundle.js"));
        assert!(is_vendored("path/to/jquery-3.4.1.min.js"));
        
        assert!(!is_vendored("src/main.js"));
        assert!(!is_vendored("lib/utils.js"));
        assert!(!is_vendored("app/components/button.js"));
    }
}