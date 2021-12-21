use warp::Filter;

#[macro_use]
extern crate tantivy;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::Index;
use tantivy::ReloadPolicy;
use tempfile::TempDir;


use log::{info};
use log::LevelFilter;

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .format_target(false)
        .filter_level(LevelFilter::Info)
        .init();


    let logseq_path = "/home/lizhenbo/src/logseq_notebook";
    let index = indexing_documents(logseq_path);
    // let searcher = build_searcher(&index);

    let search = warp::path!("query" / String)
        .map(|name| query(name) );

    warp::serve(search)
        .run(([127, 0, 0, 1], 3030))
        .await;
}

fn build_searcher(index: &tantivy::Index) -> i32 {
    // TODO remove these unwrap()
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into().unwrap();

    let searcher = reader.searcher();
    let (schema, title,body) = build_schema_dev();
    let query_parser = QueryParser::for_index(index, vec![title, body]);
    let query = query_parser.parse_query("sea whale").unwrap();
    let top_docs = searcher.search(&query, &TopDocs::with_limit(10)).unwrap();
    for (_score, doc_address) in top_docs {
        let retrieved_doc = searcher.doc(doc_address).unwrap();
        println!("{}", schema.to_json(&retrieved_doc));
    }
    1
}

fn indexing_documents(path: &str) -> tantivy::Index {
    // TODO remove these unwrap()

    let index_path = TempDir::new().unwrap();
    info!("Using temporary directory {:?}", index_path);
    let (schema, title,body) = build_schema_dev();
    let index = Index::create_in_dir(&index_path, schema.clone()).unwrap();

    let mut index_writer = index.writer(50_000_000).unwrap();



    let mut old_man_doc = Document::default();
    old_man_doc.add_text(title, "The Old Man and the Sea");
    old_man_doc.add_text(
        body,
        "He was an old man who fished alone in a skiff in the Gulf Stream and \
         he had gone eighty-four days now without taking a fish.",
    );

    index_writer.add_document(old_man_doc);
    index_writer.add_document(doc!(
    title => "Of Mice and Men",
    body => "A few miles south of Soledad, the Salinas River drops in close to the hillside \
            bank and runs deep and green. The water is warm too, for it has slipped twinkling \
            over the yellow sands in the sunlight before reaching the narrow pool. On one \
            side of the river the golden foothill slopes curve up to the strong and rocky \
            Gabilan Mountains, but on the valley side the water is lined with trees—willows \
            fresh and green with every spring, carrying in their lower leaf junctures the \
            debris of the winter’s flooding; and sycamores with mottled, white, recumbent \
            limbs and branches that arch over the pool"
    ));

    index_writer.add_document(doc!(
    title => "Of Mice and Men",
    body => "A few miles south of Soledad, the Salinas River drops in close to the hillside \
            bank and runs deep and green. The water is warm too, for it has slipped twinkling \
            over the yellow sands in the sunlight before reaching the narrow pool. On one \
            side of the river the golden foothill slopes curve up to the strong and rocky \
            Gabilan Mountains, but on the valley side the water is lined with trees—willows \
            fresh and green with every spring, carrying in their lower leaf junctures the \
            debris of the winter’s flooding; and sycamores with mottled, white, recumbent \
            limbs and branches that arch over the pool"
    ));

    index_writer.commit().unwrap();
    index
}


fn build_schema_dev() -> (tantivy::schema::Schema, tantivy::schema::Field, tantivy::schema::Field) {
    // TODO currently for dev, a bit hacky
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT);
    let schema = schema_builder.build();
    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();
    (schema, title, body)
}

fn query(term: String) -> String {
    format!("Searching {}", term)
}