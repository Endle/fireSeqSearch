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



#[cfg(test)]
mod test_tokenizer {
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
