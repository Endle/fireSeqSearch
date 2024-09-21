set -e
cargo build --features llm
rm ./fire_seq_search_server -f
cp --force target/debug/fire_seq_search_server ./fire_seq_search_server

NOTEBOOK_NAME=AstroWiki_2.0-main

RUST_BACKTRACE=1 RUST_LOG=debug ./fire_seq_search_server \
    --notebook_path ~/Documents/$NOTEBOOK_NAME \
    --obsidian-md
