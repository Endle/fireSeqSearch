use std::collections::HashMap;

use log::info;

use crate::config::ServerInformation;
use crate::indexer::chunker::split_into_obsidian_units;
use crate::indexer::store::{summary_status_str, ChunkDetail, NoteDetail, SUMMARY_OK};
use crate::indexer::{IndexerHandle, Store, SummarizerHandle};
use crate::llm_backend::LlmBackend;
use crate::note_intake::{is_stub_unit, split_into_top_level_units, NotebookSoftware};
use crate::post_query::app_uri::generate_uri_v2;

/// Hard cap on displayed snippet length. Long enough for a parent bullet plus
/// several descendants; short enough that result cards don't explode.
const SNIPPET_MAX_CHARS: usize = 500;

/// Reciprocal Rank Fusion constant. 60 is the canonical value from the
/// original Cormack et al. paper; it just works across most retrieval-system
/// pairs without per-system calibration, which is the whole point of using
/// RRF over weighted-score fusion.
const RRF_K: f32 = 60.0;

/// Top-N kept per ranking before fusion. Wide enough that the right chunk
/// almost always lands in at least one list; narrow enough that a low-relevance
/// chunk at rank 199 contributes ≤1/(60+199) ≈ 0.004 to the fused score, which
/// is dominated by any chunk that ranks well in the other list.
const RANK_DEPTH: usize = 200;

#[derive(serde::Serialize)]
pub struct PageHit {
    pub title: String,
    pub logseq_uri: String,
    pub score: f32,
    pub top_snippet: String,
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

    // ---- Dense pass ---------------------------------------------------
    // bge-m3 returns L2-normalised vectors, so dot product == cosine similarity.
    let vec = indexer.vec.read().await;
    let mut dense_scored: Vec<(f32, i64)> = vec
        .iter()
        .map(|(id, emb)| (dot(emb, &query_emb), *id))
        .collect();
    let chunk_total = vec.len();
    drop(vec);
    dense_scored
        .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top_chunk_score = dense_scored.first().map(|(s, _)| *s).unwrap_or(0.0);
    // Gate dense's contribution by min_score: anything below threshold is
    // semantically too weak to be worth ranking even at the bottom of the
    // top-200. The lexical and summary passes still get a chance to surface
    // these chunks if they're a strong literal/page-level match.
    dense_scored.retain(|(s, _)| *s >= min_score);
    dense_scored.truncate(RANK_DEPTH);
    let dense_rank: HashMap<i64, usize> = dense_scored
        .iter()
        .enumerate()
        .map(|(i, (_, id))| (*id, i))
        .collect();

    // ---- Lexical pass -------------------------------------------------
    // Substring scan: cheap at this scale and dodges every CJK-tokenizer
    // gotcha in SQLite FTS5. Solves the bare-keyword failure mode (e.g.
    // 2-char queries like `日本`) that dense retrieval can't because a
    // bare-token query embedding doesn't sit near any specific chunk's
    // topic-mixture vector. See PROGRESS.md / hybrid-retrieval notes.
    let all_chunks: Vec<ChunkDetail> =
        if should_run_lexical(term) {
            store.get_all_chunks().map_err(|e| e.to_string())?
        } else {
            Vec::new()
        };
    let mut lex_scored: Vec<(f32, i64)> = Vec::new();
    let mut top_lex_score = 0.0f32;
    if !all_chunks.is_empty() {
        let q_lower = term.to_lowercase();
        for c in &all_chunks {
            let s = lexical_score(&q_lower, &c.text);
            if s > 0.0 {
                if s > top_lex_score {
                    top_lex_score = s;
                }
                lex_scored.push((s, c.id));
            }
        }
        lex_scored
            .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        lex_scored.truncate(RANK_DEPTH);
    }
    let lex_rank: HashMap<i64, usize> = lex_scored
        .iter()
        .enumerate()
        .map(|(i, (_, id))| (*id, i))
        .collect();

