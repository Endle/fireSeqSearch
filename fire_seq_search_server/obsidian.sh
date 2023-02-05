set -e
cargo build
rm ./fire_seq_search_server -f
cp --force target/debug/fire_seq_search_server.exe ./fire_seq_search_server

RUST_BACKTRACE=1 RUST_LOG=debug ./fire_seq_search_server \
--notebook_path /c/Users/z2369li/Documents/obsidian_md/linotes \
--obsidian_md
