use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{error, info};
use tokio::time::Duration;
use walkdir::WalkDir;

use crate::indexer::chunker::{chunk_note, is_summarizable, preprocess};
use crate::indexer::store::{Store, CHUNKER_VERSION};
use crate::indexer::{IndexerError, IndexerHandle};
use crate::llm_backend::LlmBackend;
use crate::query_engine::NotebookSoftware;

pub struct Indexer {
    store: Arc<Store>,
    backend: Arc<LlmBackend>,
    notebook_path: PathBuf,
    handle: IndexerHandle,
    software: NotebookSoftware,
    /// If `Some`, mirror the notebook into this directory with each note's
    /// post-preprocess markdown and its final chunks, so noise vs information
    /// loss in the stripper can be spot-checked. Set via the
    /// `FIRE_SEQ_DUMP_PROCESSED_DIR` env var. Diagnostic-only.
    dump_processed_dir: Option<PathBuf>,
}

impl Indexer {
    pub fn new(
        store: Arc<Store>,
        backend: Arc<LlmBackend>,
        notebook_path: PathBuf,
        handle: IndexerHandle,
        software: NotebookSoftware,
    ) -> Self {
        let dump_processed_dir = std::env::var_os("FIRE_SEQ_DUMP_PROCESSED_DIR")
            .map(PathBuf::from);
        if let Some(ref d) = dump_processed_dir {
            info!("FIRE_SEQ_DUMP_PROCESSED_DIR={} — will dump stripped notes + chunks per indexed file", d.display());
        }
        Self { store, backend, notebook_path, handle, software, dump_processed_dir }
    }

    pub async fn hydrate(&self) -> Result<(), IndexerError> {
        let embeddings = self.store.load_all_embeddings()?;
        let count = embeddings.len();
        *self.handle.vec.write().await = embeddings;
        self.handle.status.write().await.indexed_chunks = count;

        let summary_embs = self.store.load_all_summary_embeddings()?;
        let summary_count = summary_embs.len();
        *self.handle.summary_vec.write().await = summary_embs.into_iter().collect();

        info!(
            "Hydrated {} chunk embeddings and {} summary embeddings from SQLite",
            count, summary_count
        );
        Ok(())
    }

