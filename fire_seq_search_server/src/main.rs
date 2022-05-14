use warp::Filter;


use tantivy::schema::*;
use tantivy::Index;
use tantivy::ReloadPolicy;

use std::fs;

use log::{info,debug};
use log::LevelFilter;
use clap::{Command,arg};
// use clap::Parser;
// #[derive(Parser, Debug)]
// #[clap(author, version, about, long_about = None)]
// struct Args {
//     #[clap(short, long)]
//     notebook_path: String,
// }
//

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .format_target(false)
        .filter_level(LevelFilter::Info)
        .init();

    let matches = Command::new("fire_seq_search_server")
        .version("0.0.1")
        .author("Zhenbo Li")
        .about("Server for fireSeqSearch: hosting logseq notebooks at 127.0.0.1")
        .arg(arg!(--notebook_path <VALUE>))
        .get_matches();

    // let args = Args::parse();
    let logseq_path: &str = matches.value_of("notebook_path").unwrap();
    let index = indexing_documents(&logseq_path);
    let (reader, query_parser) = build_reader_parser(&index);

    let call_query = warp::path!("query" / String)
        .map(move |name| query(name, &reader, &query_parser) );
    // I admit I don't know why rust closure asks me to use move.
    // I know nothing about rust

    let get_server_info = warp::path("server_info")
        .map(|| server_info() );

    let routes = warp::get().and(
        call_query
            .or(get_server_info)
    );
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;
}

fn server_info() -> String {
    info!("get_server_info called");
    // let json = serde_json::to_string(&result).unwrap();
    let json = String::from("server info stub");
    json
}

// TODO No Chinese support yet
fn query(term: String, reader: &tantivy::IndexReader, query_parser: &tantivy::query::QueryParser)
    -> String {
    // TODO HACKY CONVERT
    let term = term.replace("%20", " ");

    info!("Searching {}", term);
    let searcher = reader.searcher();



    let query = query_parser.parse_query(&term).unwrap();
    let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(10))
        .unwrap();
    let (schema, _title, _body) = build_schema_dev();
    let mut result = Vec::new();
    for (_score, doc_address) in top_docs {
        // _score = 1;
        info!("Found doc addr {:?}, score {}", &doc_address, &_score);
        let retrieved_doc = searcher.doc(doc_address).unwrap();
        result.push(schema.to_json(&retrieved_doc));
        // println!("{}", schema.to_json(&retrieved_doc));
    }
    //INVALID!
    // result.join(",")
    let json = serde_json::to_string(&result).unwrap();
    info!("Search result {}", &json);
    json
    // result[0].clone()
}

fn build_reader_parser(index: &tantivy::Index) -> (tantivy::IndexReader, tantivy::query::QueryParser) {
    // TODO remove these unwrap()
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into().unwrap();
    let (_schema, title,body) = build_schema_dev();
    let query_parser = tantivy::query::QueryParser::for_index(index, vec![title, body]);
    (reader, query_parser)
}

fn indexing_documents(path: &str) -> tantivy::Index {
    // TODO remove these unwrap()

    // let index_path = TempDir::new().unwrap();
    // info!("Using temporary directory {:?}", index_path);
    let (schema, title,body) = build_schema_dev();
    let index = Index::create_in_ram(schema.clone());

    let mut index_writer = index.writer(50_000_000).unwrap();

    // I should remove the unwrap and convert it into map
    let path = path.to_owned() + "/pages";
    let notebooks = fs::read_dir(path).unwrap();



    for note in notebooks {
        let note:std::fs::DirEntry = note.unwrap();
        let note = note.path();
        // println!("{:?}", &note);
        let note_title = note.file_stem().unwrap().to_str().unwrap();
        debug!("note title: {}", &note_title);

        let contents :String = fs::read_to_string(&note)
            .expect("Something went wrong reading the file");
        debug!("Length: {}", contents.len());

        let mut doc = Document::default();
        doc.add_text(title, note_title);
        doc.add_text(body, contents);
        index_writer.add_document(doc);
    }

    index_writer.commit().unwrap();
    index
}


fn build_schema_dev() -> (tantivy::schema::Schema, tantivy::schema::Field, tantivy::schema::Field) {
    // TODO currently for dev, a bit hacky
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT);
    let schema = schema_builder.build();
    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();
    (schema, title, body)
}