    // ---- Build chunk-detail lookup for every candidate ----------------
    // Dense candidates that aren't in `all_chunks` (because lexical was
    // skipped) need their detail fetched. When lexical ran, `all_chunks`
    // already covers everything.
    let id_to_detail: HashMap<i64, ChunkDetail> = if !all_chunks.is_empty() {
        all_chunks.into_iter().map(|c| (c.id, c)).collect()
    } else {
        let ids: Vec<i64> = dense_rank.keys().copied().collect();
        store
            .get_chunks_by_ids(&ids)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|c| (c.id, c))
            .collect()
    };

    // ---- Per-chunk RRF, projected to per-note --------------------------
    // For each candidate chunk: RRF score = 1/(k + dense_rank) + 1/(k + lex_rank),
    // with absent ranks contributing 0. Then per note, take the chunk with
    // the highest RRF as the display anchor.
    let candidate_chunks: std::collections::HashSet<i64> =
        dense_rank.keys().chain(lex_rank.keys()).copied().collect();
    let mut note_best_chunk: HashMap<i64, (f32, i64)> = HashMap::new();
    for cid in &candidate_chunks {
        let d = dense_rank
            .get(cid)
            .map(|r| 1.0 / (RRF_K + *r as f32))
            .unwrap_or(0.0);
        let l = lex_rank
            .get(cid)
            .map(|r| 1.0 / (RRF_K + *r as f32))
            .unwrap_or(0.0);
        let rrf = d + l;
        if let Some(detail) = id_to_detail.get(cid) {
            let entry = note_best_chunk
                .entry(detail.note_id)
                .or_insert((rrf, *cid));
            if rrf > entry.0 {
                *entry = (rrf, *cid);
            }
        }
    }

    // ---- Summary signal -----------------------------------------------
    // Per-note page-level dense signal, contributed via RRF rank just like
    // the chunk-level passes. Recovers notes whose gist matches the query
    // but whose individual chunks are diluted by competing topics.
    let summary_vec = indexer.summary_vec.read().await;
    let summary_total = summary_vec.len();
    let mut summary_scored: Vec<(f32, i64)> = summary_vec
        .iter()
        .map(|(nid, emb)| (dot(emb, &query_emb), *nid))
        .collect();
    drop(summary_vec);
    summary_scored
        .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top_summary_score = summary_scored.first().map(|(s, _)| *s).unwrap_or(0.0);
    summary_scored.retain(|(s, _)| *s >= min_score);
    summary_scored.truncate(RANK_DEPTH);
    let summary_rank: HashMap<i64, usize> = summary_scored
        .iter()
        .enumerate()
        .map(|(i, (_, nid))| (*nid, i))
        .collect();

    // ---- Final per-note fusion ----------------------------------------
    let mut all_note_ids: std::collections::HashSet<i64> =
        std::collections::HashSet::with_capacity(
            note_best_chunk.len() + summary_rank.len(),
        );
    all_note_ids.extend(note_best_chunk.keys());
    all_note_ids.extend(summary_rank.keys());

    let mut note_hits: Vec<(f32, i64, i64)> = Vec::new();
    for note_id in all_note_ids {
        let (chunk_rrf, chunk_id) = note_best_chunk
            .get(&note_id)
            .copied()
            .unwrap_or((0.0, -1));
        let summary_rrf = summary_rank
            .get(&note_id)
            .map(|r| 1.0 / (RRF_K + *r as f32))
            .unwrap_or(0.0);
        let combined = chunk_rrf + summary_rrf;
        if combined <= 0.0 {
            continue;
        }
        note_hits.push((combined, note_id, chunk_id));
    }

    info!(
        "hybrid retrieval: chunks={} dense_kept={} (top={:.3}, ≥{:.3}) \
         lex_kept={} (top={:.3}) summaries={} kept={} (top={:.3}) → fused notes={}",
        chunk_total,
        dense_rank.len(),
        top_chunk_score,
        min_score,
        lex_rank.len(),
        top_lex_score,
        summary_total,
        summary_rank.len(),
        top_summary_score,
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

    // Per-chunk snippet selection: split each anchor chunk back into the
    // top-level bullet units the chunker packed in, score each unit against
    // the query, pick the best. One batched embed call covers every
    // candidate across all hits, so cost scales with K (≤10) not corpus.
    let snippet_map = select_snippets(
        &note_hits,
        &id_to_detail,
        &note_map,
        backend,
        &query_emb,
        &server_info.software,
    )
        .await
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for (score, note_id, chunk_id) in &note_hits {
        let note = match note_map.get(note_id) {
            Some(n) => n,
            None => continue,
        };
        // chunk_id == -1 means this note hit via the summary signal alone;
        // there's no chunk anchor to render. Fall back to summary-derived
        // preview text.
        let (top_snippet, chunk_text_for_log) = match id_to_detail.get(chunk_id) {
            Some(c) => {
                let snippet = snippet_map
                    .get(chunk_id)
                    .cloned()
                    .unwrap_or_else(|| first_content_line(&c.text, &note.page_title));
                (truncate_chars(&snippet, SNIPPET_MAX_CHARS), preview(&c.text, 200))
            }
            None => (
                note.summary
                    .as_deref()
                    .map(|s| s.lines().next().unwrap_or("").to_string())
                    .unwrap_or_default(),
                "(summary-only hit, no chunk anchor)".to_string(),
            ),
        };
        let logseq_uri = generate_uri_v2(&note.page_title, &note.rel_path, server_info);
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
            top_snippet,
            chunk_id: *chunk_id,
            summary: note.summary.clone(),
            summary_status: status_str,
        });
    }

    Ok(result)
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Lexical match score: log-normalized term frequency, length-normalized.
/// Mimics BM25's saturation without needing IDF / inverted index. `query`
/// is already lowercased by the caller; `chunk_text` is lowercased here.
/// Returns 0.0 if the query doesn't appear in the chunk.
fn lexical_score(query_lower: &str, chunk_text: &str) -> f32 {
    if query_lower.is_empty() {
        return 0.0;
    }
    let lowered = chunk_text.to_lowercase();
    let tf = lowered.matches(query_lower).count();
    if tf == 0 {
        return 0.0;
    }
    (1.0 + tf as f32).ln() / (1.0 + chunk_text.chars().count() as f32).ln()
}

