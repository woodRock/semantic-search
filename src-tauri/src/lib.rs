// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
pub mod core;

use crate::core::indexer::Indexer;
use crate::core::search::{Search, SearchResult};
use crate::core::embedding::EmbeddingModel;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use notify::{Watcher, RecursiveMode, Event};
use std::sync::Arc;
use tokio::sync::Mutex;

// We use Qwen2.5:0.5b (or whatever the user has locally named qwen3.5-0.8b)
const MODEL_NAME: &str = "qwen3.5:0.8b";

#[tauri::command]
async fn index_directory(handle: AppHandle, dir_path: String) -> Result<(), String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut indexer = Indexer::new(app_data_dir.clone(), MODEL_NAME).map_err(|e| e.to_string())?;
    
    // Perform initial indexing
    indexer.index_directory(&PathBuf::from(&dir_path), Some(&handle)).await.map_err(|e| e.to_string())?;

    // Setup background watcher
    let dir_path_clone = dir_path.clone();
    tauri::async_runtime::spawn(async move {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();
        watcher.watch(PathBuf::from(&dir_path_clone).as_path(), RecursiveMode::Recursive).unwrap();

        for res in rx {
            match res {
                Ok(Event { kind: notify::EventKind::Modify(_) | notify::EventKind::Create(_), paths, .. }) => {
                    // Very naive incremental indexing: just re-index the whole directory on change
                    // In a production app, you'd only index the specific file
                    if let Ok(mut indexer) = Indexer::new(app_data_dir.clone(), MODEL_NAME) {
                        let _ = indexer.search.clear(); // simplistic sync
                        let _ = indexer.index_directory(&PathBuf::from(&dir_path_clone), None).await;
                    }
                }
                _ => {}
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn search(handle: AppHandle, query: String) -> Result<Vec<SearchResult>, String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let searcher = Search::new(&app_data_dir).map_err(|e| e.to_string())?;
    let embedding_model = EmbeddingModel::new(MODEL_NAME).map_err(|e| e.to_string())?;
    let results = searcher.hybrid_search(&embedding_model, &query, 20).await.map_err(|e| e.to_string())?;
    Ok(results)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![index_directory, search])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
