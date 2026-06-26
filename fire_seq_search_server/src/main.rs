use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use kill_tree::blocking::kill_tree;
use log::{error, info};

use fire_seq_search_server::http_client::{ask, endpoints};
use fire_seq_search_server::indexer::{Indexer, IndexerHandle, Store};
use fire_seq_search_server::llm_backend::{
    EndpointSource, LlmBackend, LlmBackendConfig,
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
    #[arg(long = "notebook_path")]
    notebook_path: String,
    #[arg(long = "notebook_name")]
    notebook_name: Option<String>,

    #[arg(long, default_value_t = false)]
    obsidian_md: bool,

    #[arg(long, default_value_t = false)]
    enable_journal_query: bool,

    #[arg(long, default_value_t = 10, value_name = "HITS")]
    show_top_hits: usize,

    #[arg(long, default_value_t = 120 * 2, value_name = "LEN")]
    show_summary_single_line_chars_limit: usize,

    #[arg(long = "host")]
    host: Option<String>,

    // ----- LLM backend -----
    #[arg(long)]
    embed_endpoint: Option<String>,

    #[arg(long)]
    chat_endpoint: Option<String>,

    /// Path to the embedding model. Omit (the default) to auto-download the
    /// pinned bge-m3 llamafile into `~/.cache/fire_seq_search` and use it —
    /// zero-config embedding. Pass an explicit path to use your own GGUF/
    /// llamafile instead. Ignored when `--embed-endpoint` is set.
    #[arg(long)]
    embed_model: Option<PathBuf>,

    #[arg(long, default_value = "~/llm/Qwen3.5-9B-UD-Q4_K_XL.gguf")]
    chat_model: PathBuf,

    #[arg(long, default_value = "llama-server")]
    llama_server_bin: PathBuf,

    #[arg(long, default_value_t = 8082)]
    embed_port: u16,

    #[arg(long, default_value_t = 8081)]
    chat_port: u16,

    #[arg(long, default_value = "default")]
    embed_model_name: String,

    #[arg(long, default_value = "default")]
    chat_model_name: String,

    #[arg(long, default_value = "")]
    embed_extra_args: String,

    #[arg(long, default_value = "")]
    chat_extra_args: String,

    /// Number of model layers to offload to GPU (passed as -ngl).
    /// Default 99 ≈ "all layers"; ignored on CPU-only llama-server builds.
    #[arg(long, default_value_t = 99)]
    embed_gpu_layers: u32,

    #[arg(long, default_value_t = 99)]
    chat_gpu_layers: u32,

    #[arg(long)]
    db_path: Option<String>,

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
    #[arg(long, default_value_t = 0.35)]
    min_score: f32,
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
        Some(url) => EndpointSource::External(url.clone()),
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
                extra_args: build_spawn_args(args.embed_gpu_layers, &args.embed_extra_args),
            }
        }
    };
    let chat = match &args.chat_endpoint {
        Some(url) => EndpointSource::External(url.clone()),
        None => EndpointSource::Spawn {
            model: args.chat_model.clone(),
            port: args.chat_port,
            extra_args: build_spawn_args(args.chat_gpu_layers, &args.chat_extra_args),
        },
    };
    Ok(LlmBackendConfig {
        embed,
        chat,
        embed_model_name: args.embed_model_name.clone(),
        chat_model_name: args.chat_model_name.clone(),
        llama_server_bin: args.llama_server_bin.clone(),
    })
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

fn build_spawn_args(gpu_layers: u32, extra: &str) -> Vec<String> {
    let mut args = Vec::new();
    if gpu_layers > 0 {
        args.push("-ngl".to_string());
        args.push(gpu_layers.to_string());
    }
    args.extend(split_extra_args(extra));
    args
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
    let software = if args.obsidian_md { Obsidian } else { Logseq };
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
