pub mod post_query;
pub mod markdown_parser;
pub mod language_tools;
pub mod http_client;
pub mod query_engine;
pub mod word_frequency;
pub mod llm_backend;
pub mod indexer;


use log::debug;
use crate::query_engine::ServerInformation;
use crate::query_engine::NotebookSoftware::Logseq;


#[macro_use]
extern crate lazy_static;

pub static JOURNAL_PREFIX: &str = "@journal@";


pub struct Article {
    #[allow(dead_code)]
    file_name: String,
    content: String
}

pub fn tokenize_default(sentence: &str) -> Vec<String> {
    let mut r = Vec::new();
    r.push(sentence.to_owned());
    r
}

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
        parse_pdf_links: false,
        exclude_zotero_items: false,
        software: Logseq,
        convert_underline_hierarchy: true,
        host: "127.0.0.1:22024".to_string(),
        llm_enabled: false,
        llm_max_waiting_time: 60,
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: vec!["query".to_string()],
    }
}
