pub fn highlight_keywords_in_body(body: &str, term_tokens: &Vec<String>) -> String {
    let blocks = split_body_to_blocks(body);
    String::from(body)
}

// TODO: current implementation is too naive, I believe it is buggy
pub fn split_body_to_blocks(body: &str) -> Vec<String> {
    let mut result = Vec::new();
    for line in body.lines() {
        // let t = line.trim();
        let t = line.trim_start_matches(&['-', ' ']);
        // println!("trim: {}", t);
        if !t.is_empty() {
            result.push(String::from(t));
        }
    }
    result
}