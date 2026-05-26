//! Obsidian-specific syntax rules.
//!
//! Currently thin: Obsidian notes are mostly prose under `#` headings, and the
//! shared pass (`note_intake::shared_strip`) already handles the things that
//! cross both dialects (YAML frontmatter, image embeds). Callouts (`> [!info]`),
//! highlights (`==text==`), and other Obsidian-only grammar would land here.
//!
//! Note: the heading-based chunk-splitter lives in `indexer::chunker`, not
//! here — that's chunk-granularity (driven by `CAP_TOKENS`), not dialect
//! syntax. This module owns *what gets stripped*; the chunker owns *how
//! what's left gets split*.

/// Strip Obsidian-specific syntax. Called by `note_intake::preprocess` after
/// the shared (frontmatter + image-embed) pass.
pub fn strip(input: &str) -> String {
    // Obsidian has no extra dialect rules beyond the shared pass today.
    // Returning a copy keeps the dispatch signature uniform with `logseq::strip`.
    input.to_string()
}
