pub mod semantic_query;

use std::sync::Arc;
use log::info;

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub enum NotebookSoftware {
    Logseq,
    Obsidian,
}

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
    pub llm_enabled: bool,
    pub llm_max_waiting_time: u64,
}

use crate::llm_backend::{LlmBackend, SummaryEngine};
use crate::indexer::{IndexerHandle, Store, SummarizerHandle};

pub struct QueryEngine {
    pub server_info: ServerInformation,
    pub backend: Arc<LlmBackend>,
    pub store: Arc<Store>,
    pub min_score: f32,
    pub llm: Option<Arc<SummaryEngine>>,
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
            llm: None,
            indexer: None,
            summarizer: None,
        }
    }

    pub fn generate_wordcloud(&self) -> String {
        String::from("TODO: wordcloud is turned off")
    }

    pub async fn summarize(&self, title: String) -> String {
        info!("Called summarize on {}", &title);
        self.wait_for_summarize(title).await
    }

    async fn wait_for_summarize(&self, title: String) -> String {
        let llm = self.llm.as_ref().unwrap();
        let wait_llm = tokio::time::Duration::from_millis(50);
        loop {
            if let Some(s) = llm.quick_fetch(&title).await {
                return s;
            }
            tokio::time::sleep(wait_llm).await;
        }
    }

    pub async fn get_llm_done_list(&self) -> String {
        let llm = self.llm.as_ref().unwrap();
        serde_json::to_string(&llm.get_llm_done_list().await).unwrap()
    }
}

pub fn term_preprocess(term: String) -> String {
    let term = term.replace("%20", " ");
    let term_vec = crate::decode_cjk_str(term);
    term_vec.join(" ")
}
