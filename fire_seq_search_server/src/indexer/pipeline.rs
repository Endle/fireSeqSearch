use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{error, info};
use tokio::time::Duration;
use walkdir::WalkDir;

use crate::indexer::chunker::{chunk_note, is_summarizable};
use crate::indexer::store::{Store, CHUNKER_VERSION};
use crate::indexer::{IndexerError, IndexerHandle};
use crate::llm_backend::LlmBackend;

pub struct Indexer {
    store: Arc<Store>,
    backend: Arc<LlmBackend>,
    notebook_path: PathBuf,
    handle: IndexerHandle,
}

impl Indexer {
    pub fn new(
        store: Arc<Store>,
        backend: Arc<LlmBackend>,
        notebook_path: PathBuf,
        handle: IndexerHandle,
    ) -> Self {
        Self { store, backend, notebook_path, handle }
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
        let chunks = chunk_note(&page_title, &raw);

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
        if !is_summarizable(&raw) {
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
        let notebook = &self.notebook_path;
        let mut entries = Vec::new();

        for sub in ["pages", "journals"] {
            let root = notebook.join(sub);
            if !root.exists() {
                continue;
            }

            for entry in WalkDir::new(&root).follow_links(false) {
                let entry = entry?;

                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry.path();

                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }

                let mtime = entry
                    .metadata()?
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

                entries.push((rel_path, mtime, path.to_owned()));
            }
        }

        Ok(entries)
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

fn path_to_page_title(rel_path: &str) -> String {
    let stem = Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(rel_path);
    urlencoding::decode(stem)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| stem.to_owned())
}
