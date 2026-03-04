// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::{Parser, Subcommand};
use tauri_app_lib::core::indexer::Indexer;
use tauri_app_lib::core::search::{Search, SearchMode};
use tauri_app_lib::core::embedding::EmbeddingModel;
use std::path::PathBuf;
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(name = "semantic-search")]
#[command(about = "A cross-platform local file semantic search tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Index a directory
    Index {
        /// The directory to index
        path: String,
        /// Optional index data directory
        #[arg(short, long)]
        data_dir: Option<String>,
    },
    /// Search for files
    Search {
        /// The search query
        query: String,
        /// Optional index data directory
        #[arg(short, long)]
        data_dir: Option<String>,
        /// Number of results to return
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
        /// Filter by file extension (e.g. .rs, .md)
        #[arg(short, long)]
        ext: Option<String>,
        /// Treat query as a regular expression
        #[arg(short, long)]
        regex: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Some(command) = cli.command {
        // Run CLI
        let rt = Runtime::new().unwrap();
        match command {
            Commands::Index { path, data_dir } => {
                let data_path = get_data_dir(data_dir);
                let settings = tauri_app_lib::core::settings::Settings::load(&data_path);
                println!("Indexing {} into {}...", path, data_path.display());
                rt.block_on(async {
                    let indexer = Indexer::new(data_path, &settings.model_name).expect("Failed to create indexer");
                    indexer.index_directory(&PathBuf::from(path), None).await.expect("Failed to index");
                });
                println!("Indexing complete!");
            }
            Commands::Search { query, data_dir, limit, ext, regex } => {
                let data_path = get_data_dir(data_dir);
                let settings = tauri_app_lib::core::settings::Settings::load(&data_path);
                rt.block_on(async {
                    let searcher = Search::new(&data_path).expect("Failed to create searcher");
                    let embedding_model = EmbeddingModel::new(&settings.model_name, &settings.ollama_url).expect("Failed to create embedding model");
                    let filter_ref = ext.as_deref();
                    let results = searcher.hybrid_search(
                        &embedding_model, 
                        &query, 
                        limit, 
                        filter_ref, 
                        regex, 
                        SearchMode::Hybrid, 
                        None, 
                        None, 
                        false
                    ).await.expect("Search failed");
                    
                    println!("Search results for '{}':", query);
                    for result in results {
                        println!("{:.4} - {}", result.score, result.path);
                    }
                });
            }
        }
    } else {
        // Run GUI
        tauri_app_lib::run();
    }
}

fn get_data_dir(data_dir: Option<String>) -> PathBuf {
    if let Some(dir) = data_dir {
        PathBuf::from(dir)
    } else {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).expect("Could not find home directory");
        PathBuf::from(home).join(".semantic-search")
    }
}
