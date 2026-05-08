# Phase 3 — Semantic `/query` + `/highlight` — Implementation Summary

## What shipped

- **Semantic `/query`** — replaced BM25/tantivy with cosine similarity over the
  in-memory bge-m3 embedding vec. Returns a proper JSON array (not double-encoded
  strings), grouped to one result per page, top-10 by score.
- **`POST /highlight`** — new endpoint: given `{query, chunk}`, extracts 1–2
  relevant sentences via the chat model. Called on-hover from the extension (not
  wired to JS yet — that is the next step).
- **Tantivy removed** — `tantivy`, `tantivy-tokenizer-api`, `jieba-rs` dropped
  from `Cargo.toml`. ~600 LOC of BM25 code deleted.
- **`--min-score` CLI flag** — default 0.55, controls the score cutoff for
  semantic results. No per-request override.

## Files changed

| File | Change |
|---|---|
| `src/query_engine/mod.rs` | Full rewrite — tantivy stripped, `QueryEngine::new(server_info, backend, store, min_score)` |
| `src/query_engine/semantic_query.rs` | **New** — `semantic_query()` + `PageHit` struct |
| `src/http_client/endpoints.rs` | `query` handler rewired; `highlight` handler added |
| `src/main.rs` | `QueryEngine::new` (sync, instant); `--min-score` flag; `/highlight` route |
| `src/indexer/store.rs` | Added `ChunkDetail`, `NoteDetail`, `get_chunks_by_ids`, `get_notes_by_ids` |
| `src/indexer/pipeline.rs` | `Indexer` now takes `Arc<Store>` (shared with `QueryEngine`) |
| `src/llm_backend/summary_shim.rs` | `DocData` moved here (was in `query_engine`) |
| `src/lib.rs` | Removed `pub mod load_notes`; removed tantivy tokenizer functions |
| `src/language_tools/tokenizer.rs` | Stripped to `filter_out_stopwords` + simple `tokenize` stub |
| `src/post_query/mod.rs` | Removed `post_query_wrapper` / `parse_and_serde` (tantivy-dependent) |
| `src/post_query/hit_parsed.rs` | **Deleted** (used `tantivy::TantivyDocument`) |
| `src/load_notes/mod.rs` | **Deleted** (tantivy-only note walker) |
| `Cargo.toml` | Removed `tantivy`, `tantivy-tokenizer-api`, `jieba-rs` |

## New `/query` JSON contract

```json
[
  {
    "title": "My Page",
    "logseq_uri": "logseq://graph/notebook?page=My%20Page",
    "score": 0.73,
    "top_chunk": "- the most relevant top-level bullet"
  }
]
```

At most 10 records, ordered by score descending, one record per page.

## New `POST /highlight` contract

Request:
```json
{ "query": "search term", "chunk": "full chunk text" }
```

Response:
```json
{ "highlight": "1–2 sentences extracted from the chunk most relevant to the query." }
```

## Query algorithm

1. `LlmBackend::embed([term])` → 1024-dim query vector
2. Dot product against entire in-memory vec (bge-m3 is L2-normalised → dot = cosine)
3. Filter by `min_score`, take top-50 candidates
4. `store.get_chunks_by_ids` → batch fetch `(chunk_id, note_id, text)`
5. Group by `note_id`, keep best chunk per page
6. Take top-10 pages by score
7. `store.get_notes_by_ids` → fetch `(page_title, rel_path)` for URI generation
8. Build `Vec<PageHit>` with first line of best chunk as `top_chunk`

## Startup improvement

`QueryEngine::construct` (async, waited for tantivy to load all notes into RAM) is
replaced by `QueryEngine::new` (sync, instant). Server startup is now limited by
LLM backend health-check time only.

## Verification

- `cargo build` — clean (1 pre-existing warning in `logseq_uri.rs:128`)
- `cargo test` — 46/46 pass (same count as phase 2; 1 pre-existing ignored)

## What's NOT done yet (next steps)

- Browser extension (`main.js`) not updated — still sends double-encoded JSON,
  still uses `record.summary`. Needs:
  - Remove `parseRawList` (JSON.parse per element)
  - Render `record.top_chunk` as one-liner preview
  - On-hover: `POST /highlight` → replace preview with extracted sentences
  - Remove "Summary" / "LLM" buttons and `processLlmSummary`
- Phase 4: `/ask` endpoint (full RAG, SSE streaming)
- Phase 5: delete `summary_shim.rs`, `/summarize`, `/llm_done_list`
