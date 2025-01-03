use log::{info, error};
use crate::query_engine::DocData;

use std::collections::HashMap;
use std::collections::VecDeque;
use std::process::{Command, Stdio};
use std::fs::File;

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::yield_now;
use tokio::task;
use tokio;

use std::borrow::Cow;
use std::borrow::Cow::Borrowed;


//#[cfg(feature = "llm")]
use {
    reqwest,
    reqwest::StatusCode,
    shellexpand::tilde,

    serde_derive::Deserialize,
    serde_derive::Serialize,
};



// TODO Allow user to set prompt, instead of hard-coded in code
const HARD_CODED_PROMPT_STR: &'static str = r##"
You are a seasoned summary expert, capable of condensing and summarizing given articles, papers, or posts, accurately conveying the main idea to make the content easier to understand.

You place great emphasis on user experience, never adding irrelevant content like "Summary," "The summary is as follows," "Original text," "You can check the original text if interested," or "Original link." Your summaries always convey the core information directly.

You are adept at handling various large, small, and even chaotic text content, always accurately extracting key information and summarizing the core content globally to make it easier to understand.

=== Below is the article ===

"##;

// Generated by https://transform.tools/json-to-rust-serde
#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiData {
    pub model: String,
    pub messages: Vec<Message>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlamaResponse {
    pub choices: Vec<Choice>,
    pub created: i64,
    pub id: String,
    pub model: String,
    pub object: String,
    pub usage: Usage,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub finish_reason: String,
    pub index: i64,
    pub message: Message,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub content: String,
    pub role: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub completion_tokens: i64,
    pub prompt_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthCheck {
    pub slots_idle: i64,
    pub slots_processing: i64,
    pub status: String,
}

// End genereated

const LLM_SERVER_PORT: &str = "8081"; // TODO Remove this magic number


#[derive(Debug)]
pub struct LlmJob {
    pub title: String,
    pub body : String,
    pub time : std::time::Instant, /* 16 bytes */
}

struct JobProcessor {
    done_job: HashMap<String, String>,
    job_queue: VecDeque<LlmJob>,
}

impl JobProcessor {
    pub fn new() -> Self {
        JobProcessor {
            done_job: HashMap::new(),
            job_queue: VecDeque::new(),
        }
    }
    pub fn add(&mut self, doc:DocData) {
        let title: &str = &doc.title;
        info!("Job posted for {}", &title);
        if !self.done_job.contains_key(title) {
            let job: LlmJob = LlmJob {
                title: doc.title,
                body:  doc.body,
                time:  std::time::Instant::now(),
            };
            self.job_queue.push_back(job);
        }
    }
}

use crate::ServerInformation;

pub struct LlmEngine {
    endpoint: String,
    client: reqwest::Client,
    job_cache: Arc<Mutex<JobProcessor>>,
    server_info: Arc<ServerInformation>,
}



impl LlmEngine {
    pub async fn llm_init(server_info: Arc<ServerInformation>) -> Self {
        info!("llm called");

        let lfile = locate_llamafile().await;
        let lfile:String = lfile.unwrap();

        let _cmd = Command::new("nice")
            .args([ "-n", "19",
                &lfile, "--nobrowser",
                "--port", LLM_SERVER_PORT,
            ])
            .stdout(Stdio::from(File::create("/tmp/llamafile.stdout.txt").unwrap()))
            .stderr(Stdio::from(File::create("/tmp/llamafile.stderr.txt").unwrap()))
            .spawn()
            .expect("llm model failed to launch");

        yield_now().await;
        let wait_llm = tokio::time::Duration::from_millis(500);
        tokio::time::sleep(wait_llm).await;
        task::yield_now().await;

        let endpoint = format!("http://127.0.0.1:{}", LLM_SERVER_PORT).to_string();

        loop {
            let resp = reqwest::get(endpoint.to_owned() + "/health").await;
            let resp = match resp {
                Err(_e) => {
                    info!("llm not ready");
                    let wait_llm = tokio::time::Duration::from_millis(1000);
                    tokio::time::sleep(wait_llm).await;
                    task::yield_now().await;
                    continue;
                },
                Ok(r) => r,
            };
            if resp.status() != StatusCode::from_u16(200).unwrap() {
                info!("endpoint failed");
                //TODO error handling
            }
            break;
        }

        let client = reqwest::Client::new();

        info!("llm engine initialized");
        let map = Arc::new(Mutex::new(
                JobProcessor::new()));
        Self {
            endpoint,
            client,
            job_cache: map,
            server_info,
        }
    }

    fn build_data(full_text: Cow<'_, str>) -> OpenAiData {

        fn build_message(chat:String) -> Message {
            Message{
                role: "user".to_owned(),
                content: chat,
            }
        }
        let mut msgs = Vec::new();

        let prompt_string = &HARD_CODED_PROMPT_STR;
        let mut chat_text = prompt_string.to_string();
        chat_text += &full_text;
        msgs.push( build_message(chat_text) );

        OpenAiData {
            model: "model".to_owned(),
            messages: msgs,
        }
    }
}

impl LlmEngine{
    pub async fn summarize(&self, full_text: &str) -> String {
        //http://localhost:8080/completion
        let ep = self.endpoint.to_owned() + "/v1/chat/completions";
        let data = Self::build_data( Borrowed(full_text) );
        let res = self.client.post(&ep)
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await
            .unwrap();
        let content = res.text().await.unwrap();
        let parsed: LlamaResponse = serde_json::from_str(&content).unwrap();
        let v = parsed.choices;
        let v0 = v.into_iter().next().unwrap();
        v0.message.content
        //TODO remove unwrap
    }

    pub async fn post_summarize_job(&self, doc: DocData) {
        //TODO error handler?
        let mut jcache = self.job_cache.lock().await;//.unwrap();
        jcache.add(doc);
        drop(jcache);
    }

    pub async fn call_llm_engine(&self) {
        let health = self.health().await.unwrap();
        if health.slots_idle == 0 {
            info!("No valid slot, continue");
            return;
        }

        let next_job: Option<LlmJob>;

        let mut jcache = self.job_cache.lock().await;//.unwrap();
        next_job = jcache.job_queue.pop_front();
        drop(jcache);

        let doc = match next_job {
            Some(x) => x,
            None => { return; },
        };

        let title = doc.title.to_owned();

        let jcache = self.job_cache.lock().await;
        if jcache.done_job.contains_key(&title) {
            return;
        }
        drop(jcache);

        let waiting_time = doc.time.elapsed().as_secs();
        let allowed_wait = self.server_info.llm_max_waiting_time;
        if waiting_time > allowed_wait {
            info!("Waiting for {} for {} seconds, discard",
                &title, waiting_time);
            return;
        }


        info!("Start summarize job:  {}", &title);
        let summarize_result = self.summarize(&doc.body).await;
        info!("Finished summarize job:  {}", &title);

        let mut jcache = self.job_cache.lock().await;
        jcache.done_job.insert(title, summarize_result);
        drop(jcache);
    }

    pub async fn quick_fetch(&self, title: &str) -> Option<String> {
        let jcache = self.job_cache.lock().await;
        return jcache.done_job.get(title).cloned();
    }

    pub async fn get_llm_done_list(&self) -> Vec<String> {
        let mut r = Vec::new();
        let jcache = self.job_cache.lock().await;
        for (title, _text) in &jcache.done_job {
            r.push(title.to_owned());
        }
        return r;
    }

    pub async fn health(&self) -> Result<HealthCheck, Box<dyn std::error::Error>>  {
        let res = self.client.get(self.endpoint.to_owned() + "/health")
            .send()
            .await
            .unwrap();
        let content = res.text().await.unwrap();
        let parsed: HealthCheck = serde_json::from_str(&content).unwrap();
        Ok(parsed)
    }
}

#[derive(Debug)]
struct LlamaFileDef {
    pub filename: String,
    pub filepath: Option<String>,
    pub sha256: String,
    #[allow(dead_code)] /* TODO rethink if we want auto download 2024 Sep 21 */
    pub download_link: String,
}


async fn locate_llamafile() -> Option<String> {
    let mut lf = LlamaFileDef {
        filename: "mistral-7b-instruct-v0.2.Q4_0.llamafile".to_owned(),
        filepath: None,
        sha256: "1903778f7defd921347b25327ebe5dd902f29417ba524144a8e4f7c32d83dee8".to_owned(),
        download_link: "mistral-7b-instruct-v0.2.Q4_0.llamafile".to_owned(),
    };

    let lf_base = tilde("~/.llamafile/");
    let lf_path = lf_base.to_string() + &lf.filename;
    lf.filepath = Some(  lf_path.to_owned() );
    info!("lf {:?}", &lf);

    let _ppath = std::path::Path::new(&lf_path);
    //let val = sha256::try_digest(ppath).unwrap();
    let val = "1903778f7defd921347b25327ebe5dd902f29417ba524144a8e4f7c32d83dee8";
    if val != lf.sha256 {
        error!("Wrong sha256sum for the model. Quit");
        return None;
    }

    return lf.filepath;

}

