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

    // bge-m3 returns L2-normalised vectors, so dot product == cosine similarity.
    // Score every chunk; we'll combine with the per-note summary signal below.
    let vec = indexer.vec.read().await;
    let mut all_chunk_scored: Vec<(f32, i64)> = vec
        .iter()
        .map(|(id, emb)| (dot(emb, &query_emb), *id))
        .collect();
    let chunk_total = vec.len();
    drop(vec);
    all_chunk_scored
        .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top_chunk_score = all_chunk_scored.first().map(|(s, _)| *s).unwrap_or(0.0);
    // Look up the top 200 chunks' note_ids; a chunk that ultimately wins on
    // summary score still wants its best chunk as the display anchor.
    all_chunk_scored.truncate(200);

    let chunk_ids: Vec<i64> = all_chunk_scored.iter().map(|(_, id)| *id).collect();
    let chunk_details = store.get_chunks_by_ids(&chunk_ids).map_err(|e| e.to_string())?;
    let id_to_detail: HashMap<i64, &ChunkDetail> =
        chunk_details.iter().map(|c| (c.id, c)).collect();

    // Per-note: best chunk score + chunk_id (for display anchor).
    let mut note_best_chunk: HashMap<i64, (f32, i64)> = HashMap::new();
    for (score, chunk_id) in &all_chunk_scored {
        if let Some(detail) = id_to_detail.get(chunk_id) {
            let entry = note_best_chunk
                .entry(detail.note_id)
                .or_insert((*score, *chunk_id));
            if *score > entry.0 {
                *entry = (*score, *chunk_id);
            }
        }
    }

    // Score each note's summary (page-level signal). Provides recall on
    // queries where the page's gist matches but no individual bullet stands
    // out — solves the "chunk dilution" failure mode where short stub-like
    // chunks outrank content-rich chunks.
    let summary_vec = indexer.summary_vec.read().await;
    let summary_total = summary_vec.len();
    let summary_scores: HashMap<i64, f32> = summary_vec
        .iter()
        .map(|(nid, emb)| (*nid, dot(emb, &query_emb)))
        .collect();
    drop(summary_vec);
    let top_summary_score = summary_scores.values().cloned().fold(0.0f32, f32::max);

    // Combined per-note score: max(best chunk, summary). Anchor stays the
    // best chunk so the user always has a precise bullet to drill into.
    let mut all_note_ids: std::collections::HashSet<i64> =
        std::collections::HashSet::with_capacity(
            note_best_chunk.len() + summary_scores.len(),
        );
    all_note_ids.extend(note_best_chunk.keys());
    all_note_ids.extend(summary_scores.keys());

    let mut note_hits: Vec<(f32, i64, i64)> = Vec::new();
    for note_id in all_note_ids {
        let (chunk_score, chunk_id) = note_best_chunk
            .get(&note_id)
            .copied()
            .unwrap_or((0.0, -1));
        let summary_score = summary_scores.get(&note_id).copied().unwrap_or(0.0);
        let combined = chunk_score.max(summary_score);
        if combined < min_score {
            continue;
        }
        note_hits.push((combined, note_id, chunk_id));
    }

    info!(
        "scored {} chunks (top={:.3}) + {} summaries (top={:.3}), threshold={:.3}, kept={}",
        chunk_total,
        top_chunk_score,
        summary_total,
        top_summary_score,
        min_score,
        note_hits.len(),
    );

    if note_hits.is_empty() {
        return Ok(vec![]);
    }
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
        // chunk_id == -1 means this note hit via the summary signal alone;
        // there's no chunk anchor to render. Fall back to summary-derived
        // preview text.
        let (top_chunk, chunk_text_for_log) = match id_to_detail.get(chunk_id) {
            Some(c) => (
                first_content_line(&c.text, &note.page_title),
                preview(&c.text, 200),
            ),
            None => (
                note.summary
                    .as_deref()
                    .map(|s| s.lines().next().unwrap_or("").to_string())
                    .unwrap_or_default(),
                "(summary-only hit, no chunk anchor)".to_string(),
            ),
        };
        let logseq_uri = generate_uri_v2(&note.page_title, server_info);
        let status_str = summary_status_str(note.summary_status);
        info!(
            "hit: page={:?} score={:.3} chunk_id={} summary={} chunk_text={:?}",
            note.page_title,
            score,
            chunk_id,
            status_str,
            chunk_text_for_log,
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
