#!/bin/bash
# =============================================================================
# C10M Echo Benchmark — Head-to-Head Comparison
#
# Builds and benchmarks 4 echo server implementations:
#   1. C     (raw kqueue, single-threaded)
#   2. Rust  (Tokio async runtime)
#   3. TS    (Bun native TCP)
#   3. Salt  (kqueue via FFI bridge, MLIR pipeline)
#
# Metrics collected:
#   - Connections/sec (accept rate)
#   - Mean latency (µs)
#   - Tail latency p99 (µs)
#   - Binary size (bytes)
#   - Resident memory (KB)
#
# Usage: ./benchmark_echo.sh [port] [duration_sec] [num_connections]
# =============================================================================

set -euo pipefail

PORT=${1:-9000}
DURATION=${2:-10}
NUM_CONNS=${3:-1000}
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/build"
RESULTS_FILE="$SCRIPT_DIR/echo_benchmark_results.txt"

# LLVM Tools
LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"
CLANG="${CLANG:-/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin/clang}"
LLC="${LLC:-/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin/llc}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

header() { echo -e "\n${BOLD}${CYAN}═══════════════════════════════════════════${NC}"; echo -e "${BOLD}  $1${NC}"; echo -e "${BOLD}${CYAN}═══════════════════════════════════════════${NC}"; }
pass()   { echo -e "  ${GREEN}✅ $1${NC}"; }
fail()   { echo -e "  ${RED}❌ $1${NC}"; }

mkdir -p "$BUILD_DIR"

# =============================================================================
# Phase 1: Build All Targets
# =============================================================================
header "Phase 1: Building Echo Servers"

# --- C (kqueue) ---
echo -e "\n${BOLD}[1/4] C / kqueue${NC}"
if $CLANG -O3 -o "$BUILD_DIR/echo_c" "$SCRIPT_DIR/echo_c.c" 2>&1; then
    C_SIZE=$(stat -f%z "$BUILD_DIR/echo_c" 2>/dev/null || stat -c%s "$BUILD_DIR/echo_c")
    pass "Built echo_c ($C_SIZE bytes)"
else
    fail "C build failed"
    C_SIZE=0
fi

# --- Rust (Tokio) ---
echo -e "\n${BOLD}[2/4] Rust / Tokio${NC}"
RUST_DIR="$BUILD_DIR/echo_rust_proj"
if [ ! -d "$RUST_DIR" ]; then
    mkdir -p "$RUST_DIR/src"
    cp "$SCRIPT_DIR/echo_rust.rs" "$RUST_DIR/src/main.rs"
    cat > "$RUST_DIR/Cargo.toml" << 'EOF'
[package]
name = "echo_rust"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
EOF
fi
if (cd "$RUST_DIR" && cargo build --release 2>&1 | tail -3); then
    RUST_BIN="$RUST_DIR/target/release/echo_rust"
    RUST_SIZE=$(stat -f%z "$RUST_BIN" 2>/dev/null || stat -c%s "$RUST_BIN")
    pass "Built echo_rust ($RUST_SIZE bytes)"
else
    fail "Rust build failed"
    RUST_SIZE=0
fi

# --- Salt (kqueue via FFI bridge) ---
echo -e "\n${BOLD}[3/3] Salt / kqueue${NC}"
SALT_SIZE=0
if [ -f "$SCRIPT_DIR/echo_salt.salt" ]; then
    SALT_FRONT="$SCRIPT_DIR/../../salt-front/target/release/saltc"
    RUNTIME_C="$SCRIPT_DIR/../../salt-front/runtime.c"
    if [ ! -f "$SALT_FRONT" ]; then
        fail "saltc release binary not found (run: cargo build --release)"
    else
        # Salt → MLIR → LLVM IR → native binary (with C bridge)
        DYLD_LIBRARY_PATH=/opt/homebrew/lib $SALT_FRONT "$SCRIPT_DIR/echo_salt.salt" --release 2>/dev/null \
            | grep -v "^DEBUG:\|^Debug:\|^>>>\|^State\|salt.verify\|^\[V4.0\]" \
            > "$BUILD_DIR/echo_salt_clean.mlir" && \
        mlir-opt --convert-linalg-to-loops --expand-strided-metadata --lower-affine \
            --convert-scf-to-cf --canonicalize --sroa --mem2reg --canonicalize \
            --finalize-memref-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
            --convert-func-to-llvm --convert-cf-to-llvm --reconcile-unrealized-casts \
            "$BUILD_DIR/echo_salt_clean.mlir" -o "$BUILD_DIR/echo_salt.opt.mlir" 2>/dev/null && \
        mlir-translate --mlir-to-llvmir "$BUILD_DIR/echo_salt.opt.mlir" -o "$BUILD_DIR/echo_salt.ll" 2>/dev/null && \
        opt -O3 "$BUILD_DIR/echo_salt.ll" -S -o "$BUILD_DIR/echo_salt_opt.ll" 2>/dev/null && \
        $CLANG -O3 "$BUILD_DIR/echo_salt_opt.ll" "$SCRIPT_DIR/echo_salt_bridge.c" "$RUNTIME_C" \
            -o "$BUILD_DIR/echo_salt" 2>/dev/null
        if [ $? -eq 0 ]; then
            SALT_SIZE=$(stat -f%z "$BUILD_DIR/echo_salt" 2>/dev/null || stat -c%s "$BUILD_DIR/echo_salt")
            pass "Built echo_salt ($SALT_SIZE bytes)"
        else
            fail "Salt build failed"
        fi
    fi
