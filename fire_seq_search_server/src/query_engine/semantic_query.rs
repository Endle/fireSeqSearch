use std::collections::HashMap;

use log::info;

use crate::indexer::store::{summary_status_str, ChunkDetail, NoteDetail, SUMMARY_OK};
use crate::indexer::{IndexerHandle, Store, SummarizerHandle};
use crate::llm_backend::LlmBackend;
use crate::post_query::app_uri::generate_uri_v2;
use crate::query_engine::ServerInformation;

#[derive(serde::Serialize)]
pub struct PageHit {
    pub title: String,
    pub logseq_uri: String,
    pub score: f32,
    pub top_chunk: String,
    pub chunk_id: i64,
    pub summary: Option<String>,
    pub summary_status: &'static str,
}

pub async fn semantic_query(
    term: &str,
    backend: &LlmBackend,
    indexer: &IndexerHandle,
    store: &Store,
    summarizer: Option<&SummarizerHandle>,
    min_score: f32,
    server_info: &ServerInformation,
) -> Result<Vec<PageHit>, String> {
    let embeddings = backend
        .embed(&[term.to_string()])
        .await
        .map_err(|e| e.to_string())?;
    let query_emb = embeddings
        .into_iter()
        .next()
        .ok_or_else(|| "no embedding returned".to_string())?;

    // bge-m3 returns L2-normalised vectors, so dot product == cosine similarity
    let vec = indexer.vec.read().await;
    let mut all_scored: Vec<(f32, i64)> = vec
        .iter()
        .map(|(id, emb)| (dot(emb, &query_emb), *id))
        .collect();
    all_scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top_score = all_scored.first().map(|(s, _)| *s).unwrap_or(0.0);
    let mut scored: Vec<(f32, i64)> = all_scored
        .into_iter()
        .filter(|(s, _)| *s >= min_score)
        .collect();
    scored.truncate(50);
    drop(vec);

    info!(
        "scored {} chunks, top={:.3}, threshold={:.3}, kept={}",
        indexer.vec.read().await.len(),
        top_score,
        min_score,
        scored.len(),
    );

    if scored.is_empty() {
        return Ok(vec![]);
    }

    let top_ids: Vec<i64> = scored.iter().map(|(_, id)| *id).collect();
    let chunk_details = store.get_chunks_by_ids(&top_ids).map_err(|e| e.to_string())?;
    let id_to_detail: HashMap<i64, &ChunkDetail> =
        chunk_details.iter().map(|c| (c.id, c)).collect();

    // keep best-scoring chunk per note
    let mut note_best: HashMap<i64, (f32, i64)> = HashMap::new();
    for (score, chunk_id) in &scored {
        if let Some(detail) = id_to_detail.get(chunk_id) {
            let entry = note_best.entry(detail.note_id).or_insert((*score, *chunk_id));
            if *score > entry.0 {
                *entry = (*score, *chunk_id);
            }
        }
    }

    let mut note_hits: Vec<(f32, i64, i64)> = note_best
        .into_iter()
        .map(|(note_id, (score, chunk_id))| (score, note_id, chunk_id))
        .collect();
    note_hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    note_hits.truncate(10);

    let note_ids: Vec<i64> = note_hits.iter().map(|(_, note_id, _)| *note_id).collect();
    let note_details = store.get_notes_by_ids(&note_ids).map_err(|e| e.to_string())?;
    let note_map: HashMap<i64, &NoteDetail> =
        note_details.iter().map(|n| (n.id, n)).collect();

    let mut result = Vec::new();
    for (score, note_id, chunk_id) in &note_hits {
        let note = match note_map.get(note_id) {
            Some(n) => n,
            None => continue,
        };
        let chunk = match id_to_detail.get(chunk_id) {
            Some(c) => c,
            None => continue,
        };
        let top_chunk = first_content_line(&chunk.text, &note.page_title);
        let logseq_uri = generate_uri_v2(&note.page_title, server_info);
        let status_str = summary_status_str(note.summary_status);
        info!(
            "hit: page={:?} score={:.3} chunk_id={} summary={} chunk_text={:?}",
            note.page_title,
            score,
            chunk_id,
            status_str,
            preview(&chunk.text, 200)
        );

        // If this page doesn't have a usable summary yet, ask the summarizer
        // to bump it to the top of the queue. Best-effort: we don't block on
        // it. Also persist the QUEUED_HIGH state so it stays prioritized
        // across restarts.
        if note.summary_status != SUMMARY_OK {
            if let Some(s) = summarizer {
                s.request_high_priority(note.id);
            }
            let _ = store.promote_to_high(note.id);
        }

        result.push(PageHit {
            title: note.page_title.clone(),
            logseq_uri,
            score: *score,
            top_chunk,
            chunk_id: *chunk_id,
            summary: note.summary.clone(),
            summary_status: status_str,
        });
    }

    Ok(result)
}

fn dot(a: &[f32; 1024], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn preview(s: &str, max_chars: usize) -> String {
    let cleaned = s.replace('\n', " ⏎ ");
    if cleaned.chars().count() <= max_chars {
        cleaned
    } else {
        let truncated: String = cleaned.chars().take(max_chars).collect();
        format!("{}…", truncated)
    }
}

/// Pick the first non-empty line from `text` that isn't the `# {title}` prefix
/// the chunker prepends. Falls back to the first non-empty line, then "".
fn first_content_line(text: &str, page_title: &str) -> String {
    let title_line = format!("# {}", page_title);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == title_line {
            continue;
        }
        return line.to_string();
    }
    String::new()
}
