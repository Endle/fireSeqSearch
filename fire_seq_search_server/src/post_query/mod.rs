use log::info;
use crate::query_engine::ServerInformation;
use crate::{FireSeqSearchHitParsed, tokenize_default};

pub mod logseq_uri;
pub mod highlighter;

use rayon::prelude::*;
pub fn post_query_wrapper(top_docs: Vec<(f32, tantivy::DocAddress)>,
                      term: &str,
                      searcher: &tantivy::LeasedItem<tantivy::Searcher>,
                      server_info: &ServerInformation) -> Vec<String> {
    let term_tokens = tokenize_default(&term);
    info!("get term tokens {:?}", &term_tokens);
    let result: Vec<String> = top_docs.par_iter()
        .map(|x| parse_and_serde(x)).collect();

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

fn parse_and_serde(tantivy_hit: &(f32, tantivy::DocAddress)) -> String {
    // FireSeqSearchHitParsed
    todo!()
}