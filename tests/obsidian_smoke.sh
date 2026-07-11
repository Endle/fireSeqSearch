#!/usr/bin/env bash
#
# Part 2 of the Obsidian smoke test: run fireSeqSearch against a pinned vault
# and assert the Obsidian invariants. FLAVOUR-AGNOSTIC — it consumes a
# pre-running chat backend (started by a part-1 provisioner, e.g.
# chat_llamacpp.sh or chat_ollama.sh) via env, so the same test runs against
# any chat flavour.
#
# Does the deterministic work: index a small committed vault, boot the server,
# cold-index, query, and assert walker parity, non-empty snippets, no failed
# summaries, and that every result URI resolves to a real file (so the %2F
# directory-prefix bug can't come back). Prints a RESULTS block (hits, for a
# human/LLM to judge snippet & summary *quality*) and a CHECKS block, then
# exits 0 iff every hard check passed.
#
# Required env (the chat backend — fireSeqSearch won't boot without one):
#   CHAT_ENDPOINT     e.g. http://127.0.0.1:8091   (no trailing /v1)
#   CHAT_FLAVOUR      llama-server | ollama | openai
#   CHAT_MODEL_NAME   model name the endpoint serves
#
# Usage:  CHAT_ENDPOINT=… CHAT_FLAVOUR=… CHAT_MODEL_NAME=… bash tests/obsidian_smoke.sh [query]
# (normally invoked by tests/run_smoke.sh, which starts the chat server first.)

set -uo pipefail   # not -e: teardown + CHECKS must run even after a failed assertion

# ---- test corpus -------------------------------------------------------------
# Default: the tiny committed "astro-wiki-lite" vault (2 notes + a trash/ decoy),
# so the whole workflow runs in well under a minute even with CPU-only embedding.
# Override FSQ_VAULT with a path to run against a bigger vault — e.g. a clone of
# https://github.com/Endle/AstroWiki_2.0 @ 9ce2e9bc374f1a128a727cc75dca183f5fadf72d
QUERY="${1:-compton scattering}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIX_DIR="$SCRIPT_DIR/fixtures"
VAULT_DIR="${FSQ_VAULT:-$SCRIPT_DIR/astro-wiki-lite}"
DB_PATH="$FIX_DIR/smoke.sqlite"
SERVER_LOG="$FIX_DIR/server.log"

BASE="http://127.0.0.1:3030"
INDEX_WAIT_MAX="${FSQ_INDEX_WAIT:-600}"   # seconds to wait for the index before querying anyway
SUMMARY_GRACE="${FSQ_SUMMARY_GRACE:-45}"  # seconds to let top-hit summaries land after the first query
COLD="${FSQ_COLD:-1}"                     # 1 = wipe DB for a cold-start run; 0 = reuse (fast local iteration)

# ---- check bookkeeping -------------------------------------------------------
hard_fail=0
CHECKS=()
_check() { CHECKS+=("$1|$2|$3"); }                 # status|name|detail
pass()   { _check PASS "$1" "$2"; }
warn()   { _check WARN "$1" "$2"; }
fail()   { _check FAIL "$1" "$2"; hard_fail=1; }

# Populated in step 7; pre-initialised so an early die() can print under set -u.
hits=""; nhits=0

print_report() {
  echo
  echo "===== RESULTS (query='$QUERY' via $CHAT_FLAVOUR — judge snippet/summary quality) ====="
  if [ -n "$hits" ] && echo "$hits" | jq -e 'type=="array" and length>0' >/dev/null 2>&1; then
    echo "$hits" | jq -r '.[] | "• \(.title)  [score \(.score)]  \(.summary_status)\n    uri    : \(.logseq_uri)\n    snippet: \(.top_snippet)\n    summary: \(.summary // "(none)")"'
  else
    echo "(no hits captured)"
  fi
  echo
  echo "===== CHECKS ====="
  if [ "${#CHECKS[@]}" -gt 0 ]; then
    for c in "${CHECKS[@]}"; do
      st="${c%%|*}"; rest="${c#*|}"; name="${rest%%|*}"; detail="${rest#*|}"
      printf '%-4s %-18s %s\n' "$st" "$name" "$detail"
    done
  fi
  echo
  [ "$hard_fail" -eq 0 ] && echo "===== SMOKE RESULT: PASS =====" || echo "===== SMOKE RESULT: FAIL ====="
}

