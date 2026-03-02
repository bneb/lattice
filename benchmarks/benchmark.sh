#!/bin/zsh
#
# Salt Benchmark Suite
# Usage: ./benchmark.sh [options] [benchmark_names...]
#
# Options:
#   -a, --all       Run all benchmarks
#   -t, --tag TAG   Run benchmarks matching tag (e.g., "sieve", "matrix")
#   -l, --list      List available benchmarks
#   -c, --clean     Clean build artifacts before running
#   -h, --help      Show this help
#
# Examples:
#   ./benchmark.sh sieve              # Run sieve benchmark
#   ./benchmark.sh sieve fib matmul   # Run specific benchmarks
#   ./benchmark.sh -t sieve           # Run all benchmarks with "sieve" in name
#   ./benchmark.sh -a                 # Run all benchmarks

setopt NO_ERR_EXIT  # Don't exit on non-zero returns (compile failures are expected)
cd "$(dirname "$0")"

# Paths
LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"
SALT_FRONT="../salt-front/target/release/salt-front"
RUNTIME_C="../salt-front/runtime.c"
BIN_DIR="bin"

# Terminal colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

# ============================================================================
# Data Model: Associative array keyed by "benchmark:lang"
# Value format: "time|peak_mem_mb|binary_kb|loc|status"
# status: "ok", "no_source", "build_failed"
# ============================================================================
typeset -A RESULTS
BENCH_ORDER=()
LANGS=("C" "Rust" "Salt")

set_result() {
    local _k="$1:$2"
    RESULTS[$_k]="$3|$4|$5|$6|$7"
}

get_field() {
    local _k="$1:$2"
    local _v="${RESULTS[$_k]}"
    echo "$_v" | cut -d'|' -f$(($3 + 1))
}

usage() {
    head -18 "$0" | tail -17 | sed 's/^# //' | sed 's/^#//'
    exit 0
}

# Feature tests (excluded from perf benchmarks)
EXCLUDED_TESTS=()

is_excluded() {
    local name="$1"
    for excl in "${EXCLUDED_TESTS[@]}"; do
        [[ "$name" == "$excl" ]] && return 0
    done
    return 1
}

list_benchmarks() {
    echo "${BOLD}Available performance benchmarks:${NC}"
    echo "(For Salt-only feature tests, use ./feature_tests.sh -l)"
    echo ""
    for f in *.salt; do
        name="${f%.salt}"
        is_excluded "$name" && continue
        langs=""
        [[ -f "${name}.c" ]] && langs+="C "
        [[ -f "${name}.rs" ]] && langs+="Rust "
        [[ -f "${name}.salt" ]] && langs+="Salt"
        printf "  %-25s [%s]\n" "$name" "$langs"
    done
    exit 0
}

clean_build() {
    echo "${YELLOW}Cleaning build artifacts...${NC}"
    rm -rf "$BIN_DIR"
    mkdir -p "$BIN_DIR"
}

# ============================================================================
# Compilation
# ============================================================================

compile_c() {
    local name=$1
    [[ -f "${name}.c" ]] || return 1
    /opt/homebrew/opt/llvm@${LLVM_VERSION}/bin/clang -O3 -march=native -ffast-math "${name}.c" -o "${BIN_DIR}/${name}_c" 2>/dev/null
}

compile_rust() {
    local name=$1
    [[ -f "${name}.rs" ]] || return 1
    rustc -C opt-level=3 "${name}.rs" -o "${BIN_DIR}/${name}_rs" 2>/dev/null
}

