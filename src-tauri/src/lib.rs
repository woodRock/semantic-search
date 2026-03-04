pub mod core;

use crate::core::indexer::Indexer;
use crate::core::search::Search;
use crate::core::embedding::EmbeddingModel;
use crate::core::settings::Settings;
use crate::core::chat::ChatModel;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_global_shortcut::{Shortcut, Modifiers, Code};
use notify::{Watcher, RecursiveMode, Event};

#[tauri::command]
async fn index_directory(handle: AppHandle, dir_path: String) -> Result<(), String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let settings = Settings::load(&app_data_dir);
    let indexer = Indexer::new(app_data_dir.clone(), &settings.model_name).map_err(|e| e.to_string())?;
    let indexer_arc = Arc::new(indexer);
    
    let indexer_clone = indexer_arc.clone();
    let handle_clone = handle.clone();
    let dir_path_buf = PathBuf::from(&dir_path);
    
    indexer_clone.index_directory(&dir_path_buf, Some(handle_clone)).await.map_err(|e: anyhow::Error| e.to_string())?;

    // Incremental Watcher
    let dir_path_clone = dir_path.clone();
    let model_name_clone = settings.model_name.clone();
    tauri::async_runtime::spawn(async move {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();
        watcher.watch(PathBuf::from(&dir_path_clone).as_path(), RecursiveMode::Recursive).unwrap();

        for res in rx {
            if let Ok(Event { kind, paths, .. }) = res {
                let indexer = Indexer::new(app_data_dir.clone(), &model_name_clone).unwrap();
                for path in paths {
                    match kind {
                        notify::EventKind::Modify(_) | notify::EventKind::Create(_) => {
                            let _ = indexer.index_file(&path).await;
                        }
                        notify::EventKind::Remove(_) => {
                            let _ = indexer.remove_file(&path);
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn simple_search(
    handle: AppHandle,
    query: String,
    limit: usize,
) -> Result<Vec<crate::core::search::SearchResult>, String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let settings = Settings::load(&app_data_dir);
    
    let searcher = Search::new(&app_data_dir).map_err(|e| e.to_string())?;
    let embedding_model = EmbeddingModel::new(&settings.model_name, &settings.ollama_url).map_err(|e| e.to_string())?;

    let results = searcher.hybrid_search(
        &embedding_model,
        &query,
        limit,
        None,
        false,
        crate::core::search::SearchMode::Hybrid,
        None,
        None,
        false
    ).await.map_err(|e| e.to_string())?;

    Ok(results)
}

#[tauri::command]
async fn get_settings(handle: AppHandle) -> Result<Settings, String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(Settings::load(&app_data_dir))
}

#[tauri::command]
async fn update_settings(handle: AppHandle, settings: Settings) -> Result<(), String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    settings.save(&app_data_dir).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_path(handle: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    handle.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn ask_question(handle: AppHandle, query: String, context: Vec<String>) -> Result<String, String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let settings = Settings::load(&app_data_dir);
    let chat_model = ChatModel::new(&settings.model_name, &settings.ollama_url);
    
    let clean_context: Vec<String> = context.into_iter()
        .map(|s| s.replace("<b>", "").replace("</b>", "").replace("<i>", "").replace("</i>", ""))
        .collect();
    
    chat_model.ask(&query, &clean_context).await.map_err(|e| e.to_string())
}

fn toggle_window(app: &AppHandle) {
    let window = app.get_webview_window("main").unwrap();
    if window.is_visible().unwrap() {
        window.hide().unwrap();
    } else {
        window.show().unwrap();
        window.set_focus().unwrap();
    }
}

#[tauri::command]
async fn get_file_summary(handle: AppHandle, path: String) -> Result<String, String> {
    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let settings = Settings::load(&app_data_dir);
    let searcher = Search::new(&app_data_dir).map_err(|e| e.to_string())?;
    
    let embedding_model = EmbeddingModel::new(&settings.model_name, &settings.ollama_url).map_err(|e| e.to_string())?;
    let results = searcher.hybrid_search(
        &embedding_model, 
        &format!("path:\"{}\"", path), 
        1, 
        None, 
        false, 
        crate::core::search::SearchMode::Keyword,
        None,
        None,
        false
    ).await.map_err(|e| e.to_string())?;
    
    if let Some(res) = results.first() {
        Ok(res.summary.clone())
    } else {
        Err("File not found in index".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let ctrl_shift_space = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcut(ctrl_shift_space)
                .unwrap()
                .with_handler(move |app, shortcut, _event| {
                    if shortcut == &ctrl_shift_space {
                        toggle_window(app);
                    }
                })
                .build(),
        )
        .setup(|_app| {
            #[cfg(target_os = "macos")]
            _app.set_activation_policy(tauri::ActivationPolicy::Regular);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![index_directory, simple_search, get_settings, update_settings, open_path, ask_question, get_file_summary])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
