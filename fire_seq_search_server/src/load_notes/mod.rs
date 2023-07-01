use std::fs::DirEntry;
use log::{debug, error, info, warn};
use std::process;

use rayon::prelude::*;
use crate::query_engine::ServerInformation;


pub fn read_all_notes(server_info: &ServerInformation) -> Vec<(String, String)> {
    // I should remove the unwrap and convert it into map
    let path: &str = &server_info.notebook_path;
    let path = path.to_owned();
    let pages_path = if server_info.obsidian_md {
        path.clone()
    } else{
        path.clone() + "/pages"
    };
    let mut pages: Vec<(String, String)> = read_specific_directory(&pages_path).par_iter()
        .map(|(title,md)| {
            let content = crate::markdown_parser::parse_logseq_notebook(md, title, server_info);
            (title.to_string(), content)
        }).collect();
    if server_info.enable_journal_query {
        info!("Loading journals");
        let journals_page = path.clone() + "/journals";
        let journals_iter:Vec<(String, String)>
            = read_specific_directory(&journals_page).par_iter()
            .map(|(title,md)| {
                let content = crate::markdown_parser::parse_logseq_notebook(md, title, server_info);
                (title.to_string(), content)
            }).collect();
        for (file_name, contents) in journals_iter {
            pages.push((file_name,contents));
        }

    }

    pages

}

pub fn read_specific_directory(path: &str) -> Vec<(String, String)> {
    info!("Try to read {}", &path);
    let notebooks = match std::fs::read_dir(path) {
        Ok(x) => x,
        Err(e) => {
            error!("Fatal error ({:?}) when reading {}", e, path);
            process::abort();
        }
    };
    let mut note_filenames: Vec<DirEntry> = Vec::new();
    for note in notebooks {
        let note : DirEntry = note.unwrap();
        note_filenames.push(note);
    }
    // debug!("Note titles: {:?}", &note_filenames);
    let result: Vec<(String,String)> = note_filenames.par_iter()
        .map(|note|  read_md_file_wo_parse(&note))
        .filter(|x| (&x).is_some())
        .map(|x| x.unwrap())
        .collect();
    info!("Loaded {} notes from {}", result.len(), path);
    // info!("After map {:?}", &result);

    result
}




///
///
/// # Arguments
///
/// * `note`:
///
/// returns: Option<(String, String)>
///
/// First: title (filename)
/// Second: full raw text
///
/// I would delay the parsing job, so it could be couples with server info. -Zhenbo Li 2023-02-17
/// If input is a directory or DS_STORE, return None
///
pub fn read_md_file_wo_parse(note: &std::fs::DirEntry) -> Option<(String, String)> {
    if let Ok(file_type) = note.file_type() {
        // Now let's show our entry's file type!
        debug!("{:?}: {:?}", note.path(), file_type);
        if file_type.is_dir() {
            debug!("{:?} is a directory, skipping", note.path());
            return None;
        }
    } else {
        warn!("Couldn't get file type for {:?}", note.path());
        return None;
    }

    let note_path = note.path();
    let note_title = match note_path.file_stem() {
        Some(osstr) => osstr.to_str().unwrap(),
        None => {
            error!("Couldn't get file_stem for {:?}", note.path());
            return None;
        }
    };
    debug!("note title: {}", &note_title);

    let content : String = match std::fs::read_to_string(&note_path) {
        Ok(c) => c,
        Err(e) => {
            if note_title.to_lowercase() == ".ds_store" {
                debug!("Ignore .DS_Store for mac");
            } else {
                error!("Error({:?}) when reading the file {:?}", e, note_path);
            }
            return None;
        }
    };

    Some((note_title.to_string(),content))
}

