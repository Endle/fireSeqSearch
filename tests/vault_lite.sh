#!/usr/bin/env bash
#
# Part 0, mode: lite. The committed astro-wiki-lite fixture — 2 notes plus a
# trash/ decoy. Fast (well under a minute, CPU-only) and hermetic, so it can
# assert the mechanical invariants: walker parity, URI integrity, summary
# health. It CANNOT say anything about ranking — with 2 notes, whatever comes
# back is trivially the top hit. For that, use vault_full.sh.
#
# Prints KEY=VALUE to STDOUT; diagnostics to STDERR.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VAULT="$SCRIPT_DIR/astro-wiki-lite"

log() { echo "[vault:lite] $*" >&2; }

[ -d "$VAULT" ] || { log "fixture missing: $VAULT"; exit 1; }
log "$(find "$VAULT" -name '*.md' -not -path '*/.*' -not -path '*/trash/*' | wc -l | tr -d ' ') notes"

echo "FSQ_VAULT=$VAULT"
echo "FSQ_NOTEBOOK_NAME=astro-wiki-lite"
echo "FSQ_EVAL_SET="
