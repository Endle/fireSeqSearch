use std::collections::HashSet;
use log::{debug, info};

/// ```
/// let l = fire_seq_search_server::language_tools::generate_stopwords_list();
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
    /*
    lazy_static! {
        static ref TK: crate::JiebaTokenizer = crate::JiebaTokenizer {};
    }
    */
    if crate::language_tools::is_chinese(sentence) {
        info!("Use Tokenizer for Chinese term {}", sentence);
        let mut jieba = FireSeqTokenizer {};
        //TODO don't create a tokenizer every time
        crate::tokenize_sentence_to_text_vec(&mut jieba, sentence)
    } else {
        // info!("Space Tokenizer {}", sentence);
        let result : Vec<&str> = sentence.split_whitespace()
            .collect();
        debug!("Got tokens {:?}", &result);
        let result:Vec<String> = result.iter().map(|&s|s.into()).collect();
        result
        // vec![String::from(sentence)]
    }
}

use lazy_static::lazy_static;
use tantivy_tokenizer_api::{Token, TokenStream, Tokenizer};

lazy_static! {
    static ref JIEBA: jieba_rs::Jieba = jieba_rs::Jieba::new();
}

pub const TOKENIZER_ID: &str = "fireseq_tokenizer";

#[derive(Clone)]
pub struct FireSeqTokenizer;



pub struct JiebaTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for JiebaTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index = self.index + 1;
            true
        } else {
            false
        }
    }
    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

impl Tokenizer for FireSeqTokenizer {
    type TokenStream<'a> = JiebaTokenStream;
    fn token_stream<'a>(&mut self, text: &'a str) -> JiebaTokenStream {
        let mut indices = text.char_indices().collect::<Vec<_>>();
        indices.push((text.len(), '\0'));
        let orig_tokens = JIEBA.tokenize(text, jieba_rs::TokenizeMode::Search, true);
        let mut tokens = Vec::new();
        // copy tantivy-jieba code for now
        for token in orig_tokens {
            tokens.push(Token {
                offset_from: indices[token.start].0,
                offset_to: indices[token.end].0,
                position: token.start,
                text: String::from(&text[(indices[token.start].0)..(indices[token.end].0)]),
                position_length: token.end - token.start,
            });
        }
        /*
        for i in 0..orig_tokens.len() {
            let token = &orig_tokens[i];
            match process_token_text(text, &indices, &token) {
                Some(text) => tokens.push(Token {
                    offset_from: indices[token.start].0,
                    offset_to: indices[token.end].0,
                    position: token.start,
                    text,
                    position_length: token.end - token.start,
                }),
                None => ()
            }

        }
        */
        JiebaTokenStream { tokens, index: 0 }
    }
}
