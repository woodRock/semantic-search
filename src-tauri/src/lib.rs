// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
pub mod core;

use crate::core::indexer::Indexer;
use crate::core::search::Search;
use crate::core::embedding::EmbeddingModel;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[tauri::command]
async fn index_directory(handle: AppHandle, dir_path: String) -> Result<(), String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let indexer = Indexer::new(app_data_dir).await.map_err(|e| e.to_string())?;
    indexer.index_directory(&PathBuf::from(dir_path)).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn search(handle: AppHandle, query: String) -> Result<Vec<(String, f32)>, String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let searcher = Search::new(app_data_dir).map_err(|e| e.to_string())?;
    let embedding_model = EmbeddingModel::new().map_err(|e| e.to_string())?;
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
