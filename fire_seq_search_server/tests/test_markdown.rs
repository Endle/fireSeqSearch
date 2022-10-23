use fire_seq_search_server::markdown_parser;

#[test]
fn parse() {
    let md = String::from("md");
    let _ = parse_to_plain_text(md);
}