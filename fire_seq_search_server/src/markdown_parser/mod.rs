mod markdown_to_text;
mod markdown_to_text_fireseqsearch;
mod pdf_parser;

use std::borrow::Cow;
use regex::Regex;
use crate::query_engine::ServerInformation;

// https://docs.rs/regex/latest/regex/#repetitions
// https://stackoverflow.com/a/8303552/1166518
pub fn exclude_advanced_query(md: &str) -> Cow<str> {
    if !md.contains('#') {
        return Cow::Borrowed(md);
    }

    lazy_static! {
        static ref RE: Regex = Regex::new(
            r"\#\+BEGIN_QUERY[\S\s]+?\#\+END_QUERY")
            .unwrap();
    }
    // return RE.replace_all(&md, "    ")
    return RE.replace_all(&md, "    ");
}

fn hack_specific_chars_cow(text: Cow<str>) -> String {
    //https://www.compart.com/en/unicode/U+2022
    let bullet = char::from_u32(0x00002022).unwrap();
    text.replace(bullet, " ")
}

pub fn parse_logseq_notebook(md: &str, title: &str, server_info: &ServerInformation) -> String {
    // Now we do some parsing for this file
    let content = exclude_advanced_query(md);
    let content = hack_specific_chars_cow(content);
    let content: String = markdown_to_text::convert_from_logseq(
        &content, title, server_info);
    content
}


pub fn parse_to_plain_text(md: &str) -> String {
    let plain_text: String = markdown_to_text::convert(&md);
    let plain_text = hack_specific_chars(plain_text);

    // println!("{}", &plain_text);
    plain_text
}

fn hack_specific_chars(text: String) -> String {
    //https://www.compart.com/en/unicode/U+2022
    let bullet = char::from_u32(0x00002022).unwrap();
    // println!("{}", bullet);
    text.replace(bullet, " ")
}