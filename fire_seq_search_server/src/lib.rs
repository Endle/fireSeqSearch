pub mod post_query;
pub mod load_notes;
pub mod markdown_parser;


use crate::post_query::highlight_keywords_in_body;


#[macro_use]
extern crate lazy_static;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct FireSeqSearchHitParsed {
    // pub title: String,
    pub title: String,
    pub summary: String,
    pub score: f32,
}

impl FireSeqSearchHitParsed {
    /*
    pub fn from_hit(hit: &FireSeqSearchHit) -> FireSeqSearchHitParsed {
        FireSeqSearchHitParsed {
            title: String::from(hit.title),
            score: hit.score
        }
    }

     */
    pub fn from_tantivy(doc: &tantivy::schema::Document,
                        score: f32, term_tokens: &Vec<String>) ->FireSeqSearchHitParsed {
        for _field in doc.field_values() {
            // debug!("field {:?} ", &field);
        }
        let title: &str = doc.field_values()[0].value().as_text().unwrap();
        let body: &str = doc.field_values()[1].value().as_text().unwrap();
        let summary = highlight_keywords_in_body(body, term_tokens);
        FireSeqSearchHitParsed {
            // title: String::from(title),
            title: String::from(title),
            summary,
            score,
        }
    }

}




/*TODO: Do I really need this struct?*/
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct FireSeqSearchHit<'a> {
    // pub title: String,
    pub title: &'a str,
    pub body: &'a str,
    pub score: f32,
    //field_values: Vec<FieldValue>,
}
impl<'a> FireSeqSearchHit<'a> {
    pub fn from_tantivy(doc: &tantivy::schema::Document, score: f32) ->FireSeqSearchHit {
        for _field in doc.field_values() {
            // debug!("field {:?} ", &field);
        }
        let title: &str = doc.field_values()[0].value().as_text().unwrap();
        let body: &str = doc.field_values()[1].value().as_text().unwrap();

        FireSeqSearchHit {
            // title: String::from(title),
            title,
            body,
            score
        }
    }
}



// Some sode copied from https://github.com/jiegec/tantivy-jieba
// tantivy-jieba is licensed under MIT, Copyright 2019-2020 Jiajie Chen
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

pub fn tokenize_default(sentence: &str) -> Vec<String> {
    lazy_static! {
        static ref TK: JiebaTokenizer = crate::JiebaTokenizer {};
    }
    tokenize_sentence_to_text_vec(&TK, sentence)

}
pub fn tokenize_sentence_to_text_vec(tokenizer: &JiebaTokenizer, sentence: &str) -> Vec<String> {
    let tokens = tokenize_sentence_to_vector(&tokenizer, sentence);
    tokens_to_text_vec(&tokens)
}
pub fn tokenize_sentence_to_vector(tokenizer: &JiebaTokenizer, sentence: &str)  ->  Vec<tantivy::tokenizer::Token> {
    use tantivy::tokenizer::*;
    let mut token_stream = tokenizer.token_stream(
        sentence
    );
    let mut tokens = Vec::new();

    while let Some(token) = token_stream.next() {
        tokens.push(token.clone());

    }
    tokens
}
pub fn tokens_to_text_vec(tokens: &Vec<tantivy::tokenizer::Token>) -> Vec<String> {
    let mut token_text = Vec::new();
    for token in tokens {
        token_text.push(token.text.clone());
    }
    token_text
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
        let tokens = base("张华考上了北京大学；李萍进了中等技术学校；我在百货公司当售货员：我们都有光明的前途",
             vec![
                 // "a",
                 "张华",
                 "考上",
                 "了",
                 "北京",
                 "大学",
                 "北京大学",
                 "；",
                 "李萍",
                 "进",
                 "了",
                 "中等",
                 "技术",
                 "术学",
                 "学校",
                 "技术学校",
                 "；",
                 "我",
                 "在",
                 "百货",
                 "公司",
                 "百货公司",
                 "当",
                 "售货",
                 "货员",
                 "售货员",
                 "：",
                 "我们",
                 "都",
                 "有",
                 "光明",
                 "的",
                 "前途"
             ]
        );
        // offset should be byte-indexed
        assert_eq!(tokens[0].offset_from, 0);
        assert_eq!(tokens[0].offset_to, "张华".bytes().len());
        assert_eq!(tokens[1].offset_from, "张华".bytes().len());
    }
    fn base(sentence: &str, expect_tokens: Vec<&str>) ->  Vec<tantivy::tokenizer::Token> {
        use tantivy::tokenizer::*;
        use crate::{tokenize_sentence_to_vector,tokens_to_text_vec};
        let tokenizer = crate::JiebaTokenizer {};
        let tokens = tokenize_sentence_to_vector(&tokenizer, sentence);
        let token_text = tokens_to_text_vec(&tokens);
        // check tokenized text
        assert_eq!(
            token_text,
            expect_tokens
        );
        tokens
    }


}
