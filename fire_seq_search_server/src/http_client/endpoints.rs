use std::sync::Arc;
use log::debug;

use crate::query_engine::{QueryEngine, ServerInformation};
use axum::Json;
use axum::extract::State;
use axum::{response::Html, routing::get, Router, extract::Path};

pub async fn get_server_info(State(engine_arc): State<Arc<QueryEngine>>)
                                                -> Json<ServerInformation> {
    axum::Json( engine_arc.server_info.to_owned() )
}

pub async fn query(
    Path(term) : Path<String>,
    State(engine_arc): State<Arc<QueryEngine>>
    ) -> Html<String>{

    debug!("Original Search term {}", term);
    let r = engine_arc.query_pipeline(term);
    Html(r)
}

pub async fn summarize(
    Path(title) : Path<String>,
    State(engine_arc): State<Arc<QueryEngine>>
    ) -> Html<String>{

    let r = engine_arc.summarize(title);
    Html(r)
}

pub async fn generate_word_cloud(State(engine_arc): State<Arc<QueryEngine>>)
                                                    -> Html<String> {
    let div_id = "fireSeqSearchWordcloudRawJson";
    let json = engine_arc.generate_wordcloud();

    let div = format!("<div id=\"{}\">{}</div>", div_id, json);
    Html(div)
}

