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





