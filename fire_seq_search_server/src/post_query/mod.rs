use log::info;
use crate::query_engine::ServerInformation;
use crate::tokenize_default;

pub mod logseq_uri;
pub mod highlighter;
pub mod hit_parsed;
pub mod app_uri;
pub mod obsidian_uri;

use rayon::prelude::*;
use tantivy::{LeasedItem, Searcher};
use crate::post_query::hit_parsed::FireSeqSearchHitParsed;

pub fn post_query_wrapper(top_docs: Vec<(f32, tantivy::DocAddress)>,
                      term: &str,
                      searcher: &tantivy::LeasedItem<tantivy::Searcher>,
                      server_info: &ServerInformation) -> Vec<String> {
    let term_tokens = tokenize_default(term);
    info!("get term tokens {:?}", &term_tokens);
    let result: Vec<String> = top_docs.par_iter()
        .map(|x| parse_and_serde(x, searcher, &term_tokens, server_info))
        .collect();
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

    hit_parsed.serde_to_string()
}
