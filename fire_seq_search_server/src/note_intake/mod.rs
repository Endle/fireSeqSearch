//! First-phase note ingestion: receive raw note-app markdown bytes from the
//! walker, normalize the dialect-specific syntax (Logseq macros and bullet
//! grammar, Obsidian frontmatter, image embeds, …) into clean text, and report
//! a `narrative_chars` measurement that downstream consumers (chunker,
//! summarizer gate) can act on.
//!
//! This layer is deliberately decoupled from the query / embedding / storage /
//! LLM concerns. It only knows about notebook dialects. Supporting a new
//! notebook app or a new Logseq macro means a localized edit here.

use regex::Regex;

pub mod logseq;
pub mod obsidian;

pub use logseq::{is_stub_unit, split_into_top_level_units};

/// Which notebook app a vault belongs to. Drives dialect-specific behavior
/// in this module and (via re-export) elsewhere in the crate.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub enum NotebookSoftware {
    Logseq,
    Obsidian,
}

/// Strip dialect-specific syntax from raw note markdown, leaving text that
/// downstream stages (chunker, summarizer prompt) can consume directly.
///
/// Runs a small shared pass (frontmatter, image embeds — both dialects use
/// them) and then dispatches to the per-flavour strip function. Adding a new
/// dialect rule belongs in the matching submodule, not here.
pub fn preprocess(software: &NotebookSoftware, raw: &str) -> String {
    let s = shared_strip(raw);
    match software {
        NotebookSoftware::Logseq => logseq::strip(&s),
        NotebookSoftware::Obsidian => obsidian::strip(&s),
    }
}

/// Length of a page's "narrative" content: the non-stub bullet body (Logseq)
/// or the heading-section prose (Obsidian), after stripping
/// `[[wikilink]]`/`[text](url)` syntax down to their text, dropping bare URLs
/// and bullet markers, and collapsing whitespace. Properties, frontmatter and
/// advanced-query blocks are already gone via `preprocess`.
pub fn narrative_chars(software: &NotebookSoftware, raw: &str) -> usize {
    let preprocessed = preprocess(software, raw);
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

/// Reduce markdown to plain text for content-measurement purposes:
/// `[text](url)` → `text`, `[[wikilink]]` → `wikilink`, drop URL schemes but
/// keep host+path (they carry topic information), strip bullet markers,
/// collapse whitespace. Not part of `preprocess`'s output — the chunker keeps
/// the unreduced form so the LLM sees real markdown — this is only for
/// `narrative_chars` to decide whether a page has enough content to bother
/// the summarizer with.
pub(crate) fn strip_markup(s: &str) -> String {
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

/// Pass shared between dialects: YAML frontmatter and image embeds. Both
/// Logseq and Obsidian files can carry these, so handling them once before
/// the per-flavour strip avoids duplication.
fn shared_strip(raw: &str) -> String {
    lazy_static! {
        static ref FRONTMATTER: Regex = Regex::new(r"(?s)\A---\n.*?\n---\n?").unwrap();
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
    let s = IMG_WIKI.replace_all(&s, "");
    IMG_MD.replace_all(&s, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOGSEQ: &NotebookSoftware = &NotebookSoftware::Logseq;
    const OBSIDIAN: &NotebookSoftware = &NotebookSoftware::Obsidian;

    #[test]
    fn narrative_chars_strips_markup() {
        assert_eq!(narrative_chars(LOGSEQ, "- [[NP-complete]] problem\n"), "NP-complete problem".chars().count());
        assert_eq!(narrative_chars(LOGSEQ, "- see [docs](https://example.com/x)\n"), "see docs".chars().count());
        assert_eq!(narrative_chars(LOGSEQ, "- https://example.com/path\n"), "example.com/path".chars().count());
        assert_eq!(narrative_chars(LOGSEQ, "tags:: foo\n-\n"), 0);
        assert_eq!(narrative_chars(LOGSEQ, ""), 0);
    }

    #[test]
    fn obsidian_frontmatter_stripped_by_shared_pass() {
        let md = "---\ntitle: Foo\n---\nBody.\n";
        let out = preprocess(OBSIDIAN, md);
        assert!(!out.contains("title: Foo"));
        assert!(out.contains("Body."));
    }

    #[test]
    fn image_embeds_stripped_for_both_dialects() {
        let md = "![[diagram.svg]] some prose\n";
        assert!(!preprocess(LOGSEQ, md).contains("diagram.svg"));
        assert!(!preprocess(OBSIDIAN, md).contains("diagram.svg"));
    }
}
