use log::error;
use stopwords;
use regex::RegexBuilder;

pub fn highlight_keywords_in_body(body: &str, term_tokens: &Vec<String>) -> String {

    let blocks = split_body_to_blocks(body);
    let nltk = generate_stopwords_list();

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

fn generate_stopwords_list<'a>() -> std::collections::HashSet<&'a str> {
    //TODO Avoid collect it repeatedly
    use stopwords::Stopwords;
    let mut nltk: std::collections::HashSet<&str> = stopwords::NLTK::stopwords(stopwords::Language::English).unwrap().iter().cloned().collect();
    nltk.insert("span");
    nltk.insert("class");
    nltk.insert("fireSeqSearchHighlight");

    nltk.insert("theorem");
    nltk.insert("-");
    nltk
}

pub fn recursive_wrap(sentence: &str, term_tokens: &[String]) -> String {
    if term_tokens.is_empty() {
        return String::from(sentence);
    }
    let span_start = "<span class=\"fireSeqSearchHighlight\">";
    let span_end = "</span>";
    let token = &term_tokens[0];
    let segments = split_by_single_token(sentence, token);
    // Found nothing for this token
    if segments.len() <= 1 {
        return recursive_wrap(sentence, &term_tokens[1..]);
    }

    let mut result = Vec::new();
    for seg in segments {
        let r = recursive_wrap(seg, &term_tokens[1..]);
        result.push(r);
    }
    let wrapped = vec![span_start, token, span_end].concat();

    result.join(&wrapped)
}

pub fn split_by_single_token<'a>(sentence: &'a str, token: &'a str) -> Vec<&'a str> {
    let mut result = Vec::new();
    let needle = RegexBuilder::new(token)
        .case_insensitive(true)
        .build();
    let needle = match needle {
        Ok(x) => x,
        Err(e) => {
            error!("Failed({}) to build regex for {}", e, token);
            return result;
        }
    };
    let segs: Vec<&str> = needle.split(sentence).collect();
    for seg in segs {
        result.push(seg);
    }
    result
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