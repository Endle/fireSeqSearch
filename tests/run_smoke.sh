#!/usr/bin/env bash
#
# Orchestrator: compose part 1 (chat flavour) + part 2 (fireSeqSearch smoke).
# The flavour and the test are orthogonal — swap the provisioner, same smoke.
#
# Usage:  bash tests/run_smoke.sh [llamacpp|ollama] [query]
#
# Exit code is the smoke test's: 0 iff every hard check passed.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIX_DIR="$SCRIPT_DIR/fixtures"
FLAVOUR="${1:-llamacpp}"
QUERY="${2:-compton scattering}"

case "$FLAVOUR" in
  llamacpp) PROVISIONER="$SCRIPT_DIR/chat_llamacpp.sh"; PIDFILE="$FIX_DIR/chat_llamacpp.pid" ;;
  ollama)   PROVISIONER="$SCRIPT_DIR/chat_ollama.sh";   PIDFILE="$FIX_DIR/chat_ollama.pid" ;;
  *) echo "usage: run_smoke.sh [llamacpp|ollama] [query]" >&2; exit 2 ;;
esac

stop_chat() { [ -f "$PIDFILE" ] && kill -TERM "$(cat "$PIDFILE")" 2>/dev/null; rm -f "$PIDFILE"; }
trap stop_chat EXIT

echo "== part 1: chat server ($FLAVOUR) =="
chat_env="$(bash "$PROVISIONER")" || { echo "chat provisioner failed" >&2; exit 1; }
echo "$chat_env" | sed 's/^/  /'
# Export the KEY=VALUE lines the provisioner emitted on stdout.
while IFS= read -r line; do
  [ -n "$line" ] && export "$line"
done <<< "$chat_env"

echo "== part 2: fireSeqSearch smoke =="
bash "$SCRIPT_DIR/obsidian_smoke.sh" "$QUERY"; rc=$?
exit "$rc"
