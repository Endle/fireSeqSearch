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

fn generate_logseq_journal_uri(title: &str, server_info: &ServerInformation) -> String {

    log::info!("Not implemented for journal page yet: {}", title);
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

#[cfg(test)]
mod test_logseq_uri {
    use crate::post_query::generate_logseq_uri;
    use crate::ServerInformation;

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