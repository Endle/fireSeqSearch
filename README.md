fireSeqSearch
=============

Local semantic search and RAG over your **Logseq** or **Obsidian** notes,
surfaced in your search engine.

When you google, fireSeqSearch appends hits from your personal notebook to
the search results, and optionally asks an LLM to answer your question
grounded in those notes. All local.

> Works the same on Bing, DuckDuckGo, Searx, and Metager — "google" is just
> shorthand.

> Want the pre-LLM version? Use backend release `0.9` with the latest addon.
> Details in [`docs/README-pre-llm.md`](docs/README-pre-llm.md).

More examples at [`docs/examples.md`](docs/examples.md).


What you get
------------

- **Semantic search** — finds your notes by meaning, not keyword overlap.
- **One-line LLM summary per page**, so you can scan results at a glance.
- **`/ask` Q&A** — ask a question, get a cited answer drawn from your notes.


Installation
------------

Install bottom-up: LLM backend → local server → browser extension. The
extension is useless until the server is running, and the server is useless
until the LLM backend answers.

### 1. Local LLM backend

The server talks to an OpenAI-compatible HTTP backend for embeddings and
chat. The embedding model is **`bge-m3`** (1024-dim, multilingual) — chosen
and pinned for retrieval quality. Any reasonable instruct-tuned chat model
works.

**Embedding is zero-config.** On first run the server auto-downloads a pinned,
self-contained `bge-m3` llamafile (~723 MB) into `~/.cache/fire_seq_search`
(verified by SHA-256) and launches it for you. There's nothing to install for
embeddings — the only model you choose is the chat model.

Drop the chat GGUF in `~/llm/` — that's where the server looks by default:

- `~/llm/Qwen3.5-9B-UD-Q4_K_XL.gguf` (chat)

Override the chat model with `--chat-model` if you keep it elsewhere. To use
your own embedding model instead of the auto-downloaded one, pass
`--embed-model /path/to/model` (GGUF or llamafile).

By default the server spawns its own `llama-server`. To use an existing server
(Ollama, remote llama) instead, pass `--embed-endpoint` / `--chat-endpoint`.
If you need to build `llama-server` yourself,
[`scripts/build_llama_server.sh`](scripts/build_llama_server.sh) is a worked
example of one way to do it (Vulkan, on Fedora, via podman) — not a required
step.

### 2. Local server

Install Rust: <https://doc.rust-lang.org/cargo/getting-started/installation.html>

Built and tested against current Rust stable; see
[`.github/workflows/ci.yml`](.github/workflows/ci.yml).

```
cargo install fire_seq_search_server
```

This puts `fire_seq_search_server` on your `PATH` (in `~/.cargo/bin`). The
first install builds a bundled SQLite from source, so expect it to take a few
minutes.

#### Logseq

```
fire_seq_search_server --notebook-path /home/you/logseq_notebook
```

#### Obsidian

```
fire_seq_search_server --notebook-path /home/you/vault --notebook obsidian
```

The server hosts endpoints on `http://127.0.0.1:3030`. The extension talks to
it from your browser.

#### Building from source instead

For development, or to run an unreleased revision:

```
git clone https://github.com/Endle/fireSeqSearch
cd fireSeqSearch/fire_seq_search_server
cargo build --release
```

The binary lands at `target/release/fire_seq_search_server` — use that path in
place of the bare command above.

### 3. Browser extension

Firefox only: <https://addons.mozilla.org/en-US/firefox/addon/fireseqsearch/>

Example
-------
[AstroWiki-RAG-2026-05-24.webm](https://github.com/user-attachments/assets/9de7c1f1-d4d2-4ee0-af1c-3a8b81fdd956)

Notebook provider: [AYelland/AstroWiki_2.0](https://github.com/AYelland/AstroWiki_2.0)

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
  browser extension appends to search results
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
