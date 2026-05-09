use std::sync::Arc;
use log::{debug, error, info};

use crate::llm_backend::Message;
use crate::query_engine::{term_preprocess, QueryEngine, ServerInformation};
use crate::query_engine::semantic_query::{semantic_query, PageHit};
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
) -> Json<Vec<PageHit>> {
    let term = term_preprocess(term);
    info!("Semantic search: {}", &term);

    let indexer = match &engine_arc.indexer {
        Some(h) => h,
        None => {
            debug!("Indexer not ready, returning empty results");
            return Json(vec![]);
        }
    };

    match semantic_query(
        &term,
        &engine_arc.backend,
        indexer,
        &engine_arc.store,
        engine_arc.min_score,
        &engine_arc.server_info,
    )
    .await
    {
        Ok(hits) => Json(hits),
        Err(e) => {
            error!("Semantic query failed: {}", e);
            Json(vec![])
        }
    }
}

#[derive(serde::Deserialize)]
pub struct HighlightRequest {
    pub query: String,
    pub chunk: String,
}

#[derive(serde::Serialize)]
pub struct HighlightResponse {
    pub highlight: String,
}

pub async fn highlight(
    State(engine_arc): State<Arc<QueryEngine>>,
    Json(req): Json<HighlightRequest>,
) -> Json<HighlightResponse> {
    let prompt = format!(
        "You will be given a search query and a source text. Extract 1-2 sentences \
from the source text that are most relevant to the query.\n\n\
Rules:\n\
- Return ONLY text that appears verbatim in the source.\n\
- Do NOT paraphrase, summarize, or invent content.\n\
- If nothing in the source relates to the query, return an empty string.\n\
- Do NOT explain your choice. Return the extracted text and nothing else.\n\n\
Query: {}\n\nSource:\n{}",
        req.query, req.chunk
    );
    let messages = vec![Message { role: "user".to_string(), content: prompt }];
    let highlight = engine_arc
        .backend
        .chat(messages)
        .await
        .unwrap_or_else(|_| req.chunk.lines().next().unwrap_or("").to_string());
    Json(HighlightResponse { highlight })
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
