use tantivy::HasLen;
use fire_seq_search_server::post_query::{highlight_keywords_in_body, split_body_to_blocks, split_by_single_token};

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
fn test_highlight() {
    let contents = get_english_text();
    let v = vec![String::from("juxtaposition"), String::from("of"), String::from("pronunciation")];
    let r = highlight_keywords_in_body(&contents, &v, 120);
    assert_eq!(&r, "This was fairly convincing, but still having doubts on the question of <span class=\"fireSeqSearchHighlight\">pronunciation</span>, the writer resolved to attend the Esperanto Congress to be held at Geneva in August 1906. To 14 this end he continued to read Esperanto at odd minutes and took in an Esperanto gazette. About three weeks before the congress he got a member of his family to read aloud to him every day as far as possible a page or two of Esperanto, in order to attune his ear. He never had an opportunity of speaking the language before the congress, except once for a few minutes, when he travelled some distance to attend a meeting of the nearest English group. With all these people it was perfectly easy to converse in the common tongue, <span class=\"fireSeqSearchHighlight\">pronunciation</span> and national idiom being no bar in practice. In the face of these facts it is idle to oppose a universal artificial language on the score of impossibility or inadequacy. The theoretical <span class=\"fireSeqSearchHighlight\">pronunciation</span> difficulty completely crumbled away before the test of practice. The \"war-at-any-price party,\" the whole-hoggers à tous crins (the <span class=\"fireSeqSearchHighlight\">juxtaposition</span> of the two national idioms lends a certain realism, and heightens the effect of each), are therefore driven back on their second line of attack, if the Hibernianism may be excused. \"Yes,\" they say, \"your language may be possible, but, after all, why not learn an existing language, if you've got to learn one anyway?\"");
}

#[test]
fn test_highlight_wrap() {
    let contents = "使用 git shallow clone 下载并编译 Thunderbird".to_string();
    let v = vec![String::from("thunderbird")];
    let r = highlight_keywords_in_body(&contents, &v, 120);
    assert_eq!(&r, "使用 git shallow clone 下载并编译 <span class=\"fireSeqSearchHighlight\">thunderbird</span>");
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


}

#[test]
fn test_split_by_single_token() {
    // not stabled yet
    let r = split_by_single_token("As an ounce of personal experience is worth a pound of", "personal");
    assert_eq!(r.len(), 2);

    let r = split_by_single_token("no such", "exist");
    assert_eq!(r.len(), 1);

    let r = split_by_single_token("母猪都能上树", "上");
    assert_eq!(r.len(), 2);

    let r = split_by_single_token("Это статья для тестов поиска в кириллических символах", "для");
    assert_eq!(r.len(), 2);

    let r = split_by_single_token("head is match", "head");
    assert_eq!(r.len(), 2);
    // println!("{:?}", &r);
}

#[test]
fn test_split_by_single_token_cyrillic() {
    let haystack = "Это статья для тестов поиска в кириллических символах";
    let r = split_by_single_token(haystack, "для");
    assert_eq!(r.len(), 2);


}



fn gen(s: Vec<&str>) -> Vec<String> {
    let mut r = Vec::with_capacity(s.len());
    for i in s {
        r.push(String::from(i));
    }
    r
}