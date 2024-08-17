use log::{info, error};
use crate::query_engine::ServerInformation;
use reqwest;



const LLM_SERVER_PORT: &str = "8081"; // TODO Remove this magic number
pub struct Llm_Engine {
    endpoint: String,
    client: reqwest::Client,
}

impl Llm_Engine {
    pub async fn llm_init() -> Self {
        info!("llm called");

        let lfile = locate_llamafile().await;

        let lfile:String = lfile.unwrap();

        use std::process::{Command, Stdio};
        use std::fs::File;
        let cmd = Command::new("sh")
            .args([ &lfile, "--nobrowser",
                "--port", LLM_SERVER_PORT,
                //">/tmp/llamafile.stdout", "2>/tmp/llamafile.stderr",
            ])
            .stdout(Stdio::from(File::create("/tmp/llamafile.stdout.txt").unwrap()))
            .stderr(Stdio::from(File::create("/tmp/llamafile.stderr.txt").unwrap()))
            .spawn()
            .expect("llm model failed to launch");

        use tokio::time;
        let wait_llm = time::Duration::from_millis(500);
        tokio::time::sleep(wait_llm);

        let endpoint = format!("http://127.0.0.1:{}", LLM_SERVER_PORT).to_string();


        use reqwest::StatusCode;
        loop {
            let resp = reqwest::get(endpoint.to_owned() + "/health").await;
            let resp = match resp {
                Err(e) => {
                    info!("llm not ready ");
                    let wait_llm = time::Duration::from_millis(100);
                    tokio::time::sleep(wait_llm);
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

        let wait_llm = time::Duration::from_millis(50000);
                    tokio::time::sleep(wait_llm);

        info!("llm engine initialized");
        Self {
            endpoint,
            client,
        }
    }
    // use reqwest https://stackoverflow.com/questions/14154753/how-do-i-make-an-http-request-from-rust
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
    use sha256::try_digest;
    let mut lf = LlamaFileDef {
        filename: "mistral-7b-instruct-v0.2.Q4_0.llamafile".to_owned(),
        filepath: None,
        sha256: "1903778f7defd921347b25327ebe5dd902f29417ba524144a8e4f7c32d83dee8".to_owned(),
        download_link: "mistral-7b-instruct-v0.2.Q4_0.llamafile".to_owned(),
    };

    // TODO hack in dev
    let lf_path = "/var/home/lizhenbo/Downloads/mistral-7b-instruct-v0.2.Q4_0.llamafile";
    lf.filepath = Some(  lf_path.to_owned() );
    info!("lf {:?}", &lf);

    let ppath = std::path::Path::new(lf_path);
    //let val = try_digest(ppath).unwrap();
    let val = "1903778f7defd921347b25327ebe5dd902f29417ba524144a8e4f7c32d83dee8";
    if val != lf.sha256 {
        error!("Wrong sha256sum for the model. Quit");
        return None;
    }

    return lf.filepath;

}

