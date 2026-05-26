//! Application configuration surfaced to both the server's internal code and
//! the browser addon over `/server_info`. The struct's JSON shape is part of
//! the contract with the addon — adding fields is safe (older addons ignore
//! unknown keys), removing or renaming them is a breaking change.

use crate::note_intake::NotebookSoftware;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerInformation {
    pub notebook_path: String,
    pub notebook_name: String,
    pub enable_journal_query: bool,
    pub show_top_hits: usize,
    pub show_summary_single_line_chars_limit: usize,
    pub software: NotebookSoftware,
    pub convert_underline_hierarchy: bool,
    pub host: String,
    /// Server crate version (`CARGO_PKG_VERSION`). Lets a freshly-upgraded
    /// addon notice it's talking to an older backend.
    pub version: String,
    /// Feature list the addon can gate UI on, e.g. `["query", "ask"]`. Older
    /// backends omit this field entirely — the addon must treat "absent" as
    /// "only the original `/query` path is guaranteed".
    pub capabilities: Vec<String>,
}
