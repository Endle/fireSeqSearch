use fire_seq_search_server::post_query::highlighter::{highlight_keywords_in_body, highlight_sentence_with_keywords, locate_single_keyword, split_body_to_blocks, wrap_text_at_given_spots};
use fire_seq_search_server::post_query::highlighter::build_tree;
use fire_seq_search_server::generate_server_info_for_test;

fn get_english_text() -> String {
    std::fs::read_to_string("tests/resource/pages/International Language, Past, Present & Future by Walter John Clark.md")
        .expect("Should have been able to read the file")
}




/*
#[test]
fn test_highlight_single_term_single_appearance() {
    let _ = env_logger::try_init();
    let server_info = generate_server_info_for_test();
    let content = "使用 git shallow clone 下载并编译 Thunderbird".to_string();
    let token = "thunderbird";
    let tokens = [token];
    let mut root = build_tree(&content, &server_info);

    let r = root.children[0].split_leaf_node_by_single_term(token, &server_info);
    //println!("{:?}", &r);
    assert!(r.len() >= 2);
    assert!(r[1].is_hit);
    // TODO The behaviour at here is not stable. This is hacky test case - 2024-Apr

    let r2 = root.children[0].split_leaf_node_by_terms(&tokens, &server_info);
    assert_eq!(r.len(), r2.len());

    root.parse_highlight(&tokens, &server_info);
    println!("{:?}", &root);
}

#[test]
fn test_highlight_single_term_multi_appearance() {
    let _ = env_logger::try_init();
    let server_info = generate_server_info_for_test();
    let content = "使用 git shallow clone 下载并编译 Thunderbird : compile thunderbird".to_string();
    let token = "thunderbird";
    let tokens = [token];
    let mut root = build_tree(&content, &server_info);


    root.parse_highlight(&tokens, &server_info);
    //println!("Parsed result: {:?}", &root);
    root.flattern();
    //println!("Flattern: {:?}", &root);
    assert_eq!(root.children.len(), 4);
    assert!(root.children[1].is_hit);
    assert!(root.children[3].is_hit);
}
*/

#[test]
fn test_highlight_multiple_terms() {
    let _ = env_logger::try_init();
    let server_info = generate_server_info_for_test();
    let content = "使用 git shallow clone 下载并编译 Thunderbird : compile thunderbird with git shallow".to_string();
    let token = "thunderbird";
    let token2 = "git";
    let tokens = [token, token2];
    let mut root = build_tree(&content, &server_info);


    root.parse_highlight(&tokens, &server_info);
    //println!("Parsed result: {:?}", &root);
    root.flattern();
    //println!("Flattern: {:?}", &root);
    assert!(root.children[1].is_hit);
    /*
    assert_eq!(root.children.len(), 4);
    assert!(root.children[3].is_hit);
    */
}
