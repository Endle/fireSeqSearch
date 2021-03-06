use warp::Filter;

use tantivy::schema::*;
use tantivy::{ReloadPolicy,doc};



use std::fs;
use serde_json;
use serde::Serialize;

use log::{info,debug,warn,error};
use clap::{Command,arg};
use urlencoding::decode;

use fire_seq_search_server::{JiebaTokenizer, TOKENIZER_ID};

#[derive(Debug, Clone, Serialize)]
struct ServerInformation {
    notebook_path: String,
    notebook_name: String,
    show_top_hits: usize,
}

struct DocumentSetting {
    schema: tantivy::schema::Schema,
    tokenizer: JiebaTokenizer,
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
    let document_setting: DocumentSetting = build_document_setting();

    let index = indexing_documents(&server_info, &document_setting);
    let (reader, query_parser) = build_reader_parser(&index, &document_setting);

    // TODO clone server_info is so ugly here
    let server_info_dup = server_info.clone();
    let call_query = warp::path!("query" / String)
        .map(move |name| query(name, &server_info_dup, document_setting.schema.clone(),
                               &reader, &query_parser) );

    let server_info_dup2 = server_info.clone();
    let get_server_info = warp::path("server_info")
        .map(move || serde_json::to_string( &server_info_dup2 ).unwrap() );

    let routes = warp::get().and(
        call_query
            .or(get_server_info)
    );
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;


}

fn build_document_setting() -> DocumentSetting {
    let (schema, tokenizer) = build_schema_tokenizer();
    DocumentSetting{
        schema, tokenizer
    }
}

fn build_schema_tokenizer() -> (tantivy::schema::Schema,
                                JiebaTokenizer
                                // Box<dyn tantivy::tokenizer::Tokenizer>
) {
    let mut schema_builder = SchemaBuilder::default();
    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer(TOKENIZER_ID) // Set custom tokenizer
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(text_indexing)
        .set_stored();
    let tokenizer:JiebaTokenizer = JiebaTokenizer {};

    let _title = schema_builder.add_text_field("title", text_options.clone());
    let _body = schema_builder.add_text_field("body", text_options);

    let schema = schema_builder.build();
    (schema,
        tokenizer
    // Box::new(tokenizer)
    )
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
        show_top_hits: 10
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
fn query(term: String, server_info: &ServerInformation, schema: tantivy::schema::Schema,
         reader: &tantivy::IndexReader, query_parser: &tantivy::query::QueryParser)
    -> String {

    debug!("Original Search term {}", term);

    let term = term.replace("%20", " ");
    let term_vec = decode_cjk_str(term);
    let term = term_vec.join(" ");

    info!("Searching {}", term);
    let searcher = reader.searcher();



    let query = query_parser.parse_query(&term).unwrap();
    let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(server_info.show_top_hits))
        .unwrap();
    // let schema = &server_info.schema;
    let mut result = Vec::new();
    for (_score, doc_address) in top_docs {
        // _score = 1;
        info!("Found doc addr {:?}, score {}", &doc_address, &_score);
        let retrieved_doc: tantivy::schema::Document = searcher.doc(doc_address).unwrap();
        // debug!("Found {:?}", &retrieved_doc);
        result.push(schema.to_json(&retrieved_doc));
        // println!("{}", schema.to_json(&retrieved_doc));
    }
    //INVALID!
    // result.join(",")
    let json = serde_json::to_string(&result).unwrap();
    // info!("Search result {}", &json);
    json
    // result[0].clone()
}

fn build_reader_parser(index: &tantivy::Index, document_setting: &DocumentSetting)
    -> (tantivy::IndexReader, tantivy::query::QueryParser) {
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into().unwrap();
    let title = document_setting.schema.get_field("title").unwrap();
    let body = document_setting.schema.get_field("body").unwrap();
    let query_parser = tantivy::query::QueryParser::for_index(index, vec![title, body]);
    (reader, query_parser)
}

fn indexing_documents(server_info: &ServerInformation, document_setting: &DocumentSetting) -> tantivy::Index {


    let path: &str = &server_info.notebook_path;
    let schema = &document_setting.schema;
    let index = tantivy::Index::create_in_ram(schema.clone());

    index.tokenizers().register(TOKENIZER_ID, document_setting.tokenizer.clone());

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

                // let mut doc = Document::default();
                // doc.add_text(title, note_title);
                // doc.add_text(body, contents);
                index_writer.add_document(
                    doc!{ title => note_title, body => contents}
                );
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

