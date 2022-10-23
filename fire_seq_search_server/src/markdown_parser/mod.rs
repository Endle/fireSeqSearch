use minimad::parse_text;

pub fn parse_to_plain_text(md: String) -> String {
    let text = parse_text(&md);
    println!("{:?}", &text);
    "stub".to_string()
}