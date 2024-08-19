// Everything about Tantivy should be hidden behind this component

use log::{debug, info, warn};
use crate::{Article, decode_cjk_str};
use crate::post_query::post_query_wrapper;




// This struct should be immutable when the program starts running
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerInformation {
    pub notebook_path: String,
    pub notebook_name: String,
    pub enable_journal_query: bool,
    pub show_top_hits: usize,
    pub show_summary_single_line_chars_limit: usize,
    pub parse_pdf_links: bool,
    pub exclude_zotero_items:bool,
    pub obsidian_md: bool,

    /// Experimental. Not sure if I should use this global config - 2022-12-30
    pub convert_underline_hierarchy: bool,

    pub host: String,
}

use crate::language_tools::tokenizer::FireSeqTokenizer;
struct DocumentSetting {
    schema: tantivy::schema::Schema,
    tokenizer: FireSeqTokenizer,
}

use crate::local_llm::LlmEngine;
pub struct QueryEngine {
    pub server_info: ServerInformation,
    reader: tantivy::IndexReader,
    query_parser: tantivy::query::QueryParser,
    articles: Vec<Article>,
    pub llm: Option<LlmEngine>,
}

impl QueryEngine {
    pub fn construct(server_info: ServerInformation) -> Self {

        let document_setting: DocumentSetting = build_document_setting();
        let loaded_notes = crate::load_notes::read_all_notes(&server_info);
        let loaded_articles: Vec<Article> = loaded_notes.into_iter().map(
            |x| Article{file_name:x.0, content:x.1}
        ).collect();
        let index = indexing_documents(&server_info, &document_setting, &loaded_articles);
        let (reader, query_parser) = build_reader_parser(&index, &document_setting);

        debug!("Query engine construction finished");

        QueryEngine {
            server_info,
            reader,
            query_parser,
            articles: loaded_articles,
            llm: None,
        }
    }
}

impl QueryEngine {



    pub fn generate_wordcloud(self: &Self) -> String {
        crate::word_frequency::generate_wordcloud(&self.articles)
    }

    pub fn query_pipeline(self: &Self, term: String) -> String {
        let term: String = term_preprocess(term);
        info!("Searching {}", &term);


        let server_info: &ServerInformation = &self.server_info;

        let top_docs: Vec<(f32, tantivy::DocAddress)> = self.get_top_docs(&term);
        let searcher: tantivy::Searcher = self.reader.searcher();
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

impl QueryEngine {
    pub async fn summarize(&self, title: String) -> String {
        info!("Called summarize on {}", &title);
        if cfg!(feature="llm") {
            let llm = self.llm.as_ref().unwrap();
            llm.summarize(&title).await
        } else {
            "LLM turned off".to_owned()
        }
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
        .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay) // TODO switch to manual
        .try_into().unwrap();
    let title = document_setting.schema.get_field("title").unwrap();
    let body = document_setting.schema.get_field("body").unwrap();
    let query_parser = tantivy::query::QueryParser::for_index(index, vec![title, body]);
    (reader, query_parser)
}

fn indexing_documents(server_info: &ServerInformation,
                      document_setting: &DocumentSetting,
                      pages:&Vec<crate::Article>) -> tantivy::Index {

    let schema = &document_setting.schema;
    let index = tantivy::Index::create_in_ram(schema.clone());

    index.tokenizers().register(TOKENIZER_ID, document_setting.tokenizer.clone());

    let mut index_writer = index.writer(50_000_000).unwrap();


    if server_info.obsidian_md {
        warn!("Obsidian mode.");
        assert!(!server_info.enable_journal_query);
    }

    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();


    for article in pages {
        index_writer.add_document(
            tantivy::doc!{ title => article.file_name.clone(),
                body => article.content.clone()}
        ).unwrap();
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

use crate::language_tools::tokenizer::TOKENIZER_ID;
fn build_schema_tokenizer() -> (tantivy::schema::Schema,
    FireSeqTokenizer
                                // Box<dyn tantivy::tokenizer::Tokenizer>
) {
    let mut schema_builder = tantivy::schema::SchemaBuilder::default();
    let text_indexing = tantivy::schema::TextFieldIndexing::default()
        .set_tokenizer(TOKENIZER_ID) // Set custom tokenizer
        .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions);
    let text_options = tantivy::schema::TextOptions::default()
        .set_indexing_options(text_indexing)
        .set_stored();
    let tokenizer = FireSeqTokenizer {};

    let _title = schema_builder.add_text_field("title", text_options.clone());
    let _body = schema_builder.add_text_field("body", text_options);

    let schema = schema_builder.build();
    (schema,
     tokenizer
     // Box::new(tokenizer)
    )
}

