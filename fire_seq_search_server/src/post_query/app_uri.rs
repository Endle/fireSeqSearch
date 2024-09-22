use log::{error, info};
use crate::post_query::logseq_uri::{generate_logseq_uri,parse_date_from_str};
use crate::post_query::obsidian_uri::generate_obsidian_uri;
use crate::query_engine::ServerInformation;


// Maybe I should wrap them with the same interface? -Zhenbo Li 2023-Feb-05
// Deprecated on 2024-Sep-21
pub fn generate_uri(title: &str, is_page_hit: &bool, server_info: &ServerInformation) -> String {
    if server_info.software == Obsidian {
        info!("Generating Obsidian URI for {}", title);
        if !is_page_hit {
            error!("Journal is unsupported for Obsidian yet");
            return String::from("https://github.com/Endle/fireSeqSearch/issues");
        }
        return generate_obsidian_uri(&title, *is_page_hit, &server_info);
    }

    return generate_logseq_uri(&title, *is_page_hit, &server_info);
}

use crate::query_engine::NotebookSoftware::{Logseq,Obsidian};

pub fn generate_uri_v2(title: &str, server_info: &ServerInformation) -> String {
    match &server_info.software {
        Obsidian => generate_obsidian_uri(title, true, server_info),
        Logseq => {
            let dt = parse_date_from_str(title);
            // TODO remove this duplicate calc
            //  I don't care the performance here, but I want to make code cleaner - 2024 Sep 21
            generate_logseq_uri(title, dt.is_none(), server_info)
        }
    }
}
