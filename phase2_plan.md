# Phase 2 — Implementation Plan

Build `src/indexer/`: SQLite-backed chunk store, Logseq bullet-tree chunker,
background indexing pipeline. End state: every note in the notebook has been
chunked, embedded, and persisted; an in-memory `Vec<(ChunkId, [f32; 1024])>` is
held in the server process. `/query` is *not* rewired yet (that's phase 3).

## Module layout

```
src/indexer/
  mod.rs        -- public API: Indexer, IndexerStatus, ChunkRow, errors
  store.rs      -- SQLite open/migrate, all CRUD
  chunker.rs    -- Logseq bullet-tree → Vec<Chunk>
  pipeline.rs   -- scan → diff → embed → write loop
```

## Data types (mod.rs)

```rust
pub struct ChunkRow { pub id: i64, pub embedding: [f32; 1024] }

#[derive(Clone)]
pub struct IndexerStatus {
    pub total_notes: usize,       // current scan's filesystem count
    pub indexed_notes: usize,     // notes processed this scan
    pub indexed_chunks: usize,    // chunks present in memory vec
    pub in_flight: bool,          // a scan is currently running
    pub last_scan_at: Option<u64>,
}

pub struct IndexerHandle {
    pub status: Arc<RwLock<IndexerStatus>>,
    pub vec:    Arc<RwLock<Vec<(i64, [f32; 1024])>>>,
    pub reindex_notify: Arc<tokio::sync::Notify>,
}

#[derive(thiserror::Error, Debug)]
pub enum IndexerError { Io, Sqlite, Embed, Walk, Decode }
```

## SQLite schema (store.rs)

```sql
CREATE TABLE IF NOT EXISTS notes (
  id              INTEGER PRIMARY KEY,
  rel_path        TEXT    NOT NULL UNIQUE,
  page_title      TEXT    NOT NULL,
  mtime           INTEGER NOT NULL,
  content_hash    BLOB    NOT NULL,
  chunker_version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chunks (
  id        INTEGER PRIMARY KEY,
  note_id   INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  ord       INTEGER NOT NULL,
  text      TEXT    NOT NULL,
  embedding BLOB    NOT NULL,
  UNIQUE(note_id, ord)
);

CREATE INDEX IF NOT EXISTS idx_chunks_note_id ON chunks(note_id);
```

`PRAGMA foreign_keys = ON` on every connection. Migrations run via
`CREATE TABLE IF NOT EXISTS` on `Store::open` — no migration framework yet.

`CHUNKER_VERSION: u32 = 1`. Bumped when the chunker's output for the same input
would change. Pipeline treats a row with stale version as "needs re-chunking."

### Store API

```rust
impl Store {
    pub fn open(path: &Path) -> Result<Self, IndexerError>;

    pub fn get_note(&self, rel_path: &str) -> Result<Option<NoteRow>>;
    pub fn list_paths(&self) -> Result<HashSet<String>>;

    pub fn upsert_note(&self, rel_path, page_title, mtime, hash) -> Result<i64>;
    pub fn replace_chunks(
        &self,
        note_id: i64,
        chunks: &[(usize, &str, &[f32])],
    ) -> Result<Vec<i64>>;   // returns inserted chunk ids in ord order

    pub fn delete_note(&self, rel_path: &str) -> Result<()>;

    pub fn load_all_embeddings(&self) -> Result<Vec<(i64, [f32; 1024])>>;
}
```

`replace_chunks` runs in a single transaction: `DELETE FROM chunks WHERE
note_id = ?` then bulk insert. Embeddings encoded as raw little-endian f32
bytes (4096 bytes per row).

## Chunker (chunker.rs)

Public API:

```rust
pub struct Chunk { pub ord: usize, pub text: String }

pub fn chunk_note(page_title: &str, raw: &str) -> Vec<Chunk>;
```

Internally:

1. **Preprocess.** Strip YAML frontmatter (`---\n...\n---`), Logseq advanced
   queries (`#+BEGIN_QUERY ... #+END_QUERY`), and any line matching
   `^\s*[A-Za-z][\w-]*::\s` (Logseq page/block properties).
2. **Tokenize lines.** Walk line-by-line. Classify each line:
   - `BulletStart { depth, body }` — matches `^(\s*)[-*]\s+(.*)$`. `depth =
     leading_whitespace_chars / 2` (round down).
   - `Continuation { body }` — non-empty line indented past the current bullet.
   - `Blank`.
3. **Group into top-level units.** A new chunk starts whenever we see
   `BulletStart { depth: 0, ... }`. Accumulate lines verbatim (with original
   indentation) until the next top-level bullet or EOF.
4. **Size-cap pass.** For each top-level unit, measure with `approx_tokens =
   chars / 4`. If under cap, emit as one chunk. Otherwise:
   - Walk descendants. Accumulate child subtrees into a buffer. Emit the buffer
     as a sub-chunk once `buffer_tokens >= 400`. Reset and continue.
   - Each sub-chunk re-emits the parent bullet's text as its first line
     (so the LLM sees what the descendants were children of).
   - If a single descendant subtree alone exceeds the cap, emit it as one
     oversize chunk and let the embedder hard-truncate. Don't try to split prose.
5. **Wrap.** Each emitted chunk's `text` is `format!("# {}\n\n{}", page_title,
   body)`.

`ord` is assigned in document order, 0-indexed, dense.

### Chunker tests

Five canned markdown inputs in `chunker.rs`:

- `empty_note` — no bullets → 0 chunks.
- `simple_logseq` — 3 top-level bullets, none nested → 3 chunks, each prefixed
  with `# Title\n\n`.
