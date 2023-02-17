// Everything about Tantivy should be hidden behind this component

use log::{info, warn};
use crate::{decode_cjk_str, JiebaTokenizer};
use crate::load_notes::read_specific_directory;
use crate::post_query::post_query_wrapper;
use rayon::prelude::*;
use crate::markdown_parser::parse_logseq_notebook;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerInformation {
    pub notebook_path: String,
    pub notebook_name: String,
    pub enable_journal_query: bool,
    pub show_top_hits: usize,
    pub show_summary_single_line_chars_limit: usize,


    pub obsidian_md: bool,


    /// Experimental. Not sure if I should use this global config - 2022-12-30
    pub convert_underline_hierarchy: bool,
}

struct DocumentSetting {
    schema: tantivy::schema::Schema,
    tokenizer: JiebaTokenizer,
}

pub struct QueryEngine {
    pub server_info: ServerInformation,
    reader: tantivy::IndexReader,
    query_parser: tantivy::query::QueryParser
}

impl QueryEngine {
    pub fn construct(server_info: ServerInformation) -> Self {
        let document_setting: DocumentSetting = build_document_setting();

        let index = indexing_documents(&server_info, &document_setting);
        let (reader, query_parser) = build_reader_parser(&index, &document_setting);

        QueryEngine {
            server_info,
            reader,
            query_parser
        }
    }



    pub fn query_pipeline(self: &Self, term: String) -> String {
        let term: String = term_preprocess(term);
        info!("Searching {}", &term);


        let searcher = self.reader.searcher();
        let server_info: &ServerInformation = &self.server_info;

        let top_docs: Vec<(f32, tantivy::DocAddress)> = self.get_top_docs(&term);
        let result: Vec<String> = post_query_wrapper(top_docs, &term, &searcher, &server_info);

        let json = serde_json::to_string(&result).unwrap();

        // info!("Search result {}", &json);
        json
    }

    fn get_top_docs(&self, term: &str) -> Vec<(f32, tantivy::DocAddress)> {
        let searcher = self.reader.searcher();
        let server_info: &ServerInformation = &self.server_info;
        let query: Box<dyn tantivy::query::Query> = self.query_parser.parse_query(&term).unwrap();
        let top_docs: Vec<(f32, tantivy::DocAddress)> =
            searcher.search(&query,
                            &tantivy::collector::TopDocs::with_limit(server_info.show_top_hits))
                .unwrap();

        top_docs
    }
}

fn term_preprocess(term:String) -> String {
    // in the future, I would use tokenize_sentence_to_text_vec here
    let term = term.replace("%20", " ");
    let term_vec = decode_cjk_str(term);
    term_vec.join(" ")
}


fn build_reader_parser(index: &tantivy::Index, document_setting: &DocumentSetting)
                       -> (tantivy::IndexReader, tantivy::query::QueryParser) {
    let reader = index
        .reader_builder()
        .reload_policy(tantivy::ReloadPolicy::OnCommit)
        .try_into().unwrap();
    let title = document_setting.schema.get_field("title").unwrap();
    let body = document_setting.schema.get_field("body").unwrap();
    let query_parser = tantivy::query::QueryParser::for_index(index, vec![title, body]);
    (reader, query_parser)
}

fn indexing_documents(server_info: &ServerInformation, document_setting: &DocumentSetting) -> tantivy::Index {
    let path: &str = &server_info.notebook_path;
    let schema = &document_setting.schema;
    let index = tantivy::Index::create_in_ram(schema.clone());

    index.tokenizers().register(crate::TOKENIZER_ID, document_setting.tokenizer.clone());

    let mut index_writer = index.writer(50_000_000).unwrap();


    if server_info.obsidian_md {
        warn!("Obsidian mode.");
        assert!(!server_info.enable_journal_query);
    }

    // I should remove the unwrap and convert it into map
    let path = path.to_owned();
    let pages_path = if server_info.obsidian_md {
        path.clone()
    } else{
        path.clone() + "/pages"
    };


    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    let pages: Vec<(String, String)> = read_specific_directory(&pages_path).par_iter()
        .map(|(title,md)| {
            let content = parse_logseq_notebook(md, false);
            (title.to_string(), content)
        }).collect();

    for (file_name, contents) in read_specific_directory(&pages_path) {
        // let note_title = process_note_title(file_name, &server_info);
        index_writer.add_document(
            tantivy::doc!{ title => file_name, body => contents}
        ).unwrap();
    }

    if server_info.enable_journal_query {
        info!("Loading journals");
        let journals_page = path.clone() + "/journals";
        for (note_title, contents) in read_specific_directory(&journals_page) {
            let tantivy_title = crate::JOURNAL_PREFIX.to_owned() + &note_title;
            index_writer.add_document(
                tantivy::doc!{ title => tantivy_title, body => contents}
            ).unwrap();
        }
    }

    index_writer.commit().unwrap();
    index
}



fn build_document_setting() -> DocumentSetting {
    let (schema, tokenizer) = build_schema_tokenizer();
    DocumentSetting{
        schema, tokenizer
    }
}

fn build_schema_tokenizer() -> (tantivy::schema::Schema,
                                JiebaTokenizer
                                // Box<dyn tantivy::tokenizer::Tokenizer>
) {
    let mut schema_builder = tantivy::schema::SchemaBuilder::default();
    let text_indexing = tantivy::schema::TextFieldIndexing::default()
        .set_tokenizer(crate::TOKENIZER_ID) // Set custom tokenizer
        .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions);
    let text_options = tantivy::schema::TextOptions::default()
        .set_indexing_options(text_indexing)
        .set_stored();
    let tokenizer:JiebaTokenizer = JiebaTokenizer {};

    let _title = schema_builder.add_text_field("title", text_options.clone());
    let _body = schema_builder.add_text_field("body", text_options);

    let schema = schema_builder.build();
    (schema,
     tokenizer
     // Box::new(tokenizer)
    )
}

