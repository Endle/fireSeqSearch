set -e
rm -f ./fire_seq_search_server 
#nix-shell -p cargo -p rustc -p libiconv --run "cargo build"
cargo build
cp  target/debug/fire_seq_search_server ./fire_seq_search_server
RUST_BACKTRACE=1 RUST_LOG=debug ./fire_seq_search_server \
--notebook_path /Users/zhenboli/logseq \
--exclude-zotero-items