    pub async fn scan_once(&self) -> Result<(), IndexerError> {
        {
            let mut s = self.handle.status.write().await;
            s.in_flight = true;
            s.indexed_notes = 0;
        }

        let entries = self.walk_notebook()?;
        let total = entries.len();
        self.handle.status.write().await.total_notes = total;

        let existing_paths = self.store.list_paths()?;
        let mut fs_paths: HashSet<String> = HashSet::new();

        for (rel_path, mtime, abs_path) in &entries {
            fs_paths.insert(rel_path.clone());
            if let Err(e) = self.process_note(rel_path, *mtime, abs_path).await {
                error!("Error processing {}: {}", rel_path, e);
            }
            self.handle.status.write().await.indexed_notes += 1;
        }

        for path in existing_paths.difference(&fs_paths) {
            if let Err(e) = self.delete_note(path).await {
                error!("Error deleting removed note {}: {}", path, e);
            }
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let vec_count = self.handle.vec.read().await.len();
        {
            let mut s = self.handle.status.write().await;
            s.in_flight = false;
            s.last_scan_at = Some(now);
            s.indexed_chunks = vec_count;
        }

        info!("Scan complete: {} notes, {} chunks in memory", total, vec_count);
        Ok(())
    }

    async fn process_note(
        &self,
        rel_path: &str,
        fs_mtime: i64,
        abs_path: &Path,
    ) -> Result<(), IndexerError> {
        let db_row = self.store.get_note(rel_path)?;

        // Fast path: mtime + chunker_version both match
        if let Some(ref row) = db_row {
            if row.mtime == fs_mtime && row.chunker_version == CHUNKER_VERSION {
                return Ok(());
            }
        }

        let raw = std::fs::read_to_string(abs_path)?;
        let hash_bytes = blake3::hash(raw.as_bytes()).as_bytes().to_vec();

        // Hash + version match: update mtime only, skip re-embedding
        if let Some(ref row) = db_row {
            if row.content_hash == hash_bytes && row.chunker_version == CHUNKER_VERSION {
                self.store.update_mtime(rel_path, fs_mtime)?;
                return Ok(());
            }
        }

        // Full re-chunk + re-embed.
        // IMPORTANT: do not upsert the note row until embedding has succeeded.
        // Otherwise a transient embed failure leaves a row with the new content
        // hash but no chunks — the next scan's hash-match fast-path then skips
        // re-embedding forever.
        let page_title = path_to_page_title(rel_path);
        let chunks = chunk_note(&self.software, &page_title, &raw);

        if let Some(ref dump_dir) = self.dump_processed_dir {
            if let Err(e) = dump_processed_note(dump_dir, rel_path, &raw, &chunks) {
                error!("dump_processed_note failed for {}: {}", rel_path, e);
            }
        }

        if chunks.is_empty() {
            let note_id = self.store.upsert_note(rel_path, &page_title, fs_mtime, &hash_bytes)?;
            let old_ids = self.store.get_chunk_ids_for_note(note_id)?;
            self.store.replace_chunks(note_id, &[])?;
            // No chunks ⇒ no narrative content ⇒ nothing for the summarizer to
            // do; don't leave the row queued for a summary the gate rejects.
            self.store.mark_summary_unsummarizable(note_id)?;
            let mut v = self.handle.vec.write().await;
            v.retain(|(id, _)| !old_ids.contains(id));
            drop(v);
            self.handle.summary_vec.write().await.remove(&note_id);
            return Ok(());
        }

        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let embeddings = embed_chunks(&self.backend, &texts)
            .await
            .map_err(IndexerError::Embed)?;

        if embeddings.len() != chunks.len() {
            return Err(IndexerError::Embed(format!(
                "expected {} embeddings, got {}",
                chunks.len(),
                embeddings.len()
            )));
        }
        for (i, emb) in embeddings.iter().enumerate() {
            if emb.len() != 1024 {
                return Err(IndexerError::Embed(format!(
                    "chunk {} has {}-dim embedding, want 1024",
                    i,
                    emb.len()
                )));
            }
        }

        // Embedding succeeded; safe to commit the row + chunks atomically.
        let note_id = self.store.upsert_note(rel_path, &page_title, fs_mtime, &hash_bytes)?;
        // upsert_note cleared the SQL summary state; mirror the same change
        // in-memory so a stale summary embedding doesn't keep matching for the
        // few minutes it takes the summarizer to regenerate.
        self.handle.summary_vec.write().await.remove(&note_id);
        // If the page has chunks but no real narrative content (e.g. a single
        // `[[wikilink]]`), short-circuit the summarizer: mark it OK with no
        // summary now instead of queueing an LLM call the gate will reject.
        if !is_summarizable(&self.software, &raw) {
            self.store.mark_summary_unsummarizable(note_id)?;
        }
        let old_ids = self.store.get_chunk_ids_for_note(note_id)?;

        let chunk_data: Vec<(usize, &str, &[f32])> = chunks
            .iter()
            .zip(embeddings.iter())
            .map(|(c, e)| (c.ord, c.text.as_str(), e.as_slice()))
            .collect();

        let new_ids = self.store.replace_chunks(note_id, &chunk_data)?;

        let new_pairs: Vec<(i64, [f32; 1024])> = new_ids
            .iter()
            .zip(embeddings.iter())
            .map(|(id, emb)| {
                let mut arr = [0f32; 1024];
                arr.copy_from_slice(emb);
                (*id, arr)
            })
            .collect();

        {
            let mut v = self.handle.vec.write().await;
            v.retain(|(id, _)| !old_ids.contains(id));
            v.extend(new_pairs);
        }

        info!("Indexed {} chunks for {}", chunks.len(), rel_path);
        Ok(())
    }

    async fn delete_note(&self, rel_path: &str) -> Result<(), IndexerError> {
        if let Some(row) = self.store.get_note(rel_path)? {
            let old_ids = self.store.get_chunk_ids_for_note(row.id)?;
            let mut v = self.handle.vec.write().await;
            v.retain(|(id, _)| !old_ids.contains(id));
            drop(v);
            self.handle.summary_vec.write().await.remove(&row.id);
        }
        self.store.delete_note(rel_path)?;
        Ok(())
    }

    fn walk_notebook(&self) -> Result<Vec<(String, i64, PathBuf)>, IndexerError> {
        walk_notebook_at(&self.notebook_path, &self.software)
    }

    pub async fn run(self) {
        loop {
            if let Err(e) = self.scan_once().await {
                error!("Indexer scan failed: {}", e);
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(600)) => {},
                _ = self.handle.reindex_notify.notified() => {
                    info!("Reindex triggered manually");
                },
            }
        }
    }
}

async fn embed_chunks(backend: &LlmBackend, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    let mut out = Vec::with_capacity(texts.len());
    for batch in texts.chunks(32) {
        let embeddings = backend.embed(batch).await.map_err(|e| e.to_string())?;
        out.extend(embeddings);
    }
    Ok(out)
}

