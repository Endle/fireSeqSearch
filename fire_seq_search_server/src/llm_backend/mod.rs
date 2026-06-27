pub mod model_fetch;
pub mod process;

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

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

/// Which OpenAI-compatible server an `External` endpoint actually is. They all
/// speak `/v1/chat/completions`, but differ in the edges: only llama-server
/// exposes `/health` for our readiness probe and understands the
/// `chat_template_kwargs.enable_thinking` control; Ollama/OpenAI reject unknown
/// request fields and (for OpenAI) need a Bearer key. Embeddings stay local on
/// bge-m3 regardless — this only shapes the chat path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum LlmFlavor {
    /// Local llama.cpp `llama-server` (or our bundled llamafile).
    #[default]
    #[value(name = "llama-server")]
    LlamaServer,
    /// Ollama's OpenAI-compatible endpoint.
    #[value(name = "ollama")]
    Ollama,
    /// A remote OpenAI-compatible API (OpenAI and work-alikes).
    #[value(name = "openai")]
    OpenAi,
}

impl LlmFlavor {
    /// Only llama-server exposes a `/health` endpoint we can poll for readiness.
    /// For the others we skip the probe and let the first real request surface
    /// any error.
    fn has_health_endpoint(self) -> bool {
        matches!(self, LlmFlavor::LlamaServer)
    }

    /// Only llama-server (with `--jinja`) understands `enable_thinking`. Sending
    /// it to Ollama/OpenAI risks a 400 on unknown fields, so omit it there.
    fn supports_enable_thinking(self) -> bool {
        matches!(self, LlmFlavor::LlamaServer)
    }
}

pub enum EndpointSource {
    External {
        url: String,
        flavor: LlmFlavor,
        /// Optional Bearer token, attached to every request when present.
        api_key: Option<String>,
    },
    Spawn {
        model: PathBuf,
        port: u16,
        /// GPU layers to offload (`-ngl`). Reinterpreted as a fallback policy by
        /// the spawn layer: `> 0` means "try GPU, fall back to CPU if the backend
        /// fails to come up" (the default `99`); `0` means "force CPU, skip the
        /// doomed GPU attempt". See `process::spawn`.
        gpu_layers: u32,
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
    pub flavor: LlmFlavor,
    pub api_key: Option<String>,
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
    // Omitted entirely for non-llama-server flavors, which may 400 on the field.
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_template_kwargs: Option<ChatTemplateKwargs>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: Message,
}

#[derive(Serialize)]
struct StreamChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message>,
    stream: bool,
    // Omitted entirely for non-llama-server flavors, which may 400 on the field.
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_template_kwargs: Option<ChatTemplateKwargs>,
}

// Disables the Qwen3-family thinking trace via the jinja chat template.
// Without this, every chat call generates thousands of reasoning tokens
// before the actual answer — summarizing one note then takes ~100s and the
// summary backlog drains at roughly 1/min. The `/no_think` magic string in
// the prompt is a Qwen3-original convention that the Qwen3.5 build ignores;
// this flag is the supported control.
#[derive(Serialize, Clone, Copy)]
struct ChatTemplateKwargs {
    enable_thinking: bool,
}

const NO_THINK: ChatTemplateKwargs = ChatTemplateKwargs { enable_thinking: false };

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: StreamDelta,
}

