use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use kill_tree::blocking::kill_tree;
use log::{error, info};

use fire_seq_search_server::http_client::{ask, endpoints};
use fire_seq_search_server::indexer::{Indexer, IndexerHandle, Store};
use fire_seq_search_server::llm_backend::{
    EndpointSource, LlmBackend, LlmBackendConfig, LlmError, LlmFlavour,
};
use fire_seq_search_server::app_state::AppState;
use fire_seq_search_server::config::ServerInformation;
use fire_seq_search_server::note_intake::NotebookSoftware::*;

#[derive(Parser)]
#[command(author, version)]
#[command(
    about = "Server for fireSeqSearch: hosting logseq notebooks at 127.0.0.1",
    long_about = None
)]
struct Cli {
    // ----- Notebook -----
    #[arg(long, help_heading = "Notebook")]
    notebook_path: String,

    /// Display name for the vault (defaults to the last path segment). Also names
    /// the SQLite cache file.
    #[arg(long, help_heading = "Notebook")]
    notebook_name: Option<String>,

    /// Which note app the vault belongs to; selects the dialect-aware walker,
    /// chunker, and URI generator.
    #[arg(long, value_enum, default_value_t = NotebookFlavour::Logseq, help_heading = "Notebook")]
    notebook: NotebookFlavour,

    #[arg(long, default_value_t = false, help_heading = "Notebook")]
    enable_journal_query: bool,

    // ----- Chat backend -----
    /// Chat backend preset — shorthand that expands to an endpoint + flavour
    /// (+ model name). Forms: `local` (spawn a local llama-server from
    /// `--chat-model`; the default when omitted), `ollama[:MODEL]` (local Ollama
    /// at http://localhost:11434), `openai[:MODEL]` (https://api.openai.com; key
    /// via `--chat-api-key` or `FIRE_SEQ_CHAT_API_KEY`). A trailing `:MODEL` sets
    /// `--chat-model-name`. Mutually exclusive with `--chat-endpoint` /
    /// `--chat-flavour`; use those to reach any other OpenAI-compatible server.
    #[arg(
        long,
        value_name = "PRESET",
        conflicts_with_all = ["chat_endpoint", "chat_flavour"],
        help_heading = "Chat backend"
    )]
    chat: Option<String>,

    #[arg(long, help_heading = "Chat backend")]
    chat_endpoint: Option<String>,

    /// Which OpenAI-compatible server `--chat-endpoint` points at. `llama-server`
    /// (default) gets a `/health` readiness probe and the `enable_thinking`
    /// kwarg; `ollama`/`openai` skip both (they 400 on the unknown field).
    /// Ignored when no `--chat-endpoint` is set (a spawned backend is always
    /// llama-server).
    #[arg(long, value_enum, default_value_t = LlmFlavour::LlamaServer, help_heading = "Chat backend")]
    chat_flavour: LlmFlavour,

    /// Bearer token for `--chat-endpoint` (e.g. an OpenAI API key). Falls back to
    /// the `FIRE_SEQ_CHAT_API_KEY` env var, which is preferred so the key doesn't
    /// land in your shell history or the process list.
    #[arg(long, help_heading = "Chat backend")]
    chat_api_key: Option<String>,

    /// Path to the chat model GGUF, spawned locally. Used by the `local` preset
    /// (and when no chat endpoint/preset is given); ignored for hosted presets.
    #[arg(long, default_value = "~/llm/Qwen3.5-9B-UD-Q4_K_XL.gguf", help_heading = "Chat backend")]
    chat_model: PathBuf,

    #[arg(long, default_value = "default", help_heading = "Chat backend")]
    chat_model_name: String,

    #[arg(long, default_value = "", help_heading = "Chat backend")]
    chat_extra_args: String,

    #[arg(long, default_value_t = 8081, help_heading = "Chat backend")]
    chat_port: u16,

    #[arg(long, default_value_t = 99, help_heading = "Chat backend")]
    chat_gpu_layers: u32,

    // ----- Embedding -----
    /// Path to the embedding model. Omit (the default) to auto-download the
    /// pinned bge-m3 llamafile into `~/.cache/fire_seq_search` and use it —
    /// zero-config embedding. Pass an explicit path to use your own GGUF/
    /// llamafile instead. Ignored when `--embed-endpoint` is set.
    #[arg(long, help_heading = "Embedding")]
    embed_model: Option<PathBuf>,

    #[arg(long, help_heading = "Embedding")]
    embed_endpoint: Option<String>,

    #[arg(long, default_value = "default", help_heading = "Embedding")]
    embed_model_name: String,

    #[arg(long, default_value = "", help_heading = "Embedding")]
    embed_extra_args: String,

    #[arg(long, default_value_t = 8082, help_heading = "Embedding")]
    embed_port: u16,

    /// Number of model layers to offload to GPU (passed as -ngl).
    /// Default 99 ≈ "all layers"; ignored on CPU-only llama-server builds.
    #[arg(long, default_value_t = 99, help_heading = "Embedding")]
    embed_gpu_layers: u32,

    // ----- Server & tuning -----
    #[arg(long, help_heading = "Server & tuning")]
    host: Option<String>,

    #[arg(long, default_value = "llama-server", help_heading = "Server & tuning")]
    llama_server_bin: PathBuf,

    #[arg(long, help_heading = "Server & tuning")]
    db_path: Option<String>,

    #[arg(long, default_value_t = 10, value_name = "HITS", help_heading = "Server & tuning")]
    show_top_hits: usize,

    #[arg(long, default_value_t = 120 * 2, value_name = "LEN", help_heading = "Server & tuning")]
    show_summary_single_line_chars_limit: usize,

    /// Minimum cosine similarity for the dense pass to contribute a chunk
    /// or summary to the fused ranking. Acts as a noise gate on the dense
    /// side only; the lexical (substring) pass has its own implicit floor
    /// (tf > 0) and is unaffected. Final results are top-K by RRF over
    /// the surviving dense ranks, the lexical ranks, and the summary
    /// ranks — so a chunk below this threshold can still surface if the
    /// lexical pass ranks it highly. Calibrated for bge-m3 with packed
    /// multi-bullet chunks; raise if you see dense-side noise.
    ///
    /// KNOWN: this is provisional. Obsidian smoke runs show top hits in the
    /// 0.03–0.05 band (vs ≥0.50 typical on Logseq) — the floor is too low
    /// on the Obsidian path. Right fix is probably a per-software default
    /// or a relative cutoff (must beat corpus mean by a margin). Hold until
    /// eval_retrieval.py has Obsidian queries to measure against.
    #[arg(long, default_value_t = 0.35, help_heading = "Server & tuning")]
    min_score: f32,
}

