use log::info;
use crate::Article;
use std::collections::HashSet;

pub fn generate_wordcloud(articles: &Vec<Article>) -> String {
    info!("Generating wordlist");

    for art in articles {
        let tokens = article_to_tokens(art);
    }
    "stub_cloud".to_string()
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