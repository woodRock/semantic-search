use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct OllamaEmbedRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embedding: Vec<f32>,
}

pub struct EmbeddingModel {
    model_name: String,
    base_url: String,
    client: reqwest::Client,
}

impl EmbeddingModel {
    pub fn new(model_name: &str, base_url: &str) -> Result<Self> {
        Ok(Self {
            model_name: model_name.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        })
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let req = OllamaEmbedRequest {
            model: &self.model_name,
            prompt: text,
        };

        let url = format!("{}/api/embeddings", self.base_url);
        
        let response = match self.client.post(&url).json(&req).send().await {
            Ok(res) => res,
            Err(_) => return Ok(Vec::new()), // Graceful fallback if Ollama is offline
        };

        if !response.status().is_success() {
            return Ok(Vec::new()); // Fallback on HTTP error
        }

        let parsed: OllamaEmbedResponse = match response.json().await {
            Ok(p) => p,
            Err(_) => return Ok(Vec::new()),
        };
        
        Ok(parsed.embedding)
    }
}
