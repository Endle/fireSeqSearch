use fire_seq_search_server::markdown_parser::parse_to_plain_text;


fn read_file_to_line(relative_path: &str) -> String {
    let path = vec![String::from("tests/resource/pages"),
                    relative_path.to_string()];
    let path = path.join("/");
    std::fs::read_to_string(&path)
        .expect("Should have been able to read the file")
}


#[test]
fn parse() {
    let md = read_file_to_line("blog_thunderbird_zh.md");
    let result = parse_to_plain_text(md);
}