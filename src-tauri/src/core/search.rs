use anyhow::Result;
use tantivy::{schema::*, Index, IndexWriter, query::QueryParser, collector::TopDocs, TantivyDocument};
use std::path::PathBuf;
use crate::core::embedding::EmbeddingModel;

pub struct Search {
    index: Index,
    schema: Schema,
}

impl Search {
    pub fn new(index_path: PathBuf) -> Result<Self> {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("path", STRING | STORED);
        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("content", TEXT | STORED);
        schema_builder.add_u64_field("modified", STORED);
        
        let schema = schema_builder.build();
        std::fs::create_dir_all(&index_path)?;
        let index = Index::open_or_create(tantivy::directory::MmapDirectory::open(&index_path)?, schema.clone())?;
        
        Ok(Self { index, schema })
    }

    pub fn get_writer(&self) -> Result<IndexWriter> {
        Ok(self.index.writer(50_000_000)?)
    }

    pub async fn hybrid_search(&self, _embedding_model: &EmbeddingModel, query_str: &str, limit: usize) -> Result<Vec<(String, f32)>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        
        // 1. Keyword search
        let query_parser = QueryParser::for_index(&self.index, vec![
            self.schema.get_field("title")?,
            self.schema.get_field("content")?,
        ]);
        let query = query_parser.parse_query(query_str)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        let path_field = self.schema.get_field("path")?;
        
        for (score, doc_address) in top_docs {
            let retrieved_doc = searcher.doc::<TantivyDocument>(doc_address)?;
            let path = retrieved_doc.get_first(path_field).and_then(|v| v.as_str()).unwrap_or("").to_string();
            results.push((path, score));
        }

        Ok(results)
    }
}
