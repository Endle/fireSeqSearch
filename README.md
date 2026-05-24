fireSeqSearch
=============

Local semantic search and RAG over your **Logseq** or **Obsidian** notes,
surfaced in your search engine.

When you Google something, fireSeqSearch also searches your personal notebook
and appends the hits — and, optionally, asks an LLM to answer your question
using your notes as the source. Everything runs locally; no notes leave your
machine.

More examples at <https://github.com/Endle/fireSeqSearch/blob/master/docs/examples.md>


What you get
------------

- **Semantic search appended to Google results.** Hits are ranked by dense
  embedding similarity (`bge-m3`), not keyword overlap — so `softmax` finds
  your note titled *Normalising classifier outputs*.
- **One-line LLM summary per page.** Generated in the background, shown next
  to each hit so you can tell at a glance whether a result is what you want.
- **`/ask` Q&A in the browser popup.** Type a question; the server retrieves
  relevant chunks from your notebook, streams back a cited answer, and the
  addon validates citations against the retrieved set so the model can't
  invent sources.


Installation
------------

You need **both** the local server and the browser extension.

### 1. Browser extension

Firefox only: <https://addons.mozilla.org/en-US/firefox/addon/fireseqsearch/>

### 2. Local LLM backend

The server talks to an OpenAI-compatible HTTP backend for embeddings and
chat. The embedding model is **`bge-m3`** (Q4_K_M GGUF, 1024-dim, ~700 MB) —
chosen and pinned for retrieval quality. Any reasonable instruct-tuned chat
model works.

By default the server spawns its own `llama-server`; see
[`build_llama_server.sh`](build_llama_server.sh) and
[`Containerfile`](Containerfile) for the Vulkan build. To use an existing
server (Ollama, remote llama), pass `--embed-endpoint` / `--chat-endpoint`.

### 3. Local server

Install Rust: <https://doc.rust-lang.org/cargo/getting-started/installation.html>

Min Rust version: see [`.github/workflows/rust.yml`](.github/workflows/rust.yml).

```
git clone https://github.com/Endle/fireSeqSearch
cd fireSeqSearch/fire_seq_search_server
cargo build --release
```

#### Logseq

```
./target/release/fire_seq_search_server --notebook_path /home/you/logseq_notebook
```

Or use [`debug_server.sh`](debug_server.sh) as a template.

#### Obsidian

```
./target/release/fire_seq_search_server --notebook_path /home/you/vault --obsidian_md
```

Or use [`debug_obsidian.sh`](debug_obsidian.sh) as a template.

The server hosts endpoints on `http://127.0.0.1:3030`. The extension talks to
it from your browser.


License
-------

MIT (both server and addon). Third-party libraries may have their own
licenses; see source.

LOGO: <https://www.flaticon.com/free-icon/web-browser_7328762> — Flaticon
license. UI icons by manshagraphics —
<a href="https://www.flaticon.com/free-icons/ui" title="ui icons">Flaticon</a>.


Similar projects
----------------

- [karlicoss/promnesia](https://github.com/karlicoss/promnesia) — broader
  scope; fireSeqSearch only appends notebook hits to search results.
- [Logseq Copilot](https://chrome.google.com/webstore/detail/logseq-copilot/hihgfcgbmnbomabfdbajlbpnacndeihl)


How it works
------------

```
  notes on disk                    local LLM backend
  (Logseq / Obsidian)              (llama-server / Ollama)
        │                                  │
        ▼                                  │
  chunker  ────────► embeddings ◄──────────┤
        │                                  │
        ▼                                  │
  SQLite store ──► in-memory cosine        │
        │                                  │
        ▼                                  │
   /query  /ask  ◄───────── chat ──────────┘
        │
        ▼
  browser extension appends to Google
```

- **Index:** ~10K chunks fit in a flat in-memory `Vec<[f32; 1024]>`,
  brute-force cosine. No vector DB, no ANN.
- **Storage:** SQLite holds notes and chunks; the index is rebuilt in memory
  from SQLite on startup.
- **Refresh:** mtime + Blake3 content hash detect changes. 10-minute
  background rescan; manual `POST /reindex` trigger.
- **LLM serving:** OpenAI-compatible HTTP (embed + chat). By default the
  server spawns its own `llama-server`; you can also point at a pre-running
  server (Ollama, remote llama) via `--embed-endpoint` / `--chat-endpoint`.

See [`CLAUDE.md`](CLAUDE.md) for the locked technical decisions and
rationale.


Star history
------------

[![Star History Chart](https://api.star-history.com/svg?repos=Endle/fireSeqSearch&type=Date)](https://star-history.com/#Endle/fireSeqSearch&Date)

Provided by <https://star-history.com>
