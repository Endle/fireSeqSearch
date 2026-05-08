use std::collections::HashSet;
use log::debug;

/// ```
/// let l = fire_seq_search_server::language_tools::generate_stopwords_list();
/// assert!(l.contains("the"));
/// assert!(!l.contains("thex"));
///
/// let terms = vec![String::from("the"), String::from("The"), String::from("answer")];
/// let result = fire_seq_search_server::language_tools::tokenizer::filter_out_stopwords(&terms, &l);
/// assert_eq!(result.len(), 1);
/// ```
pub fn filter_out_stopwords<'a, 'b>(
    term_tokens: &'a [String],
    nltk: &'b HashSet<String>,
) -> Vec<&'a str> {
    term_tokens
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !s.trim().is_empty())
        .filter(|s| !nltk.contains(&s.to_lowercase()))
        .collect()
}

pub fn tokenize(sentence: &str) -> Vec<String> {
    debug!("tokenize: {}", sentence);
    sentence.split_whitespace().map(|s| s.to_owned()).collect()
}
