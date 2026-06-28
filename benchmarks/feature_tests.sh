#!/bin/bash
#
# Salt Feature Tests
# Compiler validation tests for Salt-specific features (not cross-language benchmarks)
#
# Usage: ./feature_tests.sh [options] [test_names...]
#
# Options:
#   -a, --all       Run all feature tests
#   -l, --list      List available tests
#   -c, --clean     Clean build artifacts before running
#   -h, --help      Show this help

set -e
cd "$(dirname "$0")"

# Paths
LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"
SALT_FRONT="../salt-front/target/release/saltc"
RUNTIME_C="../salt-front/runtime.c"
BIN_DIR="bin"

# Terminal colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

# Salt-only feature tests (no C/Rust equivalents, test compiler features)
FEATURE_TESTS=(
    # Cooperative multitasking / yielding
    "yield_pulse"
    "yield_validation"
    "io_pulse"
    "gauntlet_p99"
    
    # GPU/Shader compilation
    "tiger"
    
    # Compiler stress tests
    "syntactic_chaos"
    "deep_recursion"
    "terminator_torture"
    "pointer_depth"
    "edge_cases"
    "nested_torture"
    "mesh_swarm"
    "dll_salt"
    "hardware_io"
    "intrinsics_sink"
    
    # Coverage tests
    "coverage_gap"
    "coverage_push"
    "bench_popcount"
    "test_popcount"
    "promotion_matrix"
    
    # Scaling tests
    "s100"
    "s500"
    "s1000"
    "s3000"
    "scaling_100"
    "scaling_1k"
)

usage() {
    head -13 "$0" | tail -12 | sed 's/^# //' | sed 's/^#//'
    exit 0
}

list_tests() {
    echo -e "${BOLD}Available Salt feature tests:${NC}"
    for name in "${FEATURE_TESTS[@]}"; do
        if [[ -f "${name}.salt" ]]; then
            printf "  %-25s\n" "$name"
        fi
    done
    exit 0
}

clean_build() {
    echo -e "${YELLOW}Cleaning build artifacts...${NC}"
    rm -rf "$BIN_DIR"
    mkdir -p "$BIN_DIR"
}

compile_salt() {
    local name="$1"
    local bin="$BIN_DIR/${name}_salt"
    
    if [[ ! -f "${name}.salt" ]]; then
        echo "no source"
        return 1
    fi
    
    # Generate MLIR
    if ! $SALT_FRONT "${name}.salt" > "$BIN_DIR/${name}.mlir" 2>/dev/null; then
        echo "frontend failed"
        return 1
    fi
    
    # MLIR -> LLVM IR
    if ! mlir-opt "$BIN_DIR/${name}.mlir" \
        --convert-scf-to-cf \
        --convert-cf-to-llvm \
        --convert-arith-to-llvm \
        --convert-func-to-llvm \
        --reconcile-unrealized-casts \
        -o "$BIN_DIR/${name}_opt.mlir" 2>/dev/null; then
        echo "mlir-opt failed"
        return 1
    fi
    
    if ! mlir-translate --mlir-to-llvmir "$BIN_DIR/${name}_opt.mlir" -o "$BIN_DIR/${name}.ll" 2>/dev/null; then
        echo "mlir-translate failed"
        return 1
    fi
    
    # LLVM IR -> Binary
    if ! clang -O3 "$BIN_DIR/${name}.ll" "$RUNTIME_C" -o "$bin" -lm 2>/dev/null; then
        echo "clang failed"
        return 1
    fi
    
    echo "$bin"
}

run_test() {
    local name="$1"
    
    echo ""
    echo -e "━━━ ${BOLD}$name${NC} ━━━"
    
    # Compile
    local result
    result=$(compile_salt "$name")
    
    if [[ "$result" == *"failed"* ]] || [[ "$result" == "no source" ]]; then
        echo -e "Status: ${RED}$result${NC}"
        return
    fi
    
    # Run with timeout
    local bin="$result"
    if timeout 5s "$bin" > /dev/null 2>&1; then
        echo -e "Status: ${GREEN}PASS${NC}"
    else
        local exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            echo -e "Status: ${YELLOW}TIMEOUT${NC}"
        else
            echo -e "Status: ${RED}FAIL (exit $exit_code)${NC}"
        fi
    fi
}

# Parse args
TESTS=()
RUN_ALL=false
DO_CLEAN=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help) usage ;;
        -l|--list) list_tests ;;
        -a|--all) RUN_ALL=true; shift ;;
        -c|--clean) DO_CLEAN=true; shift ;;
        *) TESTS+=("$1"); shift ;;
    esac
done

# Setup
mkdir -p "$BIN_DIR"
[[ "$DO_CLEAN" == true ]] && clean_build

echo -e "${BLUE}╔═══════════════════════════════════════╗${NC}"
echo -e "${BLUE}║    ${BOLD}Salt Feature Tests${NC}${BLUE}                ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════╝${NC}"

# Determine tests to run
if [[ "$RUN_ALL" == true ]]; then
    TESTS=("${FEATURE_TESTS[@]}")
fi

if [[ ${#TESTS[@]} -eq 0 ]]; then
    echo -e "${RED}No tests specified. Use -h for help.${NC}"
    exit 1
fi

# Run
for test in "${TESTS[@]}"; do
    run_test "$test"
done

echo ""
echo -e "${GREEN}Done!${NC}"
