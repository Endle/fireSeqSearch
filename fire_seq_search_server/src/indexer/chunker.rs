use regex::Regex;

use crate::query_engine::NotebookSoftware;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub ord: usize,
    pub text: String,
}

/// Top-level entry point. Dispatches on notebook flavour: Logseq notes are
/// bullet trees with their own grammar (`key:: value`, `#+BEGIN_QUERY`,
/// template parents); Obsidian notes are mostly prose with `#` headings.
/// A bullet-only chunker run over an Obsidian vault would emit zero chunks
/// because `split_into_top_level_units` discards everything outside a `-`/`*`
/// unit.
pub fn chunk_note(software: &NotebookSoftware, page_title: &str, raw: &str) -> Vec<Chunk> {
    match software {
        NotebookSoftware::Logseq => chunk_note_logseq(page_title, raw),
        NotebookSoftware::Obsidian => chunk_note_obsidian(page_title, raw),
    }
}

fn chunk_note_logseq(page_title: &str, raw: &str) -> Vec<Chunk> {
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

/// Obsidian chunker. Obsidian notes are mostly prose under `#` headings,
/// with the occasional list. The boundary unit is one heading-delimited
/// section (everything from a `#` line up to the next `#` line at the same
/// or shallower level — treated uniformly as "next heading"). Notes with no
/// headings are a single unit. Each unit is then greedy-packed up to
/// `CAP_TOKENS`; oversized units are split at blank-line paragraph
/// boundaries.
fn chunk_note_obsidian(page_title: &str, raw: &str) -> Vec<Chunk> {
    let preprocessed = preprocess(raw);
    let trimmed = preprocessed.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // Obsidian's convention is one note = one concept; users curate notes at
    // a granularity where the whole body is the unit of retrieval. Embedding
    // a small note as a single chunk gives bge-m3 the full topical context
    // and avoids splitting a 3-paragraph note into 3 chunks that then
    // compete with each other for the same query. Only fall back to the
    // heading-based splitter when the note actually exceeds the cap.
    if approx_tokens(trimmed) <= CAP_TOKENS {
        return vec![Chunk {
            ord: 0,
            text: format!("# {}\n\n{}", page_title, trimmed),
        }];
    }

    let units = split_into_obsidian_units(&preprocessed);
    let units: Vec<String> = units
        .into_iter()
        .map(|u| u.trim_matches('\n').to_string())
        .filter(|u| !u.trim().is_empty())
        .collect();

    let mut chunks = Vec::new();
    let mut ord = 0usize;
    let mut buffer = String::new();
    let mut buffer_tokens = 0usize;

    let flush = |buffer: &mut String, buffer_tokens: &mut usize, chunks: &mut Vec<Chunk>, ord: &mut usize| {
        if buffer.trim().is_empty() {
            buffer.clear();
            *buffer_tokens = 0;
            return;
        }
        let text = format!("# {}\n\n{}", page_title, buffer.trim_end());
        chunks.push(Chunk { ord: *ord, text });
        *ord += 1;
        buffer.clear();
        *buffer_tokens = 0;
    };

    for unit in units {
        let unit_tokens = approx_tokens(&unit);

        if unit_tokens > CAP_TOKENS {
            flush(&mut buffer, &mut buffer_tokens, &mut chunks, &mut ord);
            chunks.extend(split_obsidian_unit(page_title, &unit, &mut ord));
        } else if buffer_tokens > 0 && buffer_tokens + unit_tokens > CAP_TOKENS {
            flush(&mut buffer, &mut buffer_tokens, &mut chunks, &mut ord);
            buffer.push_str(&unit);
            buffer.push('\n');
            buffer_tokens = unit_tokens;
        } else {
            if !buffer.is_empty() {
                buffer.push_str("\n\n");
            }
            buffer.push_str(&unit);
            buffer_tokens += unit_tokens;
        }
    }
    flush(&mut buffer, &mut buffer_tokens, &mut chunks, &mut ord);

    chunks
}

/// Cut an Obsidian note at `#`-heading lines. A line beginning with one or
/// more `#` followed by a space (ATX heading) starts a new unit; the heading
/// line itself is included in the unit so the chunk text carries the section
/// title. Lines before the first heading form the leading unit.
pub(crate) fn split_into_obsidian_units(text: &str) -> Vec<String> {
    let mut units: Vec<String> = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        if is_atx_heading(line) {
            if !current.trim().is_empty() {
                units.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            current.push_str(line);
            current.push('\n');
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    if !current.trim().is_empty() {
        units.push(current);
    }
    units
}

fn is_atx_heading(line: &str) -> bool {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return false;
    }
    let after_hashes = t.trim_start_matches('#');
    // Must have a space (or end-of-line) after the `#` run; `#tag` is not a heading.
    matches!(after_hashes.chars().next(), Some(' ') | Some('\t'))
}

/// Split an oversized Obsidian unit at blank-line paragraph boundaries,
/// emitting the leading heading (if any) at the top of each sub-chunk.
fn split_obsidian_unit(page_title: &str, unit: &str, ord: &mut usize) -> Vec<Chunk> {
    let mut lines = unit.lines();
    let first = lines.next().unwrap_or("");
    let heading = if is_atx_heading(first) { Some(first) } else { None };
    let rest_lines: Vec<&str> = if heading.is_some() {
        lines.collect()
    } else {
        unit.lines().collect()
    };

    // Group rest into blank-line-separated paragraphs.
    let mut paragraphs: Vec<String> = Vec::new();
    let mut para = String::new();
    for line in rest_lines {
        if line.trim().is_empty() {
            if !para.trim().is_empty() {
                paragraphs.push(std::mem::take(&mut para));
            }
        } else {
            para.push_str(line);
            para.push('\n');
        }
    }
    if !para.trim().is_empty() {
        paragraphs.push(para);
    }
    if paragraphs.is_empty() {
        // Heading-only or empty — return a single chunk with just the heading
        // so the section title still reaches retrieval.
        if let Some(h) = heading {
            let c = Chunk {
                ord: *ord,
                text: format!("# {}\n\n{}", page_title, h),
            };
            *ord += 1;
            return vec![c];
        }
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut buf = String::new();
    let mut buf_tokens = 0usize;
    let heading_tokens = heading.map(approx_tokens).unwrap_or(0);

    for p in paragraphs {
        let p_tokens = approx_tokens(&p);
        if buf_tokens > 0 && buf_tokens + p_tokens > CAP_TOKENS {
            let body = match heading {
                Some(h) => format!("{}\n\n{}", h, buf.trim_end()),
                None => buf.trim_end().to_string(),
            };
            chunks.push(Chunk {
                ord: *ord,
                text: format!("# {}\n\n{}", page_title, body),
            });
            *ord += 1;
            buf.clear();
            buf_tokens = 0;
        }

        if p_tokens > CAP_TOKENS {
            // A single paragraph larger than the cap — hard split on lines.
            for line in p.lines() {
                let line_tokens = approx_tokens(line);
                if buf_tokens + heading_tokens + line_tokens > CAP_TOKENS && !buf.is_empty() {
                    let body = match heading {
                        Some(h) => format!("{}\n\n{}", h, buf.trim_end()),
                        None => buf.trim_end().to_string(),
                    };
                    chunks.push(Chunk {
                        ord: *ord,
                        text: format!("# {}\n\n{}", page_title, body),
                    });
                    *ord += 1;
                    buf.clear();
                    buf_tokens = 0;
                }
                buf.push_str(line);
                buf.push('\n');
                buf_tokens += line_tokens;
            }
            continue;
        }

        buf.push_str(&p);
        if !buf.ends_with('\n') {
            buf.push('\n');
        }
        buf_tokens += p_tokens;
    }

    if !buf.trim().is_empty() {
        let body = match heading {
            Some(h) => format!("{}\n\n{}", h, buf.trim_end()),
            None => buf.trim_end().to_string(),
        };
        chunks.push(Chunk {
            ord: *ord,
            text: format!("# {}\n\n{}", page_title, body),
        });
        *ord += 1;
    }

    chunks
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

/// Length of a page's "narrative" content: the non-stub bullet body (Logseq)
/// or the heading-section prose (Obsidian), after stripping
/// `[[wikilink]]`/`[text](url)` syntax down to their text, dropping bare URLs
/// and bullet markers, and collapsing whitespace. Properties, frontmatter and
/// advanced-query blocks are already gone via `preprocess`.
pub fn narrative_chars(software: &NotebookSoftware, raw: &str) -> usize {
    let preprocessed = preprocess(raw);
    let body = match software {
        NotebookSoftware::Logseq => split_into_top_level_units(&preprocessed)
            .into_iter()
            .filter(|u| !is_stub_unit(u))
            .flatten()
            .collect::<Vec<_>>()
            .join("\n"),
        NotebookSoftware::Obsidian => preprocessed,
    };
    if body.is_empty() {
        return 0;
    }
    strip_markup(&body).chars().count()
}

/// Deterministic, model-independent gate for whether the summarizer should
/// invoke the LLM on a page. See `SUMMARIZABLE_MIN_CHARS`.
pub fn is_summarizable(software: &NotebookSoftware, raw: &str) -> bool {
    narrative_chars(software, raw) >= SUMMARIZABLE_MIN_CHARS
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
        // Eat any leading bullet/whitespace in front of `#+BEGIN_QUERY` so the
        // block strip doesn't leave an orphan `\t- ` that merges with the next
        // line. Without this, a Logseq template like
        //     - Journal Template
        //         - #+BEGIN_QUERY ... #+END_QUERY
        //         - https://...
        // produces a corrupted `- \t- https://...` line because the prefix on
        // the BEGIN_QUERY line survives and the trailing newline gets eaten.
        static ref ADV_QUERY: Regex =
            Regex::new(r"(?msi)^[ \t]*(?:[-*]\s*)?#\+BEGIN_QUERY.*?#\+END_QUERY[ \t]*\n?").unwrap();
        // Match Logseq property lines including when they're attached to a
        // bullet, e.g. `- query-table:: false` or `\t- id:: abc-123`. The
        // optional `[-*]\s+` after leading whitespace is the only difference
        // from the original; without it any property prefixed with `- ` slips
        // through and leaks into the chunk excerpt.
        static ref PROP_LINE: Regex =
            Regex::new(r"(?m)^\s*(?:[-*]\s+)?[A-Za-z][\w-]*::\s*.*$").unwrap();
        // Logseq emits SCHEDULED/DEADLINE/CLOSED as continuation lines under
        // a task-state bullet (DONE/TODO/DOING/NOW/LATER/CANCELED/WAITING).
        // Strip only the timestamp continuation; keep the task line intact so
        // task state still reaches retrieval. Matching the task line as
        // context (rather than stripping bare `SCHEDULED:` lines) avoids
        // accidentally eating user prose that happens to type the same word.
        static ref TASK_TIMESTAMP: Regex = Regex::new(
            r"(?m)^(?P<task>\s*[-*]\s+(?:DONE|TODO|DOING|NOW|LATER|CANCELED|WAITING)\b[^\n]*)\n(?:\s*(?:SCHEDULED|DEADLINE|CLOSED):\s*<[^>]+>\s*\n?)+",
        )
        .unwrap();
        // Image embeds in both syntaxes contribute nothing to retrieval and
        // crowd snippets — an Obsidian page that starts with
        // `![[diagram.svg|align:center|550]]` would otherwise return the embed
        // markup as its top_snippet. We strip the whole embed (alt text
        // included for the standard MD form), not just the URL, because the
        // alt is usually a filename like "Pasted image 20240101.png" — noise.
        static ref IMG_WIKI: Regex = Regex::new(r"!\[\[[^\]]*\]\]").unwrap();
        static ref IMG_MD: Regex = Regex::new(r"!\[[^\]]*\]\([^)]*\)").unwrap();
    }
    let s = FRONTMATTER.replace(raw, "");
    let s = ADV_QUERY.replace_all(&s, "");
    let s = PROP_LINE.replace_all(&s, "");
    let s = TASK_TIMESTAMP.replace_all(&s, "$task\n");
    let s = IMG_WIKI.replace_all(&s, "");
    let s = IMG_MD.replace_all(&s, "");
    unwrap_template_bullets(&s)
}

/// Logseq template parent bullets ("- Journal Template") are boilerplate
/// headers the template engine inserts; they're not content, but the
/// content the user wrote *under* them is. Drop the parent line and dedent
/// its descendants by the first descendant's indent so real notes are
/// promoted to depth-0 bullets and reach the LLM unprefixed.
///
/// Match is exact (case-insensitive, trimmed) on the bullet body — a real
/// content bullet like `- Journal Template - DONE 厕所纸 20元40个` is left
/// alone because its body isn't *just* the template name.
fn unwrap_template_bullets(s: &str) -> String {
    const TEMPLATE_NAMES: &[&str] = &["journal template"];
    let lines: Vec<&str> = s.lines().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if is_depth0_bullet(line) && is_template_header(line, TEMPLATE_NAMES) {
            i += 1;
            let mut indent: Option<usize> = None;
            while i < lines.len() && !is_depth0_bullet(lines[i]) {
                let l = lines[i];
                let leading = l.bytes().take_while(|b| *b == b' ' || *b == b'\t').count();
                if indent.is_none() && !l.trim().is_empty() {
                    indent = Some(leading);
                }
                let strip = indent.unwrap_or(0).min(leading);
                out.push_str(&l[strip..]);
                out.push('\n');
                i += 1;
            }
        } else {
            out.push_str(line);
            out.push('\n');
            i += 1;
        }
    }
    out
}

fn is_template_header(line: &str, names: &[&str]) -> bool {
    let body = line.trim_start_matches(|c: char| c == '-' || c == '*' || c.is_whitespace());
    let normalized = body.trim().to_lowercase();
    names.iter().any(|n| *n == normalized)
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

    const LOGSEQ: &NotebookSoftware = &NotebookSoftware::Logseq;
    const OBSIDIAN: &NotebookSoftware = &NotebookSoftware::Obsidian;

    #[test]
    fn empty_note() {
        assert_eq!(chunk_note(LOGSEQ, "Title", "").len(), 0);
    }

    #[test]
    fn small_bullets_pack_into_one_chunk() {
        let md = "- bullet one\n- bullet two\n- bullet three\n";
        let chunks = chunk_note(LOGSEQ, "MyPage", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.starts_with("# MyPage\n\n- bullet one"));
        assert!(chunks[0].text.contains("bullet two"));
        assert!(chunks[0].text.contains("bullet three"));
    }

    #[test]
    fn stub_bullets_are_dropped() {
        let md = "- real one\n-\n-\n- real two\n-\n";
        let chunks = chunk_note(LOGSEQ, "Page", md);
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
        assert_eq!(chunk_note(LOGSEQ, "Empty", md).len(), 0);
    }


    #[test]
    fn many_small_bullets_split_at_cap() {
        // Each bullet ~80 tokens; ~10 fits under CAP=600. Use 20 to force a split.
        let bullet = format!("- {}\n", "word ".repeat(80));
        let md = bullet.repeat(20);
        let chunks = chunk_note(LOGSEQ, "Big", &md);
        assert!(chunks.len() >= 2, "should split when packed bullets exceed CAP");
        for c in &chunks {
            assert!(c.text.starts_with("# Big\n\n"));
        }
    }

    #[test]
    fn nested_bullets() {
        let md = "- parent\n  - child one\n  - child two\n  - child three\n  - child four\n";
        let chunks = chunk_note(LOGSEQ, "Page", md);
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
        let chunks = chunk_note(LOGSEQ, "Big", &md);
        assert!(chunks.len() > 1, "oversize unit should produce multiple chunks");
        for chunk in &chunks {
            assert!(chunk.text.starts_with("# Big\n\n"));
        }
    }

    #[test]
    fn summarizable_gate() {
        // Empty / stub-only / properties-only → not summarizable.
        assert!(!is_summarizable(LOGSEQ, ""));
        assert!(!is_summarizable(LOGSEQ, "-\n"));
        assert!(!is_summarizable(LOGSEQ, "title:: System/161\n-\n"));
        assert!(!is_summarizable(LOGSEQ, "file:: [x.pdf](../assets/x.pdf)\nfile-path:: ../assets/x.pdf\n-\n"));
        // Title-echo / single wikilink / placeholder → not summarizable.
        assert!(!is_summarizable(LOGSEQ, "- Bank of America\n-\n"));        // 15 chars
        assert!(!is_summarizable(LOGSEQ, "- [[I DONT KNOW]]\n"));           // 11 chars
        assert!(!is_summarizable(LOGSEQ, "- Stub now\n-\n"));               // 8 chars
        assert!(!is_summarizable(LOGSEQ, "- Met\n- M\n"));                  // 6 chars
        assert!(!is_summarizable(LOGSEQ, "- https://x.io\n-\n"));           // bare host, 4 chars
        // Real, if brief, content → summarizable.
        assert!(is_summarizable(LOGSEQ, "- A [[NP-complete]] problem\n-\n"));       // "A NP-complete problem"
        assert!(is_summarizable(LOGSEQ, "- Journal Template - DONE 厕所纸 20元40个\n"));
        assert!(is_summarizable(LOGSEQ, "- went to mos mos coffee again\n- the latte was better\n"));
        // A bookmark to a specific resource has a meaningful host+path.
        assert!(is_summarizable(LOGSEQ, "- https://uwaterloo.ca/student-success/ask-immigration-consultant\n"));
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
        assert_eq!(narrative_chars(LOGSEQ, "- [[NP-complete]] problem\n"), "NP-complete problem".chars().count());
        assert_eq!(narrative_chars(LOGSEQ, "- see [docs](https://example.com/x)\n"), "see docs".chars().count());
        assert_eq!(narrative_chars(LOGSEQ, "- https://example.com/path\n"), "example.com/path".chars().count());
        assert_eq!(narrative_chars(LOGSEQ, "tags:: foo\n-\n"), 0);
        assert_eq!(narrative_chars(LOGSEQ, ""), 0);
    }

    #[test]
    fn properties_and_frontmatter() {
        let md = "---\ntitle: Test\n---\ntags:: foo bar\n#+BEGIN_QUERY\n{:title \"q\"}\n#+END_QUERY\n- real bullet\n";
        let chunks = chunk_note(LOGSEQ, "Test", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("real bullet"));
        assert!(!chunks[0].text.contains("tags::"));
        assert!(!chunks[0].text.contains("BEGIN_QUERY"));
    }

    #[test]
    fn journal_template_header_is_stripped_and_children_promoted() {
        let md = "- Journal Template\n    - real daily record\n- other top bullet\n";
        let out = preprocess(md);
        assert!(!out.contains("Journal Template"));
        assert!(out.contains("- real daily record"));
        assert!(out.contains("- other top bullet"));
        // The child should be promoted to depth-0 (no leading whitespace).
        let mut saw_promoted = false;
        for line in out.lines() {
            if line == "- real daily record" {
                saw_promoted = true;
            }
        }
        assert!(saw_promoted, "child should be dedented to depth-0: {out:?}");
    }

    #[test]
    fn journal_template_is_case_insensitive() {
        let out = preprocess("- journal template\n    - kept\n");
        assert!(!out.to_lowercase().contains("journal template"));
        assert!(out.contains("- kept"));
    }

    #[test]
    fn journal_template_with_inline_content_is_not_touched() {
        // Content bullet that happens to start with "Journal Template" — leave alone.
        let md = "- Journal Template - DONE 厕所纸 20元40个\n";
        let out = preprocess(md);
        assert!(out.contains("Journal Template - DONE"));
    }

    #[test]
    fn journal_template_with_no_children_just_drops() {
        let md = "- Journal Template\n- real bullet\n";
        let out = preprocess(md);
        assert!(!out.contains("Journal Template"));
        assert!(out.contains("- real bullet"));
    }

    #[test]
    fn adv_query_with_bullet_prefix_does_not_leave_orphan() {
        // The real-world Logseq pattern: a `- Journal Template` parent whose
        // first child is `\t- #+BEGIN_QUERY ... #+END_QUERY` and second child
        // is a content URL bullet. Before the ADV_QUERY anchor fix, the
        // orphan `\t- ` prefix on the BEGIN_QUERY line merged with the next
        // line producing `- \t- https://...` after the template unwrap.
        let md = "- Journal Template\n\
                  \t- #+BEGIN_QUERY\n\
                  \t  {:title \"q\"}\n\
                  \t  #+END_QUERY\n\
                  \t- https://example.com/x\n\
                  - real top bullet\n";
        let out = preprocess(md);
        // The query block is gone.
        assert!(!out.contains("BEGIN_QUERY"));
        // No double-bullet artifact on the URL line.
        assert!(!out.contains("- - https://"), "double bullet leaked: {out:?}");
        assert!(!out.contains("-\t- https"), "tab-merged bullet leaked: {out:?}");
        // The URL bullet survives, dedented to depth-0.
        assert!(out.contains("- https://example.com/x"));
        assert!(out.contains("- real top bullet"));
    }

    #[test]
    fn task_state_kept_scheduled_line_stripped() {
        // Logseq emits `SCHEDULED: <date>` as a continuation line under a
        // task-state bullet. Keep the task (it conveys state to the LLM);
        // drop the timestamp continuation.
        let md = "- DONE 确定旅游计划\n  SCHEDULED: <2023-10-16 Mon>\n- next bullet\n";
        let out = preprocess(md);
        assert!(out.contains("- DONE 确定旅游计划"), "task line stripped: {out:?}");
        assert!(!out.contains("SCHEDULED"), "schedule line kept: {out:?}");
        assert!(out.contains("- next bullet"));
    }

    #[test]
    fn task_state_with_multiple_timestamp_lines() {
        // A task bullet may have several continuation timestamps. All should go.
        let md = "- TODO ship it\n  SCHEDULED: <2023-10-16 Mon>\n  DEADLINE: <2023-10-30 Mon>\n";
        let out = preprocess(md);
        assert!(out.contains("- TODO ship it"));
        assert!(!out.contains("SCHEDULED"));
        assert!(!out.contains("DEADLINE"));
    }

    #[test]
    fn bullet_prefixed_property_lines_are_stripped() {
        // Logseq emits properties attached to a bullet, e.g.
        //     - query-table:: false
        //         - query-table:: false (deeper indent)
        //         - id:: 64e8ba26-...
        // The original PROP_LINE only matched lines that started with the
        // property name (after optional whitespace); the bullet prefix made
        // the regex skip, so noise survived.
        let md = "- query-table:: false\n\t- query-table:: false\n  - id:: 64e8ba26-1234\n- real bullet\n";
        let out = preprocess(md);
        assert!(!out.contains("query-table::"), "bullet-prefixed property survived: {out:?}");
        assert!(!out.contains("id::"), "id:: property survived: {out:?}");
        assert!(out.contains("- real bullet"));
    }

    #[test]
    fn bare_scheduled_line_without_task_context_is_preserved() {
        // User prose that happens to type "SCHEDULED: <something>" with no
        // preceding task bullet must NOT be stripped — it's content.
        let md = "- some note about org-mode\n- SCHEDULED: <2023-01-01> is the keyword\n";
        let out = preprocess(md);
        assert!(out.contains("SCHEDULED:"), "user prose got eaten: {out:?}");
    }

    #[test]
    fn journal_template_preserves_nested_structure_after_dedent() {
        // First child at 4 spaces, grandchild at 8 spaces → after stripping
        // the 4-space first-child indent, grandchild keeps its remaining
        // 4-space indent and stays a child of the promoted bullet.
        let md = "- Journal Template\n    - parent kept\n        - grand kept\n";
        let out = preprocess(md);
        assert!(out.contains("- parent kept"));
        assert!(out.contains("    - grand kept"));
    }

    // ---- Obsidian path ------------------------------------------------------

    #[test]
    fn obsidian_prose_note_emits_a_chunk() {
        // The killer case the Logseq chunker silently fails on: a typical
        // Obsidian note is paragraphs with no `-` bullets. split_into_top_level_units
        // would discard everything; the Obsidian path must keep the prose.
        let md = "This note describes the Maillard reaction.\n\
                  It is a non-enzymatic browning reaction.\n";
        let chunks = chunk_note(OBSIDIAN, "Maillard", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.starts_with("# Maillard\n\n"));
        assert!(chunks[0].text.contains("Maillard reaction"));
        assert!(chunks[0].text.contains("non-enzymatic browning"));
    }

    #[test]
    fn obsidian_splits_on_headings() {
        let md = "Intro paragraph.\n\n\
                  # Section A\n\
                  Content of section A.\n\n\
                  # Section B\n\
                  Content of section B.\n";
        let chunks = chunk_note(OBSIDIAN, "Page", md);
        // Three sections (intro + A + B), all small, greedy-packed into one chunk.
        assert_eq!(chunks.len(), 1);
        let t = &chunks[0].text;
        assert!(t.contains("Intro paragraph"));
        assert!(t.contains("# Section A"));
        assert!(t.contains("# Section B"));
    }

    #[test]
    fn obsidian_oversized_section_splits_at_paragraphs() {
        // One heading with a body large enough to blow the cap; the splitter
        // must produce multiple chunks, each carrying the heading.
        let para = format!("{}\n\n", "word ".repeat(600)); // ~3000 chars => ~750 tokens
        let md = format!("# Big Section\n{}{}", para, para);
        let chunks = chunk_note(OBSIDIAN, "Page", &md);
        assert!(chunks.len() >= 2, "got {} chunks", chunks.len());
        for c in &chunks {
            assert!(c.text.starts_with("# Page\n\n"));
            assert!(c.text.contains("# Big Section"), "lost heading in {:?}", c.text);
        }
    }

    #[test]
    fn obsidian_hashtag_is_not_a_heading() {
        // `#tag` at the start of a line is a tag, not an ATX heading (no space).
        let md = "Note with a #productivity tag.\nMore prose.\n";
        let chunks = chunk_note(OBSIDIAN, "Page", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("#productivity"));
    }

    #[test]
    fn obsidian_frontmatter_is_stripped() {
        let md = "---\ntitle: Foo\ntags: [a, b]\n---\nBody text about the topic.\n";
        let chunks = chunk_note(OBSIDIAN, "Foo", md);
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].text.contains("title: Foo"));
        assert!(chunks[0].text.contains("Body text about the topic"));
    }

    #[test]
    fn obsidian_empty_note_emits_nothing() {
        assert_eq!(chunk_note(OBSIDIAN, "Page", "").len(), 0);
        assert_eq!(chunk_note(OBSIDIAN, "Page", "\n\n  \n").len(), 0);
    }

    #[test]
    fn obsidian_summarizable_gate_uses_prose() {
        // Prose-only notes (no bullets) must pass the gate when they have
        // narrative content. The Logseq path would count zero chars here.
        assert!(is_summarizable(OBSIDIAN, "A short note about coffee."));
        assert!(!is_summarizable(OBSIDIAN, ""));
        assert!(!is_summarizable(OBSIDIAN, "tiny"));
    }

    #[test]
    fn obsidian_small_note_is_a_single_chunk() {
        // The Oort Cloud / Kuiper Belt / Asteroid Belt style: a few short
        // sections (image + a couple of paragraphs) that all fit under
        // CAP_TOKENS. Obsidian users curate notes at concept-granularity;
        // splitting these into per-heading chunks fragments retrieval.
        let md = "Intro.\n\n# Section A\nBody A.\n\n# Section B\nBody B.\n";
        let chunks = chunk_note(OBSIDIAN, "Page", md);
        assert_eq!(chunks.len(), 1, "small Obsidian notes must be one chunk");
        // Must carry the whole body.
        assert!(chunks[0].text.contains("Intro"));
        assert!(chunks[0].text.contains("Body A"));
        assert!(chunks[0].text.contains("Body B"));
    }

    #[test]
    fn obsidian_oversized_note_still_splits() {
        // Above CAP, the heading-based splitter takes over.
        let para = format!("{}\n\n", "word ".repeat(600));
        let md = format!("# A\n{}# B\n{}", para, para);
        let chunks = chunk_note(OBSIDIAN, "Page", &md);
        assert!(chunks.len() >= 2, "oversized notes must split, got {}", chunks.len());
    }

    #[test]
    fn obsidian_image_embeds_stripped() {
        // Both wikilink and markdown image syntaxes. After stripping the
        // image, the chunk body should contain only the prose.
        let md = "![[solarSystem_sizes.svg|align:center|550]]\n\nThe Oort Cloud is far.\n\n![alt](other.png)\nMore prose.\n";
        let chunks = chunk_note(OBSIDIAN, "OortCloud", md);
        assert_eq!(chunks.len(), 1);
        let t = &chunks[0].text;
        assert!(!t.contains("solarSystem_sizes"), "wikilink image leaked: {t:?}");
        assert!(!t.contains("![alt]"), "MD image leaked: {t:?}");
        assert!(!t.contains("other.png"), "MD image URL leaked: {t:?}");
        assert!(t.contains("The Oort Cloud is far"));
        assert!(t.contains("More prose"));
    }

    #[test]
    fn obsidian_image_only_note_is_empty() {
        // A note that's nothing but an image embed has no narrative content
        // post-strip; it should produce no chunks (and no summary call).
        let md = "![[diagram.svg]]\n";
        assert_eq!(chunk_note(OBSIDIAN, "Page", md).len(), 0);
        assert!(!is_summarizable(OBSIDIAN, md));
    }
}
