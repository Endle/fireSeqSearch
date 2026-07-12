#!/usr/bin/env bash
#
# ONE way to get a llama-server, not the required way. fireSeqSearch talks to
# any OpenAI-compatible endpoint (`--chat-endpoint` / `--embed-endpoint`), so
# Ollama or a remote server works just as well and needs none of this.
#
# This is the maintainer's setup: an AMD GPU on Fedora, where Vulkan is the
# sane backend and ROCm is not. It builds llama-server with Vulkan support
# inside a Fedora 43 podman container (see the Containerfile next to this
# script), then copies the binary out to ~/.local/bin so it runs natively on
# the host. Adapt or ignore.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IMAGE_TAG="llamacpp-vulkan-builder"
DEST="${HOME}/.local/bin/llama-server"

echo "[1/3] building image '${IMAGE_TAG}' (first run takes 5-10 min)"
podman build -t "${IMAGE_TAG}" -f "${SCRIPT_DIR}/Containerfile" "${SCRIPT_DIR}"

echo "[2/3] extracting llama-server to ${DEST}"
mkdir -p "$(dirname "${DEST}")"
cid=$(podman create "${IMAGE_TAG}")
trap 'podman rm "${cid}" >/dev/null' EXIT
podman cp "${cid}:/src/build/bin/llama-server" "${DEST}"
chmod +x "${DEST}"

echo "[3/3] sanity check"
missing=$(ldd "${DEST}" | grep "not found" || true)
if [[ -n "${missing}" ]]; then
  echo "WARNING: missing host libraries:"
  echo "${missing}"
  echo "install them with: sudo dnf install <pkg>"
  exit 1
fi

echo
echo "OK. ${DEST} is ready."
echo "Devices llama.cpp can see:"
"${DEST}" --list-devices
