use anyhow::Result;
use crate::core::embedding::EmbeddingModel;
use crate::core::search::Search;
use crate::core::file_utils::{extract_text, get_metadata};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use tantivy::doc;
use tauri::{AppHandle, Emitter};

pub struct Indexer {
    embedding_model: EmbeddingModel,
    pub search: Search,
}

#[derive(serde::Serialize, Clone)]
struct ProgressEvent {
    message: String,
    current: usize,
    total: usize,
}

impl Indexer {
    pub fn new(index_dir: PathBuf, model_name: &str) -> Result<Self> {
        let embedding_model = EmbeddingModel::new(model_name)?;
        let search = Search::new(&index_dir)?;
        
        Ok(Self {
            embedding_model,
            search,
        })
    }

    fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        words.chunks(chunk_size)
             .map(|chunk| chunk.join(" "))
             .filter(|s| !s.trim().is_empty())
             .collect()
    }

    pub async fn index_directory(&mut self, dir_path: &Path, app: Option<&AppHandle>) -> Result<()> {
        let mut writer = self.search.get_writer()?;
        let schema = writer.index().schema();
        let path_field = schema.get_field("path")?;
        let title_field = schema.get_field("title")?;
        let content_field = schema.get_field("content")?;
        let modified_field = schema.get_field("modified")?;

        let walker = WalkBuilder::new(dir_path)
            .hidden(false)
            .git_ignore(true)
            .build();

        // Collect files first for progress
        let mut files_to_index = Vec::new();
        for entry in walker {
            if let Ok(entry) = entry {
                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    files_to_index.push(entry.path().to_path_buf());
                }
            }
        }

        let total_files = files_to_index.len();

        for (i, path) in files_to_index.into_iter().enumerate() {
            if let Some(app) = app {
                let _ = app.emit("indexing-progress", ProgressEvent {
                    message: format!("Indexing {}", path.display()),
                    current: i + 1,
                    total: total_files,
                });
            }

            if let Ok(text) = extract_text(&path) {
                if text.is_empty() { continue; }
                
                let modified = get_metadata(&path).unwrap_or(0);
                let path_str = path.to_str().unwrap_or("").to_string();
                let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();

                // 1. Add to Tantivy for keyword search
                writer.add_document(doc!(
                    path_field => path_str.clone(),
                    title_field => title.clone(),
                    content_field => text.clone(),
                    modified_field => modified,
                ))?;

                // 2. Chunk and Vectorize
                let chunks = Self::chunk_text(&text, 300); // 300 words per chunk
                for chunk in chunks {
                    if let Ok(vector) = self.embedding_model.embed(&chunk).await {
                        self.search.add_vector_chunk(path_str.clone(), chunk, vector);
                    }
                }
            }
        }

        writer.commit()?;
        self.search.save_vectors()?;
        
        if let Some(app) = app {
            let _ = app.emit("indexing-progress", ProgressEvent {
                message: "Indexing complete".to_string(),
                current: total_files,
                total: total_files,
            });
        }
        
        Ok(())
    }
}
