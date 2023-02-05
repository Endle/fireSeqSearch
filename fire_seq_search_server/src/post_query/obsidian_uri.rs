use url::Url;
use crate::query_engine::ServerInformation;

///
///
/// # Arguments
///
/// * `title`:
/// * `is_page_hit`:
/// * `server_info`:
///
/// returns: String
///
/// # Examples
/// obsidian://open?vault=linotes&file=fedi%20note
/// ```
/// use fire_seq_search_server::post_query::obsidian_uri::generate_obsidian_uri;
/// let server_info = fire_seq_search_server::generate_server_info_for_test();
/// let r = generate_obsidian_uri("fedi%20note", true, &server_info);
/// assert_eq!("obsidian://open?vault=logseq_notebook&file=fedi%20note", &r);
/// ```
pub fn generate_obsidian_uri(title: &str, _is_page_hit: bool, server_info: &ServerInformation) -> String {

    let title = urlencoding::decode(title).expect("UTF-8").to_string();
    let mut uri = Url::parse("obsidian://open").unwrap();
    // uri.set_path(&server_info.notebook_name);
    uri.query_pairs_mut()
            .append_pair("vault", &server_info.notebook_name);
    uri.query_pairs_mut()
        .append_pair("file", &title);
    let result = uri.to_string();
    //TODO too hacky here
    result.replace("+", "%20")

}


