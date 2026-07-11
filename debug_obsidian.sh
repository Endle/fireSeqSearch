set -e
cargo build --manifest-path fire_seq_search_server/Cargo.toml

export RUST_LOG="warn,fire_seq_search_server=info"
export RUST_BACKTRACE=1

# Pass --notebook-name explicitly: the auto-guess splits on '/' and a trailing
# slash on --notebook-path would leave it empty, which then breaks the
# obsidian://open?vault=… URIs the addon produces.
./fire_seq_search_server/target/debug/fire_seq_search_server \
    --notebook obsidian \
    --notebook-path ~/Documents/AstroWiki_2.0-main \
    --notebook-name AstroWiki_2.0-main
