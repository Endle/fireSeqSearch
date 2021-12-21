use warp::Filter;

fn query(term: String) -> String {
    format!("Searching {}", term)
}
#[tokio::main]
async fn main() {
    // GET /hello/warp => 200 OK with body "Hello, warp!"
    let hello = warp::path!("query" / String)
        .map(|name| query(name) );

    warp::serve(hello)
        .run(([127, 0, 0, 1], 3030))
        .await;
}