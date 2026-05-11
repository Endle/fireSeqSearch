use regex::Regex;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub ord: usize,
    pub text: String,
}

pub fn chunk_note(page_title: &str, raw: &str) -> Vec<Chunk> {
    let preprocessed = preprocess(raw);
    let units: Vec<Vec<String>> = split_into_top_level_units(&preprocessed)
        .into_iter()
        .filter(|u| !is_stub_unit(u))
        .collect();

    let mut chunks = Vec::new();
    let mut ord = 0usize;
    let mut buffer: Vec<String> = Vec::new();
    let mut buffer_tokens = 0usize;

    for unit in units {
        let unit_tokens = approx_tokens(&unit.join("\n"));

        if unit_tokens > CAP_TOKENS {
            flush_buffer(page_title, &mut buffer, &mut buffer_tokens, &mut chunks, &mut ord);
            chunks.extend(size_cap(page_title, &unit, &mut ord));
        } else if buffer_tokens > 0 && buffer_tokens + unit_tokens > CAP_TOKENS {
            flush_buffer(page_title, &mut buffer, &mut buffer_tokens, &mut chunks, &mut ord);
            buffer.extend(unit);
            buffer_tokens = unit_tokens;
        } else {
            buffer.extend(unit);
            buffer_tokens += unit_tokens;
        }
    }
    flush_buffer(page_title, &mut buffer, &mut buffer_tokens, &mut chunks, &mut ord);

    chunks
}

fn flush_buffer(
    page_title: &str,
    buffer: &mut Vec<String>,
    buffer_tokens: &mut usize,
    chunks: &mut Vec<Chunk>,
    ord: &mut usize,
) {
    if buffer.is_empty() {
        return;
    }
    let text = format!("# {}\n\n{}", page_title, buffer.join("\n"));
    chunks.push(Chunk { ord: *ord, text });
    *ord += 1;
    buffer.clear();
    *buffer_tokens = 0;
}

/// A unit is "stub" if every line, after stripping bullet markers and whitespace,
/// is empty. Logseq's editor leaves bare `-` lines as placeholders.
pub(crate) fn is_stub_unit(unit: &[String]) -> bool {
    unit.iter().all(|line| {
        let t = line.trim();
        t.is_empty() || t == "-" || t == "*"
    })
}

/// Minimum narrative-content length (in chars) below which a page is not worth
/// summarizing. Below this floor the page body is title-echo, a single
/// `[[wikilink]]`, an abbreviation, or a `Stub now` placeholder — content the
/// chunk index (which embeds `# {title}\n\n{body}`) already covers, and which
/// an LLM can only respond to with `Empty.` or by parroting the title. Either
/// way the resulting summary embedding is noise that competes with real
/// summaries. Heuristic, calibrated on the dev corpus; tune as needed.
pub const SUMMARIZABLE_MIN_CHARS: usize = 16;

/// Length of a page's "narrative" content: the non-stub bullet body, after
/// stripping `[[wikilink]]`/`[text](url)` syntax down to their text, dropping
/// bare URLs and bullet markers, and collapsing whitespace. Properties,
/// frontmatter and advanced-query blocks are already gone via `preprocess`.
pub fn narrative_chars(raw: &str) -> usize {
    let preprocessed = preprocess(raw);
    let body: String = split_into_top_level_units(&preprocessed)
        .into_iter()
        .filter(|u| !is_stub_unit(u))
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
    if body.is_empty() {
        return 0;
    }
    strip_markup(&body).chars().count()
}

/// Deterministic, model-independent gate for whether the summarizer should
/// invoke the LLM on a page. See `SUMMARIZABLE_MIN_CHARS`.
pub fn is_summarizable(raw: &str) -> bool {
    narrative_chars(raw) >= SUMMARIZABLE_MIN_CHARS
}

/// True for summary *text* that carries no topical content: `Empty.`, `""`,
/// `Summary:`, `空页面`, … — what older builds got from models that ignored the
/// (now-removed) "return the empty string" instruction for stub pages. Used to
/// scrub such rows on startup and as a last-ditch guard on fresh summaries.
/// Intentionally tiny; this is a safety net, not a behavioural contract.
pub fn is_junk_summary(summary: &str) -> bool {
    let norm: String = summary
        .trim_matches(|c: char| {
            matches!(c, '"' | '\'' | '(' | ')' | '.' | ':' | '*' | '-') || c.is_whitespace()
        })
        .to_lowercase();
    matches!(
        norm.as_str(),
        "" | "empty"
            | "empty string"
            | "empty page"
            | "empty summary"
            | "summary"
            | "n/a"
            | "na"
            | "none"
            | "no content"
            | "blank"
            | "nothing"
            | "空"
            | "空页面"
            | "空字符串"
            | "无"
            | "无内容"
    )
}

