use log::info;
use crate::query_engine::ServerInformation;


#[derive(Debug)]
struct LlamaFileDef {
    pub filename: String,
    pub filepath: Option<String>,
    pub sha256: String,
    pub download_link: String,
}
pub async fn llm_init() {
    info!("llm called");

    let lfile = locate_llamafile();
}

fn locate_llamafile() -> String {
    let mut lf = LlamaFileDef {
        filename: "mistral-7b-instruct-v0.2.Q4_0.llamafile".to_owned(),
        filepath: None,
        sha256: "1903778f7defd921347b25327ebe5dd902f29417ba524144a8e4f7c32d83dee8".to_owned(),
        download_link: "mistral-7b-instruct-v0.2.Q4_0.llamafile".to_owned(),
    };

    // TODO hack in dev
    lf.filepath = Some( "/var/home/lizhenbo/Downloads/mistral-7b-instruct-v0.2.Q4_0.llamafile".to_owned());

    info!("lf {:?}", &lf);

    return String::default();

}

