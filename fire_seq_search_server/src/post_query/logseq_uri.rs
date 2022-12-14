use log::error;
use crate::ServerInformation;

pub fn generate_logseq_uri(title: &str, is_page_hit: &bool, server_info: &ServerInformation) -> String {

    return if *is_page_hit {
        let uri = format!("logseq://graph/{}?page={}",
                          server_info.notebook_name, title);
        uri
    } else {
        generate_logseq_journal_uri(title, server_info)

    };
    // logseq://graph/logseq_notebook?page=Nov%2026th%2C%202022
}
#[derive(PartialEq, Debug)]
struct JournalDate {
    pub year: u32,
    pub month: u32,
    pub date: u32,
}

fn generate_logseq_journal_uri(title: &str, server_info: &ServerInformation) -> String {

    log::info!("Not implemented for journal page yet: {}", title);
    let dt = parse_date_from_str(title);
    // use chrono::DateTime;
    // let dt = chrono::DateTime::parse_from_str(
    //     title, "%Y_%m_%d"
    // );
    // let dt = match dt {
    //     Ok(t) => {
    //         t
    //     },
    //     Err(e) => {
    //         // Failed(ParseError(NotEnough))
    //         error!("Failed({:?}) to parse journal page: {}, use default URI", e, title);
    //         return format!("logseq://graph/{}",
    //                        server_info.notebook_name);
    //     }
    // };
    // // logseq://graph/logseq_notebook?page=Dec%2013th%2C%202022
    // let dt_str = dt.format("%b , %Y");
    // println!("{}", &dt_str);
    format!("logseq://graph/{}",
            server_info.notebook_name)
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
    use crate::post_query::logseq_uri::generate_logseq_uri;
    use crate::post_query::logseq_uri::parse_date_from_str;
    use crate::ServerInformation;

    #[test]
    fn test_parse() {
        assert_eq!(None, parse_date_from_str("22"));
        let d = parse_date_from_str("2022_12_05");
        println!("{:?}", &d);
    }
    #[test]
    fn test_generate() {
        let server_info = ServerInformation {
            notebook_path: "stub_path".to_string(),
            notebook_name: "logseq_notebook".to_string(),
            enable_journal_query: false,
            show_top_hits: 0,
            show_summary_single_line_chars_limit: 0,
        };

        // Don't encode / at here. It would be processed by serde. - 2022-11-27
        let r = generate_logseq_uri("Games/EU4", &true, &server_info);
        assert_eq!(&r, "logseq://graph/logseq_notebook?page=Games/EU4");

        let r = generate_logseq_uri("Games/赛马娘", &true, &server_info);
        assert_eq!(&r,
                   "logseq://graph/logseq_notebook?page=Games/赛马娘");
    }
}