fn strip_markup(s: &str) -> String {
    lazy_static! {
        static ref MD_LINK: Regex = Regex::new(r"\[([^\]]+)\]\([^)]*\)").unwrap();
        static ref WIKILINK: Regex = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
        // Drop only the scheme: a URL's host + path ("uwaterloo.ca/student-
        // success/...") carries the page's topic and should count as content.
        static ref URL_SCHEME: Regex = Regex::new(r"https?://").unwrap();
        static ref BULLET: Regex = Regex::new(r"(?m)^\s*[-*]\s*").unwrap();
        static ref WS: Regex = Regex::new(r"\s+").unwrap();
    }
    let s = MD_LINK.replace_all(s, "$1");
    let s = WIKILINK.replace_all(&s, "$1");
    let s = URL_SCHEME.replace_all(&s, "");
    let s = BULLET.replace_all(&s, "");
    WS.replace_all(s.trim(), " ").into_owned()
}

const TARGET_TOKENS: usize = 400;
const CAP_TOKENS: usize = 600;

fn approx_tokens(s: &str) -> usize {
    s.chars().count() / 4
}

pub fn preprocess(raw: &str) -> String {
    lazy_static! {
        static ref FRONTMATTER: Regex = Regex::new(r"(?s)\A---\n.*?\n---\n?").unwrap();
        static ref ADV_QUERY: Regex =
            Regex::new(r"(?si)#\+BEGIN_QUERY.*?#\+END_QUERY\n?").unwrap();
        static ref PROP_LINE: Regex = Regex::new(r"(?m)^\s*[A-Za-z][\w-]*::\s*.*$").unwrap();
    }
    let s = FRONTMATTER.replace(raw, "");
    let s = ADV_QUERY.replace_all(&s, "");
    let s = PROP_LINE.replace_all(&s, "");
    s.into_owned()
}

fn is_depth0_bullet(line: &str) -> bool {
    line.starts_with("- ") || line.starts_with("* ") || line == "-" || line == "*"
}

pub(crate) fn split_into_top_level_units(text: &str) -> Vec<Vec<String>> {
    let mut units: Vec<Vec<String>> = Vec::new();
    let mut current: Option<Vec<String>> = None;

    for line in text.lines() {
        if is_depth0_bullet(line) {
            if let Some(u) = current.take() {
                if !u.is_empty() {
                    units.push(u);
                }
            }
            current = Some(vec![line.to_owned()]);
        } else if let Some(ref mut u) = current {
            u.push(line.to_owned());
        }
        // lines before any bullet are discarded
    }
    if let Some(u) = current {
        if !u.is_empty() {
            units.push(u);
        }
    }
    units
}

