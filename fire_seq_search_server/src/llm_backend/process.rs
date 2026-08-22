use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use log::{info, warn};
use shellexpand::tilde;

use super::{EndpointHandle, EndpointSource, LlmError};

pub(crate) async fn resolve_endpoint(
    source: EndpointSource,
    llama_server_bin: &Path,
    is_embedding: bool,
) -> Result<EndpointHandle, LlmError> {
    match source {
        EndpointSource::External { url, flavour, api_key } => {
            if flavour.has_health_endpoint() {
                check_health(&url, Duration::from_secs(5)).await?;
            } else {
                let role = if is_embedding { "embed" } else { "chat" };
                info!(
                    "external {} backend at {} ({:?}); skipping /health probe (flavour has none)",
                    role, url, flavour
                );
            }
            Ok(EndpointHandle { url, child: None, flavour, api_key })
        }
        EndpointSource::Spawn { model, port, gpu_layers, extra_args } => {
            spawn(llama_server_bin, &model, port, gpu_layers, &extra_args, is_embedding).await
        }
    }
}

/// A `.llamafile` model is a self-contained APE binary — it *is* the server, so
/// `--llama-server-bin` is never consulted for one.
fn is_llamafile(model_path: &Path) -> bool {
    model_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext == "llamafile")
        .unwrap_or(false)
}

fn is_executable_file(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                m.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                true
            }
        }
        _ => false,
    }
}

/// Search `path_var` for an executable named `name`, tilde-expanding each entry.
///
/// The tilde expansion is the point. `execvp` does none, but bash expands PATH
/// elements itself during command lookup — so a profile line like
/// `export PATH="~/bin:$PATH"` (quoted, therefore a *literal* tilde) works for
/// everything typed at a prompt and then fails with ENOENT for every process
/// spawned from a program. Resolving the binary ourselves keeps us working on
/// those shells instead of reporting a missing llama-server that is plainly
/// there.
fn find_in_path(name: &str, path_var: &str) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .filter(|entry| !entry.as_os_str().is_empty())
        .map(|entry| PathBuf::from(tilde(entry.to_string_lossy().as_ref()).into_owned()))
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable_file(candidate))
}

/// Resolve `--llama-server-bin` to a concrete executable *before* the first
/// spawn attempt, so a missing binary is reported as itself instead of as a
/// failed GPU attempt. Mirrors `execvp`: a name containing a separator is a
/// path, anything else is looked up in PATH.
fn resolve_llama_server_bin(bin: &Path) -> Result<PathBuf, LlmError> {
    let expanded = PathBuf::from(tilde(bin.to_string_lossy().as_ref()).into_owned());
    let name = expanded.to_string_lossy().into_owned();

    if name.contains(std::path::MAIN_SEPARATOR) {
        return if is_executable_file(&expanded) {
            Ok(expanded)
        } else {
            Err(LlmError::Launch(format!(
                "llama-server not found (or not executable) at {}",
                expanded.display()
            )))
        };
    }

    let path_var = std::env::var("PATH").unwrap_or_default();
    find_in_path(&name, &path_var).ok_or_else(|| {
        LlmError::Launch(format!(
            "{} not found in PATH. Install llama.cpp's llama-server and put it \
             on PATH, pass --llama-server-bin /path/to/llama-server, or point \
             --chat-endpoint at an already-running server. PATH={}",
            name, path_var
        ))
    })
}

/// Which failures are worth a second attempt on CPU. Only "the process ran and
/// died" qualifies: a launch failure (missing binary, bad permissions) or a bad
/// config reproduces identically at `-ngl 0`, and retrying it just prints a
/// misleading "falling back to CPU" ahead of the very same error.
fn worth_cpu_retry(err: &LlmError) -> bool {
    matches!(err, LlmError::EarlyExit(_) | LlmError::HealthCheck(_))
}

/// Decide the `-ngl` values to try, in order. A non-zero `gpu_layers` (the
/// default `99`) means "attempt GPU, then fall back to CPU"; `0` means the user
/// forced CPU, so skip the doomed GPU attempt entirely.
fn spawn_attempts(gpu_layers: u32) -> Vec<u32> {
    if gpu_layers > 0 {
        vec![gpu_layers, 0]
    } else {
        vec![0]
    }
}

