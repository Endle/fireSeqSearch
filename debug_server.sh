set -e
# nix-shell -p cargo -p rustc -p libiconv --run "cargo build"
cargo build --manifest-path fire_seq_search_server/Cargo.toml

export RUST_LOG="warn,fire_seq_search_server=info"
#export RUST_LOG="debug"
export RUST_BACKTRACE=1
#RAYON_NUM_THREADS=1
./fire_seq_search_server/target/debug/fire_seq_search_server --notebook_path ~/logseq --enable-journal-query
