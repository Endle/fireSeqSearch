# LLM-first Rewrite — Progress

Tracking the rewrite of `fireSeqSearch` from a tantivy-keyword-search server with
post-hoc LLM summaries into an LLM-first server with semantic dense retrieval and
RAG-based Q&A.

## Phases

| Phase | Scope | Status |
|---|---|---|
| 1 | Replace `local_llm/` with `llm_backend/` (OpenAI-compat embed + chat) | **Done** (commit `8b5eb34`) |
| 2 | Indexer + SQLite persistence + markdown-aware chunker + embedding pipeline | **Done** |
| 3 | Semantic `/query`, drop tantivy, update browser-extension contract | Not started |
| 4 | New `/ask` endpoint with full RAG (retrieve → generate → cite), streaming | Not started |
| 5 | Delete `summary_shim.rs`, retire `/summarize` and `/llm_done_list` | Not started |

---

## Phase 1 — `llm_backend/` rewrite (Done)

**Commit:** `8b5eb34` — *Replace local_llm with llm_backend (phase 1 of LLM-first rewrite)*
**Diff:** 10 files changed, 613 insertions, 492 deletions.

### What was built

- New module `fire_seq_search_server/src/llm_backend/`:
  - `mod.rs` — `LlmBackend` with `launch`, `embed`, `chat`, `child_pids`. OpenAI-compat
    `/v1/embeddings` and `/v1/chat/completions` over `reqwest`. `LlmError` via `thiserror`.
    Three round-trip serde tests for the request/response structs.
  - `process.rs` — spawns `llama-server` (or auto-detects `.llamafile` and runs it
    directly), polls `/health` until 200 OK or timeout, redirects child stdio to
    `/tmp/fire_seq_search.{embed,chat}.{stdout,stderr}.log`.
  - `summary_shim.rs` — throwaway `SummaryEngine` adapter that preserves the
    `/summarize` + `/llm_done_list` endpoints by replicating the old `LlmEngine` API
    (`post_summarize_job`, `quick_fetch`, `get_llm_done_list`, `call_llm_engine`)
    on top of `LlmBackend::chat`. Marked for deletion in phase 4.

- `LlmBackendConfig` accepts two `EndpointSource`s independently for embed and chat:
  - `External(url)` — point at a pre-running server (Ollama, remote llama-server).
  - `Spawn { model, port, extra_args }` — fork a child llama-server.

- New CLI flags in `main.rs` (12 total):
  - `--embed-endpoint URL`, `--chat-endpoint URL`
  - `--embed-model PATH` (default `~/.cache/fire_seq_search/models/bge-m3-Q4_K_M.gguf`)
  - `--chat-model PATH` (default `~/.llamafile/mistral-7b-instruct-v0.2.Q4_0.llamafile`,
    kept for back-compat)
  - `--llama-server-bin PATH` (default `llama-server`)
  - `--embed-port` (8082), `--chat-port` (8081)
  - `--embed-model-name`, `--chat-model-name` (default `"default"`, for Ollama)
  - `--embed-extra-args`, `--chat-extra-args`

- `Ctrl-C` handler now iterates `backend.child_pids()` and `kill_tree`s every child.

### What was removed / changed

- Deleted `fire_seq_search_server/src/local_llm/` (mod.rs + example_llama_response.json).
- Removed `[features] llm = […]` from `Cargo.toml`. LLM is now mandatory; no feature gates.
- Removed `sha256` dependency (was only used to checksum the bundled llamafile).
- Added `thiserror = "1"`.
- `query_engine/mod.rs`: type rename `LlmEngine` → `SummaryEngine`; deleted three
  `cfg!(feature="llm")` guard branches.
- `obsidian.sh`: dropped `--features llm` from `cargo build`.

### Verification

- `cargo build` — clean (1 pre-existing warning in `logseq_uri.rs:128`).
- `cargo test` — 38/38 pass (35 prior + 3 new in `llm_backend::tests`).
- `cargo clippy --bin` — clean.
- No live LLM smoke test yet — gated on user environment (bge-m3 download +
  llama-server install).

---

## Locked architectural decisions

These apply to all remaining phases.

### Retrieval

- **Embedding model:** bge-m3 (1024-dim, multilingual, 8K context). Picked to
  eliminate retrieval quality as a debug variable; downgrading is a future option.
- **Vector index:** flat in-memory `Vec<(ChunkId, [f32; 1024])>`, brute-force
  cosine. No ANN, no vector DB. At ~25k chunks the math is sub-50ms.
- **Persistence:** SQLite. Tables `notes` (path, mtime, content_hash) and
  `chunks` (note_id, ord, text, embedding BLOB). Embedding is raw f32 bytes.
- **Change detection:** mtime fast-filter, content_hash as truth.
- **Chunking:** markdown-section + Logseq bullet-tree aware, ~400 tok target /
  ~600 tok cap, no overlap.
