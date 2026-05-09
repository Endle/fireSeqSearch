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
fn is_stub_unit(unit: &[String]) -> bool {
    unit.iter().all(|line| {
        let t = line.trim();
        t.is_empty() || t == "-" || t == "*"
    })
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
    fn properties_and_frontmatter() {
        let md = "---\ntitle: Test\n---\ntags:: foo bar\n#+BEGIN_QUERY\n{:title \"q\"}\n#+END_QUERY\n- real bullet\n";
        let chunks = chunk_note("Test", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("real bullet"));
        assert!(!chunks[0].text.contains("tags::"));
        assert!(!chunks[0].text.contains("BEGIN_QUERY"));
    }
}
