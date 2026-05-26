//! Application state container handed to every HTTP handler via axum's
//! `State<Arc<AppState>>`. Holds the long-lived dependencies (LLM backend,
//! SQLite store, indexer/summarizer handles) plus the configuration that
//! describes this server instance.

use std::sync::Arc;

use crate::config::ServerInformation;
use crate::indexer::{IndexerHandle, Store, SummarizerHandle};
use crate::llm_backend::LlmBackend;

pub struct AppState {
    pub server_info: ServerInformation,
    pub backend: Arc<LlmBackend>,
    pub store: Arc<Store>,
    pub min_score: f32,
    pub indexer: Option<IndexerHandle>,
    pub summarizer: Option<SummarizerHandle>,
}

impl AppState {
    pub fn new(
        server_info: ServerInformation,
        backend: Arc<LlmBackend>,
        store: Arc<Store>,
        min_score: f32,
    ) -> Self {
        AppState {
            server_info,
            backend,
            store,
            min_score,
            indexer: None,
            summarizer: None,
        }
    }
}