/// Decide whether to run the lexical pass. 1-char ASCII queries (`"a"`,
/// `"C"`) match thousands of chunks and contribute noise via RRF; we skip
/// them. 1-char CJK queries are kept because a single Han character is a
/// meaningful concept word.
fn should_run_lexical(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return false;
    }
    let mut chars = trimmed.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if chars.next().is_some() {
        // 2+ chars: always run
        return true;
    }
    // Exactly 1 char: only run if it's a CJK/Hangul/Kana ideograph or syllable.
    is_cjk_meaningful_char(first)
}

fn is_cjk_meaningful_char(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        0x4E00..=0x9FFF       // CJK Unified Ideographs
        | 0x3400..=0x4DBF     // CJK Extension A
        | 0x20000..=0x2A6DF   // CJK Extension B
        | 0x3040..=0x309F     // Hiragana
        | 0x30A0..=0x30FF     // Katakana
        | 0xAC00..=0xD7AF     // Hangul Syllables
    )
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

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}…", truncated)
    }
}

/// Pick the first non-empty line from `text` that isn't the `# {title}` prefix
/// the chunker prepends. Falls back to the first non-empty line, then "".
/// Used as a last-resort when snippet selection has no candidates.
/// True if `unit` is either a single ATX heading line, or a heading line
/// followed only by blank lines. Used to discard heading-only sections from
/// snippet candidates in the Obsidian path.
fn heading_only(unit: &str) -> bool {
    let mut saw_heading = false;
    for line in unit.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if !saw_heading {
            let after_hashes = t.trim_start_matches('#');
            let is_heading = after_hashes.len() < t.len()
                && matches!(after_hashes.chars().next(), Some(' ') | Some('\t'));
            if !is_heading {
                return false;
            }
            saw_heading = true;
        } else {
            // Any non-blank line after the heading means there's a body.
            return false;
        }
    }
    saw_heading
}

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

/// Strip the `# {page_title}\n\n` prefix the chunker prepends, returning the
/// raw body. Falls back to the original string if the prefix isn't present.
fn strip_title_prefix<'a>(chunk_text: &'a str, page_title: &str) -> &'a str {
    let prefix = format!("# {}\n\n", page_title);
    chunk_text.strip_prefix(&prefix).unwrap_or(chunk_text)
}

