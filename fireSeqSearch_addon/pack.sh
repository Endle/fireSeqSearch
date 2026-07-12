#!/usr/bin/env bash
#
# Dev helper: zip this directory into an unsigned extension package for
# temporary loading (about:debugging in Firefox). Not part of a release —
# the published addon is built by AMO from a signed upload.
#
# Usage:  bash fireSeqSearch_addon/pack.sh [dest_dir]
set -euo pipefail

ADDON_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$(cd "${ADDON_DIR}/.." && pwd)/fireSeqSearch.zip"

rm -f "${OUT}"
cd "${ADDON_DIR}"
zip -r -FS "${OUT}" * \
    --exclude '*.git*' \
    --exclude 'pack.sh' \
    --exclude 'monkeyscript.user.js' \
    --exclude 'violentmonkeyscript.user.js'

if [ $# -ge 1 ]; then
    cp -f "${OUT}" "$1"
    echo "copied to $1"
fi
echo "OK. ${OUT}"