- `nested_bullets` — one top-level with 4 children → 1 chunk preserving indent.
- `oversize_top_level` — top-level with 10 child paragraphs each ~300 tokens →
  multiple sub-chunks, each carrying parent bullet text.
- `properties_and_frontmatter` — YAML header + `tags::` line + advanced query
  → all stripped, only bullets remain.

## Pipeline (pipeline.rs)

```rust
pub struct Indexer {
    store: Arc<Store>,
    backend: Arc<LlmBackend>,
    notebook_path: PathBuf,
    handle: IndexerHandle,
}

impl Indexer {
    pub fn new(...) -> Self;
    pub async fn hydrate(&self) -> Result<()>;
    pub async fn scan_once(&self) -> Result<()>;
    pub async fn run(self) -> !;   // hydrate, scan_once, then loop forever
}
```

`hydrate` is called once on startup before the HTTP server begins serving:
`store.load_all_embeddings()` → write into `handle.vec`. Server is queryable
(against potentially stale data) immediately.

`run` body:
```
self.hydrate().await?;
loop {
    self.scan_once().await.log_err();
    tokio::select! {
        _ = sleep(Duration::from_secs(600)) => {},
        _ = handle.reindex_notify.notified() => {},
    }
}
```

`scan_once`:

1. Set `status.in_flight = true`. Reset `indexed_notes = 0`.
2. Walk notebook. Build `Vec<(rel_path, mtime, abs_path)>`. Filter: `*.md`,
   skip dotdirs, skip `assets/`. Set `status.total_notes`.
3. `existing = store.list_paths()`.
4. For each filesystem entry:
   - `db_row = store.get_note(rel_path)`.
   - If `db_row.is_some() && db_row.mtime == fs_mtime && db_row.chunker_version
     == CHUNKER_VERSION` → skip (mtime fast-path). Increment `indexed_notes`.
   - Else read file, hash with Blake3.
     - If `db_row.is_some() && db_row.hash == hash && db_row.chunker_version ==
       CHUNKER_VERSION` → just update mtime in DB; skip embedding. Increment.
     - Else: chunk → embed in batches of 32 → `store.upsert_note` →
       `store.replace_chunks` → splice into `handle.vec` (drop old chunks for
       that note, append new). Increment.
5. Sweep: for `path in existing - filesystem_paths`: `store.delete_note(path)`,
   drop those chunks from `handle.vec`.
6. Set `status.in_flight = false`, `last_scan_at = now()`.

Errors mid-scan are logged per-note; the scan continues. Only fatal errors
(SQLite poisoned, walker explodes) abort the scan.

### Embedding batching

```rust
async fn embed_chunks(backend, chunks: &[Chunk]) -> Result<Vec<Vec<f32>>> {
    let mut out = Vec::with_capacity(chunks.len());
    for batch in chunks.chunks(32) {
        let texts: Vec<String> = batch.iter().map(|c| c.text.clone()).collect();
        out.extend(backend.embed(&texts).await?);
    }
    Ok(out)
}
```

