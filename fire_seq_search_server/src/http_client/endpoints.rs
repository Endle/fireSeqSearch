use log::{debug, info};
use crate::{decode_cjk_str, post_query_wrapper, ServerInformation};

// I can't remember why I need this schema parameter. To satisfy compiler, I added _ on 2022-11-06
pub fn query(term: String, server_info: &ServerInformation, _schema: tantivy::schema::Schema,
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