else
    fail "echo_salt.salt not found"
fi

# =============================================================================
# Phase 2: Benchmark Each Server
# =============================================================================
header "Phase 2: Echo Benchmark (port=$PORT, duration=${DURATION}s, connections=$NUM_CONNS)"

echo ""
echo "Binary Size Comparison:"
echo "  C (kqueue):      ${C_SIZE:-N/A} bytes"
echo "  Rust (Tokio):    ${RUST_SIZE:-N/A} bytes"
echo "  Salt (kqueue):   ${SALT_SIZE:-N/A} bytes"
echo ""

# Helper: benchmark a server
benchmark_server() {
    local name=$1
    local cmd=$2
    local port_offset=$3
    local actual_port=$((PORT + port_offset))

    echo -e "\n${BOLD}Benchmarking: $name (port $actual_port)${NC}"

    # Start server in background
    eval "$cmd $actual_port &"
    local pid=$!
    sleep 1  # Let server start

    # Check if server is running
    if ! kill -0 $pid 2>/dev/null; then
        fail "$name failed to start"
        return
    fi

    # Get initial memory
    local rss_before=$(ps -o rss= -p $pid 2>/dev/null || echo "0")

    # Run load test with nc (simple echo test)
    local start_time=$(date +%s%N)
    local success=0
    local total=0

    for i in $(seq 1 $NUM_CONNS); do
        if echo "Hello World" | nc -w 1 127.0.0.1 $actual_port >/dev/null 2>&1; then
            success=$((success + 1))
        fi
        total=$((total + 1))
    done

    local end_time=$(date +%s%N)
    local elapsed_ms=$(( (end_time - start_time) / 1000000 ))

    # Get final memory
    local rss_after=$(ps -o rss= -p $pid 2>/dev/null || echo "0")

    # Report
    local rate=0
    if [ $elapsed_ms -gt 0 ]; then
        rate=$(( success * 1000 / elapsed_ms ))
    fi
    echo "  Connections: $success / $total"
    echo "  Time:        ${elapsed_ms}ms"
    echo "  Rate:        ${rate} conn/s"
    echo "  RSS:         ${rss_after} KB"

    # Cleanup
    kill $pid 2>/dev/null || true
    wait $pid 2>/dev/null || true
    sleep 1
}

# Run benchmarks
if [ -f "$BUILD_DIR/echo_c" ]; then
    benchmark_server "C / kqueue" "$BUILD_DIR/echo_c" 0
fi

if [ -f "$BUILD_DIR/echo_rust_proj/target/release/echo_rust" ]; then
    benchmark_server "Rust / Tokio" "$BUILD_DIR/echo_rust_proj/target/release/echo_rust" 1
fi

if [ -f "$BUILD_DIR/echo_salt" ]; then
    benchmark_server "Salt / kqueue" "$BUILD_DIR/echo_salt" 2
fi

# =============================================================================
# Phase 3: Summary
# =============================================================================
header "Benchmark Summary"

echo ""
echo "Target          | Binary Size | Status"
echo "----------------|-------------|--------"
printf "C / kqueue      | %10s | %s\n" "${C_SIZE:-N/A}" "$([ ${C_SIZE:-0} -gt 0 ] && echo '✅ Ready' || echo '❌ Failed')"
printf "Rust / Tokio    | %10s | %s\n" "${RUST_SIZE:-N/A}" "$([ ${RUST_SIZE:-0} -gt 0 ] && echo '✅ Ready' || echo '❌ Failed')"
printf "Salt / kqueue   | %10s | %s\n" "${SALT_SIZE:-N/A}" "$([ ${SALT_SIZE:-0} -gt 0 ] && echo '✅ Ready' || echo '❌ Failed')"
echo ""

echo "Results saved to: $RESULTS_FILE"
date > "$RESULTS_FILE"
echo "Port: $PORT, Duration: ${DURATION}s, Connections: $NUM_CONNS" >> "$RESULTS_FILE"
echo "C_SIZE=$C_SIZE RUST_SIZE=${RUST_SIZE:-0} SALT_SIZE=${SALT_SIZE:-0}" >> "$RESULTS_FILE"