SERVER_PID=""
cleanup() {
  # Tear down only what THIS script started: fireSeqSearch (which reaps its own
  # embed child) plus a backstop for the embed llamafile. The external chat
  # server belongs to the caller — never touch it (keeps flavour orthogonal).
  [ -n "$SERVER_PID" ] && kill -TERM "$SERVER_PID" 2>/dev/null
  sleep 2
  pkill -f 'bge-m3.llamafile' 2>/dev/null
}
trap cleanup EXIT

urlencode() { python3 -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1],safe=""),end="")' "$1"; }
urldecode() { python3 -c 'import sys,urllib.parse;print(urllib.parse.unquote(sys.argv[1]),end="")' "$1"; }
die() { echo "FATAL: $2" >&2; fail "$1" "$2"; print_report; exit 1; }

# ---- 0. preconditions --------------------------------------------------------
for bin in curl jq python3 cargo; do
  command -v "$bin" >/dev/null 2>&1 || die "missing_dep" "required tool not found: $bin"
done
[ -n "${CHAT_ENDPOINT:-}" ]   || die "chat_env" "CHAT_ENDPOINT unset (run a part-1 provisioner first)"
[ -n "${CHAT_FLAVOUR:-}" ]    || die "chat_env" "CHAT_FLAVOUR unset (llama-server|ollama|openai)"
[ -n "${CHAT_MODEL_NAME:-}" ] || die "chat_env" "CHAT_MODEL_NAME unset"
echo "[smoke] chat backend: $CHAT_FLAVOUR @ $CHAT_ENDPOINT (model $CHAT_MODEL_NAME)"

mkdir -p "$FIX_DIR"

# ---- 1. vault present --------------------------------------------------------
vault_notes="$(find "$VAULT_DIR" -type f -name '*.md' -not -path '*/.*' -not -path '*/trash/*' 2>/dev/null | wc -l | tr -d ' ')"
{ [ -d "$VAULT_DIR" ] && [ "${vault_notes:-0}" -gt 0 ] 2>/dev/null && pass "vault" "$VAULT_DIR ($vault_notes notes)"; } \
  || die "vault" "no .md notes under $VAULT_DIR (set FSQ_VAULT to a valid vault)"

# ---- 2. clean slate: free ports, cold DB -------------------------------------
pkill -f 'fire_seq_search_server --notebook' 2>/dev/null
sleep 1
rm -f "$SERVER_LOG"
[ "$COLD" = 1 ] && rm -f "$DB_PATH" "$DB_PATH-shm" "$DB_PATH-wal"

# ---- 3. build + boot ---------------------------------------------------------
echo "[smoke] building server ..."
cargo build --quiet --manifest-path "$REPO_ROOT/fire_seq_search_server/Cargo.toml" \
  || die "build" "cargo build failed"

echo "[smoke] booting server against $VAULT_DIR ..."
RUST_LOG="warn,fire_seq_search_server=info" RUST_BACKTRACE=1 \
  "$REPO_ROOT/fire_seq_search_server/target/debug/fire_seq_search_server" \
    --notebook-path "$VAULT_DIR" \
    --notebook obsidian \
    --notebook-name AstroWiki_2.0 \
    --db-path "$DB_PATH" \
    --chat-endpoint "$CHAT_ENDPOINT" \
    --chat-flavour "$CHAT_FLAVOUR" \
    --chat-model-name "$CHAT_MODEL_NAME" \
    > "$SERVER_LOG" 2>&1 &
