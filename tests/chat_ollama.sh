#!/usr/bin/env bash
#
# Part 1, flavour: ollama. Verify a local Ollama is serving the chat model, then
# print connection info (KEY=VALUE) to STDOUT; diagnostics go to STDERR. Ollama
# is a long-lived system service, so we do NOT manage its lifecycle and write no
# PID file — the orchestrator leaves it running.
#
# Env overrides:
#   FSQ_OLLAMA_ENDPOINT   base URL, no /v1   (default: http://127.0.0.1:11434)
#   FSQ_OLLAMA_MODEL      model name prefix  (default: qwen3-nothink)

set -uo pipefail

ENDPOINT="${FSQ_OLLAMA_ENDPOINT:-http://127.0.0.1:11434}"
MODEL="${FSQ_OLLAMA_MODEL:-qwen3-nothink}"

log() { echo "[chat:ollama] $*" >&2; }

# Ollama up? (native API at /api/tags; OpenAI-compat that fireSeqSearch uses is
# at $ENDPOINT/v1 — the endpoint must NOT include /v1.)
if [ "$(curl -s -o /dev/null -w '%{http_code}' "$ENDPOINT/api/tags" 2>/dev/null)" != "200" ]; then
  log "no Ollama at $ENDPOINT — start it with 'ollama serve' (or set FSQ_OLLAMA_ENDPOINT)"
  exit 1
fi

# Model present? (match by name prefix so 'qwen3-nothink' finds 'qwen3-nothink:latest')
if ! curl -s "$ENDPOINT/api/tags" | jq -e --arg m "$MODEL" 'any(.models[]?.name; startswith($m))' >/dev/null 2>&1; then
  log "model '$MODEL' not in Ollama (set FSQ_OLLAMA_MODEL to override)"
  log "available: $(curl -s "$ENDPOINT/api/tags" | jq -r '.models[]?.name' | paste -sd, -)"
  exit 1
fi
log "using $MODEL @ $ENDPOINT"

echo "CHAT_ENDPOINT=$ENDPOINT"
echo "CHAT_FLAVOUR=ollama"
echo "CHAT_MODEL_NAME=$MODEL"
