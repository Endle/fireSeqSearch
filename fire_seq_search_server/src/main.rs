use std::net::SocketAddr;

use warp::Filter;
use log::info;
use fire_seq_search_server::query_engine::{QueryEngine, ServerInformation};
use fire_seq_search_server::local_llm::LlmEngine;


use clap::Parser;

#[derive(Parser)]
#[command(author, version)]
#[command(about = "Server for fireSeqSearch: hosting logseq notebooks at 127.0.0.1",
    long_about = None)]
struct Cli{
    #[arg(long="notebook_path")]
    notebook_path: String,
    #[arg(long="notebook_name")]
    notebook_name: Option<String>,

    #[arg(long, default_value_t = false)]
    parse_pdf_links: bool,

    #[arg(long, default_value_t = false)]
    obsidian_md: bool,

    #[arg(long,default_value_t = false)]
    enable_journal_query: bool,

    #[arg(long,default_value_t = false)]
    exclude_zotero_items: bool,

    #[arg(long,default_value_t = 10, value_name="HITS")]
    show_top_hits: usize,

/*
        This is really an arbitrary limit.
        https://stackoverflow.com/a/33758289/1166518
        It doesn't mean the width limit of output,
            but a threshold between short paragraph and long paragraph
 */
    #[arg(long,default_value_t = 120*2, value_name="LEN")]
    show_summary_single_line_chars_limit: usize,

    #[arg(long="host")]
    host: Option<String>,
}

use tokio::task;

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .format_target(false)
        .init();

    let llm = task::spawn( async { LlmEngine::llm_init().await });
    //let llm = llm.await.unwrap();
    //llm.summarize("hi my friend").await;

    info!("main thread running");
    let matches = Cli::parse();
    let host: String = matches.host.clone().unwrap_or_else(|| "127.0.0.1:3030".to_string());
    let host: SocketAddr = host.parse().unwrap_or_else(
        |_| panic!("Invalid host: {}", host)
    );
    let server_info: ServerInformation = build_server_info(matches);
    let engine = QueryEngine::construct(server_info);


    let engine_arc = std::sync::Arc::new(engine);
    let arc_for_query = engine_arc.clone();
    let call_query = warp::path!("query" / String)
        .map(move |name| {
            fire_seq_search_server::http_client::endpoints::query(
                name, arc_for_query.clone() )
        });

    let arc_for_server_info = engine_arc.clone();
    let get_server_info = warp::path("server_info")
        .map(move ||
                 fire_seq_search_server::http_client::endpoints::get_server_info(
                     arc_for_server_info.clone()
                 ));

    let arc_for_wordcloud = engine_arc.clone();
    let create_word_cloud = warp::path("wordcloud")
        .map(move || {
            let div = fire_seq_search_server::http_client::endpoints::generate_word_cloud(
                arc_for_wordcloud.clone()
            );
            warp::http::Response::builder()
                .header("content-type", "text/html; charset=utf-8")
                .body(div)
                // .status(warp::http::StatusCode::OK)
        });

    let routes = warp::get().and(
        call_query
            .or(get_server_info)
            .or(create_word_cloud)
    );
    warp::serve(routes)
        .run(host)
        .await;



}





fn build_server_info(args: Cli) -> ServerInformation {
    let notebook_name = match args.notebook_name {
        Some(x) => x.to_string(),
        None => {
            let chunks: Vec<&str> = args.notebook_path.split('/').collect();
            let guess: &str = *chunks.last().unwrap();
            info!("fire_seq_search guess the notebook name is {}", guess);
            String::from(guess)
        }
    };
    ServerInformation{
        notebook_path: args.notebook_path,
        notebook_name,
        enable_journal_query: args.enable_journal_query,
        show_top_hits: args.show_top_hits,
        show_summary_single_line_chars_limit:
            args.show_summary_single_line_chars_limit,
        parse_pdf_links: args.parse_pdf_links,
        exclude_zotero_items:args.exclude_zotero_items,
        obsidian_md: args.obsidian_md,
        convert_underline_hierarchy: true,
    }
}