- **Default `min_score` for bge-m3:** 0.55 (tunable via CLI flag and query param).

### Serving

- **LLM transport:** OpenAI-compat HTTP. Embed at `/v1/embeddings`, chat at
  `/v1/chat/completions`.
- **Backend:** subprocess by default (llama.cpp `llama-server`, or `.llamafile`
  auto-detected by extension). External endpoint URL via `--{embed,chat}-endpoint`
  for users running Ollama or a shared server.
- **GPU:** Vulkan, not ROCm. Gfx1102 + Fedora-packaged ROCm 6.4 is rough; Vulkan
  works out of the box on Mesa 25.3+.
- **Cold start:** non-blocking. Indexing runs in a background task; `/server_info`
  and `/query` responses include progress while in-flight.
- **Refresh:** periodic rescan every 10 minutes + manual `POST /reindex`. No
  filesystem-watch dependency.

### API surface

- `/query` will return structured JSON grouped by page (not pre-rendered HTML).
  Server-side highlighting is dropped — the extension renders.
- `/ask` (phase 4) will be a separate endpoint with retrieve → generate → cite,
  streaming over SSE.
- `/summarize` and `/llm_done_list` are kept on life support via `summary_shim.rs`
  until phase 5.

### Hardware / corpus

- Dev hardware: Ryzen 9 7950X3D, 64 GB RAM, AMD Radeon RX 7600 XT (16 GB VRAM,
  RDNA 3 / gfx1102), Fedora 43 Silverblue, Mesa 25.3.6, Vulkan 1.4.328.
- Corpus: ~2,500 pages, growing ~2 pages/day. Sized so a flat index is fine
  indefinitely; SQLite footprint is well under 1 GB.

---

## Phase 2 — Indexer + SQLite + chunker (Done)

See `phase2_plan.md` for the full spec.

### Locked decisions

- **Chunk boundary:** Option B — top-level Logseq bullet, with all descendants
  preserved as one chunk. Indentation kept.
- **Chunk text format:** prefixed — `# {page_title}\n\n{bullet_subtree}`.
- **Oversized chunks:** split at descendant-bullet boundaries; hard-slice as
  last resort for single leaves >600 tokens.
- **Token counting:** `chars / 4` heuristic.
- **Hash:** Blake3.
- **DB location:** `~/.cache/fire_seq_search/{notebook_name}.sqlite`.
- **File walker:** recurse from notebook root, `*.md` only, skip dotdirs
  (`.logseq/`, `.git/`, `.obsidian/`) and `assets/`.
- **PDFs:** dropped in phase 2; old `parse_pdf_links` retired.
- **Frontmatter / properties:** strip both YAML `---...---` blocks and Logseq
  `key:: value` lines before chunking.
- **Embedding batch size:** 32 chunks per `/v1/embeddings` request.
- **`/server_info` shape:** add `{indexed_notes, total_notes, indexed_chunks, in_flight}`.

### What was built

- New module `fire_seq_search_server/src/indexer/`:
  - `chunker.rs` — Logseq bullet-tree splitter with YAML/property/query stripping.
    5 unit tests.
  - `store.rs` — SQLite schema (`notes`, `chunks`), `Mutex<Connection>` for
    `Send + Sync`. All CRUD helpers. 3 unit tests (roundtrip, cascade delete,
    list_paths).
  - `pipeline.rs` — `Indexer` with `hydrate`, `scan_once`, `run`. Handles
    fast-path (mtime), hash-only mtime update, full re-embed, stale-note deletion,
    and in-memory `Vec` splicing.
  - `mod.rs` — `IndexerHandle` (status + vec + reindex_notify, all behind Arc),
    `IndexerStatus`, `IndexerError`.
- New deps: `rusqlite` (bundled), `blake3`, `walkdir`.
- `main.rs`: `--db-path` flag, hydrate before server starts, `tokio::spawn`
  background scan loop, `POST /reindex` route.
- `query_engine/mod.rs`: `pub indexer: Option<IndexerHandle>`.
- `endpoints.rs`: `get_server_info` now returns `ServerInfoResponse` with
  flattened `ServerInformation` + optional `IndexerStatusJson`; new `reindex`
  handler.

### Verification

- `cargo build` — clean (1 pre-existing warning in `logseq_uri.rs`).
- `cargo test` — 46/46 pass (38 prior + 3 store + 5 chunker).

---

---

## Phase 3 — Semantic `/query`, drop tantivy (planned)

### What phase 3 does

1. Rewrite `/query` handler: embed the query term → cosine-rank the in-memory vec
   → group hits by page → return structured JSON.
2. Remove `tantivy`, `tantivy-jieba`, `jieba-rs` from `Cargo.toml` and delete the
   BM25 code paths in `query_engine/`.
