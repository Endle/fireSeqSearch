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
    sorted_pairs.truncate(200);
    // sorted_pairs


    let serialized_data = serde_json::to_string(&sorted_pairs).unwrap();
    serialized_data
}

fn article_to_tokens(art: &Article) -> Vec<String> {
    let tokens = crate::language_tools::tokenizer::tokenize(&art.content);

    //TODO use another stop word list for wordcloud
    lazy_static! {
        static ref STOPWORDS_LIST: HashSet<String> =  crate::language_tools::generate_stopwords_list();
    }
    let tokens = crate::language_tools::tokenizer::filter_out_stopwords(&tokens, &STOPWORDS_LIST);
    let tokens: Vec<&str> = tokens.into_iter().filter(|x| is_valid_for_wordcloud(x)).collect();
    info!("Got tokens {:?}", &tokens);
    let tokens : Vec<String> = tokens.into_iter().map(|x| x.to_string()).collect();
    tokens
}


fn is_valid_for_wordcloud(s:&str) -> bool{
    if is_symbol(s) {
        return false;
    }
    let invalid_end_pattern = vec!["::", "]]", "}}"];
    let invalid_start_pattern = vec!["[[", "{{", "{\\"];

    for ep in invalid_end_pattern {
        if s.ends_with(ep) {
            return false;
        }
    }
    for sp in invalid_start_pattern {
        if s.starts_with(sp) {
            return false;
        }
    }
    let logseq_exclude_list = vec!["DONE", "true", "SCHEDULED:", "collapsed", "file", "com",
                  "CLOCK:", ":LOGBOOK:", ":END:"];
    for stop in logseq_exclude_list {
        if s == stop {
            return false;
        }
    }
    //
    true
}
fn is_symbol(s:&str) -> bool {
    if s.len() == 0 { return true; }
    if s.len() > 3 { return false; }
    let mut flag = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            flag = false;
        }
    }
    flag
}