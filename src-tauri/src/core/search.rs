use anyhow::{Result, Context};
use tantivy::{schema::*, Index, IndexWriter, query::QueryParser, collector::TopDocs, TantivyDocument, SnippetGenerator};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use std::fs;
use crate::core::embedding::EmbeddingModel;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum SearchMode {
    Hybrid,
    Semantic,
    Keyword,
}

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
    pub summary: String,
    pub modified: u64,
}

impl Search {
    pub fn new(index_path: &Path) -> Result<Self> {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("path", STRING | STORED);
        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("content", TEXT | STORED);
        schema_builder.add_text_field("summary", TEXT | STORED);
        schema_builder.add_u64_field("modified", STORED);
        
        let schema = schema_builder.build();
        let tantivy_dir = index_path.join("tantivy");
        fs::create_dir_all(&tantivy_dir)?;
        
        let directory = tantivy::directory::MmapDirectory::open(&tantivy_dir)?;
        let index = match Index::open_or_create(directory, schema.clone()) {
            Ok(idx) => idx,
            Err(_) => {
                // If opening fails (likely schema mismatch), clear the directory and try again
                fs::remove_dir_all(&tantivy_dir)?;
                fs::create_dir_all(&tantivy_dir)?;
                let new_directory = tantivy::directory::MmapDirectory::open(&tantivy_dir)?;
                Index::create(new_directory, schema.clone(), tantivy::IndexSettings::default())?
            }
        };
        
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

    fn get_modified(&self, path: &str, searcher: &tantivy::Searcher) -> Option<u64> {
        let path_field = self.schema.get_field("path").unwrap();
        let query = tantivy::query::TermQuery::new(
            tantivy::Term::from_field_text(path_field, path),
            tantivy::schema::IndexRecordOption::Basic,
        );
        let top_docs = searcher.search(&query, &TopDocs::with_limit(1)).ok()?;
        if let Some((_, doc_address)) = top_docs.first() {
            let doc = searcher.doc::<TantivyDocument>(*doc_address).ok()?;
            let modified_field = self.schema.get_field("modified").unwrap();
            return doc.get_first(modified_field).and_then(|v| v.as_u64());
        }
        None
    }

    pub async fn hybrid_search(
        &self, 
        embedding_model: &EmbeddingModel, 
        query_str: &str, 
        limit: usize,
        file_type_filter: Option<&str>,
        is_regex: bool,
        search_mode: SearchMode,
        modified_after: Option<u64>,
        modified_before: Option<u64>,
        return_directory: bool,
    ) -> Result<Vec<SearchResult>> {
        println!("Starting search [Mode: {:?}] for: {}", search_mode, query_str);
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        
        let content_field = self.schema.get_field("content").context("Missing content field")?;
        let path_field = self.schema.get_field("path").context("Missing path field")?;
        let summary_field = self.schema.get_field("summary").ok();
        let modified_field = self.schema.get_field("modified").unwrap();
        
        let mut keyword_scores = HashMap::new();
        let mut snippets = HashMap::new();
        let mut summaries = HashMap::new();
        let mut final_modified = HashMap::new();

        let use_keyword = match search_mode {
            SearchMode::Hybrid | SearchMode::Keyword => true,
            SearchMode::Semantic => false,
        };

        if use_keyword {
            if is_regex {
                println!("Performing regex search");
                let regex_query = tantivy::query::RegexQuery::from_pattern(query_str, content_field)?;
                let top_docs = searcher.search(&regex_query, &TopDocs::with_limit(limit * 2))?;
                
                for (rank, (_score, doc_address)) in top_docs.into_iter().enumerate() {
                    let retrieved_doc = searcher.doc::<TantivyDocument>(doc_address)?;
                    let modified = retrieved_doc.get_first(modified_field).and_then(|v| v.as_u64()).unwrap_or(0);
                    
                    if let Some(after) = modified_after { if modified < after { continue; } }
                    if let Some(before) = modified_before { if modified > before { continue; } }

                    if let Some(path) = retrieved_doc.get_first(path_field).and_then(|v| v.as_str()) {
                        if let Some(ext) = file_type_filter {
                            if !path.ends_with(ext) { continue; }
                        }

                        let rrf_score = 1.0 / (60.0 + rank as f32);
                        keyword_scores.insert(path.to_string(), rrf_score);
                        snippets.insert(path.to_string(), format!("<i>Regex Match</i>: {}", query_str));
                        final_modified.insert(path.to_string(), modified);

                        if let Some(field) = summary_field {
                            if let Some(summary) = retrieved_doc.get_first(field).and_then(|v| v.as_str()) {
                                summaries.insert(path.to_string(), summary.to_string());
                            }
                        }
                    }
                }
            } else {
                println!("Performing keyword search");
                let query_parser = QueryParser::for_index(&self.index, vec![
                    self.schema.get_field("title")?,
                    content_field,
                ]);
                let query = query_parser.parse_query(query_str)?;
                let top_docs = searcher.search(&query, &TopDocs::with_limit(limit * 2))?;

                let snippet_generator = SnippetGenerator::create(&searcher, &*query, content_field)?;

                for (rank, (_score, doc_address)) in top_docs.into_iter().enumerate() {
                    let retrieved_doc = searcher.doc::<TantivyDocument>(doc_address)?;
                    let modified = retrieved_doc.get_first(modified_field).and_then(|v| v.as_u64()).unwrap_or(0);
                    
                    if let Some(after) = modified_after { if modified < after { continue; } }
                    if let Some(before) = modified_before { if modified > before { continue; } }

                    if let Some(path) = retrieved_doc.get_first(path_field).and_then(|v| v.as_str()) {
                        if let Some(ext) = file_type_filter {
                            if !path.ends_with(ext) { continue; }
                        }

                        let rrf_score = 1.0 / (60.0 + rank as f32);
                        keyword_scores.insert(path.to_string(), rrf_score);
                        
                        let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
                        snippets.insert(path.to_string(), snippet.to_html());
                        final_modified.insert(path.to_string(), modified);

                        if let Some(field) = summary_field {
                            if let Some(summary) = retrieved_doc.get_first(field).and_then(|v| v.as_str()) {
                                summaries.insert(path.to_string(), summary.to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut vector_scores = HashMap::new();
        
        if !is_regex {
            let use_vector = match search_mode {
                SearchMode::Hybrid | SearchMode::Semantic => true,
                SearchMode::Keyword => false,
            };

            if use_vector {
                println!("Performing vector search");
                let query_vector = embedding_model.embed(query_str).await.unwrap_or_default();
                if !query_vector.is_empty() {
                    let mut all_vectors: Vec<(&VectorDoc, f32)> = self.vector_docs
                        .iter()
                        .filter(|doc| {
                            if let Some(ext) = file_type_filter {
                                if !doc.path.ends_with(ext) { return false; }
                            }
                            // Check date filtering for vectors by doing a quick Tantivy lookup
                            if modified_after.is_some() || modified_before.is_some() {
                                let doc_mod = self.get_modified(&doc.path, &searcher).unwrap_or(0);
                                if let Some(after) = modified_after { if doc_mod < after { return false; } }
                                if let Some(before) = modified_before { if doc_mod > before { return false; } }
                                final_modified.insert(doc.path.clone(), doc_mod); // Cache it
                            }
                            true
                        })
                        .map(|doc| (doc, Self::cosine_similarity(&doc.vector, &query_vector)))
                        .collect();
                        
                    all_vectors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                    
                    for (rank, (doc, sim)) in all_vectors.into_iter().take(limit * 2).enumerate() {
                        if sim <= 0.0 { continue; }
                        let rrf_score = 1.0 / (60.0 + rank as f32);
                        let entry = vector_scores.entry(doc.path.clone()).or_insert(0.0);
                        if rrf_score > *entry {
                            *entry = rrf_score;
                        }
                        if !snippets.contains_key(&doc.path) {
                            snippets.insert(doc.path.clone(), format!("<i>Semantic match</i>: ...{}...", &doc.chunk_text.chars().take(200).collect::<String>()));
                        }
                        if !final_modified.contains_key(&doc.path) {
                            final_modified.insert(doc.path.clone(), self.get_modified(&doc.path, &searcher).unwrap_or(0));
                        }
                    }
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

        let res_vec = final_results.into_iter().map(|(path, score)| {
            let snippet = snippets.get(&path).cloned().unwrap_or_default();
            let summary = summaries.get(&path).cloned().unwrap_or_else(|| "No summary available.".to_string());
            let modified = final_modified.get(&path).cloned().unwrap_or(0);
            SearchResult { path, score, snippet, summary, modified }
        }).collect::<Vec<_>>();

        if return_directory {
            let mut dir_scores: HashMap<String, f32> = HashMap::new();
            let mut dir_summaries: HashMap<String, Vec<String>> = HashMap::new();
            let mut dir_modified: HashMap<String, u64> = HashMap::new();
            
            for res in res_vec {
                if let Some(parent) = std::path::Path::new(&res.path).parent() {
                    let parent_str = parent.to_str().unwrap_or("").to_string();
                    *dir_scores.entry(parent_str.clone()).or_insert(0.0) += res.score;
                    let filename = std::path::Path::new(&res.path).file_name().unwrap_or_default().to_string_lossy();
                    dir_summaries.entry(parent_str.clone()).or_insert(Vec::new()).push(format!("Found relevant file: {}", filename));
                    
                    let curr_mod = dir_modified.entry(parent_str).or_insert(0);
                    if res.modified > *curr_mod {
                        *curr_mod = res.modified;
                    }
                }
            }
            
            let mut dir_results: Vec<SearchResult> = dir_scores.into_iter().map(|(dir, score)| {
                let snippets = dir_summaries.get(&dir).unwrap().join("<br/>");
                let modified = dir_modified.get(&dir).cloned().unwrap_or(0);
                SearchResult {
                    path: dir,
                    score,
                    snippet: snippets,
                    summary: "Directory containing relevant files.".to_string(),
                    modified,
                }
            }).collect();
            
            dir_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            dir_results.truncate(limit);
            return Ok(dir_results);
        }

        Ok(res_vec)
    }
}
