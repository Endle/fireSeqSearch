use std::sync::Arc;
use log::debug;
use crate::query_engine::QueryEngine;
use serde_json;

pub fn get_server_info(engine_arc: Arc<QueryEngine>) -> String {
    serde_json::to_string( &engine_arc.server_info ).unwrap()
}
pub fn query(term: String, engine_arc: Arc<QueryEngine>)
             -> String {

    debug!("Original Search term {}", term);
    engine_arc.query_pipeline(term)
}


pub fn generate_word_cloud(engine_arc: Arc<QueryEngine>) -> String {
    let div_id = "fireSeqSearchWordcloudRawJson";
    let json = engine_arc.generate_wordcloud();

    let div = format!("<div id=\"{}\">{}</div>", div_id, json);
    div
}
