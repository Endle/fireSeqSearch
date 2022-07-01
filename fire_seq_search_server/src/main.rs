use warp::Filter;


use tantivy::schema::*;
use tantivy::ReloadPolicy;

use std::fs;
use serde_json;
use serde::Serialize;

use log::{info,debug,warn,error};
// use log::LevelFilter;
use clap::{Command,arg};


#[derive(Debug, Clone, Serialize)]
struct ServerInformation {
    notebook_path: String,
    notebook_name: String,
    schema: tantivy::schema::Schema,
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .format_target(false)
        .init();

    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Server for fireSeqSearch: hosting logseq notebooks at 127.0.0.1")
        .arg(arg!(--notebook_path <VALUE>))
        .arg(arg!(--notebook_name <VALUE>).required(false))
        .get_matches();


    let server_info: ServerInformation = build_server_info(&matches);
    let index = indexing_documents(&server_info);
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

fn build_schema() -> tantivy::schema::Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT);
    let schema = schema_builder.build();
    schema
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
        schema: build_schema()
    }
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

fn indexing_documents(server_info: &ServerInformation) -> tantivy::Index {
    // TODO remove these unwrap()

    // let index_path = TempDir::new().unwrap();
    // info!("Using temporary directory {:?}", index_path);
    // let (schema, title,body) = build_schema_dev();

    let path: &str = &server_info.notebook_path;
    let schema = &server_info.schema;
    let index = tantivy::Index::create_in_ram(schema.clone());

    let mut index_writer = index.writer(50_000_000).unwrap();

    // I should remove the unwrap and convert it into map
    let path = path.to_owned() + "/pages";
    let notebooks = fs::read_dir(path).unwrap();

    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    for note in notebooks {
        let note : std::fs::DirEntry = note.unwrap();


        match read_md_file(&note) {
            Some((note_title, contents)) => {
                debug!("Length: {}", contents.len());

                let mut doc = Document::default();
                doc.add_text(title, note_title);
                doc.add_text(body, contents);
                index_writer.add_document(doc);
            },
            None => (
                warn!("Skip file {:?}", note)
                )
        };
    }

    index_writer.commit().unwrap();
    index
}

fn read_md_file(note: &std::fs::DirEntry) -> Option<(String, String)> {
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
