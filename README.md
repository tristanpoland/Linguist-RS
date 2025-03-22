# Linguist-RS

A Rust port of GitHub's Linguist for language detection.

## Overview

Linguist-RS is a Rust library designed to detect programming languages in repositories, similar to GitHub's Linguist. It provides robust language detection capabilities by analyzing:

- File extensions
- Filenames
- Shebang lines
- Modelines
- File contents
- And more

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
linguist = { git = "https://github.com/tristanpoland/linguist-rs" }
```

## Usage Examples

### Detecting Language of a File

```rust
use linguist::detect;
use linguist::blob::FileBlob;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blob = FileBlob::new("path/to/your/file.rs")?;
    
    if let Some(language) = detect(&blob, false) {
        println!("Detected language: {}", language.name);
    }
    
    Ok(())
}
```

### Analyzing a Repository

```rust
use linguist::repository::Repository;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = Repository::new("/path/to/repo", "HEAD", None)?;
    let stats = repo.stats()?;
    
    println!("Primary Language: {:?}", stats.language);
    println!("Language Breakdown: {:?}", stats.language_breakdown);
    
    Ok(())
}
```

## Development Status

Currently implementing and testing various language detection strategies:

- [x] Extension-based detection
- [x] Filename-based detection
- [x] Shebang detection
- [x] Modeline detection
- [x] XML detection
- [x] Manpage detection
- [x] Heuristics refinement
- [ ] Machine learning classifier

## Contributing

Contributions are welcome! Please read the contributing guidelines and code of conduct.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## License

Distributed under the MIT License. See `LICENSE` for more information.

## Acknowledgments

- Inspired by GitHub's Linguist
- Thanks to the Rust community

## Roadmap

- [ ] Complete test suite
- [ ] Performance benchmarking
- [ ] Comprehensive language support
- [ ] CI/CD pipeline
- [ ] Documentation improvements
