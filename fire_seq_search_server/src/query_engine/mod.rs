pub mod semantic_query;

use std::sync::Arc;

use crate::note_intake::NotebookSoftware;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerInformation {
    pub notebook_path: String,
    pub notebook_name: String,
    pub enable_journal_query: bool,
    pub show_top_hits: usize,
    pub show_summary_single_line_chars_limit: usize,
    pub parse_pdf_links: bool,
    pub exclude_zotero_items: bool,
    pub software: NotebookSoftware,
    pub convert_underline_hierarchy: bool,
    pub host: String,
    /// Server crate version (`CARGO_PKG_VERSION`). Lets a freshly-upgraded
    /// addon notice it's talking to an older backend.
    pub version: String,
    /// Feature list the addon can gate UI on, e.g. `["query", "ask"]`. Older
    /// backends omit this field entirely — the addon must treat "absent" as
    /// "only the original `/query` path is guaranteed".
    pub capabilities: Vec<String>,
}

use crate::llm_backend::LlmBackend;
use crate::indexer::{IndexerHandle, Store, SummarizerHandle};

pub struct QueryEngine {
    pub server_info: ServerInformation,
    pub backend: Arc<LlmBackend>,
    pub store: Arc<Store>,
    pub min_score: f32,
    pub indexer: Option<IndexerHandle>,
    pub summarizer: Option<SummarizerHandle>,
}

impl QueryEngine {
    pub fn new(
        server_info: ServerInformation,
        backend: Arc<LlmBackend>,
        store: Arc<Store>,
        min_score: f32,
    ) -> Self {
        QueryEngine {
            server_info,
            backend,
            store,
            min_score,
            indexer: None,
            summarizer: None,
        }
    }

    pub fn generate_wordcloud(&self) -> String {
        String::from("TODO: wordcloud is turned off")
    }
}

pub fn term_preprocess(term: String) -> String {
    let term = term.replace("%20", " ");
    let term_vec = crate::decode_cjk_str(term);
    term_vec.join(" ")
}