/// For each anchor chunk in `note_hits`, split its body into top-level bullet
/// units, drop stubs, batch-embed every candidate (with `# {title}\n\n` prefix
/// to mirror the stored chunk embeddings), and return the highest-scoring
/// unit per chunk_id. A page-title match thus naturally wins for short
/// bullets, which is what we want.
async fn select_snippets(
    note_hits: &[(f32, i64, i64)],
    id_to_detail: &HashMap<i64, ChunkDetail>,
    note_map: &HashMap<i64, &NoteDetail>,
    backend: &LlmBackend,
    query_emb: &[f32],
    software: &NotebookSoftware,
) -> Result<HashMap<i64, String>, String> {
    // Each candidate carries (chunk_id, embed_text, display_text). We flatten
    // across all hits so one HTTP call covers everything.
    let mut candidates: Vec<(i64, String, String)> = Vec::new();
    for (_, note_id, chunk_id) in note_hits {
        let chunk = match id_to_detail.get(chunk_id) {
            Some(c) => c,
            None => continue,
        };
        let note = match note_map.get(note_id) {
            Some(n) => n,
            None => continue,
        };
        let body = strip_title_prefix(&chunk.text, &note.page_title);
        match software {
            NotebookSoftware::Logseq => {
                for unit in split_into_top_level_units(body) {
                    if is_stub_unit(&unit) {
                        continue;
                    }
                    let display = unit.join("\n");
                    let embed_text = format!("# {}\n\n{}", note.page_title, display);
                    candidates.push((*chunk_id, embed_text, display));
                }
            }
            NotebookSoftware::Obsidian => {
                // Obsidian chunks aren't bullet trees. Split on `#` headings;
                // if the chunk has none, the whole body is the unit.
                let units = split_into_obsidian_units(body);
                let units: Vec<String> = if units.is_empty() {
                    vec![body.to_string()]
                } else {
                    units
                };
                for unit in units {
                    let display = unit.trim_end().to_string();
                    if display.trim().is_empty() {
                        continue;
                    }
                    // Skip units whose post-heading body is empty — otherwise
                    // top_snippet shows a bare `## Some Heading` line because
                    // the next paragraph was an image embed (now stripped in
                    // preprocess) or fell into the next unit. The display
                    // would carry no information beyond the heading itself,
                    // which the page title already conveys.
                    if heading_only(&display) {
                        continue;
                    }
                    let embed_text = format!("# {}\n\n{}", note.page_title, display);
                    candidates.push((*chunk_id, embed_text, display));
                }
            }
        }
    }

    if candidates.is_empty() {
        return Ok(HashMap::new());
    }

    let inputs: Vec<String> = candidates.iter().map(|(_, e, _)| e.clone()).collect();
    let embs = backend
        .embed(&inputs)
        .await
        .map_err(|e| e.to_string())?;
    if embs.len() != candidates.len() {
        return Err(format!(
            "snippet embed count mismatch: got {}, expected {}",
            embs.len(),
            candidates.len()
        ));
    }

    let mut best: HashMap<i64, (f32, String)> = HashMap::new();
    for ((chunk_id, _, display), emb) in candidates.iter().zip(embs.iter()) {
        let score = dot(emb, query_emb);
        best.entry(*chunk_id)
            .and_modify(|cur| {
                if score > cur.0 {
                    *cur = (score, display.clone());
                }
            })
            .or_insert_with(|| (score, display.clone()));
    }
    Ok(best.into_iter().map(|(k, (_, v))| (k, v)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_score_counts_occurrences_and_normalizes() {
        let s1 = lexical_score("日本", "去日本旅游，日本的拉面好吃");
        let s0 = lexical_score("日本", "completely unrelated content");
        let s_long = lexical_score(
            "日本",
            &("日本 ".to_string() + &"x".repeat(500)),
        );
        let s_short = lexical_score("日本", "去日本");
        assert!(s1 > 0.0);
        assert_eq!(s0, 0.0);
        // Same TF=1 but shorter chunk should score higher (length normalization).
        assert!(s_short > s_long);
    }

    #[test]
    fn lexical_score_is_case_insensitive_for_ascii() {
        // Caller is expected to pre-lowercase the query.
        let s = lexical_score("japan", "I went to JAPAN last spring");
        assert!(s > 0.0);
    }

    #[test]
    fn should_run_lexical_skips_one_char_ascii() {
        assert!(!should_run_lexical("a"));
        assert!(!should_run_lexical("C"));
        assert!(!should_run_lexical(" a "));
        assert!(!should_run_lexical(""));
    }

    #[test]
    fn should_run_lexical_keeps_one_char_cjk() {
        assert!(should_run_lexical("日"));
        assert!(should_run_lexical("猫"));
        assert!(should_run_lexical("あ"));
    }

    #[test]
    fn should_run_lexical_runs_for_multi_char() {
        assert!(should_run_lexical("日本"));
        assert!(should_run_lexical("japan"));
        assert!(should_run_lexical("ab"));
    }

    #[test]
    fn heading_only_detects_heading_with_no_body() {
        assert!(heading_only("## Calculations from Transit Observations"));
        assert!(heading_only("## Calculations\n\n"));
        assert!(heading_only("   ### Heading with leading ws  "));
        // Heading + body line → not heading-only.
        assert!(!heading_only("## H\nbody text"));
        assert!(!heading_only("## H\n\nbody text"));
        // Not a heading at all.
        assert!(!heading_only("body text"));
        assert!(!heading_only("#tag is not a heading"));
        // Empty.
        assert!(!heading_only(""));
        assert!(!heading_only("   \n\n"));
    }
}
