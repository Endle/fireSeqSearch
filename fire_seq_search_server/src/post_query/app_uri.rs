use log::{error, info};
use crate::post_query::logseq_uri::{generate_logseq_uri, process_note_title};
use crate::post_query::obsidian_uri::generate_obsidian_uri;
use crate::query_engine::ServerInformation;

// Maybe I should wrap them with the same interface? -Zhenbo Li 2023-Feb-05
pub fn generate_uri(title: &str, is_page_hit: &bool, server_info: &ServerInformation) -> String {
    if server_info.obsidian_md {
        info!("Generating Obsidian URI for {}", title);
        if !is_page_hit {
            error!("Journal is unsupported for Obsidian yet");
            return String::from("https://github.com/Endle/fireSeqSearch/issues");
        }
        return generate_obsidian_uri(&title, *is_page_hit, &server_info);
    }

    return generate_logseq_uri(&title, &is_page_hit, &server_info);

}