fn size_cap(page_title: &str, unit: &[String], ord: &mut usize) -> Vec<Chunk> {
    let body = unit.join("\n");
    if approx_tokens(&body) <= CAP_TOKENS {
        let c = Chunk { ord: *ord, text: format!("# {}\n\n{}", page_title, body) };
        *ord += 1;
        return vec![c];
    }

    // Split at descendant boundaries, re-emitting the parent bullet as each sub-chunk header.
    let first_line = unit[0].clone();
    let rest = &unit[1..];

    let mut result = Vec::new();
    let mut buffer: Vec<String> = vec![first_line.clone()];
    let mut buf_tokens = approx_tokens(&first_line);

    for line in rest {
        let line_tokens = approx_tokens(line);
        if buf_tokens + line_tokens >= TARGET_TOKENS && buffer.len() > 1 {
            let text = format!("# {}\n\n{}", page_title, buffer.join("\n"));
            result.push(Chunk { ord: *ord, text });
            *ord += 1;
            buffer = vec![first_line.clone(), line.to_owned()];
            buf_tokens = approx_tokens(&first_line) + line_tokens;
        } else {
            buffer.push(line.to_owned());
            buf_tokens += line_tokens;
        }
    }

    // Emit whatever remains (or the whole thing if we never hit the target).
    if buffer.len() > 1 || result.is_empty() {
        let text = format!("# {}\n\n{}", page_title, buffer.join("\n"));
        result.push(Chunk { ord: *ord, text });
        *ord += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_note() {
        assert_eq!(chunk_note("Title", "").len(), 0);
    }

    #[test]
    fn small_bullets_pack_into_one_chunk() {
        let md = "- bullet one\n- bullet two\n- bullet three\n";
        let chunks = chunk_note("MyPage", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.starts_with("# MyPage\n\n- bullet one"));
        assert!(chunks[0].text.contains("bullet two"));
        assert!(chunks[0].text.contains("bullet three"));
    }

    #[test]
    fn stub_bullets_are_dropped() {
        let md = "- real one\n-\n-\n- real two\n-\n";
        let chunks = chunk_note("Page", md);
        assert_eq!(chunks.len(), 1, "stub `-` lines should not produce chunks");
        assert!(chunks[0].text.contains("real one"));
        assert!(chunks[0].text.contains("real two"));
        // body should not contain a bare `-` line
        let body = &chunks[0].text;
        for line in body.lines() {
            assert!(line.trim() != "-", "stub line leaked into chunk: {:?}", body);
        }
    }

    #[test]
    fn page_with_only_stubs_emits_no_chunks() {
        let md = "-\n-\n-\n";
        assert_eq!(chunk_note("Empty", md).len(), 0);
    }


    #[test]
    fn many_small_bullets_split_at_cap() {
        // Each bullet ~80 tokens; ~10 fits under CAP=600. Use 20 to force a split.
        let bullet = format!("- {}\n", "word ".repeat(80));
        let md = bullet.repeat(20);
        let chunks = chunk_note("Big", &md);
        assert!(chunks.len() >= 2, "should split when packed bullets exceed CAP");
        for c in &chunks {
            assert!(c.text.starts_with("# Big\n\n"));
        }
    }

    #[test]
    fn nested_bullets() {
        let md = "- parent\n  - child one\n  - child two\n  - child three\n  - child four\n";
        let chunks = chunk_note("Page", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("child four"));
    }

    #[test]
    fn oversize_top_level() {
        // 10 children each ~300 tokens (1200 chars each)
        let child = format!("  - {}\n", "x".repeat(1200));
        let mut md = String::from("- parent\n");
        for _ in 0..10 {
            md.push_str(&child);
        }
        let chunks = chunk_note("Big", &md);
        assert!(chunks.len() > 1, "oversize unit should produce multiple chunks");
        for chunk in &chunks {
            assert!(chunk.text.starts_with("# Big\n\n"));
        }
    }

    #[test]
    fn summarizable_gate() {
        // Empty / stub-only / properties-only → not summarizable.
        assert!(!is_summarizable(""));
        assert!(!is_summarizable("-\n"));
        assert!(!is_summarizable("title:: System/161\n-\n"));
        assert!(!is_summarizable("file:: [x.pdf](../assets/x.pdf)\nfile-path:: ../assets/x.pdf\n-\n"));
        // Title-echo / single wikilink / placeholder → not summarizable.
        assert!(!is_summarizable("- Bank of America\n-\n"));        // 15 chars
        assert!(!is_summarizable("- [[I DONT KNOW]]\n"));           // 11 chars
        assert!(!is_summarizable("- Stub now\n-\n"));               // 8 chars
        assert!(!is_summarizable("- Met\n- M\n"));                  // 6 chars
        assert!(!is_summarizable("- https://x.io\n-\n"));           // bare host, 4 chars
        // Real, if brief, content → summarizable.
        assert!(is_summarizable("- A [[NP-complete]] problem\n-\n"));       // "A NP-complete problem"
        assert!(is_summarizable("- Journal Template - DONE 厕所纸 20元40个\n"));
        assert!(is_summarizable("- went to mos mos coffee again\n- the latte was better\n"));
        // A bookmark to a specific resource has a meaningful host+path.
        assert!(is_summarizable("- https://uwaterloo.ca/student-success/ask-immigration-consultant\n"));
    }

    #[test]
    fn junk_summary_detection() {
        for s in ["Empty.", "empty", "Empty string.", "Empty page.", "Empty summary.",
                  "Summary:", "\"\"", "(\"\")", "  \"\" ", "空页面", "空字符串", "  None.  ", "n/a", "*Empty*"] {
            assert!(is_junk_summary(s), "should be junk: {s:?}");
        }
        for s in ["Keurig K-Compact coffee maker, bought and refunded.",
                  "Bank of America stock entry.",
                  "Independent Set: an NP-complete problem.",
                  "An empty array in Rust has zero capacity.",   // mentions "empty" but is real
                  "无人机航拍技巧总结。"] {
            assert!(!is_junk_summary(s), "should NOT be junk: {s:?}");
        }
    }

    #[test]
    fn narrative_chars_strips_markup() {
        assert_eq!(narrative_chars("- [[NP-complete]] problem\n"), "NP-complete problem".chars().count());
        assert_eq!(narrative_chars("- see [docs](https://example.com/x)\n"), "see docs".chars().count());
        assert_eq!(narrative_chars("- https://example.com/path\n"), "example.com/path".chars().count());
        assert_eq!(narrative_chars("tags:: foo\n-\n"), 0);
        assert_eq!(narrative_chars(""), 0);
    }

    #[test]
    fn properties_and_frontmatter() {
        let md = "---\ntitle: Test\n---\ntags:: foo bar\n#+BEGIN_QUERY\n{:title \"q\"}\n#+END_QUERY\n- real bullet\n";
        let chunks = chunk_note("Test", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("real bullet"));
        assert!(!chunks[0].text.contains("tags::"));
        assert!(!chunks[0].text.contains("BEGIN_QUERY"));
    }
}
