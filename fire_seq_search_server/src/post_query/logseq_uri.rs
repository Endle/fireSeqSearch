use log::error;
use crate::ServerInformation;
use url::Url;

// I tried to put this part when loading the notebooks, and it reduced the query sensitivity
// https://github.com/Endle/fireSeqSearch/issues/99
// 2022-12-30
fn process_note_title(file_name: &str, server_info: &ServerInformation) -> String {
    let file_name = file_name.replace("%2F", "/");
    if server_info.convert_underline_hierarchy {
        //Home In Canada___Clothes
        return file_name.replace("___", "/");
    }
    file_name.to_owned()
}

pub fn generate_logseq_uri(title: &str, is_page_hit: &bool, server_info: &ServerInformation) -> String {
    return if *is_page_hit {
        let title = process_note_title(&title, server_info);
        let mut uri = Url::parse("logseq://graph/").unwrap();
        uri.set_path(&server_info.notebook_name);
        uri.query_pairs_mut()
            .append_pair("page", &title);
        uri.to_string()
    } else {
        generate_logseq_journal_uri(&title, server_info)

    };
    // logseq://graph/logseq_notebook?page=Nov%2026th%2C%202022
}

#[derive(PartialEq, Debug)]
struct JournalDate {
    pub year: u32,
    pub month: u32,
    pub date: u32,
}

impl JournalDate {
    pub fn to_str(&self, _: &ServerInformation) -> String {
        let mut result = Vec::new();
        result.push(match self.month {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            12 => "Dec",
            _ => {
                error!("Unexpected month {}", self.month);
                "ErrMonth"
            }
        }.to_string());

        result.push(" ".to_string());
        match  self.date {
            1|21|31 => {
                let s = self.date.to_string();
                result.push(s);
                result.push("st".to_string());
            },
            2|22 => {
                let s = self.date.to_string();
                result.push(s);
                result.push("nd".to_string());
            },
            3|23 => {
                let s = self.date.to_string();
                result.push(s);
                result.push("rd".to_string());
            },
            _ => {
                let s = self.date.to_string();
                result.push(s);
                result.push("th".to_string());
            }
        };

        result.push(", ".to_string());
        result.push(self.year.to_string());

        result.concat()
    }
}


fn generate_logseq_journal_uri(title: &str, server_info: &ServerInformation) -> String {
    let mut uri = Url::parse("logseq://graph/").unwrap();
    uri.set_path(&server_info.notebook_name);

    let dt = parse_date_from_str(title);
    let dt = match dt {
        None => {
            error!("Failed to gen JournalDate from {}", title);
            return format!("logseq://graph/{}", server_info.notebook_name);
        }
        Some(x) => x
    };
    let journal_name = dt.to_str(server_info);
    uri.query_pairs_mut()
        .append_pair("page", &journal_name);
    uri.to_string()
}

fn parse_slice_to_u8(slice: Option<&str>) -> Option<u32> {
    match slice{
        Some(x) => {
            let y = x.parse::<u32>();
            match y {
                Ok(i) => Some(i),
                Err(e) => {
                    error!("Parse({}) Int Error:  ({:?})", x, e);
                    None
                }
            }
        },
        None => {
            error!("Invalid slice");
            None
        }

    }
}

fn parse_date_from_str(title: &str) -> Option<JournalDate> {
    if title.len() != 10 {
        error!("Journal length unexpected: {}", title);
        return None;
    }

    let year = match parse_slice_to_u8(title.get(0..4)) {
        Some(x) => x,
        None => {
            return None;
        }
    };
    let month = match parse_slice_to_u8(title.get(5..=6)) {
        Some(x) => x,
        None => {
            return None;
        }
    };
    let date = match parse_slice_to_u8(title.get(8..=9)) {
        Some(x) => x,
        None => {
            return None;
        }
    };
    Some(JournalDate{
        year,
        month,
        date
    })
}

#[cfg(test)]
mod test_logseq_uri {
    use crate::generate_server_info_for_test;
    use crate::post_query::logseq_uri::{generate_logseq_journal_uri, generate_logseq_uri};
    use crate::post_query::logseq_uri::parse_date_from_str;
    use crate::query_engine::ServerInformation;


    #[test]
    fn test_parse() {
        let server_info = generate_server_info_for_test();
        assert_eq!(None, parse_date_from_str("22"));
        let d = parse_date_from_str("2022_12_05");
        assert!(d.is_some());
        let d = d.unwrap();
        assert_eq!(d.to_str(&server_info), "Dec 5th, 2022");
    }
    #[test]
    fn test_generate() {

        let server_info = generate_server_info_for_test();

        let r = generate_logseq_uri("Games/EU4", &true, &server_info);
        assert_eq!(&r, "logseq://graph/logseq_notebook?page=Games%2FEU4");

        let r = generate_logseq_uri("Games/赛马娘", &true, &server_info);
        assert_eq!(&r,
                   "logseq://graph/logseq_notebook?page=Games%2F%E8%B5%9B%E9%A9%AC%E5%A8%98");
        let r = generate_logseq_journal_uri("2022_12_14", &server_info);
        assert_eq!(&r,"logseq://graph/logseq_notebook?page=Dec+14th%2C+2022");

        let r = generate_logseq_uri("fireSeqSearch___test___5", &true, &server_info);
        assert_eq!(&r,"logseq://graph/logseq_notebook?page=fireSeqSearch%2Ftest%2F5");
    }
}