#[derive(Deserialize, Default)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
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
        let mut rb = self.client.post(&url).json(&req);
        if let Some(key) = &self.embed.api_key {
            rb = rb.bearer_auth(key);
        }
        let resp = rb.send().await?;
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
            chat_template_kwargs: self.chat_template_kwargs(),
        };
        let mut rb = self
            .client
            .post(&url)
            // Override the client-wide 60s timeout: summarizing a long page on
            // CPU/limited-GPU can take longer than that. Same rationale as
            // chat_stream below.
            .timeout(Duration::from_secs(600))
            .json(&req);
        if let Some(key) = &self.chat.api_key {
            rb = rb.bearer_auth(key);
        }
        let resp = rb.send().await?;
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
            .map(|c| strip_think_artifact(&c.message.content))
            .ok_or_else(|| LlmError::Decode("no choices in chat response".into()))
    }

    /// Streaming chat completion. Returns a channel receiver yielding content
    /// deltas as they arrive from llama-server (`stream: true`). The terminal
    /// `data: [DONE]` sentinel ends the stream; transport errors are delivered
    /// as the final `Err` item. Used by `/ask`.
    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
    ) -> Result<futures::channel::mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        use futures::{SinkExt, StreamExt};

        let url = format!("{}/v1/chat/completions", self.chat.url);
        let req = StreamChatRequest {
            model: &self.chat_model_name,
            messages,
            stream: true,
            chat_template_kwargs: self.chat_template_kwargs(),
        };
        let mut rb = self
            .client
            .post(&url)
            // Override the client-wide 60s timeout: a streamed answer can take
            // longer than that to finish on CPU, and the timeout covers the
            // whole response body.
            .timeout(Duration::from_secs(600))
            .json(&req);
        if let Some(key) = &self.chat.api_key {
            rb = rb.bearer_auth(key);
        }
        let resp = rb.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Backend(format!("chat stream {}: {}", status, body)));
        }

        let (mut tx, rx) = futures::channel::mpsc::channel::<Result<String, LlmError>>(64);
        tokio::spawn(async move {
            let mut byte_stream = resp.bytes_stream();
            let mut buf: Vec<u8> = Vec::new();
            'outer: while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx.send(Err(LlmError::Http(e))).await;
                        return;
                    }
                };
                buf.extend_from_slice(&bytes);
                while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = buf.drain(..=pos).collect();
                    // A complete SSE line; it's UTF-8 (JSON from the server).
                    let line = String::from_utf8_lossy(&line);
                    let line = line.trim();
                    let payload = match line.strip_prefix("data:") {
                        Some(p) => p.trim(),
                        None => continue, // comment/keepalive/blank line
                    };
                    if payload.is_empty() {
                        continue;
                    }
                    if payload == "[DONE]" {
                        break 'outer;
                    }
                    match serde_json::from_str::<StreamChunk>(payload) {
                        Ok(sc) => {
                            if let Some(choice) = sc.choices.into_iter().next() {
                                if let Some(content) = choice.delta.content {
                                    if !content.is_empty()
                                        && tx.send(Ok(content)).await.is_err()
                                    {
                                        return; // receiver dropped (client gone)
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            // Non-JSON data line — ignore rather than abort.
                        }
                    }
                }
            }
        });
        Ok(rx)
    }

    /// The `enable_thinking=false` kwarg, but only for chat flavors that accept
    /// it (llama-server). For Ollama/OpenAI we send nothing rather than risk a
    /// 400 on an unknown field.
    fn chat_template_kwargs(&self) -> Option<ChatTemplateKwargs> {
        if self.chat.flavor.supports_enable_thinking() {
            Some(NO_THINK)
        } else {
            None
        }
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

/// With `enable_thinking=false`, Qwen3-family Jinja templates still seed the
/// assistant turn with an empty `<think>\n\n</think>` block. The opener is
/// usually consumed by the template before the model emits anything, but the
/// closer can leak into `message.content` as a bare `</think>` prefix; in rare
/// cases the model emits both tags around empty/whitespace content. Strip
/// either shape so callers see only the real answer.
fn strip_think_artifact(s: &str) -> String {
    let trimmed = s.trim_start();
    let rest = if let Some(after_open) = trimmed.strip_prefix("<think>") {
        match after_open.find("</think>") {
            Some(i) => &after_open[i + "</think>".len()..],
            None => trimmed, // unterminated — leave it alone
        }
    } else if let Some(after_close) = trimmed.strip_prefix("</think>") {
        after_close
    } else {
        trimmed
    };
    rest.trim_start().to_string()
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
    fn strip_think_artifact_handles_bare_close() {
        // The most common shape: opener consumed by the template, closer leaks.
        let out = strip_think_artifact("</think>  American Express stock note.");
        assert_eq!(out, "American Express stock note.");
    }

    #[test]
    fn strip_think_artifact_handles_empty_block() {
        // Less common: both tags around an empty/whitespace body.
        let out = strip_think_artifact("<think>\n\n</think>\n\nThe answer.");
        assert_eq!(out, "The answer.");
    }

    #[test]
    fn strip_think_artifact_preserves_clean_response() {
        // Steady-state with enable_thinking=false and a model that obeys.
        let out = strip_think_artifact("Just the summary.");
        assert_eq!(out, "Just the summary.");
    }

    #[test]
    fn strip_think_artifact_leaves_unterminated_block_alone() {
        // Don't pretend to understand truncated/malformed content.
        let input = "<think>still thinking when stream cut";
        let out = strip_think_artifact(input);
        assert_eq!(out, input);
    }

    #[test]
    fn flavor_capabilities() {
        // llama-server is the only one we probe or send enable_thinking to.
        assert!(LlmFlavor::LlamaServer.has_health_endpoint());
        assert!(LlmFlavor::LlamaServer.supports_enable_thinking());
        for f in [LlmFlavor::Ollama, LlmFlavor::OpenAi] {
            assert!(!f.has_health_endpoint());
            assert!(!f.supports_enable_thinking());
        }
    }

    #[test]
    fn chat_request_includes_kwargs_when_some() {
        let req = ChatRequest {
            model: "default",
            messages: vec![],
            chat_template_kwargs: Some(NO_THINK),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains(r#""chat_template_kwargs":{"enable_thinking":false}"#));
    }

    #[test]
    fn chat_request_omits_kwargs_when_none() {
        // Ollama/OpenAI path: the field must not appear at all, not as null.
        let req = ChatRequest {
            model: "default",
            messages: vec![],
            chat_template_kwargs: None,
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(!s.contains("chat_template_kwargs"));
    }

    #[test]
    fn stream_request_carries_stream_flag_and_conditional_kwargs() {
        // chat_stream builds a *separate* struct from chat(); guard it too so the
        // streaming path can't regress the flavor handling independently.
        let with = StreamChatRequest {
            model: "default",
            messages: vec![],
            stream: true,
            chat_template_kwargs: Some(NO_THINK),
        };
        let s = serde_json::to_string(&with).unwrap();
        assert!(s.contains(r#""stream":true"#));
        assert!(s.contains(r#""chat_template_kwargs":{"enable_thinking":false}"#));

        let without = StreamChatRequest {
            model: "default",
            messages: vec![],
            stream: true,
            chat_template_kwargs: None,
        };
        let s = serde_json::to_string(&without).unwrap();
        assert!(s.contains(r#""stream":true"#));
        assert!(!s.contains("chat_template_kwargs"));
    }

    #[test]
    fn flavor_default_is_llama_server() {
        // Load-bearing: the CLI default, spawned backends, and remote embed all
        // rely on LlamaServer being the default flavor.
        assert_eq!(LlmFlavor::default(), LlmFlavor::LlamaServer);
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
