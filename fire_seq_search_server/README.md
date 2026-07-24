# fire_seq_search_server

The backend for [fireSeqSearch](https://github.com/Endle/fireSeqSearch) — local
semantic search and RAG over your **Logseq** or **Obsidian** notes.

It runs a small local HTTP server that does dense semantic retrieval over your
notebook plus per-page LLM-generated summaries. Paired with the fireSeqSearch
browser extension, it appends hits from your personal notes to your search
engine results, and can answer questions grounded in those notes — all local.

## Install

```sh
cargo install fire_seq_search_server
```

## Run

```sh
# Logseq (default flavour)
fire_seq_search_server --notebook-path ~/logseq

# Obsidian
fire_seq_search_server --notebook obsidian --notebook-path ~/vault
```

On first run, with no embedding model configured, the server auto-downloads a
pinned `bge-m3` llamafile (~one-time, several hundred MB) into
`~/.cache/fire_seq_search` and spawns it for embeddings. See the flags below to
point at an existing llama-server / Ollama instead.

Once running, the server listens on `127.0.0.1:3030`. Point the fireSeqSearch
browser extension at it, or query it directly:

```sh
curl http://127.0.0.1:3030/query/rust
```

## Configuration

Run `fire_seq_search_server --help` for the full flag list, including
`--embed-endpoint` / `--chat-endpoint` (use a pre-running llama-server, Ollama,
or a remote OpenAI-compatible server) and `--chat <preset>`.

## Documentation & source

Full documentation, the browser extension, and development notes live in the
main repository: <https://github.com/Endle/fireSeqSearch>

## License

MIT © Zhenbo Li
