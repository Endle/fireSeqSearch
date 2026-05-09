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
        engine_arc.summarizer.as_ref(),
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
    pub chunk_id: i64,
}

#[derive(serde::Serialize)]
pub struct HighlightResponse {
    pub highlight: String,
}

const PAGE_BUDGET_CHARS: usize = 32_000; // ~8K tokens at chars/4

pub async fn highlight(
    State(engine_arc): State<Arc<QueryEngine>>,
    Json(req): Json<HighlightRequest>,
) -> Json<HighlightResponse> {
    let store = &engine_arc.store;

    // chunk lookup
    let chunk = match store.get_chunks_by_ids(&[req.chunk_id]) {
        Ok(mut v) if !v.is_empty() => v.remove(0),
        Ok(_) => {
            error!("/highlight: chunk_id {} not found", req.chunk_id);
            return Json(HighlightResponse { highlight: String::new() });
        }
        Err(e) => {
            error!("/highlight: store error: {}", e);
            return Json(HighlightResponse { highlight: String::new() });
        }
    };

    // note + page-file lookup
    let note = match store.get_notes_by_ids(&[chunk.note_id]) {
        Ok(mut v) if !v.is_empty() => v.remove(0),
        _ => {
            error!("/highlight: note_id {} not found", chunk.note_id);
            return Json(HighlightResponse { highlight: String::new() });
        }
    };

    let notebook_path = std::path::PathBuf::from(&engine_arc.server_info.notebook_path);
    let page_path = notebook_path.join(&note.rel_path);
    let page_raw = match std::fs::read_to_string(&page_path) {
        Ok(s) => s,
        Err(e) => {
            error!("/highlight: cannot read {}: {}", page_path.display(), e);
            // fall back to the chunk text alone
            chunk.text.clone()
        }
    };
    let page_clean = crate::indexer::chunker::preprocess(&page_raw);
    let page_clipped = clip_chars(&page_clean, PAGE_BUDGET_CHARS);

    // strip the chunker's "# {title}\n\n" prefix from the anchor
    let anchor = strip_title_prefix(&chunk.text, &note.page_title);

    info!(
        "/highlight in: query={:?} chunk_id={} note={:?} page_chars={} anchor_chars={}",
        req.query,
        req.chunk_id,
        note.page_title,
        page_clipped.chars().count(),
        anchor.chars().count(),
    );

    let prompt = format!(
        "You will be given a search query, an anchor (a specific bullet the user \
retrieved), and the full Markdown page that contains the anchor. Extract 1-2 \
sentences from the PAGE that best answer the query. Prefer sentences from or \
near the anchor, but if a different sentence on the page is a clearly better \
answer, use that.\n\n\
Rules:\n\
- Return ONLY text that appears verbatim in the page.\n\
- Do NOT paraphrase, summarize, or invent content.\n\
- Strip Markdown bullet markers (`-`, `*`), indentation, and `[[wikilinks]]` \
brackets from the output (keep the link text).\n\
- If nothing on the page relates to the query, return an empty string.\n\
- Do NOT explain. Return the extracted text and nothing else.\n\n\
<query>{}</query>\n\n<anchor>\n{}\n</anchor>\n\n<page>\n{}\n</page>",
        req.query, anchor, page_clipped,
    );

    let messages = vec![Message { role: "user".to_string(), content: prompt }];
    let highlight = match engine_arc.backend.chat(messages).await {
        Ok(text) => {
            info!("/highlight out: {:?}", text);
            text
        }
        Err(e) => {
            error!("/highlight chat call failed: {}", e);
            anchor.lines().next().unwrap_or("").to_string()
        }
    };
    Json(HighlightResponse { highlight })
}

fn strip_title_prefix(chunk_text: &str, page_title: &str) -> String {
    let prefix = format!("# {}\n\n", page_title);
    chunk_text
        .strip_prefix(&prefix)
        .unwrap_or(chunk_text)
        .to_string()
}

fn clip_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "\n…[truncated]"
    }
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
