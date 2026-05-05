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
    let stdout_path = format!("/tmp/fire_seq_search.{}.stdout.log", role);
    let stderr_path = format!("/tmp/fire_seq_search.{}.stderr.log", role);

    let is_llamafile = model_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext == "llamafile")
        .unwrap_or(false);

    let mut cmd = if is_llamafile {
        // .llamafile is a self-executing binary; ignore --llama-server-bin.
        let mut c = Command::new(&model_path);
        c.arg("--server").arg("--port").arg(port.to_string()).arg("--nobrowser");
        if is_embedding {
            c.arg("--embedding");
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
            c.arg("--embedding");
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
