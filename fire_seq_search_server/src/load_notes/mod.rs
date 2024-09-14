use std::fs::DirEntry;
use log::{debug, error, info, warn};
use std::process;

use rayon::prelude::*;
use crate::query_engine::ServerInformation;
use crate::JOURNAL_PREFIX;


use std::borrow::Cow;
use std::borrow::Borrow;

#[derive(Debug, Clone)]
pub struct NoteListItem {
    pub realpath: String,
    pub title:    String,
}

pub fn retrive_note_list(server_info: &ServerInformation) -> Vec<NoteListItem> {
    let path: &str = &server_info.notebook_path;
    let note_list = list_directory( Cow::from(path) , true);

    // TODO didn't handle logseq
    note_list
}

fn list_directory(path: Cow<'_, str>, recursive: bool) -> Vec<NoteListItem> {
    debug!("Listing directory {}", &path);
    let mut result = Vec::new();

    let path_ref: &str = path.borrow();
    let notebooks = match std::fs::read_dir(path_ref) {
        Ok(x) => x,
        Err(e) => {
            error!("Fatal error ({:?}) when reading {}", e, &path);
            process::abort();
        }
    };

    for note_result in notebooks {
        let entry = match note_result {
            Ok(x) => x,
            Err(e) => {
                error!("Error during looping {:?}", &e);
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(x) => x,
            Err(e) => {
                error!("Error: Can't get file type {:?}  {:?}", &entry, &e);
                continue;
            }
        };

        let entry_path = entry.path();
        let entry_path_str = entry_path.to_string_lossy();

        if file_type.is_dir() {
            if recursive {
                let next = list_directory(entry_path_str, true);
                result.extend(next);
            }
            continue;
        }

        if !entry_path_str.ends_with(".md") {
            info!("skip non-md file {:?}", &entry);
            continue;
        }

        let note_title = match entry_path.file_stem() {
            Some(osstr) => osstr.to_str().unwrap(),
            None => {
                error!("Couldn't get file_stem for {:?}", entry_path);
                continue;
            }
        };
        let row = NoteListItem {
            realpath: entry_path_str.to_string(),
            title: note_title.to_string(),
        };
        result.push(row);
    }

    return result;
}

/*
pub fn read_all_notes(server_info: &ServerInformation) -> Vec<(String, String)> {
    // I should remove the unwrap and convert it into map
    let path: &str = &server_info.notebook_path;
    let path = path.to_owned();
    let pages_path = if server_info.obsidian_md {
        path.clone()
    } else{
        path.clone() + "/pages"
    };


    let mut pages: Vec<(String, String)> = Vec:: new();

    let pages_tmp: Vec<(String, String)>  = read_specific_directory(&pages_path).par_iter()
        .map(|(title,md)| {
            let content = crate::markdown_parser::parse_logseq_notebook(md, title, server_info);
            (title.to_string(), content)
        }).collect(); //silly collect.

    if server_info.exclude_zotero_items {
        error!("exclude zotero disabled");
    }
    /*
    for (file_name, contents) in pages_tmp {
        // info!("File Name: {}", &file_name);
        if server_info.exclude_zotero_items && file_name.starts_with('@') {
            continue;
        }
        pages.push((file_name,contents));
    }
    */
    if server_info.enable_journal_query {
        info!("Loading journals");
        let journals_page = path.clone() + "/journals";
        let journals:Vec<(String, String)>
            = read_specific_directory(&journals_page).par_iter()
            .map(|(title,md)| {
                let content = crate::markdown_parser::parse_logseq_notebook(md, title, server_info);
                let tantivy_title = JOURNAL_PREFIX.to_owned() + &title;
                (tantivy_title, content)
            }).collect(); //silly collect.


        for (file_name, contents) in journals {
            pages.push((file_name,contents));
        }

    }

    pages

}


*/




