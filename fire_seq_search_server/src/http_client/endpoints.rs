use std::sync::Arc;
use log::debug;

use crate::query_engine::{QueryEngine, ServerInformation};
use axum::http::StatusCode;
use axum::Json;
use axum::extract::State;
use axum::{response::Html, extract::Path};

#[derive(serde::Serialize)]
pub struct IndexerStatusJson {
    pub total_notes: usize,
    pub indexed_notes: usize,
    pub indexed_chunks: usize,
    pub in_flight: bool,
    pub last_scan_at: Option<u64>,
}

#[derive(serde::Serialize)]
pub struct ServerInfoResponse {
    #[serde(flatten)]
    pub info: ServerInformation,
    pub indexer: Option<IndexerStatusJson>,
}

pub async fn get_server_info(
    State(engine_arc): State<Arc<QueryEngine>>,
) -> Json<ServerInfoResponse> {
    let indexer = if let Some(ref handle) = engine_arc.indexer {
        let s = handle.status.read().await;
        Some(IndexerStatusJson {
            total_notes: s.total_notes,
            indexed_notes: s.indexed_notes,
            indexed_chunks: s.indexed_chunks,
            in_flight: s.in_flight,
            last_scan_at: s.last_scan_at,
        })
    } else {
        None
    };
    Json(ServerInfoResponse { info: engine_arc.server_info.clone(), indexer })
}

pub async fn reindex(
    State(engine_arc): State<Arc<QueryEngine>>,
) -> StatusCode {
    match &engine_arc.indexer {
        Some(handle) => {
            handle.reindex_notify.notify_one();
            StatusCode::ACCEPTED
        }
        None => StatusCode::SERVICE_UNAVAILABLE,
    }
}

pub async fn query(
    Path(term): Path<String>,
    State(engine_arc): State<Arc<QueryEngine>>,
) -> Html<String> {
    debug!("Original Search term {}", term);
    Html(engine_arc.query_pipeline(term).await)
}

pub async fn summarize(
    Path(title): Path<String>,
    State(engine_arc): State<Arc<QueryEngine>>,
) -> Html<String> {
    Html(engine_arc.summarize(title).await)
}

pub async fn get_llm_done_list(
    State(engine_arc): State<Arc<QueryEngine>>,
) -> Html<String> {
    Html(engine_arc.get_llm_done_list().await)
}

pub async fn generate_word_cloud(
    State(engine_arc): State<Arc<QueryEngine>>,
) -> Html<String> {
    let div_id = "fireSeqSearchWordcloudRawJson";
    let json = engine_arc.generate_wordcloud();
    Html(format!("<div id=\"{}\">{}</div>", div_id, json))
}
