use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use log::info;
use shellexpand::tilde;

use super::{EndpointHandle, EndpointSource, LlmError};

pub(crate) async fn resolve_endpoint(
    source: EndpointSource,
    llama_server_bin: &Path,
    is_embedding: bool,
) -> Result<EndpointHandle, LlmError> {
    match source {
        EndpointSource::External(url) => {
            check_health(&url, Duration::from_secs(5)).await?;
            Ok(EndpointHandle { url, child: None })
        }
        EndpointSource::Spawn { model, port, extra_args } => {
            spawn(llama_server_bin, &model, port, &extra_args, is_embedding).await
        }
    }
}

async fn spawn(
    llama_server_bin: &Path,
    model: &Path,
    port: u16,
    extra_args: &[String],
    is_embedding: bool,
) -> Result<EndpointHandle, LlmError> {
    let model_str = tilde(model.to_string_lossy().as_ref()).into_owned();
    let model_path = PathBuf::from(&model_str);
    if !model_path.exists() {
        return Err(LlmError::Config(format!(
            "model file not found: {}",
            model_str
        )));
    }

    let role = if is_embedding { "embed" } else { "chat" };
    // Prefer XDG_RUNTIME_DIR (per-user, auto-cleaned by systemd) over /tmp so
    // two users on the same host don't clobber each other's logs. macOS sets
    // TMPDIR per-user; fall back to /tmp last.
    let log_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .or_else(|| std::env::var_os("TMPDIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let stdout_path = log_dir
        .join(format!("fire_seq_search.{}.stdout.log", role))
        .to_string_lossy()
        .into_owned();
    let stderr_path = log_dir
        .join(format!("fire_seq_search.{}.stderr.log", role))
        .to_string_lossy()
        .into_owned();

    let is_llamafile = model_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext == "llamafile")
        .unwrap_or(false);

    let mut cmd = if is_llamafile {
        // .llamafile is a polyglot APE binary. On Linux the kernel often refuses
        // to exec it directly ("Exec format error") unless binfmt_misc is set up,
        // so we hand it to /bin/sh — the file's shell prelude bootstraps APE.
        let mut c = Command::new("sh");
        c.arg(&model_path)
            .arg("--server")
            .arg("--port")
            .arg(port.to_string());
        // NB: no --nobrowser. The pinned bge-m3 llamafile is built on a newer
        // llama.cpp server that rejects that flag ("error: invalid argument:
        // --nobrowser") and exits immediately, which surfaced only as a 60s
        // health-check timeout. The newer server doesn't auto-open a browser
        // in --server mode, so the flag is unnecessary anyway.
        if is_embedding {
            // bge-m3 supports up to 8K-token inputs; raise the physical and
            // logical batch sizes so a single chunk can be embedded in one
            // shot. llama-server's default ubatch=512 rejects larger inputs
            // with HTTP 500 ("input too large to process").
            c.arg("--embedding")
                .arg("-ub").arg("8192")
                .arg("-b").arg("8192")
                .arg("-c").arg("8192");
        } else {
            // Chat backend serves background summarization AND `/ask`
            // concurrently. Each /ask packs K=8 pages × (summary + best chunk),
            // routinely hitting 3000+ prompt tokens; a summarization in the
            // sibling slot can be similarly large. 8192 was fine for the old
            // 7B but the 9B default has a larger per-token KV footprint, and
            // two concurrent prompts overflowed the shared cache. 16384 gives
            // both slots room; users on tight VRAM can override via
            // `--chat-extra-args "-c N"` (later -c wins).
            c.arg("-c").arg("16384")
                // Activate the model's Jinja chat template so the request body's
                // `chat_template_kwargs.enable_thinking=false` is actually read.
                // Without --jinja, Qwen3-family models default to thinking and
                // generate ~3000 tokens of reasoning per summary call.
                .arg("--jinja");
        }
        c
    } else {
        let bin_str = tilde(llama_server_bin.to_string_lossy().as_ref()).into_owned();
        let mut c = Command::new(&bin_str);
        c.arg("--port")
            .arg(port.to_string())
            .arg("--model")
            .arg(&model_path);
        if is_embedding {
            // See note above re: -ub / -b sizing for embedding backends.
            c.arg("--embedding")
                .arg("-ub").arg("8192")
                .arg("-b").arg("8192")
                .arg("-c").arg("8192");
        } else {
            // See note above: chat backend needs a bigger context for `/ask`
            // plus concurrent summarization on the 9B default.
            c.arg("-c").arg("16384")
                // See --jinja note in the llamafile branch above.
                .arg("--jinja");
        }
        c
    };

    for arg in extra_args {
        cmd.arg(arg);
    }

    let stdout = File::create(&stdout_path)
        .map_err(|e| LlmError::Spawn(format!("create {}: {}", stdout_path, e)))?;
    let stderr = File::create(&stderr_path)
        .map_err(|e| LlmError::Spawn(format!("create {}: {}", stderr_path, e)))?;
    cmd.stdout(Stdio::from(stdout)).stderr(Stdio::from(stderr));

    info!("spawning {} backend on port {}: {:?}", role, port, cmd);
    let child = cmd
        .spawn()
        .map_err(|e| LlmError::Spawn(format!("spawn {} backend: {}", role, e)))?;

    let url = format!("http://127.0.0.1:{}", port);
    check_health(&url, Duration::from_secs(60)).await?;
    info!("{} backend ready at {}", role, url);

    Ok(EndpointHandle { url, child: Some(child) })
}

async fn check_health(url: &str, timeout: Duration) -> Result<(), LlmError> {
    let health_url = format!("{}/health", url);
    let deadline = Instant::now() + timeout;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| LlmError::Config(e.to_string()))?;

    loop {
        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => {
                if Instant::now() >= deadline {
                    return Err(LlmError::HealthCheck(format!(
                        "{} not responsive within {:?}",
                        url, timeout
                    )));
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
