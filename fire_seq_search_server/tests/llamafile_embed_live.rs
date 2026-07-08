//! Live integration test for the zero-config embedding path: auto-download the
//! pinned bge-m3 llamafile, spawn it, and confirm it actually serves embeddings.
//!
//! This is the *one* place the pinned llamafile is exercised end-to-end — the
//! download + SHA-256 gate (`model_fetch`), the `.llamafile`-triggered spawn
//! mode (`process::build_command`), the injected `--embedding -ub 8192 …` args,
//! and the `/v1/embeddings` wire format. Unit tests cover the hashing helpers;
//! nothing else proves the published artifact boots and returns 1024-dim vectors
//! on a clean machine. Regressions this catches: a bad pin, a llama.cpp arg the
//! newer server rejects (cf. the `--nobrowser` breakage), or a dim change.
//!
//! Gated on `FIRE_SEQ_LIVE_LLAMAFILE`: unset (every normal `cargo test`, local
//! or plain Rust CI) → the test prints a skip notice and returns, because it
//! would otherwise pull ~723 MB. The dedicated `llamafile-embed.yml` workflow
//! sets the env and caches the download.

use fire_seq_search_server::llm_backend::{
    model_fetch, EndpointSource, LlmBackend, LlmBackendConfig, LlmFlavor,
};
use std::path::PathBuf;

const BGE_M3_DIM: usize = 1024;

/// True unless `FIRE_SEQ_LIVE_LLAMAFILE` is set to a non-empty value.
fn skip() -> bool {
    match std::env::var("FIRE_SEQ_LIVE_LLAMAFILE") {
        Ok(v) if !v.trim().is_empty() => false,
        _ => true,
    }
}

/// Best-effort teardown: `std::process::Child` does not kill on drop, so the
/// spawned llamafile would otherwise linger. CI discards the runner regardless;
/// this keeps local runs from leaking a process on the test port.
fn kill(pids: &[u32]) {
    for pid in pids {
        let _ = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
    }
}

#[tokio::test]
async fn downloaded_llamafile_embeds_at_expected_dim() {
    if skip() {
        eprintln!(
            "skipping downloaded_llamafile_embeds_at_expected_dim: \
             FIRE_SEQ_LIVE_LLAMAFILE not set (would download ~723 MB)"
        );
        return;
    }

    // 1. Auto-download + SHA-256 verify the pinned artifact. Idempotent: a cached
    //    file whose hash already matches skips the network (what the CI cache buys).
    let model = model_fetch::ensure_bge_m3()
        .await
        .expect("ensure_bge_m3: download + verify the pinned llamafile");
    assert!(
        model.extension().and_then(|e| e.to_str()) == Some("llamafile"),
        "expected a .llamafile path (the extension drives spawn mode), got {:?}",
        model
    );

    // 2. Spawn *only* the embed backend from that llamafile. Chat is a dummy
    //    External+Ollama endpoint: that flavor skips the /health probe, so launch
    //    never touches it, and the test never calls chat(). `-ngl 0` forces CPU —
    //    CI has no GPU, and it skips the doomed GPU attempt + fallback wait.
    let cfg = LlmBackendConfig {
        embed: EndpointSource::Spawn {
            model,
            port: 18082,
            gpu_layers: 0,
            extra_args: vec![],
        },
        chat: EndpointSource::External {
            url: "http://127.0.0.1:1".to_string(),
            flavor: LlmFlavor::Ollama,
            api_key: None,
        },
        embed_model_name: "default".to_string(),
        chat_model_name: "default".to_string(),
        llama_server_bin: PathBuf::from("llama-server"),
    };
    let backend = LlmBackend::launch(cfg)
        .await
        .expect("launch embed backend from the downloaded llamafile");
    let pids = backend.child_pids();

    // 3. The actual contract: real embeddings back, one per input, all 1024-dim.
    let texts = vec![
        "hello world".to_string(),
        "a second, longer chunk of text to embed".to_string(),
    ];
    let result = backend.embed(&texts).await;

    // Tear down before asserting so a failed assert can't leak the process.
    let out = match result {
        Ok(o) => o,
        Err(e) => {
            kill(&pids);
            panic!("embed call against the llamafile failed: {}", e);
        }
    };
    kill(&pids);

    assert_eq!(out.len(), texts.len(), "one embedding per input");
    for (i, v) in out.iter().enumerate() {
        assert_eq!(
            v.len(),
            BGE_M3_DIM,
            "embedding {} has dim {} — bge-m3 must be {}",
            i,
            v.len(),
            BGE_M3_DIM
        );
    }
}
