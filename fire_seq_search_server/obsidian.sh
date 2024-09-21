set -e
cargo build --features llm
rm ./fire_seq_search_server -f
cp --force target/debug/fire_seq_search_server ./fire_seq_search_server

RUST_BACKTRACE=1 RUST_LOG=debug ./fire_seq_search_server \
    --notebook_path ~/Documents/obsidian-hub-main \
    --obsidian-md