/// Write a side-by-side inspection file for one indexed note: the raw input,
/// the post-preprocess markdown (what the stripper hands the chunker), and
/// each final chunk (what the embedder/LLM see). Only invoked when
/// `FIRE_SEQ_DUMP_PROCESSED_DIR` is set; never affects indexing behaviour.
fn dump_processed_note(
    dump_dir: &Path,
    rel_path: &str,
    raw: &str,
    chunks: &[crate::indexer::chunker::Chunk],
) -> std::io::Result<()> {
    let out_path = dump_dir.join(rel_path);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let preprocessed = preprocess(raw);
    let mut body = String::with_capacity(raw.len() + preprocessed.len() + 256);
    body.push_str("=== RAW ===\n");
    body.push_str(raw);
    if !raw.ends_with('\n') { body.push('\n'); }
    body.push_str("\n=== PREPROCESSED (post-strip, pre-chunk) ===\n");
    body.push_str(&preprocessed);
    if !preprocessed.ends_with('\n') { body.push('\n'); }
    body.push_str(&format!("\n=== CHUNKS ({}) ===\n", chunks.len()));
    for c in chunks {
        body.push_str(&format!("\n--- chunk {} ---\n", c.ord));
        body.push_str(&c.text);
        if !c.text.ends_with('\n') { body.push('\n'); }
    }
    std::fs::write(&out_path, body)
}

pub(crate) fn walk_notebook_at(
    notebook: &Path,
    software: &NotebookSoftware,
) -> Result<Vec<(String, i64, PathBuf)>, IndexerError> {
    match software {
        NotebookSoftware::Logseq => walk_logseq(notebook),
        NotebookSoftware::Obsidian => walk_obsidian(notebook),
    }
}

/// Logseq's `pages/` and `journals/` subdirectories. `logseq/` and
/// `assets/` are skipped by construction (never entered).
fn walk_logseq(notebook: &Path) -> Result<Vec<(String, i64, PathBuf)>, IndexerError> {
    let mut entries = Vec::new();
    for sub in ["pages", "journals"] {
        let root = notebook.join(sub);
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root).follow_links(false) {
            let entry = entry?;
            if let Some(e) = collect_md_entry(notebook, &entry) {
                entries.push(e);
            }
        }
    }
    Ok(entries)
}

/// Obsidian vaults are arbitrarily nested under the vault root. Walk
/// recursively, skipping `.obsidian/` (config + plugins), `.trash/`, and
/// any dot-prefixed directory (covers `.git/`, `.stversions/`, etc.).
/// Non-`.md` files (PDFs, images, attachments) are filtered downstream by
/// `collect_md_entry`.
fn walk_obsidian(notebook: &Path) -> Result<Vec<(String, i64, PathBuf)>, IndexerError> {
    let mut entries = Vec::new();
    let walker = WalkDir::new(notebook)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if !e.file_type().is_dir() {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "trash"
        });
    for entry in walker {
        let entry = entry?;
        if let Some(e) = collect_md_entry(notebook, &entry) {
            entries.push(e);
        }
    }
    Ok(entries)
}

fn collect_md_entry(
    notebook: &Path,
    entry: &walkdir::DirEntry,
) -> Option<(String, i64, PathBuf)> {
    if !entry.file_type().is_file() {
        return None;
    }
    let path = entry.path();
    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        return None;
    }
    let mtime = entry
        .metadata()
        .ok()?
        .modified()
        .unwrap_or(UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let rel_path = path
        .strip_prefix(notebook)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned();
    Some((rel_path, mtime, path.to_owned()))
}

fn path_to_page_title(rel_path: &str) -> String {
    let stem = Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(rel_path);
    urlencoding::decode(stem)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| stem.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(p: &Path, body: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, body).unwrap();
    }

    #[test]
    fn obsidian_walker_recurses_and_skips_dotdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("Top.md"), "top");
        touch(&root.join("sub/Nested.md"), "nested");
        touch(&root.join("sub/deeper/Deep.md"), "deep");
        // Skipped: dot-dirs and the Obsidian config tree.
        touch(&root.join(".obsidian/config.json"), "{}");
        touch(&root.join(".obsidian/plugins/foo/main.md"), "should not be indexed");
        touch(&root.join(".git/HEAD"), "ref");
        touch(&root.join(".trash/Old.md"), "should not be indexed");
        // Non-md files are ignored at the file filter.
        touch(&root.join("attachments/img.png"), "binary");

        let mut got: Vec<String> =
            walk_notebook_at(root, &NotebookSoftware::Obsidian)
                .unwrap()
                .into_iter()
                .map(|(p, _, _)| p.replace('\\', "/"))
                .collect();
        got.sort();
        assert_eq!(got, vec!["Top.md", "sub/Nested.md", "sub/deeper/Deep.md"]);
    }

    #[test]
    fn logseq_walker_only_enters_pages_and_journals() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("pages/Foo.md"), "");
        touch(&root.join("journals/2024_01_01.md"), "");
        touch(&root.join("assets/x.pdf"), "");
        touch(&root.join("logseq/config.edn"), "");
        // A stray top-level .md (not in pages/) must be ignored under Logseq.
        touch(&root.join("README.md"), "");

        let mut got: Vec<String> =
            walk_notebook_at(root, &NotebookSoftware::Logseq)
                .unwrap()
                .into_iter()
                .map(|(p, _, _)| p.replace('\\', "/"))
                .collect();
        got.sort();
        assert_eq!(got, vec!["journals/2024_01_01.md", "pages/Foo.md"]);
    }
}