compile_salt() {
    local name=$1
    local abs_salt_path="$(pwd)/${name}.salt"
    local abs_bin_dir="$(pwd)/${BIN_DIR}"
    [[ -f "${name}.salt" ]] || return 1
    
    pushd ../salt-front > /dev/null || return 1
    
    ./target/release/salt-front "$abs_salt_path" --release 2>/dev/null \
        | grep -v "^DEBUG:" | grep -v "^Debug:" | grep -v "^>>>" | grep -v "^State" | grep -v "salt.verify" | grep -v "^\[V4.0\]" \
        > "${abs_bin_dir}/${name}_clean.mlir" || { popd > /dev/null; return 1; }
    
    popd > /dev/null
    
    [[ -s "${BIN_DIR}/${name}_clean.mlir" ]] || return 1
    
    mlir-opt --convert-linalg-to-loops \
             --expand-strided-metadata \
             --affine-loop-tile="tile-size=4" \
             --lower-affine \
             --convert-scf-to-cf \
             --canonicalize \
             --sroa \
             --mem2reg \
             --canonicalize \
             --finalize-memref-to-llvm \
             --convert-arith-to-llvm --convert-math-to-llvm --convert-func-to-llvm --convert-cf-to-llvm \
             --reconcile-unrealized-casts "${BIN_DIR}/${name}_clean.mlir" \
             -o "${BIN_DIR}/${name}.opt.mlir" 2>/dev/null || return 1
    mlir-translate --mlir-to-llvmir "${BIN_DIR}/${name}.opt.mlir" -o "${BIN_DIR}/${name}.ll" 2>/dev/null || return 1
    
    opt -O3 \
        "${BIN_DIR}/${name}.ll" -S -o "${BIN_DIR}/${name}_opt.ll" 2>/dev/null || \
        cp "${BIN_DIR}/${name}.ll" "${BIN_DIR}/${name}_opt.ll"
    
    local bridge_file="${name}_bridge.c"
    if [[ -f "$bridge_file" ]]; then
        clang -O3 "${BIN_DIR}/${name}_opt.ll" "$bridge_file" "$RUNTIME_C" -o "${BIN_DIR}/${name}_salt" 2>/dev/null || return 1
    else
        clang -O3 "${BIN_DIR}/${name}_opt.ll" "$RUNTIME_C" -o "${BIN_DIR}/${name}_salt" 2>/dev/null || return 1
    fi
}

# ============================================================================
# Measurement
# ============================================================================

count_loc() {
    local file=$1
    if [[ ! -f "$file" ]]; then
        echo "-"
        return
    fi
    local ext="${file##*.}"
    # Strip blank lines, then strip language-specific noise
    case "$ext" in
        c|h)
            # Remove: blank lines, // comments, /* */ comments, #include, #define, #pragma, lone braces
            sed 's|//.*||' "$file" \
                | sed '/^[[:space:]]*$/d' \
                | sed '/^[[:space:]]*#include/d' \
                | sed '/^[[:space:]]*#define/d' \
                | sed '/^[[:space:]]*#pragma/d' \
                | sed '/^[[:space:]]*[{}][[:space:]]*$/d' \
                | sed '/^[[:space:]]*\/\*/,/\*\//d' \
                | wc -l | tr -d ' '
            ;;
        rs)
            # Remove: blank lines, // comments, use statements, lone braces
            sed 's|//.*||' "$file" \
                | sed '/^[[:space:]]*$/d' \
                | sed '/^[[:space:]]*use /d' \
                | sed '/^[[:space:]]*[{}][[:space:]]*$/d' \
                | wc -l | tr -d ' '
            ;;
        salt)
            # Remove: blank lines, # comments, use/import/package/extern fn, lone braces
            sed '/^[[:space:]]*#/d' "$file" \
                | sed '/^[[:space:]]*$/d' \
                | sed '/^[[:space:]]*use /d' \
                | sed '/^[[:space:]]*import /d' \
                | sed '/^[[:space:]]*package /d' \
                | sed '/^[[:space:]]*extern fn/d' \
                | sed '/^[[:space:]]*[{}][[:space:]]*$/d' \
                | wc -l | tr -d ' '
            ;;
        *)
            grep -c '.' "$file" 2>/dev/null || echo "0"
            ;;
    esac
}

