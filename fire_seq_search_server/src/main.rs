use warp::Filter;

use tantivy::schema::*;
use tantivy::{ReloadPolicy, doc, DocAddress, LeasedItem, Searcher};
use rayon::prelude::*;



use serde_json;


use log::{info,debug};

use urlencoding::decode;


use fire_seq_search_server::{FireSeqSearchHitParsed, JiebaTokenizer, TOKENIZER_ID, tokenize_default, ServerInformation, JOURNAL_PREFIX};
use fire_seq_search_server::load_notes::read_specific_directory;



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

    #[arg(long,default_value_t = false)]
    enable_journal_query: bool,

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

    let matches = Cli::parse();

    let server_info: ServerInformation = build_server_info(matches);
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

fn build_server_info(args: Cli) -> ServerInformation {
    let notebook_name = match args.notebook_name {
        Some(x) => x.to_string(),
        None => {
            let chunks: Vec<&str> = args.notebook_path.split("/").collect();
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


// I can't remember why I need this schema parameter. To satisfy compiler, I added _ on 2022-11-06
fn query(term: String, server_info: &ServerInformation, _schema: tantivy::schema::Schema,
         reader: &tantivy::IndexReader, query_parser: &tantivy::query::QueryParser)
    -> String {

    debug!("Original Search term {}", term);

    // in the future, I would use tokenize_sentence_to_text_vec here
    let term = term.replace("%20", " ");
    let term_vec = decode_cjk_str(term);
    let term = term_vec.join(" ");

    info!("Searching {}", term);
    let searcher = reader.searcher();



    let query: Box<dyn tantivy::query::Query> = query_parser.parse_query(&term).unwrap();
    let top_docs: Vec<(f32, tantivy::DocAddress)> =
        searcher.search(&query,
                        &tantivy::collector::TopDocs::with_limit(server_info.show_top_hits))
        .unwrap();



    let result: Vec<String> = post_query_wrapper(top_docs, &term, &searcher, &server_info);



    let json = serde_json::to_string(&result).unwrap();

    // info!("Search result {}", &json);
    json
    // result[0].clone()
}

fn post_query_wrapper(top_docs: Vec<(f32, DocAddress)>,
                      term: &String,
                      searcher: &LeasedItem<Searcher>,
                      server_info: &ServerInformation) -> Vec<String> {
    let term_tokens = tokenize_default(&term);
    info!("get term tokens {:?}", &term_tokens);
    // let mut result;
    let result: Vec<String> = top_docs.par_iter()
        .map(|&x| FireSeqSearchHitParsed::from_tantivy
            (&searcher.doc(x.1).unwrap(),
             x.0,
             &term_tokens,
            server_info)
        )
        // .map(|x| FireSeqSearchHitParsed::from_hit(&x))
        .map(|p| serde_json::to_string(&p).unwrap())
        .collect();
    result
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
    let path = path.to_owned();
    let pages_path = path.clone() + "/pages";


    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    for (note_title, contents) in read_specific_directory(&pages_path) {
        index_writer.add_document(
            doc!{ title => note_title, body => contents}
        ).unwrap();
    }

    if server_info.enable_journal_query {
        info!("Loading journals");
        let journals_page = path.clone() + "/journals";
        for (note_title, contents) in read_specific_directory(&journals_page) {
            let tantivy_title = JOURNAL_PREFIX.to_owned() + &note_title;
            index_writer.add_document(
                doc!{ title => tantivy_title, body => contents}
            ).unwrap();
        }
    }

    index_writer.commit().unwrap();
    index
}
