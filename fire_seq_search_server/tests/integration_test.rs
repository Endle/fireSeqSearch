use fire_seq_search_server::FireSeqSearchHit;
use serde_json;

#[test]
fn serialize_hit() {
    let hit = FireSeqSearchHit{title: String::from("Hello")};
    let json = serde_json::to_string(&hit).unwrap();

    assert_eq!(&json, "{\"title\":\"Hello\"}");
}