measure_benchmark() {
    local name=$1 lang=$2 bin=$3 src=$4
    local loc=$(count_loc "$src")
    
    if [[ ! -x "$bin" ]]; then
        set_result "$name" "$lang" "-" "-" "-" "$loc" "skip"
        return
    fi
    
    # Time (average of 3 runs)
    local total=0
    for _ in 1 2 3; do
        sleep 0.1
        local t=$( /usr/bin/time -p "$bin" 2>&1 >/dev/null | grep real | awk '{print $2}' )
        total=$(echo "$total + ${t:-0}" | bc -l 2>/dev/null || echo "$total")
    done
    local avg=$(printf "%.3f" $(echo "$total / 3" | bc -l 2>/dev/null || echo "0"))
    
    # Peak memory via /usr/bin/time -l (stderr on macOS)
    local time_out=$(mktemp)
    /usr/bin/time -l "$bin" >/dev/null 2>"$time_out"
    local mem=$(grep "maximum resident" "$time_out" | awk '{print $1}')
    rm -f "$time_out"
    local mem_mb=$(printf "%.1f" $(echo "${mem:-0} / 1048576" | bc -l 2>/dev/null || echo "0"))
    
    # Binary size
    local size=$(stat -f%z "$bin" 2>/dev/null || echo "0")
    local size_kb=$(printf "%.1f" $(echo "$size / 1024" | bc -l 2>/dev/null || echo "0"))
    
    set_result "$name" "$lang" "$avg" "$mem_mb" "$size_kb" "$loc" "ok"
}

# ============================================================================
# Collect: compile + measure all languages for a benchmark
# ============================================================================

collect_benchmark() {
    local name=$1
    echo "  ${DIM}Collecting${NC} ${BOLD}$name${NC}..."
    
    # C
    if [[ -f "${name}.c" ]]; then
        if compile_c "$name" 2>/dev/null; then
            measure_benchmark "$name" "C" "${BIN_DIR}/${name}_c" "${name}.c"
        else
            set_result "$name" "C" "-" "-" "-" "$(count_loc "${name}.c")" "build_failed"
        fi
    else
        set_result "$name" "C" "-" "-" "-" "-" "no_source"
    fi
    
    # Rust
    if [[ -f "${name}.rs" ]]; then
        if compile_rust "$name" 2>/dev/null; then
            measure_benchmark "$name" "Rust" "${BIN_DIR}/${name}_rs" "${name}.rs"
        else
            set_result "$name" "Rust" "-" "-" "-" "$(count_loc "${name}.rs")" "build_failed"
        fi
    else
        set_result "$name" "Rust" "-" "-" "-" "-" "no_source"
    fi
    
    # Salt
    if [[ -f "${name}.salt" ]]; then
        if compile_salt "$name" 2>/dev/null; then
            measure_benchmark "$name" "Salt" "${BIN_DIR}/${name}_salt" "${name}.salt"
        else
            set_result "$name" "Salt" "-" "-" "-" "$(count_loc "${name}.salt")" "build_failed"
        fi
    else
        set_result "$name" "Salt" "-" "-" "-" "-" "no_source"
    fi
}

# ============================================================================
# Render: print results from associative array
# ============================================================================

