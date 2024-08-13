use log::{info, error};
use crate::query_engine::ServerInformation;


#[derive(Debug)]
struct LlamaFileDef {
    pub filename: String,
    pub filepath: Option<String>,
    pub sha256: String,
    pub download_link: String,
}

const LLM_SERVER_PORT: &str = "8081"; // TODO Remove this magic number
pub async fn llm_init() {
    info!("llm called");

    let lfile = locate_llamafile();

    let lfile:String = lfile.unwrap();
    use std::process::Command;

    // https://github.com/Mozilla-Ocho/llamafile/blob/main/llama.cpp/server/README.md
    let cmd = Command::new("sh")
        .args([ &lfile, "--nobrowser",
            "--port", LLM_SERVER_PORT,
        ])
        .spawn()
        .expect("llm model failed to launch");
}

fn locate_llamafile() -> Option<String> {
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

