use fire_seq_search_server::markdown_parser::{exclude_advanced_query, parse_to_plain_text};

use std::borrow::Cow;


fn load_articles() -> Vec<(String, String)> {
    let r = read_specific_directory("tests/resource/pages");
    r
}

#[test]
fn test_load_articles() {
    let r = load_articles();
    assert_eq!(r.len(), 11);
    for (title,body) in &r{
        assert!(title.len()>0);
        assert!(body.len()>0);
    }
}


fn read_file_to_line(relative_path: &str) -> String {
    let path = vec![String::from("tests/resource/pages"),
                    relative_path.to_string()];
    let path = path.join("/");
    std::fs::read_to_string(&path)
        .expect("Should have been able to read the file")
}


#[test]
fn parse() {
    let md = read_file_to_line("blog_thunderbird_zh.md");
    let result = parse_to_plain_text(&md);
    assert!(result.contains("Aug 3, 2021 - 使用 git shallow clone 下载并编译 Thunderbird"));
    assert!(!result.contains("https://developer.thunderbird.net/thunderbird-development/getting-started"));

}

#[test]
fn exclude_advance_query() {
    let md = read_file_to_line("advanced_query.md");
    let md = Cow::from(md);
    let result = exclude_advanced_query(md);
    assert!(!result.contains("exempli"));
    assert!(result.contains("In this test page we have"));


    let md = read_file_to_line("blog_thunderbird_zh.md");
    let md = Cow::from(md);
    let result = exclude_advanced_query(md.clone());
    assert_eq!(md, result);
}







// =====================
// These functions are removed in https://github.com/Endle/fireSeqSearch/pull/149/commits/7692bd9091380858b0cbeb2fa10d8c01dabcba91
//  aka https://github.com/Endle/fireSeqSearch/pull/147
// To make unit test happy, I copied them as test helper functions
// Zhenbo - 2024 Sep 21
use std::fs::DirEntry;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::process;
fn read_md_file_wo_parse(note: &std::fs::DirEntry) -> Option<(String, String)> {
    if let Ok(file_type) = note.file_type() {
        // Now let's show our entry's file type!
        if file_type.is_dir() {
            return None;
        }
    } else {
        return None;
    }

    let note_path = note.path();
    let note_title = match note_path.file_stem() {
        Some(osstr) => osstr.to_str().unwrap(),
        None => {
            return None;
        }
    };
    let content : String = match std::fs::read_to_string(&note_path) {
        Ok(c) => c,
        Err(e) => {
            if note_title.to_lowercase() == ".ds_store" {
            } else {
            }
            return None;
        }
    };

    Some((note_title.to_string(),content))
}
fn read_specific_directory(path: &str) -> Vec<(String, String)> {
    let notebooks = match std::fs::read_dir(path) {
        Ok(x) => x,
        Err(e) => {
            process::abort();
        }
    };
    let mut note_filenames: Vec<DirEntry> = Vec::new();
    for note in notebooks {
        let note : DirEntry = note.unwrap();
        note_filenames.push(note);
    }
    let result: Vec<(String,String)> = note_filenames.par_iter()
        .map(|note|  read_md_file_wo_parse(&note))
        .filter(|x| (&x).is_some())
        .map(|x| x.unwrap())
        .collect();

    result
}
