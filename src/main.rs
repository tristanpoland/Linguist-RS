//! Command-line interface for Linguist.
//!
//! This provides command-line functionality for analyzing files and repositories.

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use git2::Repository as GitRepo;

use linguist::blob::{FileBlob, BlobHelper};  // Added BlobHelper trait import
use linguist::repository::DirectoryAnalyzer;
use linguist::threading::ThreadingConfig;

#[derive(Parser)]
#[clap(name = "linguist")]
#[clap(author = "Linguist contributors")]
#[clap(version = "0.1.0")]
#[clap(about = "GitHub Linguist - language detection", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Detect the language of a file
    File {
        /// Path to the file
        #[clap(value_parser)]
        path: PathBuf,
    },
    
    /// Analyze a directory or repository
    Analyze {
        /// Path to the directory or repository
        #[clap(value_parser)]
        path: PathBuf,
        
        /// Show all files with their languages
        #[clap(short, long)]
        breakdown: bool,
        
        /// Show percentages instead of byte counts
        #[clap(short, long)]
        percentage: bool,
        
        /// Use JSON output format
        #[clap(short, long)]
        json: bool,
        
        /// Number of worker threads for parallel processing
        #[clap(short = 't', long, default_value = "0")]
        threads: usize,
        
        /// Enable parallel processing
        #[clap(long)]
        parallel: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::File { path } => {
            if !path.exists() {
                eprintln!("Error: File not found: {}", path.display());
                process::exit(1);
            }
            
            match FileBlob::new(&path) {
                Ok(blob) => {
                    println!("File: {}", path.display());
                    
                    if blob.is_binary() {
                        println!("Binary: Yes");
                    } else {
                        println!("Binary: No");
                    }
                    
                    if blob.is_text() {
                        println!("Text: Yes");
                    } else {
                        println!("Text: No");
                    }
                    
                    if blob.is_generated() {
                        println!("Generated: Yes");
                    } else {
                        println!("Generated: No");
                    }
                    
                    if blob.is_vendored() {
                        println!("Vendored: Yes");
                    } else {
                        println!("Vendored: No");
                    }
                    
                    if blob.is_documentation() {
                        println!("Documentation: Yes");
                    } else {
                        println!("Documentation: No");
                    }
                    
                    println!("Size: {} bytes", blob.size());
                    
                    if let Some(language) = blob.language() {
                        println!("Language: {}", language.name);
                        
                        if let Some(color) = &language.color {
                            println!("Color: {}", color);
                        }
                        
                        println!("Type: {:?}", language.language_type);
                        
                        if let Some(group) = language.group() {
                            if group.name != language.name {
                                println!("Group: {}", group.name);
                            }
                        }
                    } else {
                        println!("Language: Unknown");
                    }
                },
                Err(err) => {
                    eprintln!("Error analyzing file: {}", err);
                    process::exit(1);
                }
            }
        },
        Commands::Analyze { path, breakdown, percentage, json, threads, parallel } => {
            if !path.exists() {
                eprintln!("Error: Path not found: {}", path.display());
                process::exit(1);
            }
            
            // Check if it's a Git repository
            let is_git_repo = GitRepo::open(&path).is_ok();
            
            if is_git_repo {
                println!("Git repository detected. Using directory analyzer for now.");
                // TODO: Implement Git repository analysis
            }
            
            // Create directory analyzer with optional parallel processing
            let mut analyzer = if parallel || threads > 0 {
                let mut config = ThreadingConfig::default();
                if threads > 0 {
                    config.worker_threads = threads;
                    config.io_threads = threads.min(8);
                }
                DirectoryAnalyzer::with_threading(&path, config)
            } else {
                DirectoryAnalyzer::new(&path)
            };
            
            match analyzer.analyze() {
                Ok(stats) => {
                    if json {
                        // Output JSON format
                        match serde_json::to_string_pretty(&stats.language_breakdown) {
                            Ok(json) => println!("{}", json),
                            Err(err) => {
                                eprintln!("Error generating JSON: {}", err);
                                process::exit(1);
                            }
                        }
                    } else {
                        // Output text format
                        if let Some(primary) = &stats.language {
                            println!("Primary language: {}", primary);
                        } else {
                            println!("No language detected");
                        }
                        
                        println!("\nLanguage breakdown:");
                        
                        // Sort languages by size (descending)
                        let mut languages: Vec<_> = stats.language_breakdown.iter().collect();
                        languages.sort_by(|a, b| b.1.cmp(a.1));
                        
                        // Calculate total for percentages
                        let total_size = stats.total_size;
                        
                        for (language, size) in languages {
                            if percentage {
                                let percent = (*size as f64 / total_size as f64) * 100.0;
                                println!("{}: {:.1}%", language, percent);
                            } else {
                                println!("{}: {} bytes", language, size);
                            }
                        }
                        
                        // Output file breakdown if requested
                        if breakdown {
                            println!("\nFile breakdown:");
                            
                            // Sort languages alphabetically
                            let mut languages: Vec<_> = stats.file_breakdown.keys().collect();
                            languages.sort();
                            
                            for language in languages {
                                println!("\n{}:", language);
                                
                                let files = &stats.file_breakdown[language];
                                for file in files {
                                    println!("  {}", file);
                                }
                            }
                        }
                    }
                },
                Err(err) => {
                    eprintln!("Error analyzing directory: {}", err);
                    process::exit(1);
                }
            }
        }
    }
}