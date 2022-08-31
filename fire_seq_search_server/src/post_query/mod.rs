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
    let terms_selected: Vec<_> = term_tokens.into_iter()
        .filter(|&s| !nltk.contains(&*String::from(s)))
        .collect();
    println!("{:?}", &terms_selected);
    let span_start = "<span class=\"fireSeqSearchHighlight\">";
    let span_end = "</span>";

    for sentence in blocks {
        let result = recursive_wrap(&sentence, &term_tokens, &span_start, &span_end);
        println!("{}", &result);

    }


    String::from(body)
}

fn recursive_wrap(sentence: &str, term_tokens: &Vec<String>, span_start: &str, span_end: &str) -> String {
    for token in term_tokens {
        // TODO fix this unnecessary copy
        // if nltk.contains(&*String::from(token)) { continue }
        if !sentence.contains(token) {continue}

    }


    String::from("1")
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