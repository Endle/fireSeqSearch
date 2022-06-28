use warp::Filter;


use tantivy::schema::*;
use tantivy::{Index,doc};
use tantivy::ReloadPolicy;
use cang_jie::CangJieTokenizer;

use std::fs;
use serde_json;
use serde::Serialize;

use log::{info,debug,warn,error};
// use log::LevelFilter;
use clap::{Command,arg};
use urlencoding::decode;

#[derive(Debug, Clone, Serialize)]
struct ServerInformation {
    notebook_path: String,
    notebook_name: String,
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .format_target(false)
        // .filter_level(LevelFilter::Info)
        .init();

    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Server for fireSeqSearch: hosting logseq notebooks at 127.0.0.1")
        .arg(arg!(--notebook_path <VALUE>))
        .arg(arg!(--notebook_name <VALUE>).required(false))
        .get_matches();


    let server_info: ServerInformation = build_server_info(&matches);

    let index = indexing_documents(&server_info.notebook_path);
    let (reader, query_parser) = build_reader_parser(&index);

    let call_query = warp::path!("query" / String)
        .map(move |name| query(name, &reader, &query_parser) );
    // I admit I don't know why rust closure asks me to use move.
    // I know nothing about rust

    let get_server_info = warp::path("server_info")
        .map(move || serde_json::to_string( &server_info ).unwrap() );

    let routes = warp::get().and(
        call_query
            .or(get_server_info)
    );
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;
}

fn build_server_info(args: &clap::ArgMatches) -> ServerInformation {
    let notebook_path = match args.value_of("notebook_path") {
        Some(x) => x.to_string(),
        None => panic!("notebook_path has to be specified!")
    };
    let notebook_name = match args.value_of("notebook_name") {
        Some(x) => x.to_string(),
        None => {
            let chunks: Vec<&str> = notebook_path.split("/").collect();
            let guess: &str = *chunks.last().unwrap();
            info!("fire_seq_search guess the notebook name is {}", guess);
            String::from(guess)
        }
    };
    ServerInformation{
        notebook_path,
        notebook_name,
    }
}


fn decode_cjk_str(original: String) -> Vec<String> {
    let mut result = Vec::new();
    for s in original.split(' ') {
        let t = decode(s).expect("UTF-8");
        debug!("Decode {}  ->   {}", s, t);
        result.push(String::from(t));
    }

    result
}


// TODO No Chinese support yet
fn query(term: String, reader: &tantivy::IndexReader, query_parser: &tantivy::query::QueryParser)
    -> String {

    debug!("Original Search term {}", term);

    let term = term.replace("%20", " ");
    let term_vec = decode_cjk_str(term);
    let term = term_vec.join(" ");

    info!("Searching {}", term);
    let searcher = reader.searcher();



    let query = query_parser.parse_query(&term).unwrap();
    let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(10))
        .unwrap();
    let (schema, _title, _body, _tokenizer) = build_schema_dev();
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
    let (_schema, title,body, _tokenizer) = build_schema_dev();
    let query_parser = tantivy::query::QueryParser::for_index(index, vec![title, body]);
    (reader, query_parser)
}

fn indexing_documents(path: &str) -> tantivy::Index {
    // TODO remove these unwrap()

    // let index_path = TempDir::new().unwrap();
    // info!("Using temporary directory {:?}", index_path);

    let mut schema_builder = SchemaBuilder::default();
    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer(cang_jie::CANG_JIE) // Set custom tokenizer
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(text_indexing)
        .set_stored();
    let tokenizer:CangJieTokenizer = CangJieTokenizer {
        worker: std::sync::Arc::new(jieba_rs::Jieba::empty()), // empty dictionary
        option: cang_jie::TokenizerOption::Unicode,
    };


    // let (schema, title,body, tokenizer) = build_schema_dev();


    let title = schema_builder.add_text_field("title", text_options.clone());
    let body = schema_builder.add_text_field("body", text_options);

    let schema = schema_builder.build();

    let index = Index::create_in_ram(schema.clone());
    index.tokenizers().register(cang_jie::CANG_JIE, tokenizer);

    let mut index_writer = index.writer(50_000_000).unwrap();

    // I should remove the unwrap and convert it into map
    let path = path.to_owned() + "/pages";
    let notebooks = fs::read_dir(path).unwrap();



    for note in notebooks {
        let note : std::fs::DirEntry = note.unwrap();
        let note_option = read_md_file(note);

        match note_option {
            Some((note_title, contents)) => {
                debug!("Length: {}", contents.len());

                let mut doc = Document::default();
                // doc.add_text(title, note_title);
                // doc.add_text(body, contents);
                index_writer.add_document(
                    doc!{ title => note_title, body => contents}
                );
            },
            None => ()
        };
    }

    index_writer.commit().unwrap();
    index
}

fn read_md_file(note: std::fs::DirEntry) -> Option<(String, String)> {

    if let Ok(file_type) = note.file_type() {
        // Now let's show our entry's file type!
        debug!("{:?}: {:?}", note.path(), file_type);
        if file_type.is_dir() {
            debug!("{:?} is a directory, skipping", note.path());
            return None;
        }
    } else {
        warn!("Couldn't get file type for {:?}", note.path());
        return None;
    }

    let note_path = note.path();
    let note_title = match note_path.file_stem() {
        Some(osstr) => osstr.to_str().unwrap(),
        None => {
            error!("Couldn't get file_stem for {:?}", note.path());
            return None;
        }
    };
    debug!("note title: {}", &note_title);

    let contents : String = match fs::read_to_string(&note_path) {
        Ok(c) => c,
        Err(e) => {
            if note_title.to_lowercase() == ".ds_store" {
                debug!("Ignore .DS_Store for mac");
            } else {
                error!("Error({:?}) when reading the file {:?}", e, note_path);
            }
            return None;
        }
    };

    Some((note_title.to_string(),contents))
}


fn build_schema_dev() -> (tantivy::schema::Schema,
                          tantivy::schema::Field,
                          tantivy::schema::Field,
                          CangJieTokenizer) {
    // TODO currently for dev, a bit hacky
    // let mut schema_builder = Schema::builder();
    let mut schema_builder = SchemaBuilder::default();
    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer(cang_jie::CANG_JIE) // Set custom tokenizer
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(text_indexing)
        .set_stored();
    let tokenizer:CangJieTokenizer = CangJieTokenizer {
        worker: std::sync::Arc::new(jieba_rs::Jieba::empty()), // empty dictionary
        option: cang_jie::TokenizerOption::Unicode,
    };
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT);
    let schema = schema_builder.build();
    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();
    (schema, title, body, tokenizer)
}
