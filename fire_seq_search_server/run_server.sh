set -e
#  --release remove this parameter to save compile time
cargo build
rm -f ./fire_seq_search_server
# Still use the debug version
cp --force target/debug/fire_seq_search_server.exe ./fire_seq_search_server
RUST_LOG=warn ./fire_seq_search_server --notebook_path /c/Users/z2369li/Nextcloud/logseq_notebook
