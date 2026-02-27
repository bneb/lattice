#!/bin/bash
# =============================================================================
# Salt Compilation Time Benchmark
# =============================================================================
# Measures end-to-end compilation time for the Salt→MLIR→LLVM→binary pipeline.
# Tracks: compilation time, MLIR generation time, and LLVM lowering time.
#
# Usage:
#   ./compile_time_bench.sh              # Run all benchmarks
#   ./compile_time_bench.sh matmul fib   # Run specific benchmarks
# =============================================================================

set -e

SALT_COMPILER="../salt-front/target/release/salt-front"
MLIR_OPT="/opt/homebrew/opt/llvm@18/bin/mlir-opt"
MLIR_TRANSLATE="/opt/homebrew/opt/llvm@18/bin/mlir-translate"
OPT="/opt/homebrew/opt/llvm@18/bin/opt"
CLANG="/opt/homebrew/opt/llvm@18/bin/clang"

export Z3_SYS_Z3_HEADER=/opt/homebrew/include/z3.h
export LIBRARY_PATH=/opt/homebrew/lib
export DYLD_LIBRARY_PATH=/opt/homebrew/lib

# Output file for tracking historical compilation times
RESULTS_FILE="compile_times.csv"

# Initialize CSV header if file doesn't exist
if [ ! -f "$RESULTS_FILE" ]; then
    echo "timestamp,benchmark,salt_compile_ms,mlir_opt_ms,llvm_lower_ms,link_ms,total_ms,binary_size_kb" > "$RESULTS_FILE"
fi

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Collect benchmarks to run
if [ "$1" = "" ]; then
    BENCHMARKS=$(ls *.salt 2>/dev/null | sed 's/.salt$//' | sort)
else
    BENCHMARKS="$@"
fi

printf "\n\033[0;32m╔═══════════════════════════════════════════╗\033[0m\n"
printf "\033[0;32m║    \033[1mSalt Compilation Time Benchmark\033[0m\033[0;32m       ║\033[0m\n"
printf "\033[0;32m╚═══════════════════════════════════════════╝\033[0m\n\n"

printf "%-25s │ %8s │ %8s │ %8s │ %8s │ %8s\n" \
    "Benchmark" "Salt→MLIR" "MLIR-opt" "→LLVM IR" "Link" "Total"
printf "──────────────────────────┼──────────┼──────────┼──────────┼──────────┼──────────\n"

TOTAL_COMPILE=0
TOTAL_COUNT=0

for bench in $BENCHMARKS; do
    SALT_FILE="${bench}.salt"
    if [ ! -f "$SALT_FILE" ]; then
        continue
    fi

    # Check if it's a Salt-only benchmark (has a main function or is buildable)
    MLIR_FILE="/tmp/ct_${bench}.mlir"
    OPT_MLIR="/tmp/ct_${bench}.opt.mlir"
    LL_FILE="/tmp/ct_${bench}.ll"
    OBJ_FILE="/tmp/ct_${bench}.o"
    BIN_FILE="/tmp/ct_${bench}_bin"

    # Phase 1: Salt → MLIR (includes Z3 verification)
    START=$(python3 -c "import time; print(int(time.time() * 1000))")
    if ! $SALT_COMPILER "$SALT_FILE" -o "$MLIR_FILE" 2>/dev/null; then
        continue
    fi
    END=$(python3 -c "import time; print(int(time.time() * 1000))")
    SALT_MS=$((END - START))

    # Phase 2: MLIR optimization
    START=$(python3 -c "import time; print(int(time.time() * 1000))")
    if $MLIR_OPT "$MLIR_FILE" -o "$OPT_MLIR" 2>/dev/null; then
        END=$(python3 -c "import time; print(int(time.time() * 1000))")
        MLIR_MS=$((END - START))
    else
        MLIR_MS=0
        OPT_MLIR="$MLIR_FILE"
    fi

    # Phase 3: MLIR → LLVM IR
    START=$(python3 -c "import time; print(int(time.time() * 1000))")
    if $MLIR_TRANSLATE --mlir-to-llvmir "$OPT_MLIR" -o "$LL_FILE" 2>/dev/null; then
        END=$(python3 -c "import time; print(int(time.time() * 1000))")
        LLVM_MS=$((END - START))
    else
        LLVM_MS=0
    fi

    # Phase 4: LLVM → Binary (compile + link)
    START=$(python3 -c "import time; print(int(time.time() * 1000))")
    BRIDGE_FILE="${bench}_bridge.c"
    BRIDGE2="bridge.c"
    BRIDGE3="common_bridge.c"
    if [ -f "$BRIDGE_FILE" ]; then
        BRIDGES="$BRIDGE_FILE"
    elif [ -f "$BRIDGE2" ]; then
        BRIDGES="$BRIDGE2"
    elif [ -f "$BRIDGE3" ]; then
        BRIDGES="$BRIDGE3"
    else
        BRIDGES=""
    fi

    if [ -f "$LL_FILE" ] && $CLANG -O3 "$LL_FILE" $BRIDGES -o "$BIN_FILE" -lm 2>/dev/null; then
        END=$(python3 -c "import time; print(int(time.time() * 1000))")
        LINK_MS=$((END - START))
    else
        LINK_MS=0
    fi

    TOTAL_MS=$((SALT_MS + MLIR_MS + LLVM_MS + LINK_MS))
    TOTAL_COMPILE=$((TOTAL_COMPILE + TOTAL_MS))
    TOTAL_COUNT=$((TOTAL_COUNT + 1))

    # Get binary size
    if [ -f "$BIN_FILE" ]; then
        BIN_SIZE_KB=$(( $(stat -f%z "$BIN_FILE" 2>/dev/null || echo 0) / 1024 ))
    else
        BIN_SIZE_KB=0
    fi

    printf "%-25s │ %6dms │ %6dms │ %6dms │ %6dms │ %6dms\n" \
        "$bench" "$SALT_MS" "$MLIR_MS" "$LLVM_MS" "$LINK_MS" "$TOTAL_MS"

    # Append to CSV for historical tracking
    echo "$TIMESTAMP,$bench,$SALT_MS,$MLIR_MS,$LLVM_MS,$LINK_MS,$TOTAL_MS,$BIN_SIZE_KB" >> "$RESULTS_FILE"

    # Cleanup
    rm -f "$MLIR_FILE" "$OPT_MLIR" "$LL_FILE" "$OBJ_FILE" "$BIN_FILE"
done

printf "──────────────────────────┼──────────┼──────────┼──────────┼──────────┼──────────\n"

if [ "$TOTAL_COUNT" -gt 0 ]; then
    AVG_MS=$((TOTAL_COMPILE / TOTAL_COUNT))
    printf "%-25s │          │          │          │          │ %6dms\n" "TOTAL ($TOTAL_COUNT files)" "$TOTAL_COMPILE"
    printf "%-25s │          │          │          │          │ %6dms\n" "AVERAGE" "$AVG_MS"
fi

printf "\n\033[2mResults appended to %s\033[0m\n" "$RESULTS_FILE"
printf "\033[2mTimestamp: %s\033[0m\n\n" "$TIMESTAMP"
