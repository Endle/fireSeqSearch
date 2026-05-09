pub mod store;
pub mod chunker;
pub mod pipeline;
pub mod summarizer;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use store::Store;
pub use chunker::{chunk_note, Chunk};
pub use pipeline::Indexer;
pub use summarizer::{Summarizer, SummarizerHandle};

#[derive(Clone, Default)]
pub struct IndexerStatus {
    pub total_notes: usize,
    pub indexed_notes: usize,
    pub indexed_chunks: usize,
    pub in_flight: bool,
    pub last_scan_at: Option<u64>,
}

#[derive(Clone)]
pub struct IndexerHandle {
    pub status: Arc<RwLock<IndexerStatus>>,
    pub vec: Arc<RwLock<Vec<(i64, [f32; 1024])>>>,
    /// Per-note summary embedding, keyed by note_id. Filled by the summarizer
    /// after successful summary generation; cleared on note deletion.
    pub summary_vec: Arc<RwLock<HashMap<i64, [f32; 1024]>>>,
    pub reindex_notify: Arc<tokio::sync::Notify>,
}

impl Default for IndexerHandle {
    fn default() -> Self {
        Self {
            status: Arc::new(RwLock::new(IndexerStatus::default())),
            vec: Arc::new(RwLock::new(Vec::new())),
            summary_vec: Arc::new(RwLock::new(HashMap::new())),
            reindex_notify: Arc::new(tokio::sync::Notify::new()),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum IndexerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("embedding error: {0}")]
    Embed(String),
    #[error("walk error: {0}")]
    Walk(#[from] walkdir::Error),
    #[error("decode error: {0}")]
    Decode(String),
}
