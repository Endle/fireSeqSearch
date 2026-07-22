pub mod app_state;
pub mod config;
pub mod http_client;
pub mod indexer;
pub mod llm_backend;
pub mod note_intake;
pub mod post_query;
pub mod semantic_query;

use log::debug;
use crate::config::ServerInformation;
use crate::note_intake::NotebookSoftware::Logseq;


#[macro_use]
extern crate lazy_static;


pub fn decode_cjk_str(original: String) -> Vec<String> {
    use urlencoding::decode;

    let mut result = Vec::new();
    for s in original.split(' ') {
        let t = decode(s).expect("UTF-8");
        debug!("Decode {}  ->   {}", s, t);
        result.push(String::from(t));
    }

    result
}


pub fn generate_server_info_for_test() -> ServerInformation {
    ServerInformation {
        notebook_path: "stub_path".to_string(),
        notebook_name: "logseq_notebook".to_string(),
        enable_journal_query: false,
        show_top_hits: 0,
        show_summary_single_line_chars_limit: 0,
        software: Logseq,
        convert_underline_hierarchy: true,
        host: "127.0.0.1:22024".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: vec!["query".to_string()],
    }
}
