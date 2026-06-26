//! Zero-config embedding model: auto-fetch the pinned bge-m3 llamafile.
//!
//! The embedding model is not the user's problem. Rather than asking them to
//! download a GGUF and point `--embed-model` at it, we ship bge-m3 as a
//! self-contained llamafile on GitHub Releases and fetch it once into the
//! per-user cache. The spawn layer then launches it like any other model
//! (the `.llamafile` extension switches it into llamafile mode).
//!
//! Pinned by URL **and** SHA-256: an auto-download must never silently swap
//! the embedding model out from under an existing index — a different model
//! produces incompatible vectors, which would poison cosine similarity
//! against everything already stored.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::StreamExt;
use log::{info, warn};
use sha2::{Digest, Sha256};

use super::LlmError;

/// Pinned bge-m3 embedding model, packaged as a self-contained llamafile.
/// Built from <https://github.com/Endle/bge-m3-llamafile> and published on its
/// GitHub Releases. Update all three constants together when bumping the pin.
const BGE_M3_URL: &str =
    "https://github.com/Endle/bge-m3-llamafile/releases/download/20260625/bge-m3.llamafile";
const BGE_M3_SHA256: &str =
    "7a2f47411ce8624f7a56190a74e6d0089481ff91fa3fed4b76795008711855ec";
/// Cached filename. The `.llamafile` extension is load-bearing: the spawn
/// layer keys off it to launch in llamafile mode (no `--model` arg, since the
/// weights are baked into the binary).
const BGE_M3_FILENAME: &str = "bge-m3.llamafile";

fn cache_dir() -> PathBuf {
    PathBuf::from(shellexpand::tilde("~/.cache/fire_seq_search").as_ref())
}

/// Ensure the pinned bge-m3 llamafile is present and intact in the cache,
/// downloading it once if needed. Returns the path to use as the embed model.
///
/// Idempotent: a cached file whose SHA-256 already matches is returned without
/// touching the network. A cached file that fails the hash (truncated download
/// from a previous crash, or a stale pin from an older build) is re-fetched.
pub async fn ensure_bge_m3() -> Result<PathBuf, LlmError> {
    let dir = cache_dir();
    let dest = dir.join(BGE_M3_FILENAME);

    if dest.exists() {
        match sha256_file(&dest) {
            Ok(h) if h == BGE_M3_SHA256 => {
                info!("bge-m3 llamafile present and verified at {}", dest.display());
                return Ok(dest);
            }
            Ok(h) => warn!(
                "cached bge-m3 llamafile hash mismatch ({} != expected), re-downloading",
                h
            ),
            Err(e) => warn!("cannot hash cached bge-m3 llamafile ({}), re-downloading", e),
        }
    }

    std::fs::create_dir_all(&dir)
        .map_err(|e| LlmError::Config(format!("create cache dir {}: {}", dir.display(), e)))?;

    info!(
        "fetching bge-m3 embedding model (~723 MB, one-time) from {} into {}",
        BGE_M3_URL,
        dest.display()
    );

    // Download to a sibling temp file so the final rename is atomic (same
    // filesystem). A crash mid-download then can't leave a truncated file at
    // the real path that the exists() check above would later trust.
    let tmp = dir.join(format!("{}.part", BGE_M3_FILENAME));
    let hash = download_to(&tmp, BGE_M3_URL).await?;

    if hash != BGE_M3_SHA256 {
        let _ = std::fs::remove_file(&tmp);
        return Err(LlmError::Config(format!(
            "bge-m3 download hash mismatch: got {}, expected {}",
            hash, BGE_M3_SHA256
        )));
    }

    // llamafile is an executable APE binary. We invoke it via `sh` in the
    // spawn layer (which works regardless), but set +x anyway so a future
    // binfmt_misc / direct-exec path also works.
    set_executable(&tmp)?;
    std::fs::rename(&tmp, &dest)
        .map_err(|e| LlmError::Config(format!("install bge-m3 llamafile: {}", e)))?;
    info!("bge-m3 llamafile ready at {}", dest.display());
    Ok(dest)
}

/// Stream `url` to `path`, returning the lowercase hex SHA-256 of the bytes
/// written. Hashes on the fly so a 723 MB file is never re-read just to verify.
async fn download_to(path: &Path, url: &str) -> Result<String, LlmError> {
    let client = reqwest::Client::builder()
        // No overall timeout — this is a multi-hundred-MB download over an
        // arbitrary link. A connect timeout still fails a dead host fast
        // instead of hanging startup forever.
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| LlmError::Config(e.to_string()))?;

    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(LlmError::Config(format!(
            "download {} failed: HTTP {}",
            url,
            resp.status()
        )));
    }

    let mut file = std::fs::File::create(path)
        .map_err(|e| LlmError::Config(format!("create {}: {}", path.display(), e)))?;
    let mut hasher = Sha256::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .map_err(|e| LlmError::Config(format!("write {}: {}", path.display(), e)))?;
    }
    file.flush()
        .map_err(|e| LlmError::Config(format!("flush {}: {}", path.display(), e)))?;
    Ok(hex(&hasher.finalize()))
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), LlmError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| LlmError::Config(format!("chmod +x {}: {}", path.display(), e)))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), LlmError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encodes_lowercase_zero_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa2, 0xff]), "000fa2ff");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn sha256_file_matches_known_vector() {
        // SHA-256 of the empty input.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("empty");
        std::fs::write(&p, b"").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_file_matches_known_vector_abc() {
        // SHA-256("abc") — the canonical FIPS-180 test vector.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("abc");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
