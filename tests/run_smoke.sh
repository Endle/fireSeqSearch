#!/usr/bin/env bash
#
# Orchestrator: compose part 0 (vault mode) + part 1 (chat flavour) + part 2
# (fireSeqSearch smoke). All three are orthogonal — swap the vault, swap the
# provisioner, same smoke.
#
# Usage:  bash tests/run_smoke.sh [llamacpp|ollama] [lite|full] [query]
#
#   lite  the committed tests/astro-wiki-lite fixture (2 notes). Fast, hermetic,
#         proves the plumbing: walker parity, URI integrity, summary health.
#         It CANNOT judge ranking — with 2 notes there is nothing to outrank.
#   full  the real AstroWiki_2.0 vault (~366 notes; cloned + cached on first run,
#         ~1GB on disk). The only mode that can test whether score priority and /ask
#         answers are actually CORRECT: it grades both against
#         tests/astro_wiki_eval.json, whose queries all have near-miss
#         neighbours in the corpus. Takes minutes — it cold-embeds 366 notes.
#
# Exit code is the smoke test's: 0 iff every hard check passed.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIX_DIR="$SCRIPT_DIR/fixtures"
FLAVOUR="${1:-llamacpp}"
MODE="${2:-lite}"
QUERY="${3:-compton scattering}"

usage() { echo "usage: run_smoke.sh [llamacpp|ollama] [lite|full] [query]" >&2; exit 2; }

case "$FLAVOUR" in
  llamacpp) PROVISIONER="$SCRIPT_DIR/chat_llamacpp.sh"; PIDFILE="$FIX_DIR/chat_llamacpp.pid" ;;
  ollama)   PROVISIONER="$SCRIPT_DIR/chat_ollama.sh";   PIDFILE="$FIX_DIR/chat_ollama.pid" ;;
  *) usage ;;
esac
case "$MODE" in
  lite) VAULT_PROVISIONER="$SCRIPT_DIR/vault_lite.sh" ;;
  full) VAULT_PROVISIONER="$SCRIPT_DIR/vault_full.sh" ;;
  *) usage ;;
esac

stop_chat() { [ -f "$PIDFILE" ] && kill -TERM "$(cat "$PIDFILE")" 2>/dev/null; rm -f "$PIDFILE"; }
trap stop_chat EXIT

# Export the KEY=VALUE lines a provisioner emitted on stdout.
export_env() { while IFS= read -r line; do [ -n "$line" ] && export "$line"; done <<< "$1"; }

echo "== part 0: vault ($MODE) =="
vault_env="$(bash "$VAULT_PROVISIONER")" || { echo "vault provisioner failed" >&2; exit 1; }
echo "$vault_env" | sed 's/^/  /'
export_env "$vault_env"

echo "== part 1: chat server ($FLAVOUR) =="
chat_env="$(bash "$PROVISIONER")" || { echo "chat provisioner failed" >&2; exit 1; }
echo "$chat_env" | sed 's/^/  /'
export_env "$chat_env"

echo "== part 2: fireSeqSearch smoke =="
bash "$SCRIPT_DIR/obsidian_smoke.sh" "$QUERY"; rc=$?
exit "$rc"
