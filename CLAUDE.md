# fireSeqSearch

Local search server + browser extension that appends hits from your
Logseq/Obsidian notebook to Google search results. Architecture is **LLM-first**:
dense semantic retrieval over the user's notes plus per-page LLM-generated
summaries, with the goal of making search results explain themselves at a
glance.

## Surface area

- `/query/:term` — semantic search, sub-second. Returns ranked `PageHit`s
  (`title`, `logseq_uri`, `score`, `chunk_id`, `top_snippet`, `summary`,
  `summary_status`).
- `POST /ask` — deliberate Q&A. Body `{question, k?}`; SSE `meta` (sources +
  `confidence`) → `delta*` → `done` (`{cited, invalid, chars, answered,
  confidence}`) or `error`. Cited `[N]` markers validated against the retrieved
  set; anything invented lands in `done.invalid`.
- `POST /reindex` — manual rescan trigger.
- `/server_info` — config + indexer/summarizer counts + crate version +
  capabilities. Addon hard-floors on `MIN_BACKEND_VERSION` and soft-gates UI
  on `capabilities`.
- `POST /highlight` — dormant; kept as scaffolding for a future "explain this
  card" action.

## Locked decisions — don't relitigate without strong evidence

### Retrieval

- **Embedding:** `bge-m3` (1024-dim, multilingual, 8K context), Q4_K_M GGUF.
  Chosen to take retrieval quality off the debug list.
- **Index:** flat in-memory `Vec<(ChunkId, [f32; 1024])>`, brute-force cosine.
  No ANN, no vector DB. ~10K chunks fits under 50ms.
- **Storage:** SQLite (`notes` + `chunks`, raw f32 LE BLOBs). Storage, not a
  search index — all rows hydrate into the in-memory vec on startup.
- **Change detection:** mtime as fast filter, Blake3 `content_hash` as truth.
- **Dual signal:** `note_score = max(best_chunk · query, summary · query)`.
  Pages whose *gist* matches rise even when no individual chunk does.
- **`CHUNKER_VERSION`** in `store.rs` — bump it when chunker logic changes.
  Stale chunks otherwise hash-match and skip re-embedding forever.

### Chunking (flavour-aware via `--obsidian-md`)

- **Logseq:** top-level bullets are the boundary unit; stub-only `-`/`*`
  lines dropped; adjacent bullets greedy-packed to `CAP_TOKENS = 600`;
  oversized units split at descendant-bullet boundaries. Strips `key:: value`,
  `#+BEGIN_QUERY ... #+END_QUERY`, SCHEDULED/DEADLINE/CLOSED continuation
  lines, and "Journal Template" parent bullets.
- **Obsidian:** single chunk per note when total ≤ `CAP_TOKENS` (notes are
  concept-granular; splitting them fragments retrieval). Big notes fall back
  to splitting at `#` ATX headings, then paragraph-splitting oversized
  sections. `#tag` (no space) is not a heading.
- **Both:** YAML frontmatter stripped, image embeds stripped (`![[…]]` +
  `![…](…)`), chunk text is `# {page_title}\n\n{body}` so the title
  participates in retrieval.

### Walker (flavour-aware)

- **Logseq:** `pages/` + `journals/` only. `logseq/` and `assets/` skipped by
  construction.
- **Obsidian:** recursive walk over the vault; skips any dot-prefixed
  directory (`.obsidian/`, `.git/`, `.stversions/`, …) and `trash/`.

### URI generation

- **Logseq:** derived from page title (flat layout — basename is enough).
- **Obsidian:** derived from `rel_path` (minus `.md`). The `file=` query
  param MUST carry the directory prefix as `%2F` segments — otherwise
  `obsidian://open` fails on nested notes or opens a basename collision.

### LLM serving

- **Transport:** OpenAI-compat HTTP. Embed `/v1/embeddings`, chat
  `/v1/chat/completions`.
- **Backend:** subprocess `llama-server` by default; `--embed-endpoint` /
  `--chat-endpoint` point at a pre-running server (Ollama, remote llama) as
  a first-class alternative.
