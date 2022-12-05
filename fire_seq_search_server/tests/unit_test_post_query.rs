use fire_seq_search_server::post_query::{highlight_keywords_in_body, highlight_sentence_with_keywords, locate_single_keyword, split_body_to_blocks, wrap_text_at_given_spots};

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

#[test]
fn test_highlight_sentence_with_keywords() {
    let contents = std::fs::read_to_string
        ("tests/resource/pages/咖啡.md")
        .expect("Should have been able to read the file");
    let tokens = vec!["咖啡"];
    let _r = highlight_sentence_with_keywords(&contents, &tokens, 300);
}


#[test]
fn test_wrap_text_at_given_spots() {
    let contents = std::fs::read_to_string
        ("tests/resource/pages/咖啡.md")
        .expect("Should have been able to read the file");

    // Returns the length of this String, in bytes, not chars or graphemes
    // Win 164, mac & linux 163
    assert!(contents.len() == 164 || contents.len() == 163);
    let token = "咖啡";
    assert_eq!(token.len(),6);
    let mats = locate_single_keyword(&contents, token);
    assert_eq!(2, mats.len());

    for m in &mats {
        let left: usize = m.0;
        let right: usize = m.1;
        let sub = &contents[left..right];
        assert_eq!(sub, token);
        assert_eq!(right-left, 6);
    }
    let _r = wrap_text_at_given_spots(&contents, &mats, 320);
}