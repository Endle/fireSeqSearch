#!/usr/bin/env bash
# Build llama-server with Vulkan support inside a Fedora 43 podman container,
# then copy the binary out to ~/.local/bin so it runs natively on the host.
set -euo pipefail

IMAGE_TAG="llamacpp-vulkan-builder"
DEST="${HOME}/.local/bin/llama-server"

echo "[1/3] building image '${IMAGE_TAG}' (first run takes 5-10 min)"
podman build -t "${IMAGE_TAG}" -f Containerfile .

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
