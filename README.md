fireSeqSearch
=============

Local semantic search and RAG over your **Logseq** or **Obsidian** notes,
surfaced in your search engine.

When you google, fireSeqSearch appends hits from your personal notebook to the
search results — ranked by meaning rather than keyword overlap, each with a
one-line LLM summary so you can scan them at a glance. Ask a question and
`/ask` answers it with citations back to your own notes. Everything runs on
your machine; no note ever leaves it.

> Works the same on Bing, DuckDuckGo, Searx, and Metager — "google" is just
> shorthand.

[AstroWiki-RAG-2026-05-24.webm](https://github.com/user-attachments/assets/9de7c1f1-d4d2-4ee0-af1c-3a8b81fdd956)

Notebook shown: [AYelland/AstroWiki_2.0](https://github.com/AYelland/AstroWiki_2.0).
More examples in [`docs/examples.md`](docs/examples.md).


Quick start
-----------

### 1. Chat backend — Ollama

```
brew install ollama          # or your platform's package
ollama serve &
ollama pull qwen3
```

Ollama is not the only choice — fireSeqSearch also supports llama.cpp and
external APIs; see [Chat backends](#chat-backends). The embedding model is
handled automatically either way.


### 2. Server

Install Rust: <https://doc.rust-lang.org/cargo/getting-started/installation.html>

```
cargo install fire_seq_search_server
```

Installs to `~/.cargo/bin`. The first build compiles a bundled SQLite from
source — expect a few minutes.

### 3. Launch

Logseq:

```
fire_seq_search_server --notebook-path ~/logseq_notebook --chat ollama:qwen3
```

Obsidian:

```
fire_seq_search_server --notebook-path ~/vault --notebook obsidian --chat ollama:qwen3
```

The server comes up on `http://127.0.0.1:3030` immediately and indexes in the
background. On the first launch it downloads the bge-m3 embedding model
(~723 MB, one-time) into `~/.cache/fire_seq_search`.

### 4. Browser extension

Firefox: <https://addons.mozilla.org/en-US/firefox/addon/fireseqsearch/>

Search for something you've written about. Notebook hits appear alongside the
engine's results.


Chat backends
-------------

`--chat` takes a preset: `local` (spawn llama-server), `ollama[:MODEL]`, or
`openai[:MODEL]`. One chat backend serves both the summarizer and `/ask`.

### llama.cpp — best quality

The model this project is tuned against is **`Qwen3.5-9B-UD-Q4_K_XL`**. Put
the GGUF at `~/llm/Qwen3.5-9B-UD-Q4_K_XL.gguf` — the default `--chat-model`
path — and the server spawns llama-server for it with `--jinja`, which is what
lets it suppress the reasoning trace. No workaround needed, and it's the
configuration the eval set in `tests/astro_wiki_eval.json` was graded on.

```
fire_seq_search_server --notebook-path ~/vault --notebook obsidian
```

Needs a `llama-server` binary on `PATH` (override with `--llama-server-bin`).
[`scripts/build_llama_server.sh`](scripts/build_llama_server.sh) is a worked
example of building one — Vulkan, on Fedora, via podman — not a required step.
Use `--chat-model` for a GGUF kept elsewhere.

### OpenAI or any other endpoint

```
fire_seq_search_server --notebook-path ~/vault --chat openai:gpt-4o-mini
```

Key via `--chat-api-key` or `FIRE_SEQ_CHAT_API_KEY`. For any other
OpenAI-compatible server, use the granular `--chat-endpoint` /
`--chat-flavour` flags instead of a preset. Base URLs must **not** include
`/v1` — the server appends it.

### Building from source

For development, or to run an unreleased revision:

```
git clone https://github.com/Endle/fireSeqSearch
cd fireSeqSearch/fire_seq_search_server
cargo build --release
```

The binary lands at `target/release/fire_seq_search_server`.

> Want the pre-LLM, keyword-search version? Backend release `0.9` with the
> latest addon — see [`docs/README-pre-llm.md`](docs/README-pre-llm.md).


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

**Retrieval.** Dense only — no BM25, no reranker. Embeddings are `bge-m3`
(1024-dim, multilingual, 8K context) at Q4_K_M, pinned so retrieval quality
isn't a moving variable. The index is a flat in-memory `Vec<(ChunkId,
[f32; 1024])>` scanned by brute-force cosine; at ~10K chunks that's under
50ms, which is why there's no ANN structure and no vector database. A page's
score is `max(best_chunk · query, summary · query)`, so a note whose overall
gist matches surfaces even when no single chunk does.

**Chunking** is flavour-aware, because the two apps mean different things by
"a note". In Logseq, top-level bullets are the boundary unit — adjacent ones
greedy-packed to 600 tokens, oversized ones split at descendant boundaries,
with `key:: value` lines, query blocks and SCHEDULED/DEADLINE noise stripped.
In Obsidian, a note under 600 tokens stays whole (notes there are already
concept-granular; splitting them fragments retrieval); larger ones split at
`#` headings, then at paragraphs. Both strip YAML frontmatter and image
embeds, and both prepend the page title so it participates in matching.

**Storage.** SQLite holds `notes` and `chunks` with raw f32 little-endian
BLOBs. It's storage, not a search index — every row hydrates into the
in-memory vector on startup. Changes are detected by mtime as a fast filter
and a Blake3 content hash as truth. Writes are transactional: a note's
`content_hash` is committed only after every one of its chunks embeds
successfully, so a mid-scan failure can't leave rows that the hash fast-path
skips forever.

**Summarization** runs in a background worker, one sentence per page, through
a status lifecycle (`NONE → QUEUED_LOW → QUEUED_HIGH → IN_PROGRESS → OK |
FAILED`). Two filters keep it honest: a deterministic content gate skips pages
with almost no narrative text before the LLM is called at all, and a
junk-summary check drops degenerate outputs like `"Empty."` or `"Summary:"`
after. Neither is a prompt instruction — models ignore those, and the rule
can't depend on which chat backend you swapped in. Searching also bumps
priority: `/query` promotes pending top-10 pages to the high queue, so the
pages you actually look for get summarized first.

**Serving.** The LLM transport is OpenAI-compatible HTTP throughout —
`/v1/embeddings` and `/v1/chat/completions` — which is what makes
"spawn llama-server" and "point at Ollama" interchangeable. Indexing never
blocks HTTP: the server binds first, hydrates from SQLite, then scans in a
background task. Refresh is a 10-minute periodic rescan plus `POST /reindex`;
there's no filesystem watcher.

**Endpoints.** `/query/:term` returns ranked page hits. `POST /ask` streams
SSE — sources and confidence first, then answer deltas, then a done frame;
`[N]` citation markers are validated against the retrieved set and anything
invented is reported back rather than shown. `/server_info` exposes config,
version, and capabilities, which the extension uses to hard-floor on a minimum
backend version and soft-gate its UI.

[`CLAUDE.md`](CLAUDE.md) records these as locked decisions, with the rationale
and the alternatives that were considered and rejected.


Known limitations
-----------------

**Thinking models are slow over Ollama.** Qwen3-family models emit a reasoning
trace by default, and the flag that disables it
(`chat_template_kwargs.enable_thinking=false`) is one Ollama rejects — so
summaries run at roughly one note per minute. The tested workaround is a
derived model whose chat template prefills a closed `<think></think>` block;
`tests/chat_ollama.sh` uses exactly that, under the name `qwen3-nothink`.
The llama.cpp path has no such problem: it passes `--jinja` and the flag is
honoured. Tracking a real fix in
[#181](https://github.com/Endle/fireSeqSearch/issues/181).


Similar projects
----------------

- [karlicoss/promnesia](https://github.com/karlicoss/promnesia) — broader
  scope; fireSeqSearch only appends notebook hits to search results.
- [Logseq Copilot](https://chrome.google.com/webstore/detail/logseq-copilot/hihgfcgbmnbomabfdbajlbpnacndeihl)


License
-------

MIT (both server and addon). Third-party libraries may have their own
licenses; see source.

LOGO: <https://www.flaticon.com/free-icon/web-browser_7328762> — Flaticon
license. UI icons by manshagraphics —
<a href="https://www.flaticon.com/free-icons/ui" title="ui icons">Flaticon</a>.


Star history
------------

[![Star History Chart](https://api.star-history.com/svg?repos=Endle/fireSeqSearch&type=Date)](https://star-history.com/#Endle/fireSeqSearch&Date)

Provided by <https://star-history.com>
