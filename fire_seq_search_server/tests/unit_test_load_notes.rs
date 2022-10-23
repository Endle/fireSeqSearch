use fire_seq_search_server::load_notes::read_specific_path;

#[test]
fn load_articles() {
    let r = read_specific_path("tests/resource/pages");
    assert_eq!(r.len(), 6);
    for (title,body) in &r{
        assert!(title.len()>0);
        assert!(body.len()>0);
    }
}