use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::indexer::IndexerError;

pub const CHUNKER_VERSION: i64 = 1;

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
               id              INTEGER PRIMARY KEY,
               rel_path        TEXT    NOT NULL UNIQUE,
               page_title      TEXT    NOT NULL,
               mtime           INTEGER NOT NULL,
               content_hash    BLOB    NOT NULL,
               chunker_version INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS chunks (
               id        INTEGER PRIMARY KEY,
               note_id   INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
               ord       INTEGER NOT NULL,
               text      TEXT    NOT NULL,
               embedding BLOB    NOT NULL,
               UNIQUE(note_id, ord)
             );
             CREATE INDEX IF NOT EXISTS idx_chunks_note_id ON chunks(note_id);",
        )?;
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
        conn.execute(
            "INSERT INTO notes (rel_path, page_title, mtime, content_hash, chunker_version)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(rel_path) DO UPDATE SET
               page_title      = excluded.page_title,
               mtime           = excluded.mtime,
               content_hash    = excluded.content_hash,
               chunker_version = excluded.chunker_version",
            params![rel_path, page_title, mtime, hash, CHUNKER_VERSION],
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

    pub fn load_all_embeddings(&self) -> Result<Vec<(i64, [f32; 1024])>, IndexerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, embedding FROM chunks")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            Ok((id, bytes))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, bytes) = row?;
            if bytes.len() != 1024 * 4 {
                return Err(IndexerError::Decode(format!(
                    "chunk {} has {} embedding bytes, expected {}",
                    id,
                    bytes.len(),
                    1024 * 4
                )));
            }
            let mut emb = [0f32; 1024];
            for (i, chunk) in bytes.chunks_exact(4).enumerate() {
                emb[i] = f32::from_le_bytes(chunk.try_into().unwrap());
            }
            out.push((id, emb));
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
