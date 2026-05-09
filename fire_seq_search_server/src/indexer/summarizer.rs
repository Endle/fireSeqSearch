use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use log::{error, info};
use tokio::sync::mpsc;

use crate::indexer::chunker::preprocess;
use crate::indexer::store::{Store, SUMMARY_IN_PROGRESS};
use crate::indexer::IndexerHandle;
use crate::llm_backend::{LlmBackend, Message};

const PAGE_BUDGET_CHARS: usize = 8000;
const QUEUE_CAPACITY: usize = 256;
const IDLE_POLL_SECS: u64 = 60;

#[derive(Clone)]
pub struct SummarizerHandle {
    queue_high: mpsc::Sender<i64>,
}

impl SummarizerHandle {
    /// Best-effort: queue a note for high-priority summarization. If the
    /// channel is full or the worker has shut down, drop silently — the
    /// note still has whatever status it had (NONE / QUEUED_LOW / IN_PROGRESS / OK)
    /// so it'll get processed eventually.
    pub fn request_high_priority(&self, note_id: i64) {
        let _ = self.queue_high.try_send(note_id);
    }
}

pub struct Summarizer {
    store: Arc<Store>,
    backend: Arc<LlmBackend>,
    notebook_path: PathBuf,
    rx_high: mpsc::Receiver<i64>,
    handle: IndexerHandle,
}

impl Summarizer {
    pub fn spawn(
        store: Arc<Store>,
        backend: Arc<LlmBackend>,
        notebook_path: PathBuf,
        handle: IndexerHandle,
    ) -> SummarizerHandle {
        let (tx, rx) = mpsc::channel(QUEUE_CAPACITY);
        let s = Self { store, backend, notebook_path, rx_high: rx, handle };
        tokio::spawn(async move { s.run().await });
        SummarizerHandle { queue_high: tx }
    }

    async fn run(mut self) {
        if let Ok(n) = self.store.reset_in_progress() {
            if n > 0 {
                info!("summarizer: reset {} stale IN_PROGRESS rows on startup", n);
            }
        }
        if let Ok(n) = self.store.requeue_summaries_missing_embedding() {
            if n > 0 {
                info!(
                    "summarizer: backfill — requeued {} summaries missing embeddings",
                    n
                );
            }
        }
        loop {
            // 1. High-priority first, non-blocking.
            if let Ok(note_id) = self.rx_high.try_recv() {
                self.process(note_id).await;
                continue;
            }
            // 2. Low-priority backlog.
            match self.store.pull_low_priority_candidate() {
                Ok(Some(note_id)) => {
                    self.process(note_id).await;
                    continue;
                }
                Ok(None) => {} // backlog empty
                Err(e) => {
                    error!("summarizer: pull_low_priority_candidate: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            }
            // 3. Nothing pending; wait for either a high-priority hit or a
            // periodic re-check (in case the indexer added new low-pri rows).
            tokio::select! {
                some = self.rx_high.recv() => match some {
                    Some(note_id) => self.process(note_id).await,
                    None => return, // sender dropped, app shutting down
                },
                _ = tokio::time::sleep(Duration::from_secs(IDLE_POLL_SECS)) => {},
            }
        }
    }

    async fn process(&self, note_id: i64) {
        if let Err(e) = self.store.set_summary_status(note_id, SUMMARY_IN_PROGRESS) {
            error!("summarizer: set IN_PROGRESS for {}: {}", note_id, e);
            return;
        }
        match self.summarize_one(note_id).await {
            Ok((summary, embedding)) => {
                if let Err(e) = self.store.save_summary_with_embedding(
                    note_id,
                    &summary,
                    embedding.as_ref().map(|e| e.as_slice()),
                ) {
                    error!("summarizer: save_summary_with_embedding {}: {}", note_id, e);
                    return;
                }
                if let Some(emb) = embedding {
                    let mut arr = [0f32; 1024];
                    arr.copy_from_slice(&emb);
                    self.handle.summary_vec.write().await.insert(note_id, arr);
                } else {
                    // Empty summary (stub-only page): make sure stale entry, if any, is gone.
                    self.handle.summary_vec.write().await.remove(&note_id);
                }
                info!("summarized note {}: {:?}", note_id, preview(&summary, 80));
            }
            Err(e) => {
                error!("summarizer: summarize {} failed: {}", note_id, e);
                if let Ok(attempts) = self.store.record_summary_failure(note_id) {
                    info!("summarizer: note {} attempts={}", note_id, attempts);
                }
            }
        }
    }

    /// Returns (summary_text, summary_embedding). Embedding is None for stub-only
    /// pages where the summary is empty.
    async fn summarize_one(&self, note_id: i64) -> Result<(String, Option<Vec<f32>>), String> {
        let note = self
            .store
            .get_notes_by_ids(&[note_id])
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .ok_or_else(|| format!("note {} not found", note_id))?;

        let path = self.notebook_path.join(&note.rel_path);
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if raw.trim().is_empty() {
            return Ok((String::new(), None));
        }
        let clean = preprocess(&raw);
        let clipped: String = clean.chars().take(PAGE_BUDGET_CHARS).collect();

        let prompt = build_prompt(&note.page_title, &clipped);
        let messages = vec![Message { role: "user".to_string(), content: prompt }];
        let summary = self
            .backend
            .chat(messages)
            .await
            .map_err(|e| e.to_string())?
            .trim()
            .to_string();

        if summary.is_empty() {
            return Ok((summary, None));
        }

        // Embed the summary so retrieval has a page-level signal in addition
        // to chunk-level cosine matches.
        let mut embeddings = self
            .backend
            .embed(&[summary.clone()])
            .await
            .map_err(|e| e.to_string())?;
        let emb = embeddings
            .pop()
            .ok_or_else(|| "embed returned no vectors".to_string())?;
        if emb.len() != 1024 {
            return Err(format!(
                "summary embedding has {} dims, expected 1024",
                emb.len()
            ));
        }
        Ok((summary, Some(emb)))
    }
}

fn build_prompt(page_title: &str, page: &str) -> String {
    // Keep it a single user message, no CoT, no role-play. The few-shot
    // examples anchor length and style better than "be concise" rules alone.
    format!(
        "You are summarizing pages from a personal Logseq notebook so the user \
can recognize each page at a glance during search.\n\n\
Rules:\n\
- ONE sentence, under 30 words.\n\
- Concrete: name the entities, products, places, or topics actually mentioned.\n\
- No preamble (\"This page is about...\", \"The note discusses...\"). Start with the content.\n\
- If the page is essentially empty or only contains placeholder bullets, return the empty string.\n\
- Output the sentence and nothing else.\n\n\
Examples:\n\n\
Page title: CoffeeMachine\n\
Content: - (refunded) kcup\\n\\t- Keurig K-Compact Single Serve K-Cup Pod Coffee Maker\\n\\t\\t- bought on Amazon and refunded\\n\\t- 6oz = 177 mL\\n- water reservoir cleaning notes\n\
Summary: Keurig K-Compact K-Cup pod coffee maker (bought on Amazon, refunded), with cup-size and reservoir-cleaning notes.\n\n\
Page title: 2024_03_04\n\
Content: - went to mos mos coffee again\\n- the latte was better than last time\\n- TODO try their cold brew\n\
Summary: Daily journal: revisited mos mos coffee, liked the latte; want to try their cold brew.\n\n\
Page title: {title}\n\
Content:\n{page}\n\
Summary:",
        title = page_title,
        page = page,
    )
}

fn preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.replace('\n', " ")
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}…", truncated.replace('\n', " "))
    }
}

