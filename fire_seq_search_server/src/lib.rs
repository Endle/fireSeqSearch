// Some sode copied from https://github.com/jiegec/tantivy-jieba
// tantivy-jieba is licensed under MIT, Copyright 2019-2020 Jiajie Chen




#[macro_use]
extern crate lazy_static;


// use jieba_rs::Jieba;

lazy_static! {
    static ref JIEBA: jieba_rs::Jieba = jieba_rs::Jieba::new();
}


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
            tokens.push(Token {
                offset_from: indices[token.start].0,
                offset_to: indices[token.end].0,
                position: token.start,
                text: String::from(&text[(indices[token.start].0)..(indices[token.end].0)]),
                position_length: token.end - token.start,
            });
        }
        BoxTokenStream::from(JiebaTokenStream { tokens, index: 0 })
    }
}
