use log::{debug, error, info, warn};
use stopwords;
use regex::RegexBuilder;


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
    debug!("Tokens {:?}, match {:?}", term_tokens, &mats_found);
    mats_found.sort_by_key(|k| k.0);
    Some(wrap_text_at_given_spots(sentence, &mats_found,
                                  show_summary_single_line_chars_limit))

}

fn wrap_text_at_given_spots(sentence: &str, mats_found: &Vec<(usize, usize)>,
                            show_summary_single_line_chars_limit: usize) -> String {

    debug!("Wrap with span, origin len={}, match number = {}",
        sentence.len(), mats_found.len());
    let span_start = "<span class=\"fireSeqSearchHighlight\">";
    let span_end = "</span>";

    // let wrap_too_long_segment =
    //     sentence.len() > show_summary_single_line_chars_limit;
    // arbitrary seg
    let too_long_segment_remained_len = show_summary_single_line_chars_limit / 3;



    // I feel that this is not quite elegant
    let mut builder: Vec<String> = Vec::with_capacity(mats_found.len() + 1);
    let mut cursor = 0;
    let mut mat_pos = 0;

    while cursor < sentence.len() && mat_pos < mats_found.len() {
        let highlight_start = mats_found[mat_pos].0;
        if highlight_start < cursor {
            warn!("The {}-th mat skipped, cursor={}, mat={:?}", &mat_pos, &cursor, &mats_found);
            mat_pos += 1;
            continue;
        }
        // [cursor, start) should remain the same
        if cursor > highlight_start {
            error!("Unexpected Cursor = {}, highlight_start = {}", cursor, highlight_start);
        }

        let remain_seg = &sentence[cursor..highlight_start];
        if remain_seg.len() > show_summary_single_line_chars_limit {
            let brief: String = safe_generate_brief_for_too_long_segment(
                &remain_seg, too_long_segment_remained_len);
            builder.push(brief);
            // builder.push(&remain_seg[..too_long_segment_remained_len]);
            // builder.push("...");
            // builder.push(&remain_seg[
            //     remain_seg.len()-too_long_segment_remained_len..]);
        } else {
            builder.push(remain_seg.to_string());
        }

        if mats_found[mat_pos].1 > sentence.len() {
            error!("This match {:?} exceeded the sentence {}",
                &mats_found[mat_pos], sentence.len());
        }
        // let highlight_end = std::cmp::min(mats_found[mat_pos].1, sentence.len());
        let highlight_end = mats_found[mat_pos].1;


        debug!("Wrapping {}-th: ({},{})", mat_pos, highlight_start, highlight_end);
        // [start, end) be wrapped
        builder.push(span_start.to_string());
        let wrapped_word = &sentence[highlight_start..highlight_end];
        debug!("\tWrapping ({})", &wrapped_word);
        builder.push(wrapped_word.to_string());
        builder.push(span_end.to_string());

        //[end..) remains
        cursor = highlight_end;
        mat_pos += 1;
    }

    if cursor < sentence.len() {
        let remain_seg = &sentence[cursor..];
        if remain_seg.len() > show_summary_single_line_chars_limit {
            let brief = safe_generate_brief_for_too_long_segment(
                &remain_seg[..too_long_segment_remained_len], too_long_segment_remained_len
            );
            builder.push(brief);
        } else {
            builder.push(remain_seg.to_string());
        }
    }

    builder.concat()
}

fn safe_generate_brief_for_too_long_segment(remained: &str, too_long_segment_remained_len: usize) -> String {
    // let mut remain_chars = remained.chars();
    let front: String = remained.chars().take(too_long_segment_remained_len).collect();
    let end: String = remained.chars().rev().take(too_long_segment_remained_len).collect();
    let end = end.chars().rev().collect();
    vec![front, end].join("...")
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

        result.push((mat.start(), mat.end()));
        let t: &str = &sentence[mat.start()..mat.end()];
        debug!("Matched ({}) at {},{}", &t, mat.start(),mat.end());
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

use crate::ServerInformation;

pub fn generate_logseq_uri(title: &str, is_page_hit: &bool, server_info: &ServerInformation) -> String {

    return if *is_page_hit {
        let uri = format!("logseq://graph/{}?page={}",
                          server_info.notebook_name, title);
        uri
    } else {
        warn!("Not implemented for journal page yet: {}", title);
        let uri = format!("logseq://graph/{}",
                          server_info.notebook_name);
        uri
    };
    // logseq://graph/logseq_notebook?page=Nov%2026th%2C%202022
}
#[cfg(test)]
mod test_logseq_uri {
    use crate::post_query::generate_logseq_uri;
    use crate::ServerInformation;

    #[test]
    fn test_generate() {
        let server_info = ServerInformation {
            notebook_path: "stub_path".to_string(),
            notebook_name: "logseq_notebook".to_string(),
            enable_journal_query: false,
            show_top_hits: 0,
            show_summary_single_line_chars_limit: 0,
        };

        // Don't encode / at here. It would be processed by serde. - 2022-11-27
        let r = generate_logseq_uri("Games/EU4", &true, &server_info);
        assert_eq!(&r, "logseq://graph/logseq_notebook?page=Games/EU4");

        let r = generate_logseq_uri("Games/赛马娘", &true, &server_info);
        assert_eq!(&r,
        "logseq://graph/logseq_notebook?page=Games/赛马娘");
    }
}