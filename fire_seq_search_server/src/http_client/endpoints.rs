use std::sync::Arc;
use log::{debug, info};
use crate::{decode_cjk_str, post_query_wrapper};
use crate::query_engine::{QueryEngine, ServerInformation};
use serde_json;

pub fn get_server_info(engine_arc: Arc<QueryEngine>) -> String {
    serde_json::to_string( &engine_arc.server_info ).unwrap()
}
pub fn query(term: String, engine_arc: Arc<QueryEngine>)
             -> String {

    debug!("Original Search term {}", term);

    // in the future, I would use tokenize_sentence_to_text_vec here
    let term = term.replace("%20", " ");
    let term_vec = decode_cjk_str(term);
    let term = term_vec.join(" ");

    info!("Searching {}", term);
    let searcher = engine_arc.reader.searcher();
    let server_info: &ServerInformation = &engine_arc.server_info;

    let query: Box<dyn tantivy::query::Query> = engine_arc.query_parser.parse_query(&term).unwrap();
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



