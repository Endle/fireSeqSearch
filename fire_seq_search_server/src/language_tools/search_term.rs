use std::collections::HashSet;

/// ```
/// let l = fire_seq_search_server::post_query::highlighter::generate_stopwords_list();
/// assert!(l.contains("the"));
/// assert!(!l.contains("thex"));
///
/// let terms = vec![String::from("the"), String::from("The"), String::from("answer")];
/// let result = fire_seq_search_server::language_tools::search_term::filter_out_stopwords(&terms, &l);
/// assert_eq!(result.len(), 1);
/// ```
pub fn filter_out_stopwords<'a,'b>(term_tokens: &'a [String], nltk: &'b HashSet<String>) -> Vec<&'a str> {
    let term_ref: Vec<&str> = term_tokens.iter().map(|s| &**s).collect();
    let terms_selected: Vec<&str> = term_ref.into_iter()
        .filter(|&s| !nltk.contains(&(&s).to_lowercase()  )  )
        .collect();
    terms_selected
}