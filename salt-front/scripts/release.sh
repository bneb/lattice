#!/usr/bin/env bash
# Build release binaries for saltc and create a tarball.
# Usage: bash scripts/release.sh [version]
# Requires: cargo, z3 (system library)
set -euo pipefail

VERSION="${1:-$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)}"
PLATFORM=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$PLATFORM-$ARCH" in
    darwin-arm64)  TARGET="aarch64-apple-darwin" ;;
    darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
    linux-x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    linux-arm64)   TARGET="aarch64-unknown-linux-gnu" ;;
    *) echo "Unsupported platform: $PLATFORM-$ARCH"; exit 1 ;;
esac

RELEASE_DIR="target/release"
ARCHIVE="saltc-v${VERSION}-${PLATFORM}-${ARCH}.tar.gz"

echo "=== Building saltc v${VERSION} for ${TARGET} ==="
cargo build --release

if [ ! -f "$RELEASE_DIR/saltc" ]; then
    echo "ERROR: saltc binary not found at $RELEASE_DIR/saltc"
    exit 1
fi

echo "=== Creating release archive: ${ARCHIVE} ==="
cd "$RELEASE_DIR"
tar czf "../../${ARCHIVE}" saltc
cd - > /dev/null

SHA256=$(shasum -a 256 "${ARCHIVE}" | cut -d' ' -f1)
echo ""
echo "=== Release built ==="
echo "Archive:  ${ARCHIVE}"
echo "SHA256:   ${SHA256}"
echo ""
echo "Homebrew formula stanza:"
echo ""
echo "  url \"https://github.com/bneb/lattice/releases/download/v${VERSION}/${ARCHIVE}\""
echo "  sha256 \"${SHA256}\""
