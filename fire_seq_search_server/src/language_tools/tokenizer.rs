use std::collections::HashSet;
use log::{debug, info};

/// ```
/// let l = fire_seq_search_server::post_query::highlighter::generate_stopwords_list();
/// assert!(l.contains("the"));
/// assert!(!l.contains("thex"));
///
/// let terms = vec![String::from("the"), String::from("The"), String::from("answer")];
/// let result = fire_seq_search_server::language_tools::tokenizer::filter_out_stopwords(&terms, &l);
/// assert_eq!(result.len(), 1);
/// ```
pub fn filter_out_stopwords<'a,'b>(term_tokens: &'a [String], nltk: &'b HashSet<String>) -> Vec<&'a str> {
    let term_ref: Vec<&str> = term_tokens.iter().map(|s| &**s).collect();
    let terms_selected: Vec<&str> = term_ref.into_iter()
        .filter(|&s| !nltk.contains(&(&s).to_lowercase()  )  )
        .collect();
    terms_selected
}



pub fn tokenize(sentence: &str) -> Vec<String> {
    lazy_static! {
        static ref TK: crate::JiebaTokenizer = crate::JiebaTokenizer {};
    }
    if crate::language_tools::is_chinese(sentence) {
        info!("Use Tokenizer for Chinese term {}", sentence);
        crate::tokenize_sentence_to_text_vec(&TK, sentence)
    } else {
        info!("Space Tokenizer {}", sentence);
        let result : Vec<&str> = sentence.split_whitespace()
            .collect();
        debug!("Got tokens {:?}", &result);
        let result:Vec<String> = result.iter().map(|&s|s.into()).collect();
        result
        // vec![String::from(sentence)]
    }
}