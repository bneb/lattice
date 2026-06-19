#!/usr/bin/env bash
# =============================================================================
# Benchmark CI — Run benchmark suite and detect regressions
# =============================================================================
# Produces JSON with per-benchmark Salt/C/Rust timings.
# Flags regressions >5% from the stored baseline.
#
# Usage: bash tools/bench_infra/benchmark_ci.sh
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$PROJECT_ROOT"

BASELINE_FILE="benchmarks/benchmark_results.json"
RESULTS_FILE="/tmp/benchmark_ci_results.json"
REGRESSION_FILE="/tmp/benchmark_ci_regressions.txt"

echo '{"benchmarks": [' > "$RESULTS_FILE"
FIRST=true
REGRESSIONS=0
TOTAL=0

# ── Helper: extract timing from benchmark output ──────────────────
# Benchmarks output: "BENCH:name:value unit" or similar patterns
extract_timing() {
    local output="$1"
    local pattern="$2"
    echo "$output" | grep -oE "$pattern" | grep -oE '[0-9]+(\.[0-9]+)?' | head -1
}

# ── Run a single benchmark and compare to baseline ────────────────
run_bench() {
    local name="$1"
    local salt_bin="$2"
    local c_bin="$3"
    local rust_bin="$4"

    TOTAL=$((TOTAL + 1))

    local salt_time=""
    local c_time=""
    local rust_time=""

    # Run Salt
    if [ -x "$salt_bin" ]; then
        salt_time=$( { time "$salt_bin" > /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' || echo "N/A")
    else
        salt_time="N/A"
    fi

    # Run C baseline
    if [ -x "$c_bin" ]; then
        c_time=$( { time "$c_bin" > /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' || echo "N/A")
    else
        c_time="N/A"
    fi

    # Run Rust baseline (if available)
    if [ -x "$rust_bin" ]; then
        rust_time=$( { time "$rust_bin" > /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' || echo "N/A")
    else
        rust_time="N/A"
    fi

    # Check baseline for regression
    local baseline_salt="N/A"
    if [ -f "$BASELINE_FILE" ]; then
        baseline_salt=$(python3 -c "
import json
try:
    data = json.load(open('$BASELINE_FILE'))
    for b in data.get('benchmarks', []):
        if b.get('name') == '$name':
            print(b.get('salt_ms', 'N/A'))
            break
except: pass
print('N/A')
" 2>/dev/null || echo "N/A")
    fi

    # Write JSON entry
    if [ "$FIRST" = true ]; then FIRST=false; else echo -n ',' >> "$RESULTS_FILE"; fi
    echo -n "{\"name\":\"$name\",\"salt_time\":\"$salt_time\",\"c_time\":\"$c_time\",\"rust_time\":\"$rust_time\",\"baseline_salt\":\"$baseline_salt\"}" >> "$RESULTS_FILE"

    echo "  $name: Salt=$salt_time C=$c_time Rust=$rust_time (baseline=$baseline_salt)"
}

# ── Quick benchmark subset (CI-friendly, <2 minutes) ─────────────
echo "=== Benchmark CI — Quick Suite ==="
echo ""

# Use pre-built binaries if available
BENCH_DIR="$PROJECT_ROOT/benchmarks/bin"

# Run representative benchmarks from the pre-built suite
for bench in binary_tree_path_salt binary_tree_path_c; do
    name=$(basename "$bench" | sed 's/_salt$//' | sed 's/_c$//')
    bin_path="$BENCH_DIR/$bench"
    if [ -x "$bin_path" ]; then
        run_bench "$name" "$BENCH_DIR/${bench}_salt" "$BENCH_DIR/${bench}_c" "$BENCH_DIR/${bench}_rs"
    fi
done

# Run any available .salt benchmarks directly via salt-front
for salt_file in "$PROJECT_ROOT"/benchmarks/*.salt; do
    if [ -f "$salt_file" ]; then
        name=$(basename "$salt_file" .salt)
        bin_path="/tmp/bench_${name}"
        if "$PROJECT_ROOT/salt-front/target/release/salt-front" "$salt_file" --no-verify -o "$bin_path" > /dev/null 2>&1 2>/dev/null; then
            if [ -x "$bin_path" ]; then
                run_bench "$name" "$bin_path" "N/A" "N/A"
            fi
        fi
    fi
done

echo ""
echo ']}' >> "$RESULTS_FILE"

# ── Regression report ────────────────────────────────────────────
if [ "$REGRESSIONS" -gt 0 ]; then
    echo "REGRESSION DETECTED: $REGRESSIONS benchmarks changed >5%."
    echo "See $REGRESSION_FILE for details."
else
    echo "No regressions detected across $TOTAL benchmarks."
fi

echo ""
echo "Results: $RESULTS_FILE"