async fn spawn(
    llama_server_bin: &Path,
    model: &Path,
    port: u16,
    gpu_layers: u32,
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

    // GPU→CPU fallback: try each `-ngl` value in turn. llamafile's own fallback
    // only covers "no GPU present"; the painful case — GPU present but the Vulkan/
    // ROCm backend won't initialize — errors out instead, so we own the retry
    // here. try_spawn_once kills+reaps a failed child before returning, so the
    // port is free to rebind on the CPU attempt (otherwise EADDRINUSE).
    // Resolve up front — one clear error for a missing binary, instead of two
    // spawn attempts that both fail the same way.
    let bin = if is_llamafile(&model_path) {
        llama_server_bin.to_path_buf()
    } else {
        resolve_llama_server_bin(llama_server_bin)?
    };

    let attempts = spawn_attempts(gpu_layers);
    let last = attempts.len() - 1;
    for (i, ngl) in attempts.iter().enumerate() {
        match try_spawn_once(
            &bin,
            &model_path,
            port,
            *ngl,
            extra_args,
            is_embedding,
            role,
            &stdout_path,
            &stderr_path,
        )
        .await
        {
            Ok(handle) => {
                info!("{} backend ready at {} (-ngl {})", role, handle.url, ngl);
                return Ok(handle);
            }
            Err(e) if i < last && worth_cpu_retry(&e) => {
                warn!(
                    "{} backend failed to start with -ngl {} ({}); falling back to CPU (-ngl 0)",
                    role, ngl, e
                );
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("spawn_attempts always yields at least one attempt")
}

#[allow(clippy::too_many_arguments)]
async fn try_spawn_once(
    llama_server_bin: &Path,
    model_path: &Path,
    port: u16,
    gpu_layers: u32,
    extra_args: &[String],
    is_embedding: bool,
    role: &str,
    stdout_path: &str,
    stderr_path: &str,
) -> Result<EndpointHandle, LlmError> {
    let mut cmd = build_command(
        llama_server_bin,
        model_path,
        port,
        gpu_layers,
        extra_args,
        is_embedding,
    );

    let stdout = File::create(stdout_path)
        .map_err(|e| LlmError::Launch(format!("create {}: {}", stdout_path, e)))?;
    let stderr = File::create(stderr_path)
        .map_err(|e| LlmError::Launch(format!("create {}: {}", stderr_path, e)))?;
    cmd.stdout(Stdio::from(stdout)).stderr(Stdio::from(stderr));

    info!("spawning {} backend on port {}: {:?}", role, port, cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| LlmError::Launch(format!("spawn {} backend: {}", role, e)))?;

    let url = format!("http://127.0.0.1:{}", port);
    match check_health_with_child(&url, Duration::from_secs(60), &mut child).await {
        // A spawned local backend is always llama-server (or our llamafile);
        // never needs an API key.
        Ok(()) => Ok(EndpointHandle {
            url,
            child: Some(child),
            flavour: super::LlmFlavour::LlamaServer,
            api_key: None,
        }),
        Err(e) => {
            // Kill + reap before the caller rebinds the port on the next attempt.
            // (If the child already exited on its own, kill is a no-op and wait
            // just reaps the zombie.)
            let _ = child.kill();
            let _ = child.wait();
            Err(e)
        }
    }
}

fn build_command(
    llama_server_bin: &Path,
    model_path: &Path,
    port: u16,
    gpu_layers: u32,
    extra_args: &[String],
    is_embedding: bool,
) -> Command {
    let mut cmd = if is_llamafile(model_path) {
        // .llamafile is a polyglot APE binary. On Linux the kernel often refuses
        // to exec it directly ("Exec format error") unless binfmt_misc is set up,
        // so we hand it to /bin/sh — the file's shell prelude bootstraps APE.
        let mut c = Command::new("sh");
        c.arg(model_path)
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
        // Already tilde-expanded and resolved to an absolute path by
        // `resolve_llama_server_bin`.
        let mut c = Command::new(llama_server_bin);
        c.arg("--port")
            .arg(port.to_string())
            .arg("--model")
            .arg(model_path);
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

    // -ngl belongs to this attempt's fallback policy, not the user's extra_args,
    // so it goes first; a user who crams `-ngl` into extra_args still wins (later
    // arg takes precedence in llama.cpp).
    cmd.arg("-ngl").arg(gpu_layers.to_string());
    for arg in extra_args {
        cmd.arg(arg);
    }
    cmd
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

/// Health-check loop that also watches for the child exiting early. A GPU backend
/// that can't initialize its driver typically dies within a second or two; polling
/// `try_wait` lets us fail (and fall back to CPU) immediately instead of burning
/// the full 60s timeout. The timeout remains the backstop for a child that hangs
/// alive without ever becoming healthy — we don't fall back merely because GPU
/// warmup is slow.
async fn check_health_with_child(
    url: &str,
    timeout: Duration,
    child: &mut Child,
) -> Result<(), LlmError> {
    let health_url = format!("{}/health", url);
    let deadline = Instant::now() + timeout;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| LlmError::Config(e.to_string()))?;

    loop {
        // Early-exit is the reliable fallback trigger; check it before the probe.
        if let Ok(Some(status)) = child.try_wait() {
            return Err(LlmError::EarlyExit(format!(
                "{} exited early before becoming healthy ({})",
                url, status
            )));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempts_try_gpu_then_cpu() {
        // Default policy: attempt the requested GPU layers, then CPU.
        assert_eq!(spawn_attempts(99), vec![99, 0]);
        assert_eq!(spawn_attempts(33), vec![33, 0]);
    }

    #[test]
    fn attempts_force_cpu_skips_gpu() {
        // `0` is the escape hatch: skip the doomed GPU attempt entirely.
        assert_eq!(spawn_attempts(0), vec![0]);
    }

    #[test]
    fn only_runtime_failures_fall_back_to_cpu() {
        // The GPU-init signature: child came up, then died.
        assert!(worth_cpu_retry(&LlmError::EarlyExit("died".into())));
        assert!(worth_cpu_retry(&LlmError::HealthCheck("timeout".into())));
        // These reproduce identically at -ngl 0; retrying only prints a
        // misleading "falling back to CPU" before the same error.
        assert!(!worth_cpu_retry(&LlmError::Launch("no such file".into())));
        assert!(!worth_cpu_retry(&LlmError::Config("bad model".into())));
    }

    fn write_exe(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn find_in_path_picks_first_executable_match() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let wanted = write_exe(a.path(), "llama-server");
        write_exe(b.path(), "llama-server");
        let path_var = format!("{}:{}", a.path().display(), b.path().display());
        assert_eq!(find_in_path("llama-server", &path_var), Some(wanted));
    }

    #[test]
    fn find_in_path_skips_non_executable_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        // Present but not executable: `execvp` would skip it, so do we.
        std::fs::write(dir.path().join("llama-server"), b"not a program").unwrap();
        let path_var = format!("{}:/definitely/not/here", dir.path().display());
        #[cfg(unix)]
        assert_eq!(find_in_path("llama-server", &path_var), None);
        assert_eq!(find_in_path("nope", &path_var), None);
    }

    #[test]
    fn find_in_path_expands_a_literal_tilde_entry() {
        // The bug this guards: `export PATH="~/bin:$PATH"` leaves a literal
        // tilde that bash resolves during lookup and `execvp` does not.
        // Needs a scratch dir *under* $HOME, since that is the only prefix a
        // tilde can name. Skip rather than fail where that isn't possible.
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let Ok(dir) = tempfile::tempdir_in(&home) else {
            return;
        };
        let wanted = write_exe(dir.path(), "llama-server");
        let leaf = dir.path().file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(
            find_in_path("llama-server", &format!("~/{}", leaf)),
            Some(wanted)
        );
    }

    #[test]
    fn resolve_rejects_a_path_that_is_not_there() {
        let err = resolve_llama_server_bin(Path::new("/definitely/not/here/llama-server"))
            .unwrap_err();
        assert!(matches!(err, LlmError::Launch(_)), "got {:?}", err);
    }

    #[test]
    fn resolve_accepts_an_explicit_path() {
        let dir = tempfile::tempdir().unwrap();
        let bin = write_exe(dir.path(), "llama-server");
        assert_eq!(resolve_llama_server_bin(&bin).unwrap(), bin);
    }

    #[test]
    fn llamafile_detection_keys_on_the_extension() {
        assert!(is_llamafile(Path::new("/x/bge-m3.llamafile")));
        assert!(!is_llamafile(Path::new("/x/Qwen3.5-9B-UD-Q4_K_XL.gguf")));
    }
}
