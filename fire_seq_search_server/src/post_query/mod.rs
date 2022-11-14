use log::{debug, error, info};
use stopwords;
use regex::RegexBuilder;
use tantivy::HasLen;

pub fn highlight_keywords_in_body(body: &str, term_tokens: &Vec<String>,
                                  show_summary_single_line_chars_limit: usize) -> String {

    let blocks = split_body_to_blocks(body, show_summary_single_line_chars_limit);
    let nltk = generate_stopwords_list();

    let term_ref: Vec<&str> = term_tokens.iter().map(|s| &**s).collect();
    let terms_selected: Vec<&str> = term_ref.into_iter()
        .filter(|&s| !nltk.contains(s))
        .collect();
    info!("Highlight terms: {:?}", &terms_selected);


    let mut result: Vec<String> = Vec::new();
    for sentence in blocks {
        let sentence_highlight = highlight_sentence_with_keywords(
            &sentence,
            &terms_selected,
            show_summary_single_line_chars_limit
        );
        match sentence_highlight {
            Some(x) => result.push(x),
            None => ()
        }
    }

    result.join(" ")
}

fn highlight_sentence_with_keywords(sentence: &String,
                                    term_tokens: &Vec<&str>,
                                    show_summary_single_line_chars_limit: usize) -> Option<String> {

    let mut mats_found: Vec<(usize,usize)> = Vec::new();
    for t in term_tokens {
        let mut r = locate_single_keyword(sentence, t);
        mats_found.append(&mut r);
    }
    if mats_found.is_empty() {
        return None;
    }
    Some(wrap_text_at_given_spots(sentence, &mut mats_found,
                                  show_summary_single_line_chars_limit))
    // println!("Tokens {:?}, match {:?}", term_tokens, &hits_found);
}

fn wrap_text_at_given_spots(sentence: &String, mats_found: &mut Vec<(usize, usize)>,
                            show_summary_single_line_chars_limit: usize) -> String {


    let span_start = "<span class=\"fireSeqSearchHighlight\">";
    let span_end = "</span>";

    // let wrap_too_long_segment =
    //     sentence.len() > show_summary_single_line_chars_limit;
    // arbitrary seg
    let too_long_segment_remained_len = show_summary_single_line_chars_limit / 3;

    mats_found.sort_by_key(|k| k.0);

    let mut builder: Vec<&str> = Vec::with_capacity(mats_found.len() + 1);
    let mut cursor = 0;
    let mut mat_pos = 0;

    while cursor < sentence.len() && mat_pos < mats_found.len() {
        let highlight_start = mats_found[mat_pos].0;
        if highlight_start < cursor {
            mat_pos += 1;
            continue;
        }
        // [cursor, start) should remain the same
        let remain_seg = &sentence[cursor..highlight_start];
        if remain_seg.len() > show_summary_single_line_chars_limit {
            builder.push(&remain_seg[..too_long_segment_remained_len]);
            builder.push("...");
            builder.push(&remain_seg[
                remain_seg.len()-too_long_segment_remained_len..]);
        } else {
            builder.push(remain_seg);
        }


        let highlight_end = std::cmp::min(mats_found[mat_pos].1, sentence.len());

        // [start, end) be wrapped
        builder.push(span_start);
        builder.push(&sentence[highlight_start..highlight_end]);
        builder.push(span_end);

        //[end..) remains
        cursor = highlight_end;
    }
    if cursor < sentence.len() {
        builder.push(&sentence[cursor..]);
    }

    builder.join("")
}





// TODO: conjugation is not considered here
fn locate_single_keyword<'a>(sentence: &'a str, token: &'a str) -> Vec<(usize,usize)> {
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
    for mat in needle.find_iter(sentence) {
        debug!("{:?}", &mat);
        result.push((mat.start(), mat.end()));
    }
    result
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
pub fn split_body_to_blocks(body: &str, show_summary_single_line_chars_limit: usize) -> Vec<String> {

    let mut result: Vec<String> = Vec::new();
    for line in body.lines() {
        // let t = line.trim();
        let t = line.trim_start_matches(&['-', ' ']);
        // println!("trim: {}", t);

        if t.is_empty() {
            continue;
        }

        if t.len() > show_summary_single_line_chars_limit {
            debug!("We have a long paragraph ({})", t.len());
        }
        result.push(String::from(t));
    }
    result
}