/// CLI mirror of `NotebookSoftware`, kept local to `main` so the decoupled
/// `note_intake` module doesn't take a `clap` dependency just to be a flag type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
enum NotebookFlavour {
    #[default]
    Logseq,
    Obsidian,
}

impl From<NotebookFlavour> for fire_seq_search_server::note_intake::NotebookSoftware {
    fn from(f: NotebookFlavour) -> Self {
        match f {
            NotebookFlavour::Logseq => Logseq,
            NotebookFlavour::Obsidian => Obsidian,
        }
    }
}

/// A resolved `--chat` preset. `Local` spawns a llama-server (identical to
/// omitting `--chat`); `External` names a hosted OpenAI-compatible endpoint.
#[derive(Debug)]
enum ChatPreset {
    Local,
    External {
        endpoint: String,
        flavour: LlmFlavour,
        /// `:MODEL` suffix, if the user gave one. Overrides `--chat-model-name`.
        model: Option<String>,
    },
}

/// Parse a `--chat` preset string of the form `NAME[:MODEL]`. Unknown names and
/// a `:MODEL` on `local` (which has no model name, only a path) are hard errors.
fn parse_chat_preset(spec: &str) -> Result<ChatPreset, String> {
    let (name, model) = match spec.split_once(':') {
        Some((n, m)) => (n, Some(m.to_string())),
        None => (spec, None),
    };
    match name {
        "local" => match model {
            Some(_) => Err(
                "chat preset `local` takes no `:MODEL` — set the GGUF path with --chat-model".into(),
            ),
            None => Ok(ChatPreset::Local),
        },
        "ollama" => Ok(ChatPreset::External {
            endpoint: "http://localhost:11434".to_string(),
            flavour: LlmFlavour::Ollama,
            model,
        }),
        "openai" => Ok(ChatPreset::External {
            endpoint: "https://api.openai.com".to_string(),
            flavour: LlmFlavour::OpenAi,
            model,
        }),
        other => Err(format!(
            "unknown chat preset `{other}` (expected `local`, `ollama[:MODEL]`, or `openai[:MODEL]`)"
        )),
    }
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .format_target(false)
        .init();

    info!("main thread running");
    let matches = Cli::parse();
    let llm_cfg = match build_llm_config(&matches).await {
        Ok(c) => c,
        Err(e) => {
            error!("LLM config failed: {}", e);
            std::process::exit(1);
        }
    };
    let server_info: ServerInformation = build_server_info(&matches);

    let notebook_name = server_info.notebook_name.clone();
    let notebook_path = PathBuf::from(&server_info.notebook_path);

    let backend = match LlmBackend::launch(llm_cfg).await {
        Ok(b) => Arc::new(b),
        Err(e) => {
            error!("LLM backend failed to start: {}", e);
            std::process::exit(1);
        }
    };

    // ---- Store + Indexer ----
    let db_path = resolve_db_path(&matches.db_path, &notebook_name);
    if let Some(parent) = db_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            error!("Failed to create DB directory {:?}: {}", parent, e);
            std::process::exit(1);
        }
    }
    let store = match Store::open(&db_path) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            error!("Failed to open SQLite DB {:?}: {}", db_path, e);
            std::process::exit(1);
        }
    };

    let software = server_info.software.clone();
    let mut engine = AppState::new(server_info, backend.clone(), store.clone(), matches.min_score);
    info!("App state ready");

    let indexer_handle = IndexerHandle::default();
    let indexer = Indexer::new(
        store.clone(),
        backend.clone(),
        notebook_path.clone(),
        indexer_handle.clone(),
        software.clone(),
    );
    if let Err(e) = indexer.hydrate().await {
        error!("Indexer hydrate failed: {}", e);
    }
    tokio::spawn(indexer.run());
    engine.indexer = Some(indexer_handle.clone());

    // Background summarizer: drains low-priority backlog (all rows with no
    // summary yet) and accepts high-priority promotions from /query.
    let summarizer_handle = fire_seq_search_server::indexer::Summarizer::spawn(
        store,
        backend.clone(),
        notebook_path,
        indexer_handle,
        software,
    );
    engine.summarizer = Some(summarizer_handle);

    let engine_arc = Arc::new(engine);
    let backend_for_destructor = backend.clone();
    ctrlc::set_handler(move || {
        info!("Termination signal received (SIGINT/SIGTERM/SIGHUP). Exiting...");
        for pid in backend_for_destructor.child_pids() {
            info!("Kill child pid {}", pid);
            if let Err(e) = kill_tree(pid) {
                error!("kill_tree({}) failed: {:?}", pid, e);
            }
        }
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let app = axum::Router::new()
        .route("/query/:term", axum::routing::get(endpoints::query))
        .route("/highlight", axum::routing::post(endpoints::highlight))
        .route("/server_info", axum::routing::get(endpoints::get_server_info))
        .route("/reindex", axum::routing::post(endpoints::reindex))
        .route("/ask", axum::routing::post(ask::ask))
        .with_state(engine_arc.clone());

    let listener = tokio::net::TcpListener::bind(&engine_arc.server_info.host)
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn build_llm_config(args: &Cli) -> Result<LlmBackendConfig, fire_seq_search_server::llm_backend::LlmError> {
    let embed = match &args.embed_endpoint {
        // Embeddings stay local on bge-m3 (1024-dim, locked) — a remote embed
        // endpoint is assumed to be a plain llama-server, not Ollama/OpenAI.
        Some(url) => EndpointSource::External {
            url: url.clone(),
            flavour: LlmFlavour::LlamaServer,
            api_key: None,
        },
        None => {
            // No explicit --embed-model → auto-fetch the pinned bge-m3
            // llamafile so embedding is zero-config. An explicit path is
            // used verbatim (BYO GGUF/llamafile).
            let model = match &args.embed_model {
                Some(p) => p.clone(),
                None => fire_seq_search_server::llm_backend::model_fetch::ensure_bge_m3().await?,
            };
            EndpointSource::Spawn {
                model,
                port: args.embed_port,
                gpu_layers: args.embed_gpu_layers,
                extra_args: split_extra_args(&args.embed_extra_args),
            }
        }
    };
    // A `--chat` preset takes priority; otherwise fall back to the granular
    // `--chat-endpoint`/spawn path. clap guarantees `--chat` is never combined
    // with `--chat-endpoint`/`--chat-flavour`. A preset's `:MODEL` (if any)
    // overrides `--chat-model-name`.
    let mut chat_model_name = args.chat_model_name.clone();
    let chat = match &args.chat {
        Some(spec) => match parse_chat_preset(spec).map_err(LlmError::Config)? {
            ChatPreset::Local => spawn_chat(args),
            ChatPreset::External { endpoint, flavour, model } => {
                if let Some(m) = model {
                    chat_model_name = m;
                }
                EndpointSource::External {
                    url: endpoint,
                    flavour,
                    api_key: chat_api_key(args),
                }
            }
        },
        None => match &args.chat_endpoint {
            Some(url) => EndpointSource::External {
                url: url.clone(),
                flavour: args.chat_flavour,
                api_key: chat_api_key(args),
            },
            None => spawn_chat(args),
        },
    };
    Ok(LlmBackendConfig {
        embed,
        chat,
        embed_model_name: args.embed_model_name.clone(),
        chat_model_name,
        llama_server_bin: args.llama_server_bin.clone(),
    })
}

/// Chat Bearer token: CLI flag wins, else the `FIRE_SEQ_CHAT_API_KEY` env var
/// (preferred — keeps the key out of the process list).
fn chat_api_key(args: &Cli) -> Option<String> {
    args.chat_api_key
        .clone()
        .or_else(|| std::env::var("FIRE_SEQ_CHAT_API_KEY").ok())
}

/// The locally-spawned llama-server chat endpoint (the `local` preset, and the
/// default when neither `--chat` nor `--chat-endpoint` is given).
fn spawn_chat(args: &Cli) -> EndpointSource {
    EndpointSource::Spawn {
        model: args.chat_model.clone(),
        port: args.chat_port,
        gpu_layers: args.chat_gpu_layers,
        extra_args: split_extra_args(&args.chat_extra_args),
    }
}

fn split_extra_args(s: &str) -> Vec<String> {
    // shell-words preserves quoted arguments, so users can pass things like
    // `--chat-extra-args '-c "16 384" --rope-freq-base 1000000'` without the
    // value getting split on the embedded space. Falls back to whitespace
    // splitting on parse errors (unbalanced quotes etc.) so a typo is at
    // worst no worse than the prior behaviour.
    match shell_words::split(s) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("extra-args split failed ({}); falling back to whitespace split", e);
            s.split_whitespace().map(|t| t.to_owned()).collect()
        }
    }
}

