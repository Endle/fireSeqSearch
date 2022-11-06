use std::fs::DirEntry;
use log::{debug, error, info, warn};
use regex::Regex;

use rayon::prelude::*;

use crate::markdown_parser::parse_to_plain_text;

pub fn read_specific_directory(path: &str) -> Vec<(String, String)> {
    info!("Try to read {}", &path);
    let notebooks = std::fs::read_dir(path).unwrap();
    let mut note_filenames: Vec<DirEntry> = Vec::new();
    for note in notebooks {
        let note : DirEntry = note.unwrap();
        note_filenames.push(note);
    }
    // debug!("Note titles: {:?}", &note_filenames);
    let result: Vec<(String,String)> = note_filenames.par_iter()
        .map(|note|  read_md_file_and_parse(&note))
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
/// First: title
/// Second: full text (parsed)
///
/// If input is a directory or DS_STORE, return None
///
pub fn read_md_file_and_parse(note: &std::fs::DirEntry) -> Option<(String, String)> {
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


    // Now we do some parsing for this file
    let content: String = exclude_advanced_query(content);
    let content: String = parse_to_plain_text(&content);

    Some((note_title.to_string(),content))
}

// https://docs.rs/regex/latest/regex/#repetitions
// https://stackoverflow.com/a/8303552/1166518
pub fn exclude_advanced_query(md: String) -> String {
    if !md.contains("#") {
        return md;
    }

    lazy_static! {
        static ref RE: Regex = Regex::new(
            r"\#\+BEGIN_QUERY[\S\s]+?\#\+END_QUERY")
            .unwrap();
    }
    let result = RE.replace_all(&md, "    ");
    String::from(result)
    // let mat = RE.find(&md);
    // match mat {
    //     Some(m) => {
    //         todo!()
    //     },
    //     None => md
    // }
}