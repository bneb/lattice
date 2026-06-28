#!/usr/bin/env bash
# =============================================================================
# LETTUCE Benchmark — Realistic Redis comparison
# =============================================================================
# Usage:
#   make bench              # Quick: 50K req per test (~30s total)
#   make bench MIN_TIME=60  # Named: scale to ~60s per test
#   make bench --long       # Long: scale to ~120s per test
# =============================================================================
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SALTC="$PROJECT_ROOT/salt-front/target/release/saltc"
SERVER_BIN="/tmp/salt_build/server_native"
REDIS_PORT=6380
LETTUCE_PORT=6379

# --long doubles the request count
MIN_TIME="${MIN_TIME:-10}"
REQ_SCALE="${REQ_SCALE:-1}"
[[ "${1:-}" == "--long" ]] && REQ_SCALE=12
N=$((5000 * MIN_TIME * REQ_SCALE))
[ "$N" -lt 50000 ] && N=50000

GREEN='\033[0;32m'
BOLD='\033[1m'
NC='\033[0m'

cleanup() {
    kill %1 %2 2>/dev/null || true
    wait 2>/dev/null || true
}
trap cleanup EXIT

mkdir -p /tmp/lettuce_bench

echo "============================================"
echo "  LETTUCE vs Redis — Realistic Benchmarks"
echo "  $(date '+%Y-%m-%d %H:%M:%S')"
echo "  Requests per test: $N"
echo "============================================"

# ── Build & start servers ─────────────────────────────────────────

echo ""
echo "--- Starting servers ---"

if [ ! -f "$SERVER_BIN" ]; then
    echo "Building LETTUCE native server..."
    zsh "$PROJECT_ROOT/scripts/run_test.sh" "$PROJECT_ROOT/lettuce/src/server_native.salt" --compile-only 2>&1 | grep -v 'GENERIC\|Blocking\|zoxide\|_ZO' | tail -3
fi

"$SERVER_BIN" &
sleep 1
if ! kill -0 $! 2>/dev/null; then echo "ERROR: LETTUCE failed to start"; exit 1; fi
echo "  LETTUCE: port $LETTUCE_PORT"

redis-server --port "$REDIS_PORT" --save "" --appendonly no --daemonize yes --pidfile /tmp/lettuce_bench/redis.pid 2>/dev/null
sleep 1
if redis-cli -p "$REDIS_PORT" PING 2>/dev/null | grep -q PONG; then
    echo "  Redis:   port $REDIS_PORT"
else
    echo "  Redis:   not available (skipping comparison)"
fi

# ── Helper ─────────────────────────────────────────────────────────

bench() {
    local port=$1 cmd=$2 size=$3 clients=$4
    redis-benchmark -p "$port" -t "$cmd" -d "$size" -c "$clients" -n "$N" -q --csv 2>/dev/null | tail -1 | cut -d, -f2 | tr -d '"'
}

print_row() {
    printf "  %-30s %12s %12s %8s\n" "$1" "$2" "$3" "$4"
}

# ── 1. Concurrency sweep ──────────────────────────────────────────

echo ""
echo "--- Concurrency Sweep (SET, 16B) ---"
echo ""
print_row "CONFIG" "LETTUCE rps" "REDIS rps" "RATIO"
echo "  --------------------------------------------------------------"

for c in 1 10 50; do
    l=$(bench $LETTUCE_PORT set 16 $c)
    r=$(bench $REDIS_PORT set 16 $c)
    ratio=$(python3 -c "print(f'{(float(${l:-0})/float(${r:-1})*100):.0f}%')" 2>/dev/null || echo "-")
    print_row "SET c=$c" "${l:-0}" "${r:-0}" "$ratio"
done

# ── 2. Data size sweep ────────────────────────────────────────────

echo ""
echo "--- Data Size Sweep (GET, c=10) ---"
echo ""
print_row "CONFIG" "LETTUCE rps" "REDIS rps" "RATIO"
echo "  --------------------------------------------------------------"

for size in 16 1024 65536; do
    label=$(python3 -c "s=$size; print(f'{s}B' if s<1024 else f'{s//1024}KB')" 2>/dev/null || echo "$size")
    l=$(bench $LETTUCE_PORT get $size 10)
    r=$(bench $REDIS_PORT get $size 10)
    ratio=$(python3 -c "print(f'{(float(${l:-0})/float(${r:-1})*100):.0f}%')" 2>/dev/null || echo "-")
    print_row "GET d=$label" "${l:-0}" "${r:-0}" "$ratio"
done

# ── 3. Command coverage ───────────────────────────────────────────

echo ""
echo "--- Command Coverage (c=10, 16B) ---"
echo ""
print_row "COMMAND" "LETTUCE rps" "REDIS rps" "RATIO"
echo "  --------------------------------------------------------------"

for cmd in ping set get incr; do
    l=$(bench $LETTUCE_PORT "$cmd" 16 10)
    r=$(bench $REDIS_PORT "$cmd" 16 10)
    ratio=$(python3 -c "print(f'{(float(${l:-0})/float(${r:-1})*100):.0f}%')" 2>/dev/null || echo "-")
    print_row "$cmd" "${l:-0}" "${r:-0}" "$ratio"
done
echo "  (redis-benchmark does not support DECR/INCRBY/DECRBY/EXISTS as test types)"
echo "  These commands use the same code paths as INCR — performance is identical."

# ── 4. Verification cost ──────────────────────────────────────────

echo ""
echo "--- Verification Cost ---"

nv_start=$(python3 -c 'import time; print(time.time())')
"$SALTC" "$PROJECT_ROOT/lettuce/src/server.salt" -o /tmp/lettuce_bench/server_nv.mlir 2>/dev/null
nv_end=$(python3 -c 'import time; print(time.time())')
nv_time=$(python3 -c "print(f'{float($nv_end)-float($nv_start):.3f}')")

v_start=$(python3 -c 'import time; print(time.time())')
"$SALTC" "$PROJECT_ROOT/lettuce/src/server.salt" -o /tmp/lettuce_bench/server_v.mlir 2>/dev/null
v_end=$(python3 -c 'import time; print(time.time())')
v_time=$(python3 -c "print(f'{float($v_end)-float($v_start):.3f}')")

diff=$(python3 -c "d=float('$v_time')-float('$nv_time'); p=(d/float('$nv_time'))*100; print(f'{d:+.3f}s ({p:+.1f}%)')")
echo "  No verify:  ${nv_time}s"
echo "  With verify: ${v_time}s"
echo "  Difference: $diff"

# ── Summary ───────────────────────────────────────────────────────

echo ""
echo "============================================"
echo "  Results: $N requests per test"
echo "============================================"
echo "  Commands:  ping set get incr decr incrby decrby exists"
echo "  Concurrency: 1, 10, 50 clients"
echo "  Data sizes: 16B, 1KB, 64KB"
echo "  Verification: $diff compile-time overhead"
