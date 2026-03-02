#!/usr/bin/env zsh
# =============================================================================
# Salt Test Runner — Full MLIR Pipeline
# =============================================================================
# Compiles a .salt file through the full pipeline and runs it:
#   salt-front → mlir-opt → mlir-translate → clang → execute
#
# Usage:
#   ./scripts/run_test.sh tests/test_thread.salt
#   ./scripts/run_test.sh tests/test_sync.salt
#   ./scripts/run_test.sh examples/http_server.salt    # compile only (server)
#
# Options:
#   --compile-only    Build but don't execute
#   --verbose         Show each pipeline stage
#   --bridge FILE     Include additional C bridge file(s)
# =============================================================================

set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"

# LLVM tools — override with: LLVM_VERSION=19 ./scripts/run_test.sh ...
LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"
export DYLD_LIBRARY_PATH=/opt/homebrew/lib

# Defaults
COMPILE_ONLY=false
VERBOSE=false
EXTRA_BRIDGES=()
SALT_FILE=""
LIB_MODE=false

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --compile-only) COMPILE_ONLY=true; shift ;;
        --lib) LIB_MODE=true; shift ;;
        --verbose) VERBOSE=true; shift ;;
        --bridge) EXTRA_BRIDGES+=("$2"); shift 2 ;;
        *) SALT_FILE="$1"; shift ;;
    esac
done

if [[ -z "$SALT_FILE" ]]; then
    echo "Usage: $0 [--compile-only] [--verbose] [--bridge file.c] <file.salt>"
    exit 1
fi

# Derive output names from input
BASENAME=$(basename "$SALT_FILE" .salt)
TMP_DIR="/tmp/salt_build"
mkdir -p "$TMP_DIR"

MLIR_OUT="$TMP_DIR/${BASENAME}.mlir"
OPT_OUT="$TMP_DIR/${BASENAME}.opt.mlir"
LL_OUT="$TMP_DIR/${BASENAME}.ll"
BIN_OUT="$TMP_DIR/${BASENAME}"

# Determine which C bridges to link
BRIDGES=("$SALT_FRONT/runtime.c")

# Auto-detect bridges needed based on imports in the salt file
if grep -q 'std\.net\|std\.http\|std\.io\.reactor\|TcpListener\|TcpStream\|Poller\|KqueueReactor\|http_tcp_connect\|salt_http_get' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/std/net/http_bridge.c")
fi

# Detect Facet Window bridge
# Detect Facet Window bridge
LD_FLAGS=(-lm)
if grep -q 'facet_window_open' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/user/facet/window/facet_window.m")
    LD_FLAGS+=("-framework" "Cocoa" "-framework" "CoreGraphics" "-fobjc-arc")
fi

# Detect Facet GPU bridge
if grep -q 'facet_gpu' "$SALT_FILE" 2>/dev/null || grep -q 'facet_gpu' $(dirname "$SALT_FILE")/*.salt 2>/dev/null || grep -q 'facet_gpu' $(dirname "$SALT_FILE")/../*/*.salt 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/user/facet/gpu/facet_gpu.m")
    LD_FLAGS+=("-framework" "Metal" "-fobjc-arc")
fi

# Detect SPSC/kernel stub bridge (provides volatile_read_i64, cpu_pause, idle_halt)
if grep -q 'volatile_read_i64\|volatile_write_i64\|cpu_pause' "$SALT_FILE" 2>/dev/null; then
    if [[ -f "$PROJECT_ROOT/tests/bridges/spsc_bridge.c" ]]; then
        BRIDGES+=("$PROJECT_ROOT/tests/bridges/spsc_bridge.c")
    fi
fi


# Add explicit bridges
BRIDGES+=("${EXTRA_BRIDGES[@]}")

log() { [[ "$VERBOSE" == true ]] && echo "  → $1" || true; }

# Step 1: salt-front → MLIR
log "salt-front → MLIR"
if [[ "$LIB_MODE" == true ]]; then
    "$SALT_FRONT/target/release/salt-front" "$SALT_FILE" --lib --release > "$MLIR_OUT"
else
    "$SALT_FRONT/target/release/salt-front" "$SALT_FILE" --release > "$MLIR_OUT"
fi
echo "  ✓ MLIR generated"

# Step 2: mlir-opt (lowering passes)
log "mlir-opt → optimized MLIR"
mlir-opt "$MLIR_OUT" \
    --allow-unregistered-dialect \
    --canonicalize --cse --loop-invariant-code-motion --sccp --canonicalize --cse \
    --lower-affine \
    --convert-scf-to-cf \
    --convert-vector-to-llvm \
    --convert-cf-to-llvm \
    --convert-arith-to-llvm \
    --convert-math-to-llvm \
    --convert-func-to-llvm \
    --reconcile-unrealized-casts \
    -o "$OPT_OUT"
echo "  ✓ MLIR optimized"

# Step 3: Strip salt.verify ops (no LLVM lowering for verification dialect)
sed -i '' '/"salt.verify"/d' "$OPT_OUT"

# Step 4: mlir-translate → LLVM IR
log "mlir-translate → LLVM IR"
mlir-translate --mlir-to-llvmir "$OPT_OUT" -o "$LL_OUT"
echo "  ✓ LLVM IR generated"

# Step 4: clang → native binary
log "clang → binary"
# Note: ${LD_FLAGS[@]} splits correctly in zsh/bash
clang -O3 "$LL_OUT" "${BRIDGES[@]}" -o "$BIN_OUT" "${LD_FLAGS[@]}" 2>&1 | grep -v "^warning:" || true
echo "  ✓ Binary linked: $BIN_OUT"

# Step 5: Execute
if [[ "$COMPILE_ONLY" == false ]]; then
    echo ""
    echo "--- Running $BASENAME ---"
    "$BIN_OUT"
    EXIT_CODE=$?
    echo ""
    echo "--- Exit code: $EXIT_CODE ---"
    exit $EXIT_CODE
fi
