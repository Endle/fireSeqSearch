set -e
cd fire_seq_search_server
cargo build --release
rm ./fire_seq_search_server
cp --force target/release/fire_seq_search_server.exe ./fire_seq_search_server
RUST_LOG=warn ./fire_seq_search_server --notebook_path /c/Users/z2369li/Nextcloud/logseq_notebook
