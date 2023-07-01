use log::info;
use crate::Article;
use std::collections::{HashMap, HashSet};


use rayon::prelude::*;

// let x: Vec<Vec<_>> = vec![vec![1, 2], vec![3, 4]];
// let y: Vec<_> = x.into_par_iter().flatten().collect();
pub fn generate_wordcloud(articles: &Vec<Article>) -> String {
    info!("Generating wordlist");

    let tokens: Vec<String> = articles.par_iter().map(article_to_tokens)
        .flatten().collect();
    // for art in articles {
    //     let tokens = article_to_tokens(art);
    // }
    info!("After flatten, we got {} tokens", tokens.len());

    // silly group by
    let mut freq: HashMap<String,i64> = HashMap::new();
    for t in tokens {
        match freq.get(&t) {
            Some(count) => { freq.insert(t, count + 1); }
            None => { freq.insert(t, 1 as i64); }
        }
    }



    let mut sorted_pairs: Vec<(String,i64)> = freq.into_iter().collect();
    sorted_pairs.sort_by(|a, b| b.1.cmp(&a.1));
    sorted_pairs.truncate(50);
    // sorted_pairs


    let serialized_data = serde_json::to_string(&sorted_pairs).unwrap();
    serialized_data
}

fn article_to_tokens(art: &Article) -> Vec<String> {
    let tokens = crate::language_tools::tokenizer::tokenize(&art.content);

    //TODO use another stop word list for wordcloud
    lazy_static! {
        static ref STOPWORDS_LIST: HashSet<String> = crate::post_query::highlighter::generate_stopwords_list();
    }
    let tokens = crate::language_tools::tokenizer::filter_out_stopwords(&tokens, &STOPWORDS_LIST);
    info!("Got tokens {:?}", &tokens);
    let tokens : Vec<String> = tokens.into_iter().map(|x| x.to_string()).collect();
    tokens
}