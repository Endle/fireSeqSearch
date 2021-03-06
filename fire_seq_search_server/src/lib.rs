// Some sode copied from https://github.com/jiegec/tantivy-jieba
// tantivy-jieba is licensed under MIT, Copyright 2019-2020 Jiajie Chen




#[macro_use]
extern crate lazy_static;


// use jieba_rs::Jieba;

lazy_static! {
    static ref JIEBA: jieba_rs::Jieba = jieba_rs::Jieba::new();
}

pub const TOKENIZER_ID: &str = "fss_tokenizer";

use tantivy::tokenizer::{BoxTokenStream, Token, TokenStream, Tokenizer};

pub struct JiebaTokenStream {
    tokens: Vec<Token>,
    index: usize,
}


#[derive(Clone)]
pub struct JiebaTokenizer;

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

impl Tokenizer for JiebaTokenizer {
    fn token_stream<'a>(&self, text: &'a str) -> BoxTokenStream<'a> {
        let mut indices = text.char_indices().collect::<Vec<_>>();
        indices.push((text.len(), '\0'));
        let orig_tokens = JIEBA.tokenize(text, jieba_rs::TokenizeMode::Search, true);
        let mut tokens = Vec::new();
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
        BoxTokenStream::from(JiebaTokenStream { tokens, index: 0 })
    }
}

/*
Thoughts on lowercase  2022-07-04:
tanvity's default tokenizer will lowercase all English characters.
    https://docs.rs/tantivy/latest/tantivy/tokenizer/index.html
    I'm just trying my best to simulate it
However, I think there could be a better approach
1. use https://github.com/pemistahl/lingua-rs to determine the language of the text
2. Select proper tokenizer
 */
fn process_token_text(text: &str, indices: &Vec<(usize, char)>, token: &jieba_rs::Token<'_>) -> Option<String> {
    let raw = String::from(&text[(indices[token.start].0)..(indices[token.end].0)]);
    let lower = raw.to_lowercase();
    if lower.trim().is_empty() {
        None
    } else {
        Some(lower)
    }
}

// ============= BELOW IS TEST CASES ====================


#[cfg(test)]
mod test_tokenizer {
    #[test]
    fn english() {
        let tokens = base("Travel to japan", vec!["travel", "to", "japan"]);
    }

    #[test]
    fn simple_zh() {
        let tokens = base("???????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????",
             vec![
                 // "a",
                 "??????",
                 "??????",
                 "???",
                 "??????",
                 "??????",
                 "????????????",
                 "???",
                 "??????",
                 "???",
                 "???",
                 "??????",
                 "??????",
                 "??????",
                 "??????",
                 "????????????",
                 "???",
                 "???",
                 "???",
                 "??????",
                 "??????",
                 "????????????",
                 "???",
                 "??????",
                 "??????",
                 "?????????",
                 "???",
                 "??????",
                 "???",
                 "???",
                 "??????",
                 "???",
                 "??????"
             ]
        );
        // offset should be byte-indexed
        assert_eq!(tokens[0].offset_from, 0);
        assert_eq!(tokens[0].offset_to, "??????".bytes().len());
        assert_eq!(tokens[1].offset_from, "??????".bytes().len());
    }
    fn base(sentence: &str, expect_tokens: Vec<&str>) ->  Vec<tantivy::tokenizer::Token> {
        use tantivy::tokenizer::*;
        let tokenizer = crate::JiebaTokenizer {};
        let mut token_stream = tokenizer.token_stream(
            sentence
        );
        let mut tokens = Vec::new();
        let mut token_text = Vec::new();
        while let Some(token) = token_stream.next() {
            tokens.push(token.clone());
            token_text.push(token.text.clone());
        }
        // check tokenized text
        assert_eq!(
            token_text,
            expect_tokens
        );
        tokens
    }
}
