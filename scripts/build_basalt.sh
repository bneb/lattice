#!/usr/bin/env zsh
set -euo pipefail

# =============================================================================
# Basalt Build & Benchmark
# =============================================================================
# Usage:
#   ./scripts/bench_basalt.sh                  # Full 40-run benchmark (Salt vs C)
#   ./scripts/bench_basalt.sh --build-only     # Build and run once (replaces build_basalt.sh)
#   ./scripts/bench_basalt.sh --runs 10        # Custom run count
# =============================================================================

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

# Defaults — discover project root relative to this script's location
SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
PROJECT=${1:-$PROJECT_ROOT}
BUILD_ONLY=false
RUNS=40

# Parse flags (skip positional $1 which is PROJECT)
shift 2>/dev/null || true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --build-only) BUILD_ONLY=true; shift ;;
        --runs) RUNS="$2"; shift 2 ;;
        *) shift ;;
    esac
done

MODEL=$PROJECT/.bench_basalt/models/stories15M.bin
TOK=$PROJECT/.bench_basalt/models/tokenizer.bin
SF=$PROJECT/salt-front
OUT=/tmp/salt_build

# =============================================================================
# Build Basalt
# =============================================================================
build_basalt() {
    mkdir -p "$OUT"
    local COMBINED=$OUT/basalt_combined.salt
    echo "// Auto-generated build file for Basalt" > "$COMBINED"
    echo "package main" >> "$COMBINED"
    echo "use std.core.ptr.Ptr" >> "$COMBINED"
    echo "" >> "$COMBINED"
    for f in basalt/src/kernels.salt basalt/src/sampler.salt basalt/src/quant.salt basalt/src/transformer.salt basalt/src/model_loader.salt basalt/src/tokenizer.salt basalt/src/main.salt; do
        echo "// ---- Module: $(basename $f) ----" >> "$COMBINED"
        grep -v "^package " "$PROJECT/$f" | grep -v "^use basalt\." >> "$COMBINED"
        echo "" >> "$COMBINED"
    done
    echo "Built source: $COMBINED"

    echo "Running salt-front..."
    $SF/target/release/salt-front "$COMBINED" --no-verify | grep -v "^DEBUG" | grep -v "^Debug" > $OUT/basalt.mlir

    echo "Running mlir-opt..."
    mlir-opt $OUT/basalt.mlir \
        --allow-unregistered-dialect \
        --canonicalize --cse --loop-invariant-code-motion --sccp --canonicalize --cse \
        --lower-affine \
        --convert-scf-to-cf --convert-vector-to-llvm --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm --convert-func-to-llvm \
        --reconcile-unrealized-casts -o $OUT/basalt.opt.mlir

    sed -i '' '/"salt.verify"/d' $OUT/basalt.opt.mlir

    echo "Running mlir-translate..."
    mlir-translate --mlir-to-llvmir $OUT/basalt.opt.mlir -o $OUT/basalt.ll

    echo "Running clang..."
    clang -O3 -ffast-math -march=native $OUT/basalt.ll $SF/runtime.c -o $OUT/basalt -lm -Wno-override-module

    echo "Build complete: $OUT/basalt"
}

# =============================================================================
# Main
# =============================================================================

if [[ "$BUILD_ONLY" == true ]]; then
    echo "╔══════════════════════════════════════════════╗"
    echo "║   Basalt Build & Run                         ║"
    echo "║   $(date '+%Y-%m-%d %H:%M:%S')                        ║"
    echo "╚══════════════════════════════════════════════╝"
    echo ""
    build_basalt
    echo ""
    echo "----------------------------------------------------------------"
    $OUT/basalt
    exit 0
fi

echo "╔══════════════════════════════════════════════╗"
echo "║   Basalt vs C — ${RUNS}-Run Benchmark              ║"
echo "║   $(date '+%Y-%m-%d %H:%M:%S')                        ║"
echo "╚══════════════════════════════════════════════╝"
echo ""

# --- Build C baseline ---
echo "Building C (llama2.c)..."
C_SRC=$PROJECT/.bench_basalt/llama2.c/run.c
C_BIN=/tmp/salt_bench/llama2c
mkdir -p /tmp/salt_bench
clang -O3 -ffast-math -march=native "$C_SRC" -o "$C_BIN" -lm

# --- Build Salt (Basalt) ---
echo "Building Salt (Basalt)..."
build_basalt
SALT_BIN=$OUT/basalt
echo ""

# --- Run benchmarks ---
echo "Running $RUNS iterations each (best of 3 shown per run)..."
echo ""

C_BEST_OVERALL=0
echo -n "C:    "
for i in $(seq 1 $RUNS); do
    output=$($C_BIN "$MODEL" -z "$TOK" -n 256 2>&1)
    toks=$(echo "$output" | grep -oE 'achieved tok/s: [0-9.]+' | awk '{print $NF}')
    toks_int=${toks%.*}
    if (( toks_int > C_BEST_OVERALL )); then C_BEST_OVERALL=$toks_int; fi
    printf "%4d " $toks_int
done
echo ""
echo "      Best: $C_BEST_OVERALL tok/s"
echo ""

SALT_BEST_OVERALL=0
echo -n "Salt: "
for i in $(seq 1 $RUNS); do
    output=$($SALT_BIN "$MODEL" "$TOK" 2>&1)
    toks=$(echo "$output" | grep "tok/s:" | awk '{print $2}')
    toks_int=${toks%.*}
    if (( toks_int > SALT_BEST_OVERALL )); then SALT_BEST_OVERALL=$toks_int; fi
    printf "%4d " $toks_int
done
echo ""
echo "      Best: $SALT_BEST_OVERALL tok/s"
echo ""

# Ratio
if (( C_BEST_OVERALL > 0 )); then
    RATIO_100=$(( SALT_BEST_OVERALL * 100 / C_BEST_OVERALL ))
    RATIO_INT=$(( RATIO_100 / 100 ))
    RATIO_FRAC=$(( RATIO_100 % 100 ))
    printf "Ratio: Salt/C = %d.%02dx\n" $RATIO_INT $RATIO_FRAC
fi

echo ""

# Save results
RESULTS=$PROJECT/.bench_basalt/results.txt
cat > "$RESULTS" << EOF
Basalt Benchmark Results
$(date -u '+%Y-%m-%d %H:%M:%S') UTC

Hardware: Apple M4
OS: macOS 15.6

Model: stories15M.bin
Tokens: 256
Runs: $RUNS

llama2.c (-O3 -ffast-math -march=native): ${C_BEST_OVERALL} tok/s
Basalt:   ${SALT_BEST_OVERALL} tok/s
Ratio:    ${RATIO_INT}.${RATIO_FRAC}x
EOF

echo "Results saved to .bench_basalt/results.txt"
