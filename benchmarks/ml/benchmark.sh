#!/bin/zsh
#
# benchmark.sh - Unified ML Benchmark Runner
#
# Usage: ./benchmark.sh [--salt] [--c] [--python] [--all]
#

set -e
cd "$(dirname "$0")"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

# Paths
# Paths
SALT_BIN="../../salt-front/target/release/saltc"

# Build Salt if missing
if [ ! -f "$SALT_BIN" ]; then
    echo -e "${GREEN}[Setup] Building Salt compiler...${NC}"
    (cd ../../salt-front && cargo build --release)
fi

RUNTIME_C="../../salt-front/runtime.c"
LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"
export DYLD_LIBRARY_PATH="/opt/homebrew/lib:${DYLD_LIBRARY_PATH:-}"

print_header() {
    echo -e "${BLUE}╔═══════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║  ${BOLD}KeuOS Training Benchmark${NC}${BLUE}          ║${NC}"
    echo -e "${BLUE}╚═══════════════════════════════════════╝${NC}"
    echo ""
}

prepare_data() {
    if [ ! -f "data/mnist_train_images.bin" ]; then
        echo -e "${GREEN}Preparing MNIST data...${NC}"
        python3 prepare_data.py
    fi
}

build_c() {
    echo -e "${GREEN}[C] Building...${NC}"
    clang -O3 -ffast-math -march=native keuos_train.c -lm -o keuos_train_c 2>&1
}

run_c() {
    echo -e "${GREEN}[C] Running...${NC}"
    ./keuos_train_c
}

build_salt() {
    echo -e "${GREEN}[Salt] Building...${NC}"
    $SALT_BIN keuos_train.salt --release 2>/dev/null \
        | grep -v "^DEBUG:" | grep -v "^>" \
        > keuos_train_clean.mlir
    
    mlir-opt --convert-linalg-to-loops \
             --expand-strided-metadata \
             --affine-loop-tile="tile-size=8" \
             --lower-affine \
             --convert-scf-to-cf \
             --canonicalize \
             --sroa --mem2reg \
             --canonicalize \
             --finalize-memref-to-llvm \
             --convert-arith-to-llvm --convert-math-to-llvm --convert-func-to-llvm --convert-cf-to-llvm \
             --reconcile-unrealized-casts \
             keuos_train_clean.mlir -o keuos_train.opt.mlir 2>/dev/null

    mlir-translate --mlir-to-llvmir keuos_train.opt.mlir -o keuos_train.ll 2>/dev/null
    
    # LLVM-level optimization (Crucial for vectorization and LICM)
    opt -O3 keuos_train.ll -S -o keuos_train_opt.ll 2>/dev/null
    
    clang -O3 -ffast-math -march=native keuos_train_opt.ll ml_bridge.c $RUNTIME_C -lm -o keuos_train_salt 2>/dev/null
}

run_salt() {
    echo -e "${GREEN}[Salt] Running...${NC}"
    ./keuos_train_salt
}

run_python() {
    echo -e "${GREEN}[PyTorch] Running...${NC}"
    if [ ! -d ".venv" ]; then
        python3 -m venv .venv
        source .venv/bin/activate
        pip install torch numpy scikit-learn -q
    else
        source .venv/bin/activate
    fi
    python3 keuos_train.py
}

# Parse args
RUN_C=false
RUN_SALT=false
RUN_PYTHON=false

for arg in "$@"; do
    case $arg in
        --c) RUN_C=true ;;
        --salt) RUN_SALT=true ;;
        --python) RUN_PYTHON=true ;;
        --all) RUN_C=true; RUN_SALT=true; RUN_PYTHON=true ;;
        *) echo "Usage: $0 [--c] [--salt] [--python] [--all]"; exit 1 ;;
    esac
done

# Default to --all if no args
if [ "$RUN_C" = false ] && [ "$RUN_SALT" = false ] && [ "$RUN_PYTHON" = false ]; then
    RUN_C=true
    RUN_SALT=true
    RUN_PYTHON=true
fi

print_header
prepare_data

if [ "$RUN_C" = true ]; then
    echo ""
    build_c
    run_c
fi

if [ "$RUN_SALT" = true ]; then
    echo ""
    build_salt
    run_salt
fi

if [ "$RUN_PYTHON" = true ]; then
    echo ""
    run_python
fi

echo ""
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BOLD}Benchmark Complete${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
