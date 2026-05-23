use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use log::warn;
use rusqlite::{params, Connection};

use crate::indexer::IndexerError;

pub const CHUNKER_VERSION: i64 = 4;

// Summary lifecycle states stored in `notes.summary_status`.
pub const SUMMARY_NONE: i64 = 0;
pub const SUMMARY_QUEUED_LOW: i64 = 1;
pub const SUMMARY_QUEUED_HIGH: i64 = 2;
pub const SUMMARY_IN_PROGRESS: i64 = 3;
pub const SUMMARY_OK: i64 = 4;
pub const SUMMARY_FAILED: i64 = 5;
pub const SUMMARY_MAX_ATTEMPTS: i64 = 3;

/// Coarse public-facing status; keep in sync with the constants above.
pub fn summary_status_str(s: i64) -> &'static str {
    match s {
        SUMMARY_OK => "ok",
        SUMMARY_FAILED => "failed",
        _ => "pending",
    }
}

pub struct ChunkDetail {
    pub id: i64,
    pub note_id: i64,
    pub text: String,
}

pub struct NoteDetail {
    pub id: i64,
    pub page_title: String,
    pub rel_path: String,
    pub summary: Option<String>,
    pub summary_status: i64,
}

pub struct NoteRow {
    pub id: i64,
    pub mtime: i64,
    pub content_hash: Vec<u8>,
    pub chunker_version: i64,
}

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, IndexerError> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS notes (
               id                INTEGER PRIMARY KEY,
               rel_path          TEXT    NOT NULL UNIQUE,
               page_title        TEXT    NOT NULL,
               mtime             INTEGER NOT NULL,
               content_hash      BLOB    NOT NULL,
               chunker_version   INTEGER NOT NULL,
               summary           TEXT,
               summary_status    INTEGER NOT NULL DEFAULT 0,
               summary_attempts  INTEGER NOT NULL DEFAULT 0,
               summary_embedding BLOB
             );
             CREATE TABLE IF NOT EXISTS chunks (
               id        INTEGER PRIMARY KEY,
               note_id   INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
               ord       INTEGER NOT NULL,
               text      TEXT    NOT NULL,
               embedding BLOB    NOT NULL,
               UNIQUE(note_id, ord)
             );
             CREATE INDEX IF NOT EXISTS idx_chunks_note_id ON chunks(note_id);
             CREATE INDEX IF NOT EXISTS idx_notes_summary_status ON notes(summary_status);",
        )?;
        // Best-effort migration for DBs created before the summary columns
        // existed. ADD COLUMN on an existing column errors with SQLITE_ERROR
        // and message starting with "duplicate column name"; only that case
        // is ignored. Any other failure (disk full, locking, schema drift)
        // propagates so the operator sees it instead of running on a
        // half-migrated DB.
        for stmt in [
            "ALTER TABLE notes ADD COLUMN summary TEXT",
            "ALTER TABLE notes ADD COLUMN summary_status INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE notes ADD COLUMN summary_attempts INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE notes ADD COLUMN summary_embedding BLOB",
            "CREATE INDEX IF NOT EXISTS idx_notes_summary_status ON notes(summary_status)",
        ] {
            match conn.execute(stmt, []) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(_, Some(ref msg)))
                    if msg.starts_with("duplicate column name") => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn get_note(&self, rel_path: &str) -> Result<Option<NoteRow>, IndexerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, mtime, content_hash, chunker_version FROM notes WHERE rel_path = ?1",
        )?;
        let mut rows = stmt.query_map(params![rel_path], |row| {
            Ok(NoteRow {
                id: row.get(0)?,
                mtime: row.get(1)?,
                content_hash: row.get(2)?,
                chunker_version: row.get(3)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn list_paths(&self) -> Result<HashSet<String>, IndexerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT rel_path FROM notes")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut set = HashSet::new();
        for row in rows {
            set.insert(row?);
        }
        Ok(set)
    }

    pub fn upsert_note(
        &self,
        rel_path: &str,
        page_title: &str,
        mtime: i64,
        hash: &[u8],
    ) -> Result<i64, IndexerError> {
        let conn = self.conn.lock().unwrap();
        // We only call upsert_note when content changed (chunks were just
        // re-embedded), so any cached summary is stale: clear it and queue
        // for re-summarization at low priority.
        conn.execute(
            "INSERT INTO notes (rel_path, page_title, mtime, content_hash, chunker_version,
                                summary, summary_status, summary_attempts)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, 0)
             ON CONFLICT(rel_path) DO UPDATE SET
               page_title       = excluded.page_title,
               mtime            = excluded.mtime,
               content_hash     = excluded.content_hash,
               chunker_version  = excluded.chunker_version,
               summary          = NULL,
               summary_status   = ?6,
               summary_attempts = 0",
            params![rel_path, page_title, mtime, hash, CHUNKER_VERSION, SUMMARY_QUEUED_LOW],
        )?;
        let id: i64 = conn.query_row(
            "SELECT id FROM notes WHERE rel_path = ?1",
            params![rel_path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn update_mtime(&self, rel_path: &str, mtime: i64) -> Result<(), IndexerError> {
        self.conn
            .lock()
            .unwrap()
            .execute("UPDATE notes SET mtime = ?1 WHERE rel_path = ?2", params![mtime, rel_path])?;
        Ok(())
    }

    pub fn get_chunk_ids_for_note(&self, note_id: i64) -> Result<HashSet<i64>, IndexerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM chunks WHERE note_id = ?1")?;
        let rows = stmt.query_map(params![note_id], |row| row.get(0))?;
        let mut set = HashSet::new();
        for row in rows {
            set.insert(row?);
        }
        Ok(set)
    }

    pub fn replace_chunks(
        &self,
        note_id: i64,
        chunks: &[(usize, &str, &[f32])],
    ) -> Result<Vec<i64>, IndexerError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM chunks WHERE note_id = ?1", params![note_id])?;
        let mut ids = Vec::with_capacity(chunks.len());
        for (ord, text, emb) in chunks {
            let emb_bytes: Vec<u8> = emb.iter().flat_map(|f| f.to_le_bytes()).collect();
            tx.execute(
                "INSERT INTO chunks (note_id, ord, text, embedding) VALUES (?1, ?2, ?3, ?4)",
                params![note_id, *ord as i64, text, emb_bytes],
            )?;
            ids.push(tx.last_insert_rowid());
        }
        tx.commit()?;
        Ok(ids)
    }

    pub fn delete_note(&self, rel_path: &str) -> Result<(), IndexerError> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM notes WHERE rel_path = ?1", params![rel_path])?;
        Ok(())
    }

    /// Full table scan returning every chunk's (id, note_id, text). Used by
    /// the lexical retrieval pass; at the project's scale (~2.5k chunks /
    /// ~5MB text) this is cheaper than maintaining a parallel in-memory
    /// mirror. Revisit if the corpus grows an order of magnitude.
    pub fn get_all_chunks(&self) -> Result<Vec<ChunkDetail>, IndexerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, note_id, text FROM chunks")?;
        let rows = stmt.query_map([], |row| {
            Ok(ChunkDetail { id: row.get(0)?, note_id: row.get(1)?, text: row.get(2)? })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_chunks_by_ids(&self, ids: &[i64]) -> Result<Vec<ChunkDetail>, IndexerError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT id, note_id, text FROM chunks WHERE id IN ({placeholders})");
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
            Ok(ChunkDetail { id: row.get(0)?, note_id: row.get(1)?, text: row.get(2)? })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_notes_by_ids(&self, ids: &[i64]) -> Result<Vec<NoteDetail>, IndexerError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, page_title, rel_path, summary, summary_status \
             FROM notes WHERE id IN ({placeholders})"
        );
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
            Ok(NoteDetail {
                id: row.get(0)?,
                page_title: row.get(1)?,
                rel_path: row.get(2)?,
                summary: row.get(3)?,
                summary_status: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    // ---- Summary lifecycle methods ----

    pub fn pull_low_priority_candidate(&self) -> Result<Option<i64>, IndexerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id FROM notes
             WHERE summary_status IN (?1, ?2)
               AND summary_attempts < ?3
             ORDER BY id ASC
             LIMIT 1",
        )?;
        // Distinguish "no candidate" (normal) from real DB errors. The old
        // code .ok()'d both, which made the summarizer look idle and silently
        // loop on a broken DB.
        match stmt.query_row(
            params![SUMMARY_NONE, SUMMARY_QUEUED_LOW, SUMMARY_MAX_ATTEMPTS],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_summary_status(&self, note_id: i64, status: i64) -> Result<(), IndexerError> {
        self.conn.lock().unwrap().execute(
            "UPDATE notes SET summary_status = ?1 WHERE id = ?2",
            params![status, note_id],
        )?;
        Ok(())
    }

    pub fn promote_to_high(&self, note_id: i64) -> Result<(), IndexerError> {
        // Only promote if currently NONE or QUEUED_LOW; do not stomp on IN_PROGRESS / OK / FAILED.
        self.conn.lock().unwrap().execute(
            "UPDATE notes SET summary_status = ?1
             WHERE id = ?2 AND summary_status IN (?3, ?4)",
            params![SUMMARY_QUEUED_HIGH, note_id, SUMMARY_NONE, SUMMARY_QUEUED_LOW],
        )?;
        Ok(())
    }

    pub fn save_summary_with_embedding(
        &self,
        note_id: i64,
        summary: &str,
        embedding: Option<&[f32]>,
    ) -> Result<(), IndexerError> {
        // An empty summary (page below the content floor) is stored as NULL,
        // not "". Storing "" would make `requeue_summaries_missing_embedding`
        // re-queue the row on every startup forever (it matches "summary
        // present, embedding absent"); NULL means "summarized, nothing to say".
        let summary_opt: Option<&str> = if summary.is_empty() { None } else { Some(summary) };
        let emb_bytes: Option<Vec<u8>> =
            embedding.map(|e| e.iter().flat_map(|f| f.to_le_bytes()).collect());
        self.conn.lock().unwrap().execute(
            "UPDATE notes
             SET summary = ?1, summary_status = ?2, summary_embedding = ?3
             WHERE id = ?4",
            params![summary_opt, SUMMARY_OK, emb_bytes, note_id],
        )?;
        Ok(())
    }

    /// Mark a note as "summarized, nothing to say": clears any summary text and
    /// embedding, status OK. Used for pages that fail the deterministic
    /// summarizability gate, both at index time and during startup cleanup.
    pub fn mark_summary_unsummarizable(&self, note_id: i64) -> Result<(), IndexerError> {
        self.conn.lock().unwrap().execute(
            "UPDATE notes
             SET summary = NULL, summary_status = ?1, summary_embedding = NULL,
                 summary_attempts = 0
             WHERE id = ?2",
            params![SUMMARY_OK, note_id],
        )?;
        Ok(())
    }

    /// (note_id, rel_path, summary_text) for every note that currently carries a
    /// summary or a summary embedding. Used by the startup pass that scrubs
    /// summaries the current rules would reject (unsummarizable pages) or that
    /// older builds botched (`Empty.`, `""`, …).
    pub fn list_summarized_notes(&self) -> Result<Vec<(i64, String, Option<String>)>, IndexerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, rel_path, summary FROM notes
             WHERE summary IS NOT NULL OR summary_embedding IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Reset a single note for re-summarization at low priority: clears the
    /// stale summary + embedding and zeroes the attempt counter.
    pub fn requeue_summary(&self, note_id: i64) -> Result<(), IndexerError> {
        self.conn.lock().unwrap().execute(
            "UPDATE notes
             SET summary = NULL, summary_status = ?1, summary_embedding = NULL,
                 summary_attempts = 0
             WHERE id = ?2",
            params![SUMMARY_QUEUED_LOW, note_id],
        )?;
        Ok(())
    }

    pub fn clear_summary_embedding(&self, note_id: i64) -> Result<(), IndexerError> {
        self.conn.lock().unwrap().execute(
            "UPDATE notes SET summary_embedding = NULL WHERE id = ?1",
            params![note_id],
        )?;
        Ok(())
    }

    pub fn load_all_summary_embeddings(
        &self,
    ) -> Result<Vec<(i64, [f32; 1024])>, IndexerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, summary_embedding FROM notes WHERE summary_embedding IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            Ok((id, bytes))
        })?;
        let mut out = Vec::new();
        let mut skipped = 0usize;
        for row in rows {
            let (id, bytes) = row?;
            if bytes.len() != 1024 * 4 {
                // A single corrupted row shouldn't take down all retrieval.
                // Skip it; the next summarizer pass will regenerate the
                // embedding (or operator can manually requeue).
                warn!(
                    "note {} has {} summary embedding bytes, expected {} — skipping",
                    id,
                    bytes.len(),
                    1024 * 4
                );
                skipped += 1;
                continue;
            }
            let mut emb = [0f32; 1024];
            for (i, c) in bytes.chunks_exact(4).enumerate() {
                emb[i] = f32::from_le_bytes(c.try_into().unwrap());
            }
            out.push((id, emb));
        }
        if skipped > 0 {
            warn!("load_all_summary_embeddings: skipped {} corrupted rows", skipped);
        }
        Ok(out)
    }

    /// (ok, pending, failed) — counts grouped by terminal vs non-terminal status.
    pub fn count_summary_status(&self) -> Result<(i64, i64, i64), IndexerError> {
        let conn = self.conn.lock().unwrap();
        let total: i64 =
            conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?;
        let ok: i64 = conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE summary_status = ?1",
            params![SUMMARY_OK],
            |r| r.get(0),
        )?;
        let failed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE summary_status = ?1",
            params![SUMMARY_FAILED],
            |r| r.get(0),
        )?;
        Ok((ok, total - ok - failed, failed))
    }

    pub fn record_summary_failure(&self, note_id: i64) -> Result<i64, IndexerError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE notes
             SET summary_attempts = summary_attempts + 1,
                 summary_status   = CASE WHEN summary_attempts + 1 >= ?1 THEN ?2 ELSE ?3 END
             WHERE id = ?4",
            params![SUMMARY_MAX_ATTEMPTS, SUMMARY_FAILED, SUMMARY_NONE, note_id],
        )?;
        let attempts: i64 = conn.query_row(
            "SELECT summary_attempts FROM notes WHERE id = ?1",
            params![note_id],
            |row| row.get(0),
        )?;
        Ok(attempts)
    }

    /// On startup, re-queue anything left as IN_PROGRESS from a prior crash.
    pub fn reset_in_progress(&self) -> Result<usize, IndexerError> {
        let n = self.conn.lock().unwrap().execute(
            "UPDATE notes SET summary_status = ?1 WHERE summary_status = ?2",
            params![SUMMARY_QUEUED_LOW, SUMMARY_IN_PROGRESS],
        )?;
        Ok(n)
    }

    /// Migration helper: rows from earlier versions can have summary_status=OK
    /// but summary_embedding=NULL. Re-queue them so the new summarizer flow
    /// generates an embedding alongside the text.
    pub fn requeue_summaries_missing_embedding(&self) -> Result<usize, IndexerError> {
        let n = self.conn.lock().unwrap().execute(
            "UPDATE notes
             SET summary = NULL,
                 summary_status = ?1,
                 summary_attempts = 0
             WHERE summary_status = ?2
               AND summary IS NOT NULL
               AND summary_embedding IS NULL",
            params![SUMMARY_QUEUED_LOW, SUMMARY_OK],
        )?;
        Ok(n)
    }


    pub fn load_all_embeddings(&self) -> Result<Vec<(i64, [f32; 1024])>, IndexerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, embedding FROM chunks")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            Ok((id, bytes))
        })?;
        let mut out = Vec::new();
        let mut skipped = 0usize;
        for row in rows {
            let (id, bytes) = row?;
            if bytes.len() != 1024 * 4 {
                // Skip corrupted rows rather than failing all retrieval. The
                // next indexer scan will re-embed the parent note on a
                // hash/version mismatch; until then the page is missing from
                // dense retrieval but still findable via lexical.
                warn!(
                    "chunk {} has {} embedding bytes, expected {} — skipping",
                    id,
                    bytes.len(),
                    1024 * 4
                );
                skipped += 1;
                continue;
            }
            let mut emb = [0f32; 1024];
            for (i, chunk) in bytes.chunks_exact(4).enumerate() {
                emb[i] = f32::from_le_bytes(chunk.try_into().unwrap());
            }
            out.push((id, emb));
        }
        if skipped > 0 {
            warn!("load_all_embeddings: skipped {} corrupted rows", skipped);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_memory() -> Store {
        Store::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn roundtrip_note_and_chunks() {
        let store = open_memory();
        let hash = b"aaaabbbbccccddddeeeeffffgggghhhh".to_vec();
        let note_id = store.upsert_note("pages/test.md", "test", 1234, &hash).unwrap();
        assert!(note_id > 0);

        let emb: Vec<f32> = (0..1024).map(|i| i as f32 / 1024.0).collect();
        let emb2: Vec<f32> = (0..1024).map(|i| i as f32 / 512.0).collect();
        let chunks = vec![
            (0usize, "chunk zero", emb.as_slice()),
            (1usize, "chunk one", emb2.as_slice()),
        ];
        let ids = store.replace_chunks(note_id, &chunks).unwrap();
        assert_eq!(ids.len(), 2);

        let loaded = store.load_all_embeddings().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].1[0], 0.0);
        assert!((loaded[1].1[1] - 1.0 / 512.0).abs() < 1e-5);
    }

    #[test]
    fn delete_cascades_chunks() {
        let store = open_memory();
        let hash = vec![0u8; 32];
        let note_id = store.upsert_note("pages/del.md", "del", 0, &hash).unwrap();
        let emb = vec![0f32; 1024];
        store.replace_chunks(note_id, &[(0, "x", &emb)]).unwrap();
        assert_eq!(store.load_all_embeddings().unwrap().len(), 1);

        store.delete_note("pages/del.md").unwrap();
        assert_eq!(store.load_all_embeddings().unwrap().len(), 0);
    }

    #[test]
    fn list_paths_and_get_note() {
        let store = open_memory();
        let hash = vec![0u8; 32];
        store.upsert_note("a.md", "a", 100, &hash).unwrap();
        store.upsert_note("b.md", "b", 200, &hash).unwrap();

        let paths = store.list_paths().unwrap();
        assert!(paths.contains("a.md"));
        assert!(paths.contains("b.md"));

        let row = store.get_note("a.md").unwrap().unwrap();
        assert_eq!(row.mtime, 100);
        assert_eq!(row.chunker_version, CHUNKER_VERSION);
    }
}
