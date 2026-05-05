pub mod process;
pub mod summary_shim;

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use summary_shim::SummaryEngine;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(thiserror::Error, Debug)]
pub enum LlmError {
    #[error("failed to spawn llama-server: {0}")]
    Spawn(String),
    #[error("backend not healthy: {0}")]
    HealthCheck(String),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("response decode error: {0}")]
    Decode(String),
    #[error("backend error: {0}")]
    Backend(String),
    #[error("config error: {0}")]
    Config(String),
}

pub enum EndpointSource {
    External(String),
    Spawn {
        model: PathBuf,
        port: u16,
        extra_args: Vec<String>,
    },
}

pub struct LlmBackendConfig {
    pub embed: EndpointSource,
    pub chat: EndpointSource,
    pub embed_model_name: String,
    pub chat_model_name: String,
    pub llama_server_bin: PathBuf,
}

pub(crate) struct EndpointHandle {
    pub url: String,
    pub child: Option<std::process::Child>,
}

pub struct LlmBackend {
    embed: EndpointHandle,
    chat: EndpointHandle,
    embed_model_name: String,
    chat_model_name: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedDatum>,
}

#[derive(Deserialize)]
struct EmbedDatum {
    embedding: Vec<f32>,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: Message,
}

impl LlmBackend {
    pub async fn launch(cfg: LlmBackendConfig) -> Result<Self, LlmError> {
        let bin = cfg.llama_server_bin;
        let embed = process::resolve_endpoint(cfg.embed, &bin, true).await?;
        let chat = process::resolve_endpoint(cfg.chat, &bin, false).await?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| LlmError::Config(e.to_string()))?;
        Ok(Self {
            embed,
            chat,
            embed_model_name: cfg.embed_model_name,
            chat_model_name: cfg.chat_model_name,
            client,
        })
    }

    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LlmError> {
        let url = format!("{}/v1/embeddings", self.embed.url);
        let req = EmbedRequest {
            model: &self.embed_model_name,
            input: texts,
        };
        let resp = self.client.post(&url).json(&req).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Backend(format!("embed {}: {}", status, body)));
        }
        let body: EmbedResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Decode(e.to_string()))?;
        Ok(body.data.into_iter().map(|d| d.embedding).collect())
    }

    pub async fn chat(&self, messages: Vec<Message>) -> Result<String, LlmError> {
        let url = format!("{}/v1/chat/completions", self.chat.url);
        let req = ChatRequest {
            model: &self.chat_model_name,
            messages,
        };
        let resp = self.client.post(&url).json(&req).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Backend(format!("chat {}: {}", status, body)));
        }
        let body: ChatResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Decode(e.to_string()))?;
        body.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| LlmError::Decode("no choices in chat response".into()))
    }

    pub fn child_pids(&self) -> Vec<u32> {
        let mut pids = Vec::new();
        if let Some(c) = &self.embed.child {
            pids.push(c.id());
        }
        if let Some(c) = &self.chat.child {
            pids.push(c.id());
        }
        pids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_response_deserializes() {
        let json = r#"{"data":[{"embedding":[0.1,0.2,0.3]},{"embedding":[0.4,0.5,0.6]}]}"#;
        let parsed: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn chat_response_deserializes() {
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"hello"}}]}"#;
        let parsed: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.choices[0].message.content, "hello");
    }

    #[test]
    fn embed_request_serializes() {
        let inputs = vec!["a".to_string(), "b".to_string()];
        let req = EmbedRequest {
            model: "default",
            input: &inputs,
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains(r#""model":"default""#));
        assert!(s.contains(r#""input":["a","b"]"#));
    }
}
