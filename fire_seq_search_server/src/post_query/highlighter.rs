use std::collections::HashSet;
use std::ops::Range;
use log::{debug, error, info, warn};
use regex::RegexBuilder;

use lazy_static::lazy_static;
use crate::post_query::highlighter::HighlightStatusWithWords::{Highlight, Lowlight};
use crate::query_engine::ServerInformation;

lazy_static! {
    static ref STOPWORDS_LIST: HashSet<String> =  crate::language_tools::generate_stopwords_list();
}


// pub for test
#[derive(Debug, Clone)]
pub struct RenderBlock {
    text: String,
    pub children: Vec<RenderBlock>,
    pub is_hit: bool,
    is_link: bool,
}
fn build_block() -> RenderBlock {
    RenderBlock {
        text: String::default(),
        children: Vec::new(),
        is_hit: false,
        is_link: false,
    }
}
fn build_block_with_str(s: String) -> RenderBlock {
    RenderBlock {
        text: s,
        children: Vec::new(),
        is_hit: false,
        is_link: false,
    }
}
impl RenderBlock {
    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.children.is_empty()
    }
    fn check(&self) {
        if !self.text.is_empty() {
            assert!(self.children.is_empty());
        }
        if !self.children.is_empty() {
            assert!(self.text.is_empty());
        }
    }
    /*
     * If there are one or more highlighted terms, return the result (a tree)
     * If we find nothing, return an empty Vector
     */
    // pub for test
    pub fn split_leaf_node_by_single_term(&self, token: &str, server_info: &ServerInformation) ->Vec<RenderBlock>{
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
        let mat = needle.find(&self.text);
        match mat {
            None => {return result;},
            Some(m) => {
                // part 1
                if m.start() > 0 {
                    let s = &self.text[0..m.start()];
                    let b = build_block_with_str(s.to_string());
                    result.push(b);
                }

                // part 2
                let s = &self.text[m.start()..m.end()];
                let mut b = build_block_with_str(s.to_string());
                b.is_hit = true;
                result.push(b);

                // part 3
                let s = &self.text[m.end()..];
                let b = build_block_with_str(s.to_string());
                let blocks_postfix = b.split_leaf_node_by_single_term(token, server_info);
                if blocks_postfix.is_empty() {
                    result.push(b);
                } else {
                    result.extend_from_slice(&blocks_postfix);
                }
            }
        }
        result
    }
    // pub for test
    pub fn split_leaf_node_by_terms(&self, terms: &[&str], server_info: &ServerInformation) ->Vec<RenderBlock>{
        if terms.is_empty() { return Vec::new(); }
        info!("Highlighting token: {:?}", terms);
        let r = self.split_leaf_node_by_single_term(terms[0], server_info);
        if r.is_empty() { return self.split_leaf_node_by_terms(&terms[1..], server_info); }
        let mut result = Vec::new();
        info!("We have {} blocks: {:?}", r.len(), &r);
        for block in r {
            if block.is_hit { result.push(block); }
            else {
                let next_r = block.split_leaf_node_by_terms(&terms[1..], server_info);
                if next_r.is_empty() { result.push(block) ;}
                else {result.extend_from_slice(&next_r);}
            }
        }
        result
    }
    // pub for test
    pub fn parse_highlight(&mut self, terms: &[&str], server_info: &ServerInformation) {
        self.check();
        if self.is_hit { return ; }
        if self.children.is_empty() {
            let child = self.split_leaf_node_by_terms(terms, server_info);
            info!("Children list: {:?}", &child);
            if !child.is_empty() {
                self.children = child;
                self.text = String::default();
            }
        }
        for i in 0..self.children.len() {
            self.children[i].parse_highlight(terms, server_info);
        }
    }
}
// pub for test
pub fn build_tree(body: &str, server_info: &ServerInformation) -> RenderBlock {
    let show_summary_single_line_chars_limit: usize = server_info.show_summary_single_line_chars_limit;
    let blocks: Vec<String> = split_body_to_blocks(body, show_summary_single_line_chars_limit);
    let mut root = build_block();
    for b in blocks {
        let mut child = build_block();
        child.text = b;
        root.children.push(child);
    }
    root
}