render_table() {
    local _name _lang _st _loc _t _mem _bsz
    for _name in "${BENCH_ORDER[@]}"; do
        echo ""
        echo "${BLUE}━━━ ${BOLD}$_name${NC}${BLUE} ━━━${NC}"
        printf "%-6s │ %8s │ %8s │ %8s │ %4s\n" "Lang" "Time" "Peak Mem" "Binary" "LOC"
        printf "%s\n" "───────┼──────────┼──────────┼──────────┼─────"
        
        for _lang in "${LANGS[@]}"; do
            _st=$(get_field "$_name" "$_lang" 4)
            _loc=$(get_field "$_name" "$_lang" 3)
            
            case "$_st" in
                ok)
                    _t=$(get_field "$_name" "$_lang" 0)
                    _mem=$(get_field "$_name" "$_lang" 1)
                    _bsz=$(get_field "$_name" "$_lang" 2)
                    printf "%-6s │ %7ss │ %6sMB │ %6sKB │ %4s\n" "$_lang" "$_t" "$_mem" "$_bsz" "$_loc"
                    ;;
                build_failed)
                    printf "%-6s │ ${RED}%27s${NC} │ %4s\n" "$_lang" "build failed" "$_loc"
                    ;;
                *)
                    printf "%-6s │ %27s │ %4s\n" "$_lang" "-" "$_loc"
                    ;;
            esac
        done
    done
}

render_summary() {
    echo ""
    echo "${BOLD}═══ Summary ═══${NC}"
    
    local _total=0 _salt_faster=0 _salt_fails=0
    local _name _ss _st _ct
    for _name in "${BENCH_ORDER[@]}"; do
        _ss=$(get_field "$_name" "Salt" 4)
        (( _total++ )) || true
        if [[ "$_ss" == "ok" ]]; then
            _st=$(get_field "$_name" "Salt" 0)
            _ct=$(get_field "$_name" "C" 0)
            if [[ "$_ct" != "-" ]] && (( $(echo "$_st <= $_ct" | bc -l 2>/dev/null || echo 0) )); then
                (( _salt_faster++ )) || true
            fi
        elif [[ "$_ss" == "build_failed" ]]; then
            (( _salt_fails++ )) || true
        fi
    done
    
    local _salt_ok=$(( _total - _salt_fails ))
    echo "  Benchmarks: ${BOLD}${_total}${NC} total, ${GREEN}${_salt_ok} Salt build OK${NC}, ${RED}${_salt_fails} Salt build failed${NC}"
    echo "  Salt ≤ C:   ${BOLD}${_salt_faster}/${_total}${NC}"
}

# ============================================================================
# Main
# ============================================================================

BENCHMARKS=()
RUN_ALL=false
TAG=""
DO_CLEAN=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help) usage ;;
        -l|--list) list_benchmarks ;;
        -a|--all) RUN_ALL=true; shift ;;
        -t|--tag) TAG="$2"; shift 2 ;;
        -c|--clean) DO_CLEAN=true; shift ;;
        *) BENCHMARKS+=("$1"); shift ;;
    esac
done

mkdir -p "$BIN_DIR"
[[ "$DO_CLEAN" == true ]] && clean_build

echo "${GREEN}╔═══════════════════════════════════════╗${NC}"
echo "${GREEN}║    ${BOLD}Salt Benchmark Suite${NC}${GREEN}              ║${NC}"
echo "${GREEN}╚═══════════════════════════════════════╝${NC}"

# Determine benchmarks
if [[ "$RUN_ALL" == true ]]; then
    for f in *.salt; do
        name="${f%.salt}"
        is_excluded "$name" || BENCHMARKS+=("$name")
    done
elif [[ -n "$TAG" ]]; then
    for f in *${TAG}*.salt; do 
        [[ -f "$f" ]] && BENCHMARKS+=("${f%.salt}")
    done
fi

if [[ ${#BENCHMARKS[@]} -eq 0 ]]; then
    echo "${RED}No benchmarks specified. Use -h for help.${NC}"
    exit 1
fi

# Phase 1: Collect
echo ""
echo "${CYAN}Phase 1: Collecting measurements...${NC}"
for bench in "${BENCHMARKS[@]}"; do
    BENCH_ORDER+=("$bench")
    collect_benchmark "$bench"
done

# Phase 2: Render
echo ""
echo "${CYAN}Phase 2: Results${NC}"
render_table

# Phase 3: Summary
render_summary

echo ""
echo "${GREEN}Done!${NC}"