Validate every returned vec has length 1024 before storing; mismatch → error
out the whole note (don't partially insert).

### Vec splicing

`handle.vec` is `Arc<RwLock<Vec<(i64, [f32; 1024])>>>`. To replace a note's
chunks:

```rust
let mut v = handle.vec.write().await;
v.retain(|(id, _)| !old_ids.contains(id));
v.extend(new_id_embedding_pairs);
```

Order doesn't matter — phase 3's cosine search iterates the whole vec.

## Wiring (main.rs)

New CLI flag:
```
--db-path PATH    default: ~/.cache/fire_seq_search/{notebook_name}.sqlite
```

In `main`, after `LlmBackend::launch` succeeds, before binding the HTTP server:

```rust
let store = Arc::new(Store::open(&db_path)?);
let handle = IndexerHandle::default();
let indexer = Indexer::new(store, backend.clone(), notebook_path, handle.clone());
indexer.hydrate().await?;     // hot path: load embeddings into memory
tokio::spawn(indexer.run());  // background scans
```

`QueryEngine` gains `pub indexer: Option<IndexerHandle>` (paralleling `llm`).
Wire it before `engine_arc` is built.

`/server_info` adds `indexer` field:
```json
{ "indexer": { "total_notes": 2480, "indexed_notes": 2480,
               "indexed_chunks": 24611, "in_flight": false,
               "last_scan_at": 1799999999 } }
```

`POST /reindex`: handler calls `engine.indexer.as_ref().unwrap().reindex_notify.notify_one()`,
returns 202.

## Cargo.toml additions

```toml
rusqlite = { version = "0.31", features = ["bundled", "blob"] }
blake3   = "1"
walkdir  = "2"
```

Already have `tokio`, `serde`, `thiserror`, `reqwest`.

## Files modified

- `Cargo.toml` — add 3 deps above.
- `src/lib.rs` — `pub mod indexer;`.
- `src/main.rs` — `--db-path` flag, indexer wiring, `/reindex` route.
- `src/query_engine/mod.rs` — `pub indexer: Option<IndexerHandle>`.
- `src/http_client/endpoints.rs` — extend `get_server_info`, add `reindex`.
- New: `src/indexer/{mod,store,chunker,pipeline}.rs`.

No file deletions in phase 2. (Old `markdown_parser/` stays — still wired into
the legacy tantivy `/query` path, removed in phase 3.)

## Implementation order

1. **Cargo.toml + skeleton** — add deps, create empty `indexer/` files,
   `pub mod indexer` in lib.rs. Verify `cargo build` clean.
2. **`store.rs`** — schema, open/migrate, all CRUD. Unit tests with in-memory
   SQLite (`":memory:"`): roundtrip a note + 3 chunks, delete cascade,
   load_all_embeddings.
3. **`chunker.rs`** — preprocessor + walker. The 5 canned tests above.
4. **`pipeline.rs`** — hydrate + scan_once. No unit tests; integration too
   heavy. Manual smoke via main.
5. **`mod.rs`** — wire up public surface, `IndexerHandle::default`.
6. **`main.rs`** — `--db-path`, `Store::open`, `Indexer::new`, hydrate, spawn run.
7. **`/server_info`** + **`/reindex`** routes.
8. **Verify** — `cargo build`, `cargo test`, `cargo clippy --bin`.
9. **Smoke test** against the user's actual notebook (requires bge-m3 +
   llama-server set up). Watch `/server_info` go from `in_flight: true` to
   complete; `POST /reindex` retriggers.
10. **Commit** — one squash commit summarizing phase 2.

## Verification

- `cargo test` — store + chunker tests pass.
- `cargo clippy --bin` — clean.
- Run server. Observe:
  - First boot: `indexed_chunks: 0` → climbs as scan proceeds → terminal value.
  - Second boot: `indexed_chunks` populated immediately from hydrate.
  - Edit one note's body, wait 10 min: `indexed_chunks` updates (or hit
    `POST /reindex` to skip the wait).
  - Delete a note from disk: rows disappear after next scan.
- SQLite file size at rest: ~100 MB for the user's 2,500-page corpus
  (24k chunks × 4 KB embedding + text).

## Out of scope for phase 2

- No cosine search. Vec is built; nothing reads it.
- No `/query` rewrite.
- No tantivy removal.
- No browser-extension changes.
- No PDF handling.
- No filesystem-watch.
- No CLI override for `--db-path` other than the basic flag.
- No retry logic on embed failures beyond "log + continue with next note."

## Risks / known gotchas

- **Cold start on a 2,500-page notebook with no DB** — first scan re-embeds
  everything. ~5–15 min on the 7600 XT via Vulkan. Server stays up; `/query`
  via the legacy tantivy path keeps working throughout.
- **bge-m3 returning <1024-dim vec** — would mean wrong model loaded.
  Pipeline aborts that note with a clear error; no DB pollution.
- **mtime granularity on some filesystems is 1 sec** — fine for our purposes;
  the hash check is the safety net.
- **SQLite WAL mode** — enable on `Store::open` for concurrent reads while
  pipeline writes (`PRAGMA journal_mode = WAL`).
