cd fire_seq_search_server
cargo build
cp --force target/debug/fire_seq_search_server .
./fire_seq_search_server --notebook-path /home/lizhenbo/src/logseq_notebook
