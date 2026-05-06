# fireSeqSearch — LLM-first Rewrite Plan

This is the design doc for turning fireSeqSearch from a tantivy-keyword server
with a bolted-on summarization endpoint into an LLM-first server with semantic
retrieval and RAG-based Q&A.

For the rolling status of which phases have shipped, see `PROGRESS.md`.

---

## Why rewrite

The original architecture put tantivy at the center: BM25-style keyword search
returned hits, and the LLM ran a one-shot summarization on each hit. That makes
the LLM decoration. The goal is the inverse — the LLM is the brain, retrieval
is dense semantic search over the user's personal notes, and tantivy is either
demoted to a supplement or removed entirely.

Two query modes drive the design:

1. **Live search-result augmentation** (latency-critical). Triggered by the
   browser extension on every Google search. Embedding-only — no generation in
   the live path. Sub-50ms target.
2. **Deliberate Q&A** (latency-tolerant). Triggered by the user explicitly
   asking the notebook a question. Full RAG: retrieve → generate → cite.
   Streaming response.

---

## Locked decisions

### Retrieval

- **Embedding model:** bge-m3. 1024-dim, multilingual, 8K context. Chosen to
  take retrieval quality off the debug list. Smaller model is a future option
  if quality holds up.
- **Vector index:** flat in-memory `Vec<(ChunkId, [f32; 1024])>`, brute-force
  cosine. No ANN, no vector DB. At ~25k chunks the math is well under 50 ms.
- **Persistence:** SQLite. `notes` and `chunks` tables; embeddings stored as
  raw-f32 BLOBs. SQLite is the on-disk vector *storage*, not a search index —
  on startup we load all rows into the in-memory `Vec` and search there.
- **Change detection:** mtime as a fast-filter, `content_hash` as the truth.
  A note is re-embedded only when both indicate a real change.
- **Chunking:** markdown-section + Logseq bullet-tree aware. ~400 token target,
  ~600 token hard cap, no overlap. Bullet hierarchy is preserved in the chunk
  text so the LLM can see the indentation that gives Logseq notes their meaning.
- **Default `min_score` for bge-m3:** 0.55. Tunable via CLI flag and query param.

### LLM serving

- **Transport:** OpenAI-compatible HTTP. Embed at `/v1/embeddings`, chat at
  `/v1/chat/completions`. Streaming chat for `/ask`, non-streaming for
  background summarization.
- **Backend:** subprocess by default — llama.cpp's `llama-server`, or a
  `.llamafile` auto-detected by extension and run directly. The user can opt
  out via `--embed-endpoint URL` / `--chat-endpoint URL` to point at a shared
  server (Ollama, remote llama-server, etc).
- **Single binary preferred** but REST-to-external is a first-class option,
  not an afterthought.
- **GPU:** Vulkan. ROCm 6.4 + gfx1102 on Fedora is rough; Vulkan works on stock
  Mesa 25.3+ and is fast enough for bge-m3.

### Indexing lifecycle

- **Cold start:** non-blocking. The HTTP server comes up immediately; indexing
  runs in a background task. `/server_info` and `/query` include progress while
  the index is in-flight (so the browser extension can degrade gracefully).
- **Refresh:** periodic rescan every 10 minutes + `POST /reindex` for manual
  triggers. No filesystem-watch dependency for now.
- **Startup latency target:** "load embeddings from SQLite into the in-memory
  `Vec`," not "re-embed the world." Re-embedding only happens when a note's
  hash has changed.

### API surface

- `/query` (phase 3) — structured JSON grouped by page. Server-side highlighting
  is dropped; the extension does its own rendering.
- `/ask` (phase 4) — separate endpoint, full RAG, streamed over SSE, cites
  source chunks.
- `/summarize` + `/llm_done_list` — kept alive on a throwaway shim
  (`llm_backend/summary_shim.rs`) through phases 1–4. Removed in phase 5.
- `/server_info`, `/wordcloud` — unchanged for now.

### Hardware / corpus baseline

