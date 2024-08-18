use std::sync::Arc;
use log::debug;
use serde_json;

use crate::query_engine::{QueryEngine, ServerInformation};
use axum::Json;

pub async fn get_server_info(
    State(engine_arc): State<Arc<QueryEngine>>
    ) -> Json<ServerInformation> {
    axum::Json( engine_arc.server_info.to_owned() )
    //serde_json::to_string( &engine_arc.server_info ).unwrap()
}


use axum::extract::State;
use axum::{response::Html, routing::get, Router, extract::Path};

//pub async fn query(term: String, engine_arc: Arc<QueryEngine>)
pub async fn query(
    Path(term) : Path<String>,
    State(engine_arc): State<Arc<QueryEngine>>
    ) -> Html<String>{

    //debug!("Original Search term {}", term);
    //let r = engine_arc.query_pipeline(term);
    let r = "abcdd".to_owned() + &term;
    Html(r)
}


pub fn generate_word_cloud(engine_arc: Arc<QueryEngine>) -> String {
    let div_id = "fireSeqSearchWordcloudRawJson";
    let json = engine_arc.generate_wordcloud();

    let div = format!("<div id=\"{}\">{}</div>", div_id, json);
    div
}
