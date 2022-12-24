// Everything about Tantivy should be hidden behind this component

use log::info;
use crate::JiebaTokenizer;
use crate::load_notes::read_specific_directory;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerInformation {
    pub notebook_path: String,
    pub notebook_name: String,
    pub enable_journal_query: bool,
    pub show_top_hits: usize,
    pub show_summary_single_line_chars_limit: usize,
}

struct DocumentSetting {
    schema: tantivy::schema::Schema,
    tokenizer: JiebaTokenizer,
}

pub struct QueryEngine {
    pub server_info: ServerInformation,
    pub reader: tantivy::IndexReader,
    pub query_parser: tantivy::query::QueryParser
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


    // I should remove the unwrap and convert it into map
    let path = path.to_owned();
    let pages_path = path.clone() + "/pages";


    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    for (note_title, contents) in read_specific_directory(&pages_path) {
        index_writer.add_document(
            tantivy::doc!{ title => note_title, body => contents}
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

