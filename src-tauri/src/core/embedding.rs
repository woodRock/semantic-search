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
    client: reqwest::Client,
}

impl EmbeddingModel {
    pub fn new(model_name: &str) -> Result<Self> {
        Ok(Self {
            model_name: model_name.to_string(),
            client: reqwest::Client::new(),
        })
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let req = OllamaEmbedRequest {
            model: &self.model_name,
            prompt: text,
        };

        let response = self.client
            .post("http://localhost:11434/api/embeddings")
            .json(&req)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Ollama API error: {}", response.status()));
        }

        let parsed: OllamaEmbedResponse = response.json().await?;
        Ok(parsed.embedding)
    }
}
