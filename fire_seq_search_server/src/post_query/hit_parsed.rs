use log::debug;
use crate::JOURNAL_PREFIX;
use crate::post_query::app_uri::generate_uri_v2;
use crate::post_query::highlighter::highlight_keywords_in_body;
use crate::query_engine::ServerInformation;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct FireSeqSearchHitParsed {
    // pub title: String,
    pub title: String,
    pub summary: String,
    pub score: f32,
    pub metadata: String,
    pub logseq_uri: String,
}

use tantivy::schema::document::OwnedValue;
impl FireSeqSearchHitParsed {

    //TODO remove these dup code
    fn take_str_from_doc(doc: &tantivy::TantivyDocument, pos:usize) -> &str {
        /*
        let title: &str = doc.field_values()[0].value().as_text().unwrap();
        let body: &str = doc.field_values()[1].value().as_text().unwrap();
        */
        let v: &OwnedValue =  doc.field_values()[pos].value();
        match v{
            OwnedValue::Str(s) => s,
            _ => panic!("Wrong type")
        }
    }
    pub fn from_tantivy(doc: &tantivy::TantivyDocument,
                        score: f32, term_tokens: &Vec<String>,
                        server_info: &ServerInformation) ->FireSeqSearchHitParsed {

        let title = Self::take_str_from_doc(doc, 0);
        let body = Self::take_str_from_doc(doc, 1);
        let summary = highlight_keywords_in_body(body, term_tokens, server_info);

        let mut is_page_hit = true;
        let title = if title.starts_with(JOURNAL_PREFIX) {
            assert!(server_info.enable_journal_query);
            debug!("Found a journal hit {}", title);
            is_page_hit = false;
            let t = title.strip_prefix(JOURNAL_PREFIX);
            t.unwrap().to_string()
        } else {
            title.to_string()
        };

        let logseq_uri = generate_uri_v2(&title, server_info);

        debug!("Processing a hit, title={}, uri={}, summary_len={}", &title, &logseq_uri,summary.len());

        let metadata: String = if is_page_hit {
            String::from("page_hit")
        } else {
            String::from("journal_hit")
        };

        FireSeqSearchHitParsed {
            title,
            summary,
            score,
            logseq_uri,
            metadata,
        }
    }

    // Wrap this part into a function, so I can document it and add tests - ZLi 2023-Jan
    pub fn serde_to_string(self: &Self) -> String {
        serde_json::to_string(&self).unwrap()
    }

}



#[cfg(test)]
mod test_serde {
    // use crate::generate_server_info_for_test;
    // use crate::post_query::hit_parsed::FireSeqSearchHitParsed;
    // use crate::post_query::logseq_uri::generate_logseq_uri;


    // fn get_parsed_hit(title: &str) -> FireSeqSearchHitParsed {
    //     let server_info = generate_server_info_for_test();
    //     let logseq_uri = generate_logseq_uri(title, &true, &server_info);
    //     FireSeqSearchHitParsed{
    //         title: title.to_owned(),
    //         summary: String::from("summary"),
    //         score: 1.0,
    //         logseq_uri,
    //         metadata: String::from("meta")
    //     }
    // }
    // fn serde(title: &str) -> String {
    //     let h = get_parsed_hit(title);
    //     h.serde_to_string()
    // }

    // TODO: This solution is buggy. Consider PR#100, which might be a better idea. -Zli, 2023-Jan
    // This test disabled on 2023-Feb-02 for PR #112
    // #[test]
    // fn test_serde_uri() {
    //     assert!(serde("EU4").contains("\"logseq://graph/logseq_notebook?page=EU4\""));
    //
    //     assert!(serde("Games/EU4").contains("\"logseq://graph/logseq_notebook?page=Games/EU4\""));
    //
    // }
}
