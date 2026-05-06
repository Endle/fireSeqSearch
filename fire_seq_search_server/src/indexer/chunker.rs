use regex::Regex;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub ord: usize,
    pub text: String,
}

pub fn chunk_note(page_title: &str, raw: &str) -> Vec<Chunk> {
    let preprocessed = preprocess(raw);
    let units = split_into_top_level_units(&preprocessed);
    let mut chunks = Vec::new();
    let mut ord = 0usize;
    for unit in units {
        chunks.extend(size_cap(page_title, &unit, &mut ord));
    }
    chunks
}

const TARGET_TOKENS: usize = 400;
const CAP_TOKENS: usize = 600;

fn approx_tokens(s: &str) -> usize {
    s.chars().count() / 4
}

fn preprocess(raw: &str) -> String {
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

fn split_into_top_level_units(text: &str) -> Vec<Vec<String>> {
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
    fn simple_logseq() {
        let md = "- bullet one\n- bullet two\n- bullet three\n";
        let chunks = chunk_note("MyPage", md);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].text.starts_with("# MyPage\n\n- bullet one"));
        assert!(chunks[1].text.contains("bullet two"));
        assert_eq!(chunks[2].ord, 2);
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
    fn properties_and_frontmatter() {
        let md = "---\ntitle: Test\n---\ntags:: foo bar\n#+BEGIN_QUERY\n{:title \"q\"}\n#+END_QUERY\n- real bullet\n";
        let chunks = chunk_note("Test", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("real bullet"));
        assert!(!chunks[0].text.contains("tags::"));
        assert!(!chunks[0].text.contains("BEGIN_QUERY"));
    }
}
