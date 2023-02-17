mod markdown_to_text;
use regex::Regex;

// https://docs.rs/regex/latest/regex/#repetitions
// https://stackoverflow.com/a/8303552/1166518
pub fn exclude_advanced_query(md: String) -> String {
    if !md.contains('#') {
        return md;
    }

    lazy_static! {
        static ref RE: Regex = Regex::new(
            r"\#\+BEGIN_QUERY[\S\s]+?\#\+END_QUERY")
            .unwrap();
    }
    let result = RE.replace_all(&md, "    ");
    String::from(result)
}

pub fn parse_logseq_notebook(md: String) -> String {

    // Now we do some parsing for this file
    let content: String = exclude_advanced_query(md);
    let content: String = parse_to_plain_text(&content);
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