use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct OllamaEmbedRequest<'a> {
    model: &'a str,
    input: Vec<&'a str>,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
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
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
        })
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let res = self.embed_batch(vec![text]).await?;
        res.into_iter().next().ok_or_else(|| anyhow!("No embedding returned"))
    }

    pub async fn embed_batch(&self, inputs: Vec<&str>) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() { return Ok(Vec::new()); }

        let req = OllamaEmbedRequest {
            model: &self.model_name,
            input: inputs,
        };

        let url = format!("{}/api/embed", self.base_url);
        
        let response = self.client.post(&url).json(&req).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama API error ({}): {}", status, err_text));
        }

        let parsed: OllamaEmbedResponse = response.json().await?;
        Ok(parsed.embeddings)
    }
}
