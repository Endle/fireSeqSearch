mod markdown_to_text;
mod pdf_parser;

use std::borrow::Cow;
use regex::Regex;
use crate::query_engine::ServerInformation;

// https://docs.rs/regex/latest/regex/#repetitions
// https://stackoverflow.com/a/8303552/1166518
pub fn exclude_advanced_query(md: Cow<'_,str>) -> Cow<'_, str> {
    if !md.contains('#') {
        return md;
    }

    lazy_static! {
        static ref RE: Regex = Regex::new(
            r"\#\+BEGIN_QUERY[\S\s]+?\#\+END_QUERY")
            .unwrap();
    }
    return RE.replace_all(&md, "    ").into_owned().into();
}

fn hack_specific_chars_cow(text: Cow<str>) -> String {
    //https://www.compart.com/en/unicode/U+2022
    let bullet = char::from_u32(0x00002022).unwrap();
    text.replace(bullet, " ")
}

use crate::query_engine::NotebookSoftware;
use std::borrow::Borrow;
use log::info;

fn remove_obsidian_header<'a>(content: Cow<'a, str>) -> Cow<'a, str> {
    lazy_static! {
        static ref RE: Regex = Regex::new(
            r"---[\s\S]*?---"
        ).unwrap();
    }
    info!("from {:?}", &content);
    let cr = content.borrow();
    let ret: Cow<str> = RE.replace(cr, "    ");
    info!("into {:?}", &ret);
    ret.into_owned().into()
}

pub fn parse_logseq_notebook(md: Cow<'_,str>, title: &str, server_info: &ServerInformation) -> String {
    // Now we do some parsing for this file
    let content = exclude_advanced_query(md);
    let content = hack_specific_chars_cow(content);

    let content = Cow::from(content);
    let content = match &server_info.software {
        NotebookSoftware::Obsidian => remove_obsidian_header(content),
        _ => content,
    };
    let content: String = markdown_to_text::convert_from_logseq(
        &content, title, server_info);

    //let content = content.into_owned();
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
