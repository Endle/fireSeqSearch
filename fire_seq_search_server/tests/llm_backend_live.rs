//! Live integration test for the LLM backend against a real OpenAI-compatible
//! server (Ollama in CI). It exercises the actual HTTP paths — embedding, chat,
//! and streaming chat — plus the `LlmFlavour::Ollama` shim (no `/health` probe,
//! no `enable_thinking` field). Unit tests cover the pure logic; this is the
//! only place the request/response wire format is checked against a real server.
//!
//! Gated on `FIRE_SEQ_LIVE_OLLAMA`: when that env var is unset (every normal
//! `cargo test` run, local or in the plain Rust CI), each test prints a skip
//! notice and returns, so this file is a no-op unless a server is provisioned.
//! The dedicated `llm-backend.yml` workflow sets the env and pulls tiny models.

use fire_seq_search_server::llm_backend::{
    EndpointSource, LlmBackend, LlmBackendConfig, LlmFlavour, Message,
};
use std::path::PathBuf;

/// Reads the Ollama endpoint from the env, or returns None to signal "skip".
fn endpoint() -> Option<String> {
    match std::env::var("FIRE_SEQ_LIVE_OLLAMA") {
        Ok(v) if !v.trim().is_empty() => Some(v),
        _ => None,
    }
}

fn chat_model() -> String {
    std::env::var("FIRE_SEQ_LIVE_CHAT_MODEL").unwrap_or_else(|_| "qwen2.5:0.5b".to_string())
}

fn embed_model() -> String {
    std::env::var("FIRE_SEQ_LIVE_EMBED_MODEL").unwrap_or_else(|_| "all-minilm".to_string())
}

/// Build a backend with both roles pointed at the same Ollama server, using the
/// Ollama flavour so the health probe is skipped and `enable_thinking` is omitted.
async fn launch(url: &str) -> LlmBackend {
    let cfg = LlmBackendConfig {
        embed: EndpointSource::External {
            url: url.to_string(),
            flavour: LlmFlavour::Ollama,
            api_key: None,
        },
        chat: EndpointSource::External {
            url: url.to_string(),
            flavour: LlmFlavour::Ollama,
            api_key: None,
        },
        embed_model_name: embed_model(),
        chat_model_name: chat_model(),
        // Unused: both roles are External, so nothing is spawned.
        llama_server_bin: PathBuf::from("llama-server"),
    };
    LlmBackend::launch(cfg)
        .await
        .expect("launch backend against live Ollama")
}

#[tokio::test]
async fn embed_returns_a_vector() {
    let Some(url) = endpoint() else {
        eprintln!("skipping embed_returns_a_vector: FIRE_SEQ_LIVE_OLLAMA not set");
        return;
    };
    let backend = launch(&url).await;

    let texts = vec!["hello world".to_string(), "a second chunk".to_string()];
    let out = backend.embed(&texts).await.expect("embed call");

    // One embedding per input, each a non-empty float vector.
    assert_eq!(out.len(), texts.len(), "one embedding per input");
    for (i, v) in out.iter().enumerate() {
        assert!(!v.is_empty(), "embedding {} is empty", i);
    }
    // All rows share a width — the dim contract retrieval relies on.
    assert_eq!(out[0].len(), out[1].len(), "embeddings differ in dimension");
}

#[tokio::test]
async fn chat_returns_text() {
    let Some(url) = endpoint() else {
        eprintln!("skipping chat_returns_text: FIRE_SEQ_LIVE_OLLAMA not set");
        return;
    };
    let backend = launch(&url).await;

    let answer = backend
        .chat(vec![Message {
            role: "user".to_string(),
            content: "Reply with exactly the word: pong".to_string(),
        }])
        .await
        .expect("chat call");

    assert!(!answer.trim().is_empty(), "chat returned empty text");
}

#[tokio::test]
async fn chat_stream_yields_deltas() {
    use futures::StreamExt;

    let Some(url) = endpoint() else {
        eprintln!("skipping chat_stream_yields_deltas: FIRE_SEQ_LIVE_OLLAMA not set");
        return;
    };
    let backend = launch(&url).await;

    let mut rx = backend
        .chat_stream(vec![Message {
            role: "user".to_string(),
            content: "Count to three.".to_string(),
        }])
        .await
        .expect("open chat stream");

    let mut assembled = String::new();
    while let Some(item) = rx.next().await {
        assembled.push_str(&item.expect("stream delta"));
    }
    assert!(
        !assembled.trim().is_empty(),
        "stream produced no content deltas"
    );
}
