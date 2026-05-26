use crate::post_query::logseq_uri::{generate_logseq_uri, parse_date_from_str};
use crate::post_query::obsidian_uri::generate_obsidian_uri;
use crate::config::ServerInformation;
use crate::note_intake::NotebookSoftware::{Logseq, Obsidian};

/// Build the `logseq://` / `obsidian://` URI for a note hit.
///
/// `page_title` is the file basename (file_stem). `rel_path` is the
/// vault-relative path *including* the `.md` extension (e.g.
/// `"E. ISM & Emission/Light-Matter Interactions/Compton Scattering.md"`).
///
/// For Logseq, pages live in a flat `pages/`+`journals/` layout — basename
/// is enough and `rel_path` is unused. For Obsidian, notes are nested
/// arbitrarily; the `file=` query param must carry the directory prefix or
/// `obsidian://open` fails to resolve the note (or opens a basename
/// collision in the wrong folder).
pub fn generate_uri_v2(
    page_title: &str,
    rel_path: &str,
    server_info: &ServerInformation,
) -> String {
    match &server_info.software {
        Obsidian => {
            let file = rel_path.strip_suffix(".md").unwrap_or(rel_path);
            generate_obsidian_uri(file, server_info)
        }
        Logseq => {
            let dt = parse_date_from_str(page_title);
            generate_logseq_uri(page_title, dt.is_none(), server_info)
        }
    }
}
