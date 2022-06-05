set -e
cd fire_seq_search_server
cargo build
rm ./fire_seq_search_server
cp --force target/debug/fire_seq_search_server.exe ./fire_seq_search_server
RUST_BACKTRACE=1 RUST_LOG=debug ./fire_seq_search_server --notebook_path /c/Users/z2369li/Nextcloud/logseq_notebook
