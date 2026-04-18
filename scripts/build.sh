#!/usr/bin/env zsh
# =============================================================================
# Salt Compiler Build Script
# =============================================================================
# Builds the salt-front compiler with Z3 dependencies.
#
# Usage:
#   ./scripts/build.sh              # Debug build
#   ./scripts/build.sh --release    # Release build
#   ./scripts/build.sh --test       # Build + run cargo tests
# =============================================================================

set -euo pipefail
export PATH="/opt/homebrew/bin:$PATH"

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"

# Z3 dependencies
export Z3_SYS_Z3_HEADER=/opt/homebrew/include/z3.h
export LIBRARY_PATH=/opt/homebrew/lib
export DYLD_LIBRARY_PATH=/opt/homebrew/lib

cd "$SALT_FRONT"

if [[ "${1:-}" == "--release" ]]; then
    echo "🔨 Building salt-front (release)..."
    cargo build --release
    echo "✅ Release build complete: $SALT_FRONT/target/release/salt-front"
elif [[ "${1:-}" == "--test" ]]; then
    echo "🧪 Building and testing salt-front..."
    cargo test 2>&1 | tail -20
    echo "✅ Tests complete"
else
    echo "🔨 Building salt-front (debug)..."
    cargo build
    echo "✅ Debug build complete: $SALT_FRONT/target/debug/salt-front"
fi

    for mod in \
        "std/time.salt" \
        "std/thread/thread.salt" \
        "user/os/process.salt" \
        "user/os/ipc_ring.salt" \
        "user/os/worker_ring.salt" \
        "user/netd/virtio_bridge.salt" \
        "user/browser/alloc/airlock.salt" \
        "user/browser/font.salt" \
        "user/browser/css_utils.salt" \
        "user/browser/css.salt" \
        "user/browser/css_lexer.salt" \
        "user/browser/http_lexer.salt" \
        "user/browser/dom.salt" \
        "user/browser/lexer.salt" \
        "user/browser/html_serializer.salt" \
        "user/browser/paint.salt" \
        "user/browser/events.salt" \
        "user/browser/layout.salt" \
        "user/browser/timers.salt" \
        "user/browser/js_quickjs.salt" \
        "user/browser/websocket.salt" \
        "user/browser/worker.salt" \
        "user/browser/compositor.salt" \
        "user/browser/chrome.salt" \
        "user/browser/media.salt" \
        "user/browser/main.salt" \
        "user/browser/transpiler.salt" \
        "user/browser/net.salt" \
        "user/browser/storage.salt" \
        "user/browser/custom_elements.salt" \
        "user/browser/selectors.salt" ; do
        :
    done