- **Embed model is zero-config:** with no `--embed-model` and no
  `--embed-endpoint`, the server auto-downloads a pinned `bge-m3` llamafile
  (URL + SHA-256 in `llm_backend/model_fetch.rs`) into
  `~/.cache/fire_seq_search` and spawns it. The `.llamafile` extension is what
  switches `process.rs` into llamafile mode (no `--model` arg). An explicit
  `--embed-model PATH` overrides; bump all three pin constants together.
- **One chat backend, shared between summarizer and `/ask`.** Deliberate —
  one process to warm, one set of args to tune. If quality demands more, the
  lever is `--chat-endpoint` pointed at a bigger server, which upgrades both
  paths at once. Don't fork the chat path.
- **Embed backend args:** `--embedding -ub 8192 -b 8192 -c 8192 -ngl 99`
  injected by default. `-ub 8192` is required — llama-server's default
  512-token ubatch rejects packed chunks with HTTP 500.
- **GPU:** Vulkan, not ROCm. gfx1102 + ROCm 6.4 on Fedora is rough; Vulkan
  works on stock Mesa 25.3+. We build llama-server in podman/Fedora 43
  (`Containerfile`, `build_llama_server.sh`).

### Summarization

- One sentence per page, background worker, status lifecycle in
  `notes.summary_status`: `NONE → QUEUED_LOW → QUEUED_HIGH → IN_PROGRESS →
  OK | FAILED`.
- **Deterministic content gate** (`is_summarizable`) — pages below
  `SUMMARIZABLE_MIN_CHARS` (16 narrative chars post-strip) skip the LLM call
  entirely. NOT a prompt instruction: models ignore those, and the rule
  must not depend on swappable chat backends.
- **Junk-summary defense** (`is_junk_summary`) — `"Empty."`, `"Summary:"`,
  `"空页面"` and friends are dropped post-LLM, not embedded.
- **Lazy-eval bump** — `/query` promotes pending top-10 pages to the
  high-priority queue, so what users actually search for gets summarized
  first.
- Summary text + embedding written atomically; queries never see a
  half-applied state.

### Indexing lifecycle

- Cold start is non-blocking — HTTP up immediately, indexer hydrates from
  SQLite, then `scan_once` in a background task.
- Refresh: 10-min periodic + `POST /reindex`. No filesystem-watch dep.
- **Transactional**: `upsert_note` only fires after embeddings succeed for
  all of a note's chunks. Prior versions committed `content_hash` before
  chunks, leaving orphan rows whose hash-match fast-path then skipped
  re-embedding forever.

## Don't propose these

- **ANN indexing** (HNSW, faiss). Brute-force cosine over ~10K vectors beats
  maintaining an ANN structure at this scale.
- **Vector database** (Qdrant, Chroma). SQLite + in-memory Vec is enough.
- **Filesystem-watching** (notify, inotify). 10-min periodic + `/reindex`
  covers the use case.
- **Server-side HTML rendering** of results. The browser extension renders.
- **BM25 hybrid / reranker.** Bringing tantivy back doubles complexity for
  unproven gain. Revisit only if `eval_retrieval.py` shows dense failing on
  a class of queries.
- **Per-chunk LLM context blurbs** (Anthropic-style contextual retrieval).
  Page summaries cover most of the same value at a fraction of indexing cost.
- **Late chunking** (Jina). Requires per-token hidden states that
  llama-server's `/v1/embeddings` doesn't expose.
- **Forking the chat path** into separate summarizer / `/ask` models.

## Where things live

- Indexer + chunker + summarizer + store: `fire_seq_search_server/src/indexer/`
- Query path: `fire_seq_search_server/src/query_engine/semantic_query.rs`
- /ask: `fire_seq_search_server/src/http_client/ask.rs`
- URI generation: `fire_seq_search_server/src/post_query/`
- Browser extension: `fireSeqSearch_addon/main.js`

## Running

- **Logseq:** `bash debug_server.sh`
- **Obsidian:** `bash debug_obsidian.sh` (points at `~/Documents/AstroWiki_2.0-main`; edit for other vaults)
- **Tests:** `cd fire_seq_search_server && cargo test --all-targets`
- **/query smoke:** `.claude/agents/fsq-smoke.md` (Logseq), `.claude/agents/obsidian-smoke.md` (Obsidian)
- **/ask smoke:** `.claude/agents/ask-smoke.md`
- **Eval regression set:** `./eval_retrieval.py`
