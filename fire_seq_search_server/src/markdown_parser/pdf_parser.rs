use std::path::Path;
use log::{debug, error};
use pulldown_cmark::Tag;
use crate::query_engine::ServerInformation;

extern crate pdf_extract;
extern crate lopdf;
use pdf_extract::*;
use lopdf::*;

pub(crate) fn try_parse_pdf(tag: &Tag, server_info: &ServerInformation) -> Option<String> {

    let destination_uri = match tag {
        Tag::Image(link_type, destination_uri, title) => {
            if !destination_uri.ends_with(".pdf") {
                return None;
            }
            debug!("Trying to parse PDF {:?}", tag);
            println!("{:?}", &tag);
            destination_uri.replace("../", "")
        },
        _ => {return None;}
    };

    let path = Path::new(&server_info.notebook_path);
    let pdf_path = path.join(destination_uri);
    println!("{:?}, {:?}", &pdf_path, pdf_path.is_file());
    if !pdf_path.is_file() {
        error!("pdf_path is not a file, skipping {:?}", &pdf_path);
        return None;
    }

    let doc = Document::load(pdf_path).unwrap();


    None
}