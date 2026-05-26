use crate::note_intake::{is_stub_unit, preprocess, split_into_top_level_units, NotebookSoftware};

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
    let preprocessed = preprocess(&NotebookSoftware::Logseq, raw);
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
    let preprocessed = preprocess(&NotebookSoftware::Obsidian, raw);
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

const TARGET_TOKENS: usize = 400;
const CAP_TOKENS: usize = 600;

fn approx_tokens(s: &str) -> usize {
    s.chars().count() / 4
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
    fn properties_and_frontmatter_dont_leak_into_chunks() {
        // End-to-end sanity check that the chunker pipeline drives preprocess
        // correctly. Detailed preprocess unit tests live in note_intake/.
        let md = "---\ntitle: Test\n---\ntags:: foo bar\n#+BEGIN_QUERY\n{:title \"q\"}\n#+END_QUERY\n- real bullet\n";
        let chunks = chunk_note(LOGSEQ, "Test", md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("real bullet"));
        assert!(!chunks[0].text.contains("tags::"));
        assert!(!chunks[0].text.contains("BEGIN_QUERY"));
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
        // post-strip; it should produce no chunks. Gate behavior is tested
        // in indexer::summarizer.
        let md = "![[diagram.svg]]\n";
        assert_eq!(chunk_note(OBSIDIAN, "Page", md).len(), 0);
    }
}