fn resolve_db_path(db_path_arg: &Option<String>, notebook_name: &str) -> PathBuf {
    match db_path_arg {
        Some(p) => PathBuf::from(shellexpand::tilde(p).as_ref()),
        None => {
            let expanded = shellexpand::tilde("~/.cache/fire_seq_search").into_owned();
            PathBuf::from(format!("{}/{}.sqlite", expanded, notebook_name))
        }
    }
}

fn build_server_info(args: &Cli) -> ServerInformation {
    let notebook_name = match &args.notebook_name {
        Some(x) => x.clone(),
        None => {
            let chunks: Vec<&str> = args.notebook_path.split('/').collect();
            let guess: &str = chunks.last().unwrap();
            info!("fire_seq_search guess the notebook name is {}", guess);
            String::from(guess)
        }
    };
    let host: String = args.host.clone().unwrap_or_else(|| "127.0.0.1:3030".to_string());
    let software = args.notebook.into();
    ServerInformation {
        notebook_path: args.notebook_path.clone(),
        notebook_name,
        enable_journal_query: args.enable_journal_query,
        show_top_hits: args.show_top_hits,
        show_summary_single_line_chars_limit: args.show_summary_single_line_chars_limit,
        software,
        convert_underline_hierarchy: true,
        host,
        // This build always launches the LLM backend (it's a hard dependency
        // at startup), so "ask" is always advertised. If a `--no-llm` mode is
        // ever added, drop "ask" from `capabilities` accordingly — the addon
        // already gates on it.
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: vec!["query".to_string(), "ask".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_extra_args_empty_is_no_args() {
        // The default for --{embed,chat}-extra-args; must not yield a stray "".
        assert!(split_extra_args("").is_empty());
        assert!(split_extra_args("   ").is_empty());
    }

    #[test]
    fn split_extra_args_plain_whitespace() {
        assert_eq!(
            split_extra_args("-ngl 0 --jinja"),
            vec!["-ngl", "0", "--jinja"]
        );
    }

    #[test]
    fn split_extra_args_preserves_quoted_value() {
        // The whole reason for shell-words: an embedded space inside quotes must
        // stay one argument, not split into two.
        assert_eq!(
            split_extra_args(r#"-c "16 384" --foo"#),
            vec!["-c", "16 384", "--foo"]
        );
    }

    #[test]
    fn split_extra_args_unbalanced_quote_falls_back() {
        // Unbalanced quotes are a parse error; we fall back to whitespace split
        // rather than dropping the args entirely.
        assert_eq!(split_extra_args(r#"-a "oops"#), vec!["-a", "\"oops"]);
    }

    #[test]
    fn resolve_db_path_explicit_wins() {
        let p = resolve_db_path(&Some("/var/data/notes.sqlite".to_string()), "ignored");
        assert_eq!(p, PathBuf::from("/var/data/notes.sqlite"));
    }

    #[test]
    fn resolve_db_path_default_uses_cache_and_name() {
        let p = resolve_db_path(&None, "myvault");
        let s = p.to_string_lossy();
        // Default lives under the shared cache dir and is keyed by notebook name.
        assert!(s.ends_with("/.cache/fire_seq_search/myvault.sqlite"), "got {}", s);
        // tilde must be expanded, never passed through literally to SQLite.
        assert!(!s.contains('~'), "tilde not expanded: {}", s);
    }

    #[test]
    fn cli_definition_is_valid() {
        // Catches clap misconfig at test time — e.g. a `conflicts_with` naming an
        // arg ID that doesn't exist, or a duplicate long. Cheap insurance now that
        // the struct carries headings and cross-arg conflicts.
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn preset_ollama_defaults_and_optional_model() {
        // Bare `ollama`: endpoint + flavour filled in, model deferred to
        // --chat-model-name (None here).
        match parse_chat_preset("ollama").unwrap() {
            ChatPreset::External { endpoint, flavour, model } => {
                assert_eq!(endpoint, "http://localhost:11434");
                assert_eq!(flavour, LlmFlavour::Ollama);
                assert_eq!(model, None);
            }
            _ => panic!("expected External"),
        }
        // `ollama:MODEL`: the suffix becomes the model name.
        match parse_chat_preset("ollama:qwen3-nothink").unwrap() {
            ChatPreset::External { flavour, model, .. } => {
                assert_eq!(flavour, LlmFlavour::Ollama);
                assert_eq!(model.as_deref(), Some("qwen3-nothink"));
            }
            _ => panic!("expected External"),
        }
    }

    #[test]
    fn preset_openai_endpoint_and_flavour() {
        match parse_chat_preset("openai:gpt-4o").unwrap() {
            ChatPreset::External { endpoint, flavour, model } => {
                assert_eq!(endpoint, "https://api.openai.com");
                assert_eq!(flavour, LlmFlavour::OpenAi);
                assert_eq!(model.as_deref(), Some("gpt-4o"));
            }
            _ => panic!("expected External"),
        }
    }

    #[test]
    fn preset_local_is_spawn() {
        assert!(matches!(parse_chat_preset("local").unwrap(), ChatPreset::Local));
    }

    #[test]
    fn preset_local_rejects_model_suffix() {
        // `local` spawns from a path (--chat-model), so `:MODEL` is meaningless.
        assert!(parse_chat_preset("local:foo").is_err());
    }

    #[test]
    fn preset_unknown_name_errors() {
        let err = parse_chat_preset("vllm:x").unwrap_err();
        assert!(err.contains("unknown chat preset"), "got {}", err);
    }

    #[test]
    fn notebook_flavour_maps_to_software() {
        use fire_seq_search_server::note_intake::NotebookSoftware;
        let logseq: NotebookSoftware = NotebookFlavour::Logseq.into();
        let obsidian: NotebookSoftware = NotebookFlavour::Obsidian.into();
        assert_eq!(logseq, NotebookSoftware::Logseq);
        assert_eq!(obsidian, NotebookSoftware::Obsidian);
    }
}