pub fn highlight_keywords_in_body(body: &str, term_tokens: &Vec<String>,
                                  server_info: &ServerInformation) -> String {

    let nltk = &STOPWORDS_LIST;

    let terms_selected: Vec<&str> = crate::language_tools::tokenizer::filter_out_stopwords(
        term_tokens, nltk);
    info!("Highlight terms: {:?}", &terms_selected);


    let mut result: Vec<String> = Vec::new();
    let mut tree_root: RenderBlock = build_tree(body, server_info);
    tree_root.parse_highlight(&terms_selected, server_info);

    let show_summary_single_line_chars_limit: usize = server_info.show_summary_single_line_chars_limit;
    let blocks: Vec<String> = split_body_to_blocks(body, show_summary_single_line_chars_limit);
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

pub fn highlight_sentence_with_keywords(sentence: &str,
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


///
///
/// # Arguments
///
/// * `s`: snip from the notes
///
/// returns: String
///
/// # Examples
///
/// ```
/// use fire_seq_search_server::post_query::highlighter::apply_html_escape;
/// let s = "i < j; i > k;";
/// let t = apply_html_escape(s);
/// assert_eq!(&t, "i &lt; j; i &gt; k;");
/// ```
pub fn apply_html_escape(s: &str) -> String {
    let t = html_escape::encode_safe(s);
    t.to_string()
}

enum HighlightStatusWithWords {
    Highlight(String),
    Lowlight(String)
}
fn get_lowlight_brief(s:&str, show_summary_single_line_chars_limit:usize,
                      too_long_segment_remained_len:usize) -> HighlightStatusWithWords {
    if s.len() > show_summary_single_line_chars_limit {
        let brief: String = safe_generate_brief_for_too_long_segment(
            s, too_long_segment_remained_len);
        Lowlight(apply_html_escape(&brief))
    } else {
        Lowlight(apply_html_escape(s) )
    }
}

pub fn wrap_text_at_given_spots(sentence: &str, mats_found: &Vec<(usize, usize)>,
                                show_summary_single_line_chars_limit: usize) -> String {

    debug!("Wrap with span, origin len={}, match number = {}",
        sentence.len(), mats_found.len());
    let span_start = "<span class=\"fireSeqSearchHighlight\">";
    let span_end = "</span>";

    // let wrap_too_long_segment =
    //     sentence.len() > show_summary_single_line_chars_limit;
    // arbitrary seg
    let too_long_segment_remained_len = show_summary_single_line_chars_limit / 3;

    let mut bricks: Vec<HighlightStatusWithWords> = Vec::with_capacity(mats_found.len() + 1);

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

        let remain_seg = safe_string_slice(sentence, cursor..highlight_start);

        bricks.push(
            get_lowlight_brief(remain_seg, show_summary_single_line_chars_limit,
                               too_long_segment_remained_len));

        if mats_found[mat_pos].1 > sentence.len() {
            error!("This match {:?} exceeded the sentence {}",
                &mats_found[mat_pos], sentence.len());
        }
        // let highlight_end = std::cmp::min(mats_found[mat_pos].1, sentence.len());
        let highlight_end = mats_found[mat_pos].1;

        debug!("Wrapping {}-th: ({},{})", mat_pos, highlight_start, highlight_end);
        // [start, end) be wrapped

        let wrapped_word = safe_string_slice(sentence,
                                             highlight_start..highlight_end);
        debug!("\tWrapping ({})", &wrapped_word);
        bricks.push(Highlight(apply_html_escape(wrapped_word)));

        //[end..) remains
        cursor = highlight_end;
        mat_pos += 1;
    }

    if cursor < sentence.len() {
        let remain_seg = safe_string_slice(sentence,cursor..sentence.len());
        bricks.push(
            get_lowlight_brief(remain_seg, show_summary_single_line_chars_limit,
                               too_long_segment_remained_len));
    }

    let mut builder: Vec<String> = Vec::with_capacity(mats_found.len() * 3);
    for x in bricks {
        match x {
            Highlight(s) => {
                builder.push(span_start.to_string());
                builder.push(s);
                builder.push(span_end.to_string());
            },
            Lowlight(s) => {
                builder.push(s);
            }
        }
    }
    builder.concat()
}

/*
This is a temporary mitigation. I'll try to find out why it cuts the char at wrong boundary.
    - Zhenbo Li 2022-12-05
 */
fn safe_string_slice(sentence: &str, range: Range<usize>) -> &str {
    match &sentence.get(range.to_owned()) {
        None => {
            error!("Wrong char boundary, {} at range({:?})", sentence, range);
            ""
        }
        Some(x) => { x }
    }
}

fn safe_generate_brief_for_too_long_segment(remained: &str, too_long_segment_remained_len: usize) -> String {
    // let mut remain_chars = remained.chars();
    let front: String = remained.chars().take(too_long_segment_remained_len).collect();
    let end: String = remained.chars().rev().take(too_long_segment_remained_len).collect();
    let end = end.chars().rev().collect();
    vec![front, end].join("...")
}




// TODO: conjugation is not considered here
pub fn locate_single_keyword<'a>(sentence: &'a str, token: &'a str) -> Vec<(usize,usize)> {
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




// TODO: current implementation is too naive, I believe it is buggy
pub fn split_body_to_blocks(body: &str, show_summary_single_line_chars_limit: usize) -> Vec<String> {

    let mut result: Vec<String> = Vec::new();
    for line in body.lines() {
        let t = line.trim_start_matches(&['-', ' ']);

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