SERVER_PID=$!

# ---- 4. readiness ------------------------------------------------------------
ready=0
for _ in $(seq 1 60); do
  [ "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/server_info" 2>/dev/null)" = "200" ] && { ready=1; break; }
  kill -0 "$SERVER_PID" 2>/dev/null || break   # process died
  sleep 1
done
if [ "$ready" != 1 ]; then
  echo "----- server.log tail -----"; tail -20 "$SERVER_LOG"
  die "boot" "server never returned 200 on /server_info"
fi
pass "boot" "server up"

# ---- 5. wait for the cold index ----------------------------------------------
echo "[smoke] indexing (cold) ..."
prev_chunks=-1; stable=0
for _ in $(seq 1 $((INDEX_WAIT_MAX / 3)) ); do
  info="$(curl -s "$BASE/server_info")"
  in_flight="$(echo "$info" | jq -r '.indexer.in_flight')"
  chunks="$(echo "$info" | jq -r '.indexer.indexed_chunks')"
  [ "$in_flight" = "false" ] && break
  [ "$chunks" = "$prev_chunks" ] && stable=$((stable+1)) || stable=0
  [ "$stable" -ge 3 ] && break     # plateaued
  prev_chunks="$chunks"
  sleep 3
done
info="$(curl -s "$BASE/server_info")"
idx_notes="$(echo "$info"  | jq -r '.indexer.indexed_notes')"
idx_chunks="$(echo "$info" | jq -r '.indexer.indexed_chunks')"
in_flight="$(echo "$info"  | jq -r '.indexer.in_flight')"
complete=0; [ "$in_flight" = "false" ] && complete=1
[ "$complete" = 1 ] || warn "index_incomplete" "still in_flight after ${INDEX_WAIT_MAX}s — coverage checks below are best-effort (raise FSQ_INDEX_WAIT)"

# `indexed` only hard-FAILs on a *finished* empty index; a partial index that has
# some chunks is fine (the query checks exercise whatever's there so far).
if [ "${idx_chunks:-0}" -gt 0 ] 2>/dev/null; then
  pass "indexed" "$idx_notes notes / $idx_chunks chunks$( [ "$complete" = 1 ] || echo ' (partial)' )"
elif [ "$complete" = 1 ]; then
  fail "indexed" "index finished with 0 chunks"
else
  warn "indexed" "0 chunks at the ${INDEX_WAIT_MAX}s snapshot; index still warming"
fi

# ---- 6. walker parity (only meaningful once the index has finished) ----------
# Mirror the walker's skips: any dot-dir (.obsidian/.git/…) and trash/.
found="$(find "$VAULT_DIR" -type f -name '*.md' -not -path '*/.*' -not -path '*/trash/*' | wc -l | tr -d ' ')"
if [ "$complete" != 1 ]; then
  warn "walker_parity" "index incomplete ($idx_notes/$found so far) — coverage not asserted"
elif [ "$found" -gt 0 ] 2>/dev/null; then
  pct=$(( idx_notes * 100 / found ))
  if [ "$idx_notes" -gt "$found" ]; then
    fail "walker_parity" "indexed $idx_notes > found $found — dot-dirs leaking into the walk"
  elif [ "$pct" -lt 50 ]; then
    fail "walker_parity" "indexed $idx_notes vs found $found (${pct}%) — walker dropping files"
  elif [ "$pct" -lt 90 ]; then
    warn "walker_parity" "indexed $idx_notes vs found $found (${pct}%)"
  else
    pass "walker_parity" "indexed $idx_notes vs found $found (${pct}%)"
  fi
else
  warn "walker_parity" "found 0 .md files under vault — check fixture"
fi

