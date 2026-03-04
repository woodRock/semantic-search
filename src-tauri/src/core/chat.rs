use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: String,
}

pub struct ChatModel {
    model_name: String,
    base_url: String,
    client: reqwest::Client,
}

impl ChatModel {
    pub fn new(model_name: &str, base_url: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn ask(&self, query: &str, context: &[String]) -> Result<String> {
        let combined_context = context.join("\n\n---\n\n");
        let prompt = format!(
            "Use the following snippets from my local files to answer the question. If the answer isn't in the context, say so.\n\nContext:\n{}\n\nQuestion: {}",
            combined_context, query
        );

        let req = OllamaChatRequest {
            model: &self.model_name,
            messages: vec![ChatMessage {
                role: "user",
                content: &prompt,
            }],
            stream: false,
        };

        let url = format!("{}/api/chat", self.base_url);
        
        let response = self.client.post(&url).json(&req).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Ollama API error: {}", response.status()));
        }

        let parsed: OllamaChatResponse = response.json().await?;
        Ok(parsed.message.content)
    }
}