3. Update the browser extension (`main.js`) to consume the new JSON shape.
   All three ship in one commit — no compatibility window.

### Proposed new `/query` JSON contract

Current response: `Vec<String>` where each string is itself a JSON-encoded object
(double-encoded). The extension calls `JSON.parse(rawRecord)` on each element.

Proposed response: a proper JSON array, directly parseable, grouped by page:

```json
[
  {
    "title": "My Page",
    "logseq_uri": "logseq://graph/notebook?page=My%20Page",
    "top_score": 0.73,
    "chunks": [
      { "score": 0.73, "text": "- relevant bullet\n  - child bullet" }
    ]
  }
]
```

- Records ordered by `top_score` descending.
- If multiple chunks from the same page are in the top-k, they are all listed
  under `chunks` (ordered by score desc).
- Chunk `text` is plain markdown — no server-side HTML. Extension escapes before
  inserting into the DOM.
- `logseq_uri` is generated server-side (same logic as today, moved into the new
  query path).

### Server-side plan

New file `src/query_engine/semantic_query.rs` (or inline in `mod.rs`):

```rust
pub struct ChunkHit   { pub score: f32, pub text: String }
pub struct PageHit    { pub title: String, pub logseq_uri: String,
                        pub top_score: f32, pub chunks: Vec<ChunkHit> }

pub async fn semantic_query(
    term: &str,
    backend: &LlmBackend,
    indexer: &IndexerHandle,
    store: &Store,
    top_k: usize,
    min_score: f32,
) -> Result<Vec<PageHit>, ...>
```

Steps:
1. `backend.embed(&[term.to_owned()])` → 1024-dim query vector.
2. `indexer.vec.read()` → iterate all `(chunk_id, emb)`, compute cosine similarity,
   collect top-k above `min_score`.
3. `store` lookup: for each chunk_id, fetch `(note_id, ord, text)` + join to
   `notes.page_title` and `notes.rel_path` for URI generation.
4. Group by `note_id`. Build `Vec<PageHit>` sorted by top_score.

`top_k` default 20, `min_score` default 0.55 (both CLI-configurable; `min_score`
also overridable per-request via `?min_score=` query param).

New `Store` methods needed:
- `get_chunks_by_ids(ids: &[i64]) -> Vec<ChunkDetail>` — batch fetch text + note_id.
- `get_notes_by_ids(ids: &[i64]) -> Vec<NoteDetail>` — batch fetch title + rel_path.

The existing `/query/:term` route is replaced in-place; the handler signature and
route path stay the same.

### Extension changes

`main.js` changes:
- Remove `parseRawList` (which does `JSON.parse` on each element) — response is
  already a proper JSON array after `response.json()`.
- Update `buildListItems` to render `record.chunks` instead of `record.summary`.
  Each chunk's `text` is escaped (use `textContent`, not `innerHTML`) and shown
  as a `<pre>`-style or `<p>` block.
- Keep `record.score` display (now `record.top_score`).
- Remove the "Summary" / "LLM" button logic that polls `/summarize` and
  `/llm_done_list` — those endpoints are still alive (phase 5 removes them) but
  there's no point wiring them to semantic chunks.

### Open questions — need answers before starting

1. **Top-k and grouping**: Return top-20 chunks grouped into at most N pages? Or
   just return all chunks above `min_score` up to some cap? What's the right max
   number of pages to show in the sidebar?

2. **Chunk text display in extension**: The chunk text is Logseq bullet markdown
   (e.g. `- bullet\n  - child`). Should the extension render it as raw text in a
   `<pre>` block, convert bullet indentation to an `<ul>`, or just show the first
   line (the top-level bullet) as the "summary" and hide descendants?

3. **`logseq_uri` vs Obsidian**: The current `createHrefToLogseq` uses
   `record.logseq_uri` if present, else falls back to constructing a logseq://
   URL. For Obsidian users, the server currently generates `obsidian://` URIs
   differently. The new JSON should carry a single `uri` field that the server
   populates correctly for both notebook types — or should the extension handle
   this with a `software` field from `/server_info`?

4. **`min_score` CLI flag**: Add `--min-score FLOAT` (default 0.55) to the CLI,
   and accept `?min_score=` as a query param on `/query`? Or keep it server-only
   for now?

5. **Tantivy removal scope**: `query_engine/mod.rs` currently builds a full
   tantivy in-RAM index on startup (loading all notes). Phase 3 removes this.
   The `load_notes/` module is used only by tantivy — can it be deleted entirely,
   or does anything else depend on it?

---

## Open questions deferred to later phases

- **Phase 4:** SSE vs chunked transfer for `/ask` streaming. Lean: SSE.
- **Phase 5:** is there any value in keeping a keyword fallback for very short
  queries where dense retrieval is weak (e.g. exact-match page-title lookups)?
  Defer until we see real-world `/query` quality.