# ---- 7. query (twice: first triggers the lazy summary bump on top hits) ------
enc="$(urlencode "$QUERY")"
curl -s "$BASE/query/$enc" >/dev/null                 # warm: promote top-10 summaries
sleep "$SUMMARY_GRACE"
hits="$(curl -s "$BASE/query/$enc")"
if ! echo "$hits" | jq -e 'type == "array"' >/dev/null 2>&1; then
  echo "$hits" | head -3
  die "query" "/query did not return a JSON array"
fi
nhits="$(echo "$hits" | jq 'length')"
{ [ "$nhits" -ge 1 ] 2>/dev/null && pass "query_hits" "$nhits hit(s) for '$QUERY'"; } \
  || fail "query_hits" "0 hits for '$QUERY'"

# ---- 8. hit-level assertions (only meaningful with ≥1 hit) -------------------
if [ "$nhits" -ge 1 ] 2>/dev/null; then
  top_snip="$(echo "$hits" | jq -r '.[0].top_snippet // ""')"
  [ -n "$top_snip" ] && pass "snippet_nonempty" "top snippet present" \
    || fail "snippet_nonempty" "top hit has an empty top_snippet (chunker emitted nothing)"

  top_score="$(echo "$hits" | jq -r '.[0].score')"
  awk "BEGIN{exit !($top_score >= 0.50)}" 2>/dev/null && pass "top_score" "top score $top_score" \
    || warn "top_score" "top score $top_score < 0.50 (weak, or term thin in corpus — Obsidian floor runs low)"

  failed_summaries="$(echo "$hits" | jq '[.[] | select(.summary_status == "failed")] | length')"
  [ "$failed_summaries" -eq 0 ] && pass "summaries" "no failed summaries" \
    || fail "summaries" "$failed_summaries hit(s) with summary_status=failed"

  # Frontmatter leak: only a snippet that *starts* with a '---' fence indicates a
  # strip regression. A '---' mid-snippet is a Markdown horizontal rule (legit),
  # so match leading fences only to avoid false positives.
  if echo "$hits" | jq -e '[.[].top_snippet // "" | select(startswith("---"))] | length > 0' >/dev/null 2>&1; then
    warn "frontmatter" "a snippet starts with a '---' fence (possible frontmatter leak)"
  else
    pass "frontmatter" "no leading '---' fence in snippets"
  fi

  # URI integrity (the %2F prefix bug): every hit's file= param, url-decoded,
  # must resolve to a real .md under the vault. A nested note whose prefix got
  # dropped would decode to a bare basename that doesn't exist — caught here.
  unresolved=0; nested=0
  for i in $(seq 0 $((nhits - 1)) ); do
    uri="$(echo "$hits" | jq -r ".[$i].logseq_uri")"
    raw="${uri#*file=}"; raw="${raw%%&*}"
    dec="$(urldecode "$raw")"
    case "$dec" in */*) nested=$((nested+1));; esac
    [ -f "$VAULT_DIR/$dec.md" ] || { unresolved=$((unresolved+1)); echo "[smoke] unresolved URI: $uri -> $dec.md" >&2; }
  done
  if [ "$unresolved" -gt 0 ]; then
    fail "uri_resolves" "$unresolved/$nhits result URIs don't resolve to a vault file (dir-prefix / %2F bug?)"
  else
    pass "uri_resolves" "all $nhits URIs resolve ($nested nested, carrying the %2F prefix)"
  fi
fi

# ---- 9. log scan -------------------------------------------------------------
if grep -q 'panic' "$SERVER_LOG"; then
  fail "log_panic" "'panic' in server log: $(grep -m1 panic "$SERVER_LOG")"
else
  pass "log_panic" "no panics"
fi
errcount="$(grep -icE 'error|(HTTP )?500|input too large' "$SERVER_LOG" || true)"
{ [ "$errcount" -gt 0 ] && warn "log_errors" "$errcount error/500-ish line(s) in log (review — embed 500s are a known class)"; } \
  || pass "log_errors" "no error lines"

# ---- report ------------------------------------------------------------------
print_report
exit "$hard_fail"
