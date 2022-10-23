use pulldown_cmark::{Parser, Options, html};



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