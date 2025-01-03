pub mod post_query;
pub mod load_notes;
pub mod markdown_parser;
pub mod language_tools;
pub mod http_client;
pub mod query_engine;
pub mod word_frequency;
pub mod local_llm;


use log::debug;
use crate::query_engine::ServerInformation;
use crate::query_engine::NotebookSoftware::Logseq;


#[macro_use]
extern crate lazy_static;

pub static JOURNAL_PREFIX: &str = "@journal@";


pub struct Article {
    #[allow(dead_code)] /* TODO rethink if we need it 2024 Sep 21 */
    file_name: String,
    content: String
}

// Based on https://github.com/jiegec/tantivy-jieba
// tantivy-jieba is licensed under MIT, Copyright 2019-2020 Jiajie Chen
// I had heavy modifications on it
/*
lazy_static! {
    static ref JIEBA: jieba_rs::Jieba = jieba_rs::Jieba::new();
}
*/

//pub const TOKENIZER_ID: &str = "fss_tokenizer";


/*
impl Tokenizer for JiebaTokenizer {
    type TokenStream<'a> = JiebaTokenStream;
    fn token_stream<'a>(&mut self, text: &'a str) -> JiebaTokenStream {
        let mut indices = text.char_indices().collect::<Vec<_>>();
        indices.push((text.len(), '\0'));
        let jieba : jieba_rs::Jieba = jieba_rs::Jieba::new(); //TODO use a static one
        let orig_tokens = jieba.tokenize(text, jieba_rs::TokenizeMode::Search, true);
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
        JiebaTokenStream { tokens, index: 0 }

    }
}
*/

/*
Thoughts on lowercase  2022-07-04:
tanvity's default tokenizer will lowercase all English characters.
    https://docs.rs/tantivy/latest/tantivy/tokenizer/index.html
    I'm just trying my best to simulate it
However, I think there could be a better approach
1. use https://github.com/pemistahl/lingua-rs to determine the language of the text
2. Select proper tokenizer
fn process_token_text(text: &str, indices: &Vec<(usize, char)>, token: &jieba_rs::Token<'_>) -> Option<String> {
    let raw = String::from(&text[(indices[token.start].0)..(indices[token.end].0)]);
    let lower = raw.to_lowercase();
    if lower.trim().is_empty() {
        None
    } else {
        Some(lower)
    }
}
 */

// TODO use stub now
pub fn tokenize_default(sentence: &str) -> Vec<String> {
    let mut r = Vec::new();
    r.push(sentence.to_owned());
    r
}
/*
// TODO: Move tokenizer-related things into language_tools
pub fn tokenize_default(sentence: &str) -> Vec<String> {
    /*
    lazy_static! {
        static ref TK: JiebaTokenizer = crate::JiebaTokenizer {};
    }
    */
    // TODO use static tokenizer
    let mut tokenizer = crate::JiebaTokenizer{};
    if language_tools::is_chinese(sentence) {
        info!("Use Tokenizer for Chinese term {}", sentence);
        tokenize_sentence_to_text_vec(&mut tokenizer, sentence)
    } else {
        // info!("Space Tokenizer {}", sentence);
        let result : Vec<&str> = sentence.split_whitespace()
            .collect();
        // debug!("Got tokens {:?}", &result);
        let result:Vec<String> = result.iter().map(|&s|s.into()).collect();
        result
        // vec![String::from(sentence)]
    }
}
*/


use crate::language_tools::tokenizer::FireSeqTokenizer;
pub fn tokenize_sentence_to_text_vec(tokenizer: &mut FireSeqTokenizer, sentence: &str) -> Vec<String> {
    let tokens = tokenize_sentence_to_vector(tokenizer, sentence);
    tokens_to_text_vec(&tokens)
}
pub fn tokenize_sentence_to_vector(tokenizer: &mut FireSeqTokenizer, sentence: &str)  ->  Vec<tantivy::tokenizer::Token> {
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



pub fn decode_cjk_str(original: String) -> Vec<String> {
    use urlencoding::decode;

    let mut result = Vec::new();
    for s in original.split(' ') {
        let t = decode(s).expect("UTF-8");
        debug!("Decode {}  ->   {}", s, t);
        result.push(String::from(t));
    }

    result
}



// ============= BELOW IS TEST CASES ====================
pub fn generate_server_info_for_test() -> ServerInformation {
    let server_info = ServerInformation {
        notebook_path: "stub_path".to_string(),
        notebook_name: "logseq_notebook".to_string(),
        enable_journal_query: false,
        show_top_hits: 0,
        show_summary_single_line_chars_limit: 0,
        parse_pdf_links: false,
        exclude_zotero_items: false,
        software: Logseq,
        convert_underline_hierarchy: true,
        host: "127.0.0.1:22024".to_string(),
        llm_enabled: false,
        llm_max_waiting_time: 60,
    };
    server_info
}

/*
#[cfg(test)]
mod test_tokenizer {
    #[test]
    fn english() {
        let _tokens = base("Travel to japan", vec!["travel", "to", "japan"]);
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
*/

