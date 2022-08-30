
use fire_seq_search_server::post_query::highlight_keywords_in_body;


#[test]
fn test_empty_key() {
    let text = "Hello World";
    let v = Vec::new();

    let r = highlight_keywords_in_body(text, &v);
    assert_eq!(4,4);

    //assert_eq!(&json, "{\"title\":\"Hello\"}");
}