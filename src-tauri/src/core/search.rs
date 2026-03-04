use anyhow::{Result, Context};
use tantivy::{schema::*, Index, IndexWriter, query::QueryParser, collector::TopDocs, TantivyDocument, SnippetGenerator};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use std::fs;
use crate::core::embedding::EmbeddingModel;
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct VectorDoc {
    pub path: String,
    pub chunk_text: String,
    pub vector: Vec<f32>,
}

pub struct Search {
    index: Index,
    schema: Schema,
    vector_docs: Vec<VectorDoc>,
    vector_store_path: PathBuf,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
    pub snippet: String,
}

impl Search {
    pub fn new(index_path: &Path) -> Result<Self> {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("path", STRING | STORED);
        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("content", TEXT | STORED);
        schema_builder.add_u64_field("modified", STORED);
        
        let schema = schema_builder.build();
        let tantivy_dir = index_path.join("tantivy");
        fs::create_dir_all(&tantivy_dir)?;
        let index = Index::open_or_create(tantivy::directory::MmapDirectory::open(&tantivy_dir)?, schema.clone())?;
        
        let vector_store_path = index_path.join("vectors.bin");
        let vector_docs = if vector_store_path.exists() {
            let data = fs::read(&vector_store_path)?;
            rmp_serde::from_slice(&data).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Self { index, schema, vector_docs, vector_store_path })
    }

    pub fn get_writer(&self) -> Result<IndexWriter> {
        Ok(self.index.writer(50_000_000)?)
    }

    pub fn add_vector_chunk(&mut self, path: String, chunk_text: String, vector: Vec<f32>) {
        self.vector_docs.push(VectorDoc { path, chunk_text, vector });
    }

    pub fn save_vectors(&self) -> Result<()> {
        let data = rmp_serde::to_vec(&self.vector_docs)?;
        fs::write(&self.vector_store_path, data)?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        let mut writer = self.get_writer()?;
        writer.delete_all_documents()?;
        writer.commit()?;
        self.vector_docs.clear();
        self.save_vectors()?;
        Ok(())
    }

    pub fn remove_file(&mut self, path: &str) -> Result<()> {
        let mut writer = self.get_writer()?;
        let path_field = self.schema.get_field("path")?;
        writer.delete_term(tantivy::Term::from_field_text(path_field, path));
        writer.commit()?;
        
        self.vector_docs.retain(|doc| doc.path != path);
        self.save_vectors()?;
        Ok(())
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot_product / (norm_a * norm_b) }
    }

    pub async fn hybrid_search(&self, embedding_model: &EmbeddingModel, query_str: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        
        let content_field = self.schema.get_field("content").context("Missing content field")?;
        let path_field = self.schema.get_field("path").context("Missing path field")?;
        
        let query_parser = QueryParser::for_index(&self.index, vec![
            self.schema.get_field("title")?,
            content_field,
        ]);
        let query = query_parser.parse_query(query_str)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let snippet_generator = SnippetGenerator::create(&searcher, &*query, content_field)?;

        let mut keyword_scores = HashMap::new();
        let mut snippets = HashMap::new();

        for (rank, (_score, doc_address)) in top_docs.into_iter().enumerate() {
            let retrieved_doc = searcher.doc::<TantivyDocument>(doc_address)?;
            if let Some(path) = retrieved_doc.get_first(path_field).and_then(|v| v.as_str()) {
                let rrf_score = 1.0 / (60.0 + rank as f32);
                keyword_scores.insert(path.to_string(), rrf_score);
                
                let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
                snippets.insert(path.to_string(), snippet.to_html());
            }
        }

        let query_vector = embedding_model.embed(query_str).await.unwrap_or_default();
        let mut vector_scores = HashMap::new();
        
        if !query_vector.is_empty() {
            let mut all_vectors: Vec<(&VectorDoc, f32)> = self.vector_docs
                .iter()
                .map(|doc| (doc, Self::cosine_similarity(&doc.vector, &query_vector)))
                .collect();
                
            all_vectors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            
            for (rank, (doc, _sim)) in all_vectors.into_iter().take(limit * 2).enumerate() {
                let rrf_score = 1.0 / (60.0 + rank as f32);
                let entry = vector_scores.entry(doc.path.clone()).or_insert(0.0);
                if rrf_score > *entry {
                    *entry = rrf_score;
                }
                if !snippets.contains_key(&doc.path) {
                    snippets.insert(doc.path.clone(), format!("<i>Semantic match</i>: ...{}...", &doc.chunk_text.chars().take(200).collect::<String>()));
                }
            }
        }

        let mut combined_scores: HashMap<String, f32> = HashMap::new();
        for (path, score) in keyword_scores {
            *combined_scores.entry(path).or_insert(0.0) += score;
        }
        for (path, score) in vector_scores {
            *combined_scores.entry(path).or_insert(0.0) += score;
        }

        let mut final_results: Vec<_> = combined_scores.into_iter().collect();
        final_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        final_results.truncate(limit);

        Ok(final_results.into_iter().map(|(path, score)| {
            let snippet = snippets.get(&path).cloned().unwrap_or_default();
            SearchResult { path, score, snippet }
        }).collect())
    }
}
