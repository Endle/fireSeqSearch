use tantivy::HasLen;
use stopwords;

pub fn highlight_keywords_in_body(body: &str, term_tokens: &Vec<String>) -> String {
    use stopwords::Stopwords;
    let blocks = split_body_to_blocks(body);
    //TODO Avoid collect it repeatly
    let mut nltk: std::collections::HashSet<&str> = stopwords::NLTK::stopwords(stopwords::Language::English).unwrap().iter().cloned().collect();
    nltk.insert("span");
    nltk.insert("class");
    nltk.insert("fireSeqSearchHighlight");

//TODO remove unnecessary copy
    let terms_selected: Vec<String> = term_tokens.into_iter()
        .filter(|&s| !nltk.contains(&*String::from(s)))
        .map(|s| String::from(s))
        .collect();
    // println!("{:?}", &terms_selected);


    let mut result = Vec::new();
    for sentence in blocks {
        let r = recursive_wrap(&sentence, &terms_selected);
        // println!("{}", &result);
        if sentence != r {
            result.push(r);
        }
    }
    // println!("{:?}", &result);

    result.join(" ")
}

fn recursive_wrap(sentence: &str, term_tokens: &[String]) -> String {
    if term_tokens.is_empty() {
        return String::from(sentence);
    }
    let span_start = "<span class=\"fireSeqSearchHighlight\">";
    let span_end = "</span>";
    let token = &term_tokens[0];
    if !sentence.contains(token) {
        return recursive_wrap(sentence, &term_tokens[1..]);
    }
    let mut result = Vec::new();
    for seg in sentence.split(token) {
        let r = recursive_wrap(seg, &term_tokens[1..]);
        result.push(r);
    }
    let wrapped = vec![span_start, token, span_end].join("");
    /*
    Linter asked me to change it, but I got
    49 |     let wrapped = vec![span_start, token, span_end].join(..);
   |                                                          ^^ expected `&str`, found struct `RangeFull`
     */
    result.join(&wrapped)
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