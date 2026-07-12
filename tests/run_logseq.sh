#!/usr/bin/env bash
#
# Boot the server against a Logseq notebook, for manual poking and for the
# smoke agents (.claude/agents/{fsq-smoke,ask-smoke,strip-audit}.md).
# Obsidian's equivalent is tests/run_smoke.sh, which is scripted end-to-end.
#
# Usage:  bash tests/run_logseq.sh
#         FIRE_SEQ_LOGSEQ_PATH=~/notes bash tests/run_logseq.sh
set -e

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NOTEBOOK="${FIRE_SEQ_LOGSEQ_PATH:-$HOME/logseq}"

cargo build --manifest-path "$REPO_DIR/fire_seq_search_server/Cargo.toml"

export RUST_LOG="warn,fire_seq_search_server=info"
export RUST_BACKTRACE=1

"$REPO_DIR/fire_seq_search_server/target/debug/fire_seq_search_server" \
    --notebook-path "$NOTEBOOK" \
    --enable-journal-query
