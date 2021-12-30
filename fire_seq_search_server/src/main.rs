use warp::Filter;

#[macro_use]
extern crate tantivy;
use tantivy::schema::*;
use tantivy::Index;
use tantivy::ReloadPolicy;

use std::{fs, io};

use log::{info};
use log::LevelFilter;

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .format_target(false)
        .filter_level(LevelFilter::Info)
        .init();


    let logseq_path = "/home/lizhenbo/src/logseq_notebook";
    let index = indexing_documents(logseq_path);
    let (reader, query_parser) = build_reader_parser(&index);
    let _searcher = build_searcher(&reader, &query_parser);

    let search = warp::path!("query" / String)
        .map(|name| query(name) );

    warp::serve(search)
        .run(([127, 0, 0, 1], 3030))
        .await;
}


fn build_reader_parser(index: &tantivy::Index) -> (tantivy::IndexReader, tantivy::query::QueryParser) {
    // TODO remove these unwrap()
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into().unwrap();
    let (schema, title,body) = build_schema_dev();
    let query_parser = tantivy::query::QueryParser::for_index(index, vec![title, body]);
    (reader, query_parser)
}

fn build_searcher(reader: &tantivy::IndexReader, query_parser: &tantivy::query::QueryParser) -> i32 {
    let searcher = reader.searcher();

    let query = query_parser.parse_query("softmax").unwrap();
    let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(10))
        .unwrap();
    let (schema, _title, _body) = build_schema_dev();
    for (_score, doc_address) in top_docs {
        // _score = 1;
        println!("Found doc addr {:?}, score {}", &doc_address, &_score);
        let retrieved_doc = searcher.doc(doc_address).unwrap();
        println!("{}", schema.to_json(&retrieved_doc));
    }
    1
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
        info!("note title: {}", &note_title);

        let contents :String = fs::read_to_string(&note)
            .expect("Something went wrong reading the file");
        info!("Length: {}", contents.len());

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

fn query(term: String) -> String {
    format!("Searching {}", term)
}