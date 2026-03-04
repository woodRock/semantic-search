use anyhow::Result;
use crate::core::embedding::EmbeddingModel;
use crate::core::search::Search;
use crate::core::file_utils::{extract_text, get_metadata};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use tantivy::doc;

pub struct Indexer {
    _embedding_model: EmbeddingModel,
    search: Search,
}

impl Indexer {
    pub async fn new(index_dir: PathBuf) -> Result<Self> {
        let embedding_model = EmbeddingModel::new()?;
        let search = Search::new(index_dir)?;
        
        Ok(Self {
            _embedding_model: embedding_model,
            search,
        })
    }

    pub async fn index_directory(&self, dir_path: &Path) -> Result<()> {
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

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                let path = entry.path();
                
                if let Ok(text) = extract_text(path) {
                    if text.is_empty() { continue; }
                    
                    let modified = get_metadata(path).unwrap_or(0);
                    let path_str = path.to_str().unwrap_or("").to_string();
                    let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();

                    writer.add_document(doc!(
                        path_field => path_str,
                        title_field => title,
                        content_field => text,
                        modified_field => modified,
                    ))?;
                }
            }
        }

        writer.commit()?;
        Ok(())
    }
}
