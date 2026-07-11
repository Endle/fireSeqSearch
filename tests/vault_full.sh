#!/usr/bin/env bash
#
# Part 0, mode: full. The real AstroWiki_2.0 vault — ~366 notes across 14
# topic dirs, with genuine near-miss neighbours (Compton Scattering vs.
# Inverse-Compton vs. Thomson; Oort Cloud vs. Oort Constants). That corpus is
# what makes ranking and /ask answers *falsifiable*: the lite fixture can only
# prove the plumbing works, this one can prove the right page wins.
#
# The vault is a git clone, cached at ~/.cache/fire_seq_search/AstroWiki_2.0 and
# pinned to $PIN so tests/astro_wiki_eval.json (the gold set, tuned against that
# revision) stays meaningful. First run downloads ~500MB and lands ~1GB on disk
# (the vault carries figures); after that it's a no-op. Point FSQ_VAULT at an
# existing copy to skip the fetch entirely.
#
# Prints KEY=VALUE to STDOUT; diagnostics to STDERR.
#
# Env overrides:
#   FSQ_VAULT   use this vault instead of the cached clone (skips the fetch)
#   FSQ_PIN     check out this revision instead of the pinned one

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="https://github.com/Endle/AstroWiki_2.0"
PIN="${FSQ_PIN:-9ce2e9bc374f1a128a727cc75dca183f5fadf72d}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/fire_seq_search/AstroWiki_2.0"
EVAL_SET="$SCRIPT_DIR/astro_wiki_eval.json"

log() { echo "[vault:full] $*" >&2; }

VAULT="${FSQ_VAULT:-$CACHE}"

if [ -n "${FSQ_VAULT:-}" ]; then
  log "using FSQ_VAULT=$VAULT (no fetch; gold set assumes AstroWiki_2.0 @ ${PIN:0:8})"
elif [ -d "$VAULT/.git" ]; then
  have="$(git -C "$VAULT" rev-parse HEAD 2>/dev/null)"
  if [ "$have" != "$PIN" ]; then
    log "cached clone at ${have:0:8}, want ${PIN:0:8} — fetching"
    git -C "$VAULT" fetch --depth 1 origin "$PIN" >&2 \
      && git -C "$VAULT" checkout --quiet --detach "$PIN" >&2 \
      || { log "could not move cache to $PIN — delete $VAULT and retry"; exit 1; }
  fi
  log "cached clone @ ${PIN:0:8}"
else
  log "cloning $REPO into $CACHE (one-time, ~500MB — the vault carries figures)"
  mkdir -p "$(dirname "$CACHE")"
  rm -rf "$CACHE"
  git clone --quiet --depth 1 "$REPO" "$CACHE" >&2 || { log "clone failed"; exit 1; }
  git -C "$CACHE" fetch --quiet --depth 1 origin "$PIN" >&2 \
    && git -C "$CACHE" checkout --quiet --detach "$PIN" >&2 \
    || log "WARNING: could not pin to $PIN — gold set may not match this revision"
fi

notes="$(find "$VAULT" -type f -name '*.md' -not -path '*/.*' -not -path '*/trash/*' 2>/dev/null | wc -l | tr -d ' ')"
[ "${notes:-0}" -gt 0 ] || { log "no .md notes under $VAULT"; exit 1; }
[ "$notes" -ge 300 ] || log "WARNING: only $notes notes (expected ~366) — gold set was tuned on the full vault"
log "$notes notes"

echo "FSQ_VAULT=$VAULT"
echo "FSQ_NOTEBOOK_NAME=AstroWiki_2.0"
echo "FSQ_EVAL_SET=$EVAL_SET"