- Dev hardware: Ryzen 9 7950X3D, 64 GB RAM, AMD Radeon RX 7600 XT (16 GB VRAM,
  gfx1102), Fedora 43 Silverblue, Mesa 25.3.6, Vulkan 1.4.328.
- Corpus: ~2,500 pages, +2/day. Sized so the flat-vector strategy never needs
  to be revisited; total embedding footprint is ~100 MB.

---

## Phased rollout

### Phase 1 — `llm_backend/` rewrite — **shipped (`8b5eb34`)**

Replace `local_llm/` with `llm_backend/`. New module is OpenAI-compat over
`reqwest`, supports both spawn and external endpoints, and exposes `embed`,
`chat`, `child_pids`. The legacy `/summarize` + `/llm_done_list` endpoints
keep working through `summary_shim.rs`.

Twelve new CLI flags cover model paths, ports, model names, extra args, and
external endpoint URLs.

See `PROGRESS.md` for the verification checklist.

### Phase 2 — indexer + SQLite + chunker

New `src/indexer/` module. Submodules:

- `chunker.rs` — Logseq bullet-tree splitter (Option B). Each top-level bullet
  with all its descendants becomes one chunk. Chunk text is prefixed with
  `# {page_title}\n\n` so embedded text always carries page context. Indentation
  is preserved inside the chunk body so bullet hierarchy survives.
- `store.rs` — SQLite schema (`notes`, `chunks`) with upsert/read helpers.
  Embeddings are raw f32 BLOBs.
- `pipeline.rs` — scan notebook → diff `notes` table by mtime + hash → chunk
  changed notes → batch through `LlmBackend::embed` → write back.
- `walker.rs` (or inline in `pipeline.rs`) — recurse from notebook root, take
  `*.md` only, skip dotdirs (`.logseq/`, `.git/`, `.obsidian/`) and `assets/`.

#### Chunker rules

- Boundary = top-level bullet. Each chunk = one top-level bullet + descendants.
- Cap ~600 tokens, target ~400. When a top-level bullet's subtree exceeds the
  cap, split at descendant-bullet boundaries (each accumulated subtree emitted
  once it crosses target). If a single leaf is itself >600, hard-slice that
  one — it's rare and not worth special-casing further.
- Token counting: `chars / 4` heuristic. Cap is fuzzy; HF `tokenizers` not worth
  the dep.
- Strip before chunking: YAML frontmatter (`---...---`), Logseq page properties
  and block properties (`key:: value` lines), `#+BEGIN_QUERY ... #+END_QUERY`
  blocks. Page title comes from the filename, not a `title::` property.
- PDFs are out of scope for phase 2. Old `parse_pdf_links` is dropped.

#### SQLite schema (`store.rs`)

```sql
CREATE TABLE notes (
  id              INTEGER PRIMARY KEY,
  rel_path        TEXT    NOT NULL UNIQUE,
  page_title      TEXT    NOT NULL,
  mtime           INTEGER NOT NULL,   -- unix seconds
  content_hash    BLOB    NOT NULL,   -- blake3, 32 bytes
  chunker_version INTEGER NOT NULL    -- bumped to invalidate all chunks
);

CREATE TABLE chunks (
  id        INTEGER PRIMARY KEY,
  note_id   INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  ord       INTEGER NOT NULL,
  text      TEXT    NOT NULL,
  embedding BLOB    NOT NULL,         -- raw f32 LE bytes, 1024 * 4 = 4096
  UNIQUE(note_id, ord)
);
CREATE INDEX idx_chunks_note_id ON chunks(note_id);
```

DB path: `~/.cache/fire_seq_search/{notebook_name}.sqlite`. CLI override
deferred until a user actually wants it.

Hash: Blake3.

#### Pipeline

1. **Hydrate** — on startup load all `(chunk_id, embedding)` rows into the
   in-memory `Vec<(ChunkId, [f32; 1024])>`. Server is queryable immediately.
