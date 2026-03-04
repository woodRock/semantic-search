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
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a str>,
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
            format: None,
        };

        let url = format!("{}/api/chat", self.base_url);
        
        let response = self.client.post(&url).json(&req).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Ollama API error: {}", response.status()));
        }

        let parsed: OllamaChatResponse = response.json().await?;
        Ok(parsed.message.content)
    }

    pub async fn ask_json(&self, prompt: &str) -> Result<String> {
        let req = OllamaChatRequest {
            model: &self.model_name,
            messages: vec![ChatMessage {
                role: "user",
                content: prompt,
            }],
            stream: false,
            format: Some("json"),
        };

        let url = format!("{}/api/chat", self.base_url);
        
        let response = self.client.post(&url).json(&req).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Ollama API error: {}", response.status()));
        }

        let parsed: OllamaChatResponse = response.json().await?;
        Ok(parsed.message.content)
    }

    pub async fn stream_chat<F>(&self, prompt: &str, is_json: bool, mut callback: F) -> Result<String>
    where
        F: FnMut(String),
    {
        use futures::StreamExt;
        use tokio::io::AsyncBufReadExt;
        use tokio_util::io::StreamReader;

        let req = OllamaChatRequest {
            model: &self.model_name,
            messages: vec![ChatMessage {
                role: "user",
                content: prompt,
            }],
            stream: true,
            format: if is_json { Some("json") } else { None },
        };

        let url = format!("{}/api/chat", self.base_url);
        let response = self.client.post(&url).json(&req).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Ollama API error: {}", response.status()));
        }

        let stream = response
            .bytes_stream()
            .map(|result| result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
        
        let reader = StreamReader::new(stream);
        let mut lines = reader.lines();

        let mut full_content = String::new();

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() { continue; }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(content) = json["message"]["content"].as_str() {
                    full_content.push_str(content);
                    callback(content.to_string());
                }
                if json["done"].as_bool().unwrap_or(false) {
                    break;
                }
            }
        }

        Ok(full_content)
    }
}
