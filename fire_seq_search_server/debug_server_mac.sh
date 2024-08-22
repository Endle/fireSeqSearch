set -e
rm -f ./fire_seq_search_server 
#nix-shell -p cargo -p rustc -p libiconv --run "cargo build"
cargo build
cp  target/debug/fire_seq_search_server ./fire_seq_search_server

export RUST_LOG="warn,fire_seq_search_server=info"
#export RUST_LOG="debug"
export RUST_BACKTRACE=1
./fire_seq_search_server --notebook_path ~/logseq --enable-journal-query
