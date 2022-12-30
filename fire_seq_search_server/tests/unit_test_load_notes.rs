use fire_seq_search_server::load_notes::{exclude_advanced_query, read_specific_directory};
use fire_seq_search_server::markdown_parser::parse_to_plain_text;


fn load_articles() -> Vec<(String, String)> {
    let r = read_specific_directory("tests/resource/pages");
    r
}

#[test]
fn test_load_articles() {
    let r = load_articles();
    assert_eq!(r.len(), 11);
    for (title,body) in &r{
        assert!(title.len()>0);
        assert!(body.len()>0);
    }
}


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
    let result = parse_to_plain_text(&md);
    assert!(result.contains("Aug 3, 2021 - 使用 git shallow clone 下载并编译 Thunderbird"));
    assert!(!result.contains("https://developer.thunderbird.net/thunderbird-development/getting-started"));

}

#[test]
fn exclude_advance_query() {
    let md = read_file_to_line("advanced_query.md");
    let result = exclude_advanced_query(md);
    assert!(!result.contains("exempli"));
    assert!(result.contains("In this test page we have"));


    let md = read_file_to_line("blog_thunderbird_zh.md");
    let result = exclude_advanced_query(md.clone());
    assert_eq!(md, result);
}