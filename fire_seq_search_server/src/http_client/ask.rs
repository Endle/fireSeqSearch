//! `POST /ask` — deliberate Q&A over the corpus. SSE-streamed RAG.
//!
//! Wire format (Server-Sent Events):
//!   event: meta   data: {"question": "...", "sources": [{"idx":1,"title":..,
//!                        "logseq_uri":..,"score":..,"summary_status":..}, ...]}
//!   event: delta  data: {"text": "..."}        (repeated, token chunks)
//!   event: done   data: {"cited":[1,3],"invalid":[],"chars":N,"answered":bool}
//!   event: error  data: {"message": "..."}     (terminal, on failure)
//!
//! Retrieval reuses `semantic_query` — which already bumps pending-summary
//! pages onto the high-priority summarizer queue — then feeds the top-K pages
//! (`summary` + best chunk) to a single streamed chat call. Cited `[N]` markers
//! in the answer are validated server-side against the retrieved set.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::{SinkExt, Stream, StreamExt};
use log::{error, info};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::llm_backend::Message;
use crate::query_engine::semantic_query::semantic_query;
use crate::query_engine::QueryEngine;

#[derive(Deserialize)]
pub struct AskRequest {
    pub question: String,
    #[serde(default)]
    pub k: Option<usize>,
}

const DEFAULT_K: usize = 4;
const MAX_K: usize = 8;
/// Per-source excerpt cap (~600 tokens at chars/4) — matches the chunker's
/// `CAP_TOKENS`, so a single packed chunk fits without truncation.
const EXCERPT_BUDGET_CHARS: usize = 2400;

const NO_NOTES_MSG: &str = "I don't have any notes covering that.";

lazy_static! {
    static ref CITATION_RE: regex::Regex = regex::Regex::new(r"\[(\d+)\]").unwrap();
}

type EventTx = futures::channel::mpsc::Sender<Result<Event, Infallible>>;

pub async fn ask(
    State(engine): State<Arc<QueryEngine>>,
    Json(req): Json<AskRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (mut tx, rx) = futures::channel::mpsc::channel::<Result<Event, Infallible>>(64);
    tokio::spawn(async move {
        if let Err(e) = run_ask(&engine, &req, &mut tx).await {
            error!("/ask: {}", e);
            let _ = send_event(&mut tx, "error", json!({ "message": e })).await;
        }
    });
    Sse::new(rx).keep_alive(KeepAlive::default())
}

async fn run_ask(engine: &QueryEngine, req: &AskRequest, tx: &mut EventTx) -> Result<(), String> {
    let question = req.question.trim().to_string();
    if question.is_empty() {
        return Err("empty question".to_string());
    }
    let k = req.k.unwrap_or(DEFAULT_K).clamp(1, MAX_K);
    info!("/ask: {:?} (k={})", question, k);

    let indexer = engine.indexer.as_ref().ok_or("indexer not ready")?;

    let hits = semantic_query(
        &question,
        &engine.backend,
        indexer,
        &engine.store,
        engine.summarizer.as_ref(),
        engine.min_score,
        &engine.server_info,
    )
    .await?;
    let hits: Vec<_> = hits.into_iter().take(k).collect();

    let sources: Vec<Value> = hits
        .iter()
        .enumerate()
        .map(|(i, h)| {
            json!({
                "idx": i + 1,
                "title": h.title,
                "logseq_uri": h.logseq_uri,
                "score": h.score,
                "summary_status": h.summary_status,
            })
        })
        .collect();
    send_event(tx, "meta", json!({ "question": question, "sources": sources })).await?;

    if hits.is_empty() {
        send_event(tx, "delta", json!({ "text": NO_NOTES_MSG })).await?;
        send_event(
            tx,
            "done",
            json!({ "cited": [], "invalid": [], "chars": NO_NOTES_MSG.chars().count(), "answered": false }),
        )
        .await?;
        return Ok(());
    }

    // Pull each anchor chunk's full text so the model gets the packed bullet
    // context, not just the single best bullet.
    let chunk_ids: Vec<i64> = hits.iter().map(|h| h.chunk_id).filter(|id| *id >= 0).collect();
    let chunks = engine
        .store
        .get_chunks_by_ids(&chunk_ids)
        .map_err(|e| e.to_string())?;
    let chunk_text: HashMap<i64, &str> = chunks.iter().map(|c| (c.id, c.text.as_str())).collect();

    let mut context = String::new();
    for (i, h) in hits.iter().enumerate() {
        let n = i + 1;
        context.push_str(&format!("## Source [{}]: {}\n", n, h.title));
        if let Some(s) = &h.summary {
            if !s.trim().is_empty() {
                context.push_str("Summary: ");
                context.push_str(s.trim());
                context.push('\n');
            }
        }
        let excerpt = chunk_text
            .get(&h.chunk_id)
            .map(|t| strip_title_prefix(t, &h.title))
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .map(|b| clip_chars(b, EXCERPT_BUDGET_CHARS))
            .unwrap_or_else(|| h.top_snippet.clone());
        if !excerpt.is_empty() {
            context.push_str("Excerpt:\n");
            context.push_str(&excerpt);
            context.push('\n');
        }
        context.push('\n');
    }

    let system = "You answer questions using ONLY the numbered sources the user provides; \
they are excerpts from the user's personal notes. Rules:\n\
- Use only information present in the sources. Never add facts from your own knowledge.\n\
- After each sentence or claim, cite the source it came from in square brackets, e.g. [1] or [2][3].\n\
- If the sources do not contain enough information to answer, say so plainly in one sentence and stop. Do not guess.\n\
- Be concise: a few sentences, not an essay. Reply in the same language as the question.";
    let user = format!("Question: {}\n\nSources:\n\n{}", question, context);
    let messages = vec![
        Message { role: "system".to_string(), content: system.to_string() },
        Message { role: "user".to_string(), content: user },
    ];

    let mut stream = engine
        .backend
        .chat_stream(messages)
        .await
        .map_err(|e| e.to_string())?;
    let mut answer = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(delta) => {
                answer.push_str(&delta);
                send_event(tx, "delta", json!({ "text": delta })).await?;
            }
            Err(e) => return Err(format!("chat stream: {}", e)),
        }
    }

    let valid: HashSet<usize> = (1..=hits.len()).collect();
    let mut cited: Vec<usize> = Vec::new();
    let mut invalid: Vec<usize> = Vec::new();
    for cap in CITATION_RE.captures_iter(&answer) {
        if let Ok(n) = cap[1].parse::<usize>() {
            if valid.contains(&n) {
                if !cited.contains(&n) {
                    cited.push(n);
                }
            } else if !invalid.contains(&n) {
                invalid.push(n);
            }
        }
    }
    cited.sort_unstable();
    invalid.sort_unstable();
    if !invalid.is_empty() {
        info!("/ask: model cited non-retrieved sources {:?} (answer kept)", invalid);
    }
    send_event(
        tx,
        "done",
        json!({
            "cited": cited,
            "invalid": invalid,
            "chars": answer.chars().count(),
            "answered": !cited.is_empty(),
        }),
    )
    .await?;
    Ok(())
}

async fn send_event(tx: &mut EventTx, name: &str, data: Value) -> Result<(), String> {
    tx.send(Ok(Event::default().event(name).data(data.to_string())))
        .await
        .map_err(|_| "client disconnected".to_string())
}

/// Strip the chunker's `# {page_title}\n\n` prefix, if present.
fn strip_title_prefix<'a>(chunk_text: &'a str, page_title: &str) -> &'a str {
    let prefix = format!("# {}\n\n", page_title);
    chunk_text.strip_prefix(&prefix).unwrap_or(chunk_text)
}

fn clip_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "\n…[truncated]"
    }
}
