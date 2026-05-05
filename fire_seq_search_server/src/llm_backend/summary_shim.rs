// THROWAWAY MODULE.
// Preserves the existing /summarize and /llm_done_list endpoints by adapting
// the legacy LlmEngine API onto the new LlmBackend. Deleted in phase 4 when
// /ask replaces /summarize.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use log::{error, info};
use tokio::sync::Mutex;

use crate::query_engine::DocData;

use super::{LlmBackend, Message};

const HARD_CODED_PROMPT_STR: &str = r##"
You are a seasoned summary expert, capable of condensing and summarizing given articles, papers, or posts, accurately conveying the main idea to make the content easier to understand.

You place great emphasis on user experience, never adding irrelevant content like "Summary," "The summary is as follows," "Original text," "You can check the original text if interested," or "Original link." Your summaries always convey the core information directly.

You are adept at handling various large, small, and even chaotic text content, always accurately extracting key information and summarizing the core content globally to make it easier to understand.

=== Below is the article ===

"##;

#[derive(Debug)]
struct LlmJob {
    title: String,
    body: String,
    time: Instant,
}

struct JobProcessor {
    done_job: HashMap<String, String>,
    job_queue: VecDeque<LlmJob>,
}

impl JobProcessor {
    fn new() -> Self {
        Self {
            done_job: HashMap::new(),
            job_queue: VecDeque::new(),
        }
    }

    fn add(&mut self, doc: DocData) {
        if self.done_job.contains_key(&doc.title) {
            return;
        }
        info!("Job posted for {}", &doc.title);
        self.job_queue.push_back(LlmJob {
            title: doc.title,
            body: doc.body,
            time: Instant::now(),
        });
    }
}

pub struct SummaryEngine {
    backend: Arc<LlmBackend>,
    job_cache: Arc<Mutex<JobProcessor>>,
    max_waiting_time: u64,
}

impl SummaryEngine {
    pub fn new(backend: Arc<LlmBackend>, max_waiting_time: u64) -> Self {
        Self {
            backend,
            job_cache: Arc::new(Mutex::new(JobProcessor::new())),
            max_waiting_time,
        }
    }

    pub fn child_pids(&self) -> Vec<u32> {
        self.backend.child_pids()
    }

    pub async fn post_summarize_job(&self, doc: DocData) {
        let mut jcache = self.job_cache.lock().await;
        jcache.add(doc);
    }

    pub async fn quick_fetch(&self, title: &str) -> Option<String> {
        let jcache = self.job_cache.lock().await;
        jcache.done_job.get(title).cloned()
    }

    pub async fn get_llm_done_list(&self) -> Vec<String> {
        let jcache = self.job_cache.lock().await;
        jcache.done_job.keys().cloned().collect()
    }

    pub async fn call_llm_engine(&self) {
        let job = {
            let mut jcache = self.job_cache.lock().await;
            match jcache.job_queue.pop_front() {
                Some(j) => j,
                None => return,
            }
        };

        {
            let jcache = self.job_cache.lock().await;
            if jcache.done_job.contains_key(&job.title) {
                return;
            }
        }

        let waiting_time = job.time.elapsed().as_secs();
        if waiting_time > self.max_waiting_time {
            info!(
                "Discarding stale summarize job for {} ({}s old)",
                &job.title, waiting_time
            );
            return;
        }

        info!("Start summarize job: {}", &job.title);
        let prompt = format!("{}{}", HARD_CODED_PROMPT_STR, job.body);
        let messages = vec![Message {
            role: "user".to_owned(),
            content: prompt,
        }];

        match self.backend.chat(messages).await {
            Ok(content) => {
                info!("Finished summarize job: {}", &job.title);
                let mut jcache = self.job_cache.lock().await;
                jcache.done_job.insert(job.title, content);
            }
            Err(e) => {
                error!("Summarize job {} failed: {}", &job.title, e);
            }
        }
    }
}
