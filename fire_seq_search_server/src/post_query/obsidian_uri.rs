use url::Url;
use crate::query_engine::ServerInformation;

/// Build `obsidian://open?vault=…&file=…` for the given vault-relative path
/// (`.md` extension stripped). Obsidian resolves `file=` against the entire
/// vault, so for a note inside subdirectories the `file=` value MUST carry
/// the directory prefix — otherwise Obsidian either fails to open the note
/// or opens a basename collision in the wrong folder. `url`'s
/// `form_urlencoded` encoder maps `/` → `%2F`, ` ` → `+`, `&` → `%26`; the
/// trailing `+` → `%20` replace matches Obsidian's expected form.
///
/// # Examples
/// ```
/// use fire_seq_search_server::post_query::obsidian_uri::generate_obsidian_uri;
/// let server_info = fire_seq_search_server::generate_server_info_for_test();
/// let r = generate_obsidian_uri("fedi note", &server_info);
/// assert_eq!("obsidian://open?vault=logseq_notebook&file=fedi%20note", &r);
/// ```
pub fn generate_obsidian_uri(file_path: &str, server_info: &ServerInformation) -> String {
    let file_path = urlencoding::decode(file_path)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| file_path.to_string());
    let mut uri = Url::parse("obsidian://open").unwrap();
    uri.query_pairs_mut()
        .append_pair("vault", &server_info.notebook_name);
    uri.query_pairs_mut()
        .append_pair("file", &file_path);
    uri.to_string().replace('+', "%20")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate_server_info_for_test;
    use crate::note_intake::NotebookSoftware;

    fn server_info(vault: &str) -> ServerInformation {
        let mut s = generate_server_info_for_test();
        s.notebook_name = vault.to_string();
        s.software = NotebookSoftware::Obsidian;
        s
    }

    #[test]
    fn top_level_file() {
        let r = generate_obsidian_uri("Compton Scattering", &server_info("AstroWiki_2.0-main"));
        assert_eq!(
            r,
            "obsidian://open?vault=AstroWiki_2.0-main&file=Compton%20Scattering"
        );
    }

    #[test]
    fn nested_file_keeps_directory_prefix() {
        let r = generate_obsidian_uri(
            "E. ISM & Emission/Light-Matter Interactions/Compton Scattering",
            &server_info("AstroWiki_2.0-main"),
        );
        assert_eq!(
            r,
            "obsidian://open?vault=AstroWiki_2.0-main&file=E.%20ISM%20%26%20Emission%2FLight-Matter%20Interactions%2FCompton%20Scattering"
        );
    }
}


