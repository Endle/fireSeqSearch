use log::info;
use crate::query_engine::ServerInformation;
use crate::{FireSeqSearchHitParsed, tokenize_default};

pub mod logseq_uri;
pub mod highlighter;

use rayon::prelude::*;
use tantivy::{LeasedItem, Searcher};

pub fn post_query_wrapper(top_docs: Vec<(f32, tantivy::DocAddress)>,
                      term: &str,
                      searcher: &tantivy::LeasedItem<tantivy::Searcher>,
                      server_info: &ServerInformation) -> Vec<String> {
    let term_tokens = tokenize_default(&term);
    info!("get term tokens {:?}", &term_tokens);
    let result: Vec<String> = top_docs.par_iter()
        .map(|x| parse_and_serde(x, searcher, &term_tokens, server_info))
        .collect();

    // let result: Vec<String> = top_docs.par_iter()
    //     .map(|&x| FireSeqSearchHitParsed::from_tantivy
    //         (&searcher.doc(x.1).unwrap(),
    //          x.0,
    //          &term_tokens,
    //          server_info)
    //     )
    //     // .map(|x| FireSeqSearchHitParsed::from_hit(&x))
    //     .map(|p| serde_json::to_string(&p).unwrap())
    //     .collect();
    result
}

fn parse_and_serde(x: &(f32, tantivy::DocAddress),
                   searcher: &LeasedItem<Searcher>, term_tokens: &Vec<String>,
                   server_info: &ServerInformation) -> String {
    // FireSeqSearchHitParsed
    let doc = searcher.doc(x.1).unwrap();
    let score = x.0;
    let hit_parsed = FireSeqSearchHitParsed::from_tantivy(
        &doc, score, term_tokens, server_info
    ); // it also provides the highlight


    serde_json::to_string(&hit_parsed).unwrap()
}