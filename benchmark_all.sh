#!/bin/bash
set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

RUN_C=false
RUN_C_BATCH=false
RUN_SALT=true
RUN_SALT_BATCH=false # Run batched salt only if requested
RUN_TORCH=false

# Flag Parsing
for arg in "$@"
do
    case $arg in
        --c)
        RUN_C=true
        ;;
        --c-batch)
        RUN_C_BATCH=true
        ;;
        --torch)
        RUN_TORCH=true
        ;;
        --all)
        RUN_C=true
        RUN_C_BATCH=true
        RUN_SALT=true
        RUN_SALT_BATCH=true
        RUN_TORCH=true
        ;;
        --batch)
        RUN_SALT_BATCH=true
        RUN_C_BATCH=true
        ;;
        --no-salt)
        RUN_SALT=false
        ;;
    esac
done

echo -e "${BLUE}=== Sovereign Benchmark Suite ===${NC}"

# 1. Build & Run C Baseline
if [ "$RUN_C" = true ]; then
    echo -e "${GREEN}[C] Building & Running C Baseline (Online)...${NC}"
    clang -O3 -ffast-math benchmarks/ml/sovereign_train.c -o sovereign_train_c
    ./sovereign_train_c || echo "C failed"
fi

# 1.5. Build & Run C Baseline (Batched)
if [ "$RUN_C_BATCH" = true ]; then
    echo -e "${GREEN}[C] Building & Running C Baseline (Batched)...${NC}"
    clang -O3 -ffast-math benchmarks/ml/sovereign_train_batch.c -o sovereign_train_c_batch
    ./sovereign_train_c_batch || echo "C Batch failed"
fi

# 2. Run Salt Sovereign (Standard)
if [ "$RUN_SALT" = true ]; then
    echo -e "${GREEN}[Salt] Running Standard Sovereign V3...${NC}"
    ./scripts/run_test.sh benchmarks/ml/sovereign_train.salt || echo "Salt failed"
fi

# 3. Run Salt Sovereign (Batched)
if [ "$RUN_SALT_BATCH" = true ]; then
    echo -e "${GREEN}[Salt] Running Batched Sovereign V3...${NC}"
    ./scripts/run_test.sh benchmarks/ml/sovereign_train_batch.salt || echo "Salt Batch failed"
fi

# 4. Run PyTorch (Reference)
if [ "$RUN_TORCH" = true ]; then
    echo -e "${GREEN}[PyTorch] Running PyTorch Reference...${NC}"
    # Use virtual env if available
    if [ -d "benchmarks/ml/.venv" ]; then
        source benchmarks/ml/.venv/bin/activate
    fi
    python3 benchmarks/ml/compare_pytorch.py || echo "PyTorch failed (check dependencies)"
fi

echo -e "${BLUE}=== Benchmark Complete ===${NC}"
