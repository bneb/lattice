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

# ── Helper: convert time(1) output to seconds ─────────────────────
# Input format: "0m2.345s"  Output: "2.345"
time_to_seconds() {
    local t="$1"
    if [[ "$t" == "N/A" || -z "$t" ]]; then
        echo "N/A"
        return
    fi
    python3 -c "
import re
m = re.match(r'(\d+)m([\d.]+)s', '$t')
if m:
    print(int(m.group(1)) * 60 + float(m.group(2)))
else:
    print('N/A')
" 2>/dev/null || echo "N/A"
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

    # Look up baseline from benchmark_results.json (format: {"name": {"salt": {"time_s": float}}})
    local baseline_salt="N/A"
    if [ -f "$BASELINE_FILE" ]; then
        baseline_salt=$(python3 -c "
import json
try:
    data = json.load(open('$BASELINE_FILE'))
    entry = data.get('$name')
    if entry and 'salt' in entry and 'time_s' in entry['salt']:
        print(entry['salt']['time_s'])
    else:
        print('N/A')
except Exception:
    print('N/A')
" 2>/dev/null || echo "N/A")
    fi

    # Compare: flag regression if Salt time is >5% worse than baseline
    local salt_sec
    salt_sec=$(time_to_seconds "$salt_time")
    if [ "$salt_sec" != "N/A" ] && [ "$baseline_salt" != "N/A" ]; then
        if python3 -c "
import sys
cur = float('$salt_sec')
base = float('$baseline_salt')
sys.exit(0 if base > 0 and cur > base * 1.05 else 1)
" 2>/dev/null; then
            REGRESSIONS=$((REGRESSIONS + 1))
            local pct_diff
            pct_diff=$(python3 -c "print(f'{((float('$salt_sec')-float('$baseline_salt'))/float('$baseline_salt')*100):.1f}')" 2>/dev/null)
            echo "  >> REGRESSION: $name is ${pct_diff}% slower than baseline (${salt_sec}s vs ${baseline_salt}s)" | tee -a "$REGRESSION_FILE"
        fi
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

# Run any available .salt benchmarks directly via saltc
for salt_file in "$PROJECT_ROOT"/benchmarks/*.salt; do
    if [ -f "$salt_file" ]; then
        name=$(basename "$salt_file" .salt)
        bin_path="/tmp/bench_${name}"
        if "$PROJECT_ROOT/salt-front/target/release/saltc" "$salt_file" --danger-no-verify -o "$bin_path" > /dev/null 2>&1 2>/dev/null; then
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
    exit 1
else
    echo "No regressions detected across $TOTAL benchmarks."
fi

echo ""
echo "Results: $RESULTS_FILE"
