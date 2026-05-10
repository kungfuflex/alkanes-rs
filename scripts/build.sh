#!/usr/bin/env bash
# Reproducible mainnet alkanes.wasm builder.
#
# Same source tree → byte-identical wasm. The resulting dist/alkanes.wasm
# is what we ship to the v2.2.0-alpha.1 subkube shadow pod; its sha256
# is recorded in dist/SHA256SUMS and tracked in git so any divergence
# from a fresh rebuild surfaces immediately.
#
# Usage:
#   scripts/build.sh                      # build, write dist/alkanes.wasm + dist/SHA256SUMS
#   scripts/build.sh --verify             # build twice in scratch dirs, assert hashes match
#   scripts/build.sh --verify-against H   # build, assert sha256 == H

set -euo pipefail
cd "$(dirname "$0")/.."

DOCKERFILE="scripts/Dockerfile.wasm"
OUT_DIR="dist"

build_to() {
    local dest="$1"
    echo "→ Building reproducible mainnet wasm into ${dest}…"
    mkdir -p "${dest}"
    # Classic `docker build` (works without buildx). The Dockerfile has no
    # BuildKit-only syntax so this stays portable.
    local tag="alkanes-rs-wasm-build:reproduce-$$"
    docker build \
        --file "${DOCKERFILE}" \
        --target wasm \
        --tag "${tag}" \
        .
    # Extract /alkanes.wasm via a throwaway container. `scratch` images
    # have no command, so create-only is fine.
    local cid
    cid=$(docker create "${tag}")
    docker cp "${cid}:/alkanes.wasm" "${dest}/alkanes.wasm"
    docker rm "${cid}" >/dev/null
    docker rmi "${tag}" >/dev/null 2>&1 || true
    sha256sum "${dest}/alkanes.wasm" | tee "${dest}/SHA256SUMS"
}

case "${1:-}" in
    --verify)
        A=".verify-a"
        B=".verify-b"
        rm -rf "${A}" "${B}"
        build_to "${A}"
        build_to "${B}"
        ah=$(awk '{print $1}' "${A}/SHA256SUMS")
        bh=$(awk '{print $1}' "${B}/SHA256SUMS")
        if [ "${ah}" != "${bh}" ]; then
            echo "REPRODUCIBILITY FAILURE:"
            echo "  build a: ${ah}"
            echo "  build b: ${bh}"
            exit 1
        fi
        echo "OK: both builds hashed to ${ah}"
        rm -rf "${A}" "${B}"
        ;;
    --verify-against)
        expected="${2:?expected sha256 required as second argument}"
        build_to "${OUT_DIR}"
        actual=$(awk '{print $1}' "${OUT_DIR}/SHA256SUMS")
        if [ "${actual}" != "${expected}" ]; then
            echo "MISMATCH: expected ${expected}, got ${actual}"
            exit 1
        fi
        echo "OK: ${actual} matches expected"
        ;;
    *)
        build_to "${OUT_DIR}"
        ;;
esac
