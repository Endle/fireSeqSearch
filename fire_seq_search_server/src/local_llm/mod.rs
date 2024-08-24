use log::{info, error};
use crate::query_engine::ServerInformation;
use reqwest;
use std::collections::HashMap;
use std::collections::VecDeque;



const LLM_SERVER_PORT: &str = "8081"; // TODO Remove this magic number
use std::sync::Arc;
//use std::sync::Mutex;
use tokio::sync::Mutex;

struct JobProcessor {
    done_job: HashMap<String, String>,
    job_queue: VecDeque<DocData>,
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
            self.job_queue.push_back(doc);
        }
    }
}

pub struct LlmEngine {
    endpoint: String,
    client: reqwest::Client,
    job_cache: Arc<Mutex<JobProcessor>>,
    //job_cache :Arc<Mutex<HashMap<String, Option<String> >>>,
}

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiData {
    pub model: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

use tokio::task::yield_now;
use tokio::task;
use crate::query_engine::DocData;
impl LlmEngine {
    pub async fn llm_init() -> Self {
        info!("llm called");

        let lfile = locate_llamafile().await;
        let lfile:String = lfile.unwrap();

        use std::process::{Command, Stdio};
        use std::fs::File;
        let _cmd = Command::new("sh")
            .args([ &lfile, "--nobrowser",
                "--port", LLM_SERVER_PORT,
                //">/tmp/llamafile.stdout", "2>/tmp/llamafile.stderr",
            ])
            .stdout(Stdio::from(File::create("/tmp/llamafile.stdout.txt").unwrap()))
            .stderr(Stdio::from(File::create("/tmp/llamafile.stderr.txt").unwrap()))
            .spawn()
            .expect("llm model failed to launch");

        use tokio::time;
        yield_now().await;
        let wait_llm = time::Duration::from_millis(500);
        tokio::time::sleep(wait_llm).await;
        task::yield_now().await;

        let endpoint = format!("http://127.0.0.1:{}", LLM_SERVER_PORT).to_string();


        use reqwest::StatusCode;
        loop {
            let resp = reqwest::get(endpoint.to_owned() + "/health").await;
            let resp = match resp {
                Err(_e) => {
                    info!("llm not ready");
                    let wait_llm = time::Duration::from_millis(1000);
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
        let mut map = Arc::new(Mutex::new(
                JobProcessor::new()));
        Self {
            endpoint,
            client,
            job_cache: map
        }
    }

    fn build_data(full_text: &str) -> OpenAiData {
        fn build_message(full_text:&str) -> Message {
            Message{
                role: "user".to_owned(),
                content: full_text.to_owned(),
            }
        }
        let mut msgs = Vec::new();
        msgs.push( build_message(full_text) );
        OpenAiData {
            model: "model".to_owned(),
            messages: msgs,
        }
    }
}

    use axum::debug_handler;
impl LlmEngine{
    pub async fn summarize(&self, full_text: &str) -> String {
        info!("summarize called");
        //http://localhost:8080/completion
        let ep = self.endpoint.to_owned() + "/v1/chat/completions";
        let data = Self::build_data(full_text);
        let res = self.client.post(&ep)
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await
            .unwrap();
        info!(" response {:?}", &res);
        let content = res.text().await.unwrap();
        info!(" text {:?}", &content);
        content
            //TODO remove unwrap
    }

    pub async fn post_summarize_job(&self, doc: DocData) {
        //TODO error handler?
        let mut jcache = self.job_cache.lock().await;//.unwrap();
        jcache.add(doc);
        drop(jcache);

        //TODO why can't I call self.poll in this function?
        //yield_now().await;
        //self.poll().await;
    }

    pub async fn call_llm_engine(&self) {
        let mut next_job: Option<DocData> = None;
        let mut jcache = self.job_cache.lock().await;//.unwrap();
        next_job = jcache.job_queue.pop_front();
        drop(jcache);


        let doc = match next_job {
            Some(x) => x,
            None => { return; },
        };
        let title = doc.title.to_owned();
        let summarize_result = self.summarize(&doc.body).await;
        let mut jcache = self.job_cache.lock().await;//.unwrap();
        next_job = jcache.job_queue.pop_front();
        info!("get summarize result {}", &title);
        jcache.done_job.insert(title, summarize_result);
        drop(jcache);

    }

    pub async fn health(&self) -> Result<(), Box<dyn std::error::Error>>  {
        info!("Calling health check");
        let resp = reqwest::get(self.endpoint.to_owned() + "/health")
                .await?
                .headers().to_owned()
                //.status()
                //.text().await?
                ;
        info!("Health check: {:#?}", resp);
        Ok(())
    }
}

#[derive(Debug)]
struct LlamaFileDef {
    pub filename: String,
    pub filepath: Option<String>,
    pub sha256: String,
    pub download_link: String,
}


async fn locate_llamafile() -> Option<String> {
    // TODO
    let mut lf = LlamaFileDef {
        filename: "mistral-7b-instruct-v0.2.Q4_0.llamafile".to_owned(),
        filepath: None,
        sha256: "1903778f7defd921347b25327ebe5dd902f29417ba524144a8e4f7c32d83dee8".to_owned(),
        download_link: "mistral-7b-instruct-v0.2.Q4_0.llamafile".to_owned(),
    };

    // TODO hack in dev
    //let lf_path = "/var/home/lizhenbo/Downloads/mistral-7b-instruct-v0.2.Q4_0.llamafile";
    let lf_base = "/Users/zhenboli/.llamafile/";
    let lf_path = lf_base.to_owned() + &lf.filename;
    lf.filepath = Some(  lf_path.to_owned() );
    info!("lf {:?}", &lf);

    let ppath = std::path::Path::new(&lf_path);
    //let val = sha256::try_digest(ppath).unwrap();
    let val = "1903778f7defd921347b25327ebe5dd902f29417ba524144a8e4f7c32d83dee8";
    if val != lf.sha256 {
        error!("Wrong sha256sum for the model. Quit");
        return None;
    }

    return lf.filepath;

}

