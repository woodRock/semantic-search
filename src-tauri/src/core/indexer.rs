use anyhow::Result;
use crate::core::embedding::EmbeddingModel;
use crate::core::search::Search;
use crate::core::settings::Settings;
use crate::core::file_utils::{extract_text, get_metadata};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use tantivy::doc;
use tauri::{AppHandle, Emitter};

use std::sync::{Arc, Mutex};

pub struct Indexer {
    embedding_model: Arc<EmbeddingModel>,
    pub search: Arc<Mutex<Search>>,
    pub settings: Settings,
}

#[derive(serde::Serialize, Clone)]
struct ProgressEvent {
    message: String,
    current: usize,
    total: usize,
}

impl Indexer {
    pub fn new(index_dir: PathBuf, model_name: &str) -> Result<Self> {
        let settings = Settings::load(&index_dir);
        let embedding_model = Arc::new(EmbeddingModel::new(model_name, &settings.ollama_url)?);
        let search = Arc::new(Mutex::new(Search::new(&index_dir)?));
        
        Ok(Self {
            embedding_model,
            search,
            settings,
        })
    }

    fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        words.chunks(chunk_size)
             .map(|chunk| chunk.join(" "))
             .filter(|s| !s.trim().is_empty())
             .collect()
    }

    async fn generate_summary(&self, text: &str) -> String {
        "Summary disabled for speed.".to_string()
    }

    pub async fn index_file(&self, path: &Path) -> Result<()> {
        if !path.is_file() { return Ok(()); }
        
        let path_str = path.to_str().unwrap_or("").to_string();
        println!("Indexing file: {}", path_str);
        self.remove_file(path)?;
        
        if let Ok(text) = extract_text(path) {
            if text.is_empty() { return Ok(()); }
            
            let summary = self.generate_summary(&text).await;
            let modified = get_metadata(path).unwrap_or(0);
            let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();

            {
                let search = self.search.lock().unwrap();
                let mut writer = search.get_writer()?;
                let schema = writer.index().schema();
                let path_field = schema.get_field("path")?;
                let title_field = schema.get_field("title")?;
                let content_field = schema.get_field("content")?;
                let summary_field = schema.get_field("summary")?;
                let modified_field = schema.get_field("modified")?;

                writer.add_document(doc!(
                    path_field => path_str.clone(),
                    title_field => title,
                    content_field => text.clone(),
                    summary_field => summary,
                    modified_field => modified,
                ))?;
                writer.commit()?;
            }

            let chunks = Self::chunk_text(&text, 300);
            let chunk_refs: Vec<&str> = chunks.iter().map(|s| s.as_str()).collect();
            if let Ok(vectors) = self.embedding_model.embed_batch(chunk_refs).await {
                let mut search = self.search.lock().unwrap();
                for (chunk, vector) in chunks.into_iter().zip(vectors.into_iter()) {
                    search.add_vector_chunk(path_str.clone(), chunk, vector);
                }
            }
        }
        Ok(())
    }

    pub fn remove_file(&self, path: &Path) -> Result<()> {
        let path_str = path.to_str().unwrap_or("");
        let mut search = self.search.lock().unwrap();
        search.remove_file(path_str)
    }

    pub async fn index_directory(&self, dir_path: &Path, app: Option<AppHandle>) -> Result<()> {
        let mut builder = WalkBuilder::new(dir_path);
        builder.hidden(false).git_ignore(true);
        
        for ignored in &self.settings.ignored_paths {
            builder.add_ignore(ignored);
        }
        
        let walker = builder.build();

        let mut files_to_index = Vec::new();
        for entry in walker {
            if let Ok(entry) = entry {
                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    files_to_index.push(entry.path().to_path_buf());
                }
            }
        }

        let total_files = files_to_index.len();
        let app_arc = app.map(Arc::new);

        use futures::StreamExt;
        let file_stream = futures::stream::iter(files_to_index.into_iter().enumerate())
            .map(|(i, path)| {
                let app_inner = app_arc.clone();
                async move {
                    if let Some(app) = &app_inner {
                        let _ = app.emit("indexing-progress", ProgressEvent {
                            message: format!("Indexing {}", path.display()),
                            current: i + 1,
                            total: total_files,
                        });
                    }
                    let _ = self.index_file(&path).await;
                }
            })
            .buffer_unordered(4); // Process 4 files at a time

        file_stream.collect::<()>().await;

        {
            let search = self.search.lock().unwrap();
            search.save_vectors()?;
        }
        
        if let Some(app) = &app_arc {
            let _ = app.emit("indexing-progress", ProgressEvent {
                message: "Indexing complete".to_string(),
                current: total_files,
                total: total_files,
            });
        }
        
        Ok(())
    }
}
