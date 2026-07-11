#!/usr/bin/env bash
#
# Part 1, flavour: llama-cpp. Start a native llama-server as a pre-running,
# OpenAI-compatible chat endpoint for the smoke test. Prints connection info
# (KEY=VALUE) to STDOUT; all diagnostics go to STDERR so an orchestrator can
# capture just the env with $(...). Backgrounds the server; writes its PID to
# fixtures/chat_llamacpp.pid so the caller can stop it.
#
# The model default tracks the server's own `--chat-model` default (main.rs), so
# the smoke exercises the chat backend users actually get rather than a
# test-only stand-in whose quality says nothing about theirs.
#
# Env overrides:
#   FSQ_CHAT_MODEL   chat GGUF path   (default: ~/llm/Qwen3.5-9B-UD-Q4_K_XL.gguf)
#   FSQ_LLAMA_BIN    llama-server     (default: llama-server on PATH)
#   FSQ_CHAT_PORT    port             (default: 8091)

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIX_DIR="$SCRIPT_DIR/fixtures"; mkdir -p "$FIX_DIR"

MODEL="${FSQ_CHAT_MODEL:-$HOME/llm/Qwen3.5-9B-UD-Q4_K_XL.gguf}"
LLAMA_BIN="${FSQ_LLAMA_BIN:-$(command -v llama-server || echo "$HOME/.local/bin/llama-server")}"
PORT="${FSQ_CHAT_PORT:-8091}"
LOG="$FIX_DIR/chat_llamacpp.log"
PIDFILE="$FIX_DIR/chat_llamacpp.pid"

log() { echo "[chat:llamacpp] $*" >&2; }

[ -f "$MODEL" ]     || { log "chat GGUF not found: $MODEL (set FSQ_CHAT_MODEL)"; exit 1; }
[ -x "$LLAMA_BIN" ] || { log "llama-server not found/executable: $LLAMA_BIN (set FSQ_LLAMA_BIN)"; exit 1; }

# Clear any prior test chat server started from this binary.
pkill -f "$LLAMA_BIN" 2>/dev/null && sleep 1

log "starting $(basename "$LLAMA_BIN") on :$PORT (model $(basename "$MODEL"))"
# -c / --jinja / -ngl mirror what fireSeqSearch would spawn itself: a big context
# for /ask + concurrent summaries, --jinja so enable_thinking=false is honoured,
# and Metal offload.
"$LLAMA_BIN" --model "$MODEL" --port "$PORT" -c 16384 --jinja -ngl 99 > "$LOG" 2>&1 &
echo $! > "$PIDFILE"

# Wait for /health (llama-server's readiness probe — the LlamaServer flavour).
ready=0
for _ in $(seq 1 60); do
  [ "$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT/health" 2>/dev/null)" = "200" ] && { ready=1; break; }
  kill -0 "$(cat "$PIDFILE")" 2>/dev/null || { log "llama-server exited early; log tail:"; tail -15 "$LOG" >&2; exit 1; }
  sleep 1
done
[ "$ready" = 1 ] || { log "not healthy within 60s; log tail:"; tail -15 "$LOG" >&2; exit 1; }
log "ready"

# Connection info — STDOUT only. llama-server ignores the request `model` field,
# so the name is nominal; "default" matches fireSeqSearch's default.
echo "CHAT_ENDPOINT=http://127.0.0.1:$PORT"
echo "CHAT_FLAVOUR=llama-server"
echo "CHAT_MODEL_NAME=default"