2. **Scan** — walk notebook, build `(rel_path, mtime)` map.
3. **Diff** — for each filesystem entry, compare against `notes` row:
   - Missing in DB → new note, embed all chunks.
   - mtime changed → read file, compute hash; if hash differs, re-chunk + re-embed
     (`DELETE FROM chunks WHERE note_id = ?` then insert).
   - mtime unchanged → skip.
   - DB rows for files no longer on disk → delete.
4. **Embed** — batch 32 chunks per `/v1/embeddings` request.
5. **Update in-memory vec** atomically per note (lock, swap that note's rows).

Driven from `main.rs` as a `tokio::spawn`'d task: hydrate on boot, then loop
{ scan; sleep 10 min }. `POST /reindex` triggers an out-of-cycle scan via a
`tokio::sync::Notify`.

`/server_info` extended with `{indexed_notes, total_notes, indexed_chunks,
in_flight}`.

Phase 2 explicitly does not touch `/query` or tantivy. The new in-memory vec
is built and maintained but not yet consumed — phase 3 wires it to `/query`.

### Phase 3 — semantic `/query`, drop tantivy

Rewrite `/query` to:

1. Embed the query string via `LlmBackend::embed`.
2. Cosine-rank against the in-memory vector.
3. Group hits by note, attach surrounding context, return structured JSON.

Once the new `/query` is in place:

- Remove `tantivy`, `tantivy-jieba`, related imports from `Cargo.toml`.
- Delete the BM25 paths in `query_engine/`.
- Update the browser extension to consume the new JSON contract in the same PR.

### Phase 4 — `/ask` endpoint

New endpoint `POST /ask`:

1. Embed the question.
2. Retrieve top-k chunks (default k=8).
3. Build a RAG prompt with citations.
4. Stream the response via SSE. Each token frame interleaves with optional
   citation events so the extension can render footnote markers as they arrive.

Add streaming support to `LlmBackend::chat` (the non-streaming variant stays
for background summarization-style tasks if any survive).

### Phase 5 — retire summary shim

- Delete `llm_backend/summary_shim.rs`.
- Remove `/summarize`, `/llm_done_list` routes.
- Remove the `SummaryEngine` reference from `QueryEngine`.
- Drop the polling loop in `main.rs`.

---

## Things deliberately *not* on the roadmap

- Approximate-nearest-neighbor indexing. At ~25k chunks brute-force cosine is
  faster than building and maintaining an ANN structure.
- A dedicated vector database (Qdrant, Chroma, etc). SQLite is enough storage;
  the in-memory `Vec` is enough index.
- Filesystem-watching (notify, inotify, etc). 10-minute periodic + manual
  `/reindex` covers the actual use case without a new dep.
- A second, smaller embedding model behind a feature flag. We commit to bge-m3
  and revisit only if quality is good enough that downsizing is worthwhile.
- Server-side HTML rendering of search results. The extension renders.
- Backwards compatibility for the old `/query` `Vec<String>` HTML contract once
  phase 3 ships. The extension is updated in the same PR.

---

## Earlier research (kept for reference)

Before locking the architecture above, surveyed several existing tools and
write-ups:

- **Khoj** — closest off-the-shelf fit; works against markdown folders, has an
  Obsidian plugin, no native Logseq plugin. Rejected because we want to own the
  ranking + RAG behavior.
- **Quivr** — pivoted to enterprise customer support; not Logseq-aware.
- **AnythingLLM** — desktop app; doesn't fit a server + browser-extension shape.
- **Ollama + ChromaDB/Qdrant + LangChain/LlamaIndex** — viable DIY stack;
  rejected in favor of a single Rust binary that subscribes to OpenAI-compat
  HTTP, so the user can swap in Ollama at the endpoint level if they want.
- **Calvin C. Chan's logseq-rag blog series** — concrete reference for chunking
  Logseq bullets.
- **Karpathy's "skip the vector DB" approach** — works under ~100k tokens; our
  corpus is ~10× that, so we still need a real index, but the spirit (keep it
  simple, avoid premature ANN) carried into the flat-vector decision.
- **Khoj GitHub issue #141** — confirms there's no native Logseq plugin yet.
