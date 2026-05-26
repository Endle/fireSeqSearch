//! Logseq-specific syntax rules: advanced-query blocks, `key:: value` property
//! lines, task-state timestamp continuations, template-bullet unwrapping, and
//! the depth-0 bullet primitives the chunker reuses for splitting.
//!
//! Adding a new Logseq macro or grammar rule goes here.

use regex::Regex;

/// Strip Logseq-specific syntax. Called by `note_intake::preprocess` after the
/// shared (frontmatter + image-embed) pass.
pub fn strip(input: &str) -> String {
    lazy_static! {
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
    }
    let s = ADV_QUERY.replace_all(input, "");
    let s = PROP_LINE.replace_all(&s, "");
    let s = TASK_TIMESTAMP.replace_all(&s, "$task\n");
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

pub(crate) fn is_depth0_bullet(line: &str) -> bool {
    line.starts_with("- ") || line.starts_with("* ") || line == "-" || line == "*"
}

/// Group consecutive lines under each depth-0 bullet into a single unit. The
/// chunker greedy-packs these units up to its token cap. Lines before any
/// bullet are discarded.
pub fn split_into_top_level_units(text: &str) -> Vec<Vec<String>> {
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

/// A unit is "stub" if every line, after stripping bullet markers and
/// whitespace, is empty. Logseq's editor leaves bare `-` lines as placeholders.
pub fn is_stub_unit(unit: &[String]) -> bool {
    unit.iter().all(|line| {
        let t = line.trim();
        t.is_empty() || t == "-" || t == "*"
    })
}

#[cfg(test)]
mod tests {
    use crate::note_intake::preprocess;
    use crate::note_intake::NotebookSoftware;

    const LOGSEQ: &NotebookSoftware = &NotebookSoftware::Logseq;

    #[test]
    fn journal_template_header_is_stripped_and_children_promoted() {
        let md = "- Journal Template\n    - real daily record\n- other top bullet\n";
        let out = preprocess(LOGSEQ, md);
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
        let out = preprocess(LOGSEQ, "- journal template\n    - kept\n");
        assert!(!out.to_lowercase().contains("journal template"));
        assert!(out.contains("- kept"));
    }

    #[test]
    fn journal_template_with_inline_content_is_not_touched() {
        // Content bullet that happens to start with "Journal Template" — leave alone.
        let md = "- Journal Template - DONE 厕所纸 20元40个\n";
        let out = preprocess(LOGSEQ, md);
        assert!(out.contains("Journal Template - DONE"));
    }

    #[test]
    fn journal_template_with_no_children_just_drops() {
        let md = "- Journal Template\n- real bullet\n";
        let out = preprocess(LOGSEQ, md);
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
        let out = preprocess(LOGSEQ, md);
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
        let out = preprocess(LOGSEQ, md);
        assert!(out.contains("- DONE 确定旅游计划"), "task line stripped: {out:?}");
        assert!(!out.contains("SCHEDULED"), "schedule line kept: {out:?}");
        assert!(out.contains("- next bullet"));
    }

    #[test]
    fn task_state_with_multiple_timestamp_lines() {
        // A task bullet may have several continuation timestamps. All should go.
        let md = "- TODO ship it\n  SCHEDULED: <2023-10-16 Mon>\n  DEADLINE: <2023-10-30 Mon>\n";
        let out = preprocess(LOGSEQ, md);
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
        let out = preprocess(LOGSEQ, md);
        assert!(!out.contains("query-table::"), "bullet-prefixed property survived: {out:?}");
        assert!(!out.contains("id::"), "id:: property survived: {out:?}");
        assert!(out.contains("- real bullet"));
    }

    #[test]
    fn bare_scheduled_line_without_task_context_is_preserved() {
        // User prose that happens to type "SCHEDULED: <something>" with no
        // preceding task bullet must NOT be stripped — it's content.
        let md = "- some note about org-mode\n- SCHEDULED: <2023-01-01> is the keyword\n";
        let out = preprocess(LOGSEQ, md);
        assert!(out.contains("SCHEDULED:"), "user prose got eaten: {out:?}");
    }

    #[test]
    fn journal_template_preserves_nested_structure_after_dedent() {
        // First child at 4 spaces, grandchild at 8 spaces → after stripping
        // the 4-space first-child indent, grandchild keeps its remaining
        // 4-space indent and stays a child of the promoted bullet.
        let md = "- Journal Template\n    - parent kept\n        - grand kept\n";
        let out = preprocess(LOGSEQ, md);
        assert!(out.contains("- parent kept"));
        assert!(out.contains("    - grand kept"));
    }
}
