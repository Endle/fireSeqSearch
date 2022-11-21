use fire_seq_search_server::post_query::{highlight_keywords_in_body, split_body_to_blocks};

fn get_english_text() -> String {
    std::fs::read_to_string("tests/resource/pages/International Language, Past, Present & Future by Walter John Clark.md")
        .expect("Should have been able to read the file")
}

#[test]
fn test_empty_key() {
    let text = "Hello World";
    let v = Vec::new();

    let r = highlight_keywords_in_body(text, &v, 120);
    assert_eq!(4,4);

    assert_eq!(&r, "");
}



#[test]
fn test_highlight_wrap() {
    let contents = "使用 git shallow clone 下载并编译 Thunderbird".to_string();
    let v = vec![String::from("thunderbird")];
    let r = highlight_keywords_in_body(&contents, &v, 120);
    assert_eq!(&r, "使用 git shallow clone 下载并编译 <span class=\"fireSeqSearchHighlight\">Thunderbird</span>");
}


#[test]
fn test_split_to_block() {
    // This part is still hacky
    let contents = get_english_text();
    let blocks = split_body_to_blocks(&contents, 120);

    assert_eq!("As an ounce of personal experience is worth a pound of second-hand recital, a brief statement may here be given of the way in which the present writer came to take up Esperanto, and of the experiences which soon led him to the conviction of its absolute practicability and utility.", &blocks[0]);
    assert_eq!("Now, quite apart from the obvious fact that the nations will never agree to give the preference to the language of one of them to the prejudice of the others, this argument involves the 16 suggestion that an artificial language is no easier to learn than a natural one. We thus come to the question of ease as a qualification.", &blocks[12]);
    assert_eq!(14, blocks.len());
}


#[test]
fn test_split_long_article_to_block() {
    let contents = std::fs::read_to_string
        ("tests/resource/pages/feditips.md")
        .expect("Should have been able to read the file");
    let _a = split_body_to_blocks(&contents, 120);

    //I didn't finish the test
}

