use std::ffi::OsStr;
use std::path::Path;
use log::{debug, error, info};
use pulldown_cmark::Tag;
use crate::query_engine::ServerInformation;

extern crate pdf_extract;


pub(crate) fn try_parse_pdf(tag: &Tag, server_info: &ServerInformation) -> Option<String> {

    let destination_uri = match tag {
        Tag::Image(link_type, destination_uri, title) => {
            if !destination_uri.ends_with(".pdf") {
                return None;
            }
            debug!("Trying to parse PDF {:?}", tag);
            // println!("{:?}", &tag);
            destination_uri.replace("../", "")
        },
        _ => {return None;}
    };

    let path = Path::new(&server_info.notebook_path);
    let pdf_path = path.join(destination_uri);
    // println!("{:?}, {:?}", &pdf_path, pdf_path.is_file());
    if !pdf_path.is_file() {
        error!("pdf_path is not a file, skipping {:?}", &pdf_path);
        return None;
    }


    let text = match pdf_extract::extract_text(&pdf_path) {
            Ok(s) => {s}
            Err(e) => {
                error!("Failed({:?} to load pdf {:?}", e, pdf_path);
                return None;
            }
    };

    match pdf_path.file_name() {
        None => {error!("Extracted text len {}, file_name() failed", text.len());}
        Some(f) => {info!("Extracted text from {:?} len {}", f, text.len());}
    };


    Some(text)
}