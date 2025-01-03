// Everything about Tantivy should be hidden behind this component

use log::{debug, info, error};
use crate::decode_cjk_str;
use crate::post_query::post_query_wrapper;
use std::sync::Arc;



use std::borrow::Cow;

#[derive(Debug, Clone, serde::Serialize,PartialEq)]
pub enum NotebookSoftware {
    Logseq,
    Obsidian,
}

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
    pub software: NotebookSoftware,

    /// Experimental. Not sure if I should use this global config - 2022-12-30
    pub convert_underline_hierarchy: bool,

    pub host: String,

    pub llm_enabled: bool,
    pub llm_max_waiting_time: u64, /* in secs */
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
    //articles: Vec<Article>, //TODO remove it. only word cloud needs it
    pub llm: Option<Arc<LlmEngine>>,
}

use tantivy::IndexWriter;
use tantivy::TantivyDocument;

use crate::load_notes::NoteListItem;
use futures::stream::FuturesUnordered;
 use futures::StreamExt;

 use tantivy::doc;

impl QueryEngine {
    pub async fn construct(server_info: ServerInformation) -> Self {

        let document_setting: DocumentSetting = build_document_setting();
        let note_list = crate::load_notes::retrive_note_list(&server_info);
        let index: tantivy::Index = QueryEngine::build_index(&server_info,
            &document_setting,
            note_list).await;
        let (reader, query_parser) = build_reader_parser(&index, &document_setting);

        debug!("Query engine construction finished");

        QueryEngine {
            server_info,
            reader,
            query_parser,
        //    articles: Vec::new(),
         //   articles: loaded_articles,
            llm: None,
        }
    }

    async fn load_single_note(
        server_info: &ServerInformation,
        document_setting: &DocumentSetting,
        note: NoteListItem,
        index_writer: &IndexWriter<TantivyDocument>) {

        let raw_content = match std::fs::read_to_string(&note.realpath) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to read {:?} err({:?}, skipping", &note, &e);
                return;
            }
        };

        let content = crate::markdown_parser::parse_logseq_notebook(
            Cow::from(raw_content), &note.title, server_info);

        let schema = &document_setting.schema;
        let title = schema.get_field("title").unwrap();
        let body = schema.get_field("body").unwrap();
        index_writer.add_document(
            tantivy::doc!{
                title => note.title,
                body => content,
            }
        ).unwrap();
    }

    async fn load_all_notes(server_info: &ServerInformation,
        document_setting: &DocumentSetting,
        note_list: Vec<NoteListItem>,
        index_writer: &IndexWriter<TantivyDocument>) {

        let mut futs: FuturesUnordered<_>  = FuturesUnordered::new();
        for article in note_list {
            futs.push(
                QueryEngine::load_single_note(
                    server_info,
                    document_setting,
                    article,
                    index_writer)
            );
        }
        while let Some(_result) = futs.next().await {}
    }
    async fn build_index(server_info: &ServerInformation,
        document_setting: &DocumentSetting,
        note_list: Vec<NoteListItem>) -> tantivy::Index {

        let schema = &document_setting.schema;
        let index = tantivy::Index::create_in_ram(schema.clone());

        index.tokenizers().register(TOKENIZER_ID, document_setting.tokenizer.clone());
        let mut index_writer = index.writer(50_000_000).unwrap();

        QueryEngine::load_all_notes(&server_info,
            &document_setting,
            note_list,
            &index_writer).await;

        index_writer.commit().unwrap();
        index
    }
}

#[derive(Debug)]
pub struct DocData {
    pub title: String,
    pub body: String,
}
use tantivy::schema::OwnedValue;
impl DocData {
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
    pub fn retrive(searcher: &tantivy::Searcher, docid: tantivy::DocAddress) -> Self {
        let doc: tantivy::TantivyDocument = searcher.doc(docid).unwrap();
        let title = Self::take_str_from_doc(&doc, 0).to_owned();
        let body = Self::take_str_from_doc(&doc, 1).to_owned();
        Self {
            title, body
        }
    }
}

impl QueryEngine {
    pub fn generate_wordcloud(self: &Self) -> String {
        String::from("TODO: wordcloud is turned off")
        //crate::word_frequency::generate_wordcloud(&self.articles)
    }

    pub async fn query_pipeline(self: &Self, term: String) -> String {
        let term: String = term_preprocess(term);
        info!("Searching {}", &term);


        let server_info: &ServerInformation = &self.server_info;

        let top_docs: Vec<(f32, tantivy::DocAddress)> = self.get_top_docs(&term);
        let searcher: tantivy::Searcher = self.reader.searcher();

        if cfg!(feature="llm") {
            for (_f, docid) in &top_docs {
                let doc = DocData::retrive(&searcher, *docid);
                let llm = self.llm.as_ref().unwrap();
                llm.post_summarize_job(doc).await;
            }
        }


        let result: Vec<String> = post_query_wrapper(top_docs, &term, &searcher, &server_info);


        let json = serde_json::to_string(&result).unwrap();

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
    async fn wait_for_summarize(&self, title: String) -> String {
        let llm = self.llm.as_ref().unwrap();
        let wait_llm = tokio::time::Duration::from_millis(50);
        // TODO maybe add a guard to make sure don't wait too long?
        loop {
            let result = llm.quick_fetch(&title).await;
            match result {
                Some(s) => { return s; },
                None => { }
            };
            tokio::time::sleep(wait_llm).await;
        }
    }
    pub async fn summarize(&self, title: String) -> String {
        info!("Called summarize on {}", &title);
        if cfg!(feature="llm") {
            self.wait_for_summarize(title).await
        } else {
            "LLM turned off".to_owned()
        }
    }
    pub async fn get_llm_done_list(&self) -> String {
        if cfg!(feature="llm") {
            let llm = self.llm.as_ref().unwrap();
            let result = &llm.get_llm_done_list().await;
            let json = serde_json::to_string(&result).unwrap();
            return json;
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

