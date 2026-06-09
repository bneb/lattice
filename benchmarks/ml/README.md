# 🧠 Salt ML Training Benchmark

**KeuOS**: A 2-layer neural network trained on MNIST, demonstrating Salt's MLIR-optimized performance.

## Results

| Metric | Salt | C | Notes |
|--------|------|---|-------|
| **Training Time** | **6.3s** | 6.3s | |
| **Test Accuracy** | 97% | 97% | |
| **Macro F1** | 0.97 | 0.97 | |
| **Lines of Code** | 235 | 345 | 32% less |

---

## Quick Start

```bash
cd benchmarks/ml

# 1. Setup Python environment (for reference comparisons)
python3 -m venv .venv && source .venv/bin/activate
pip install torch torchvision

# 2. Prepare MNIST data
python3 prepare_data.py

# 3. Run benchmarks
./benchmark.sh --all
```

---

## Model Architecture

```
Input (784) → Dense(128) → ReLU → Dense(10) → Softmax → Class
```

| Layer | Parameters |
|-------|------------|
| W1, b1 | 784×128 + 128 = 100,480 |
| W2, b2 | 128×10 + 10 = 1,290 |
| **Total** | **101,770 weights** |

**Training**: 8 epochs, SGD with lr=0.001, Xavier initialization.

---

## How Salt Optimizes the Pipeline

Salt uses three key MLIR-level optimizations, paired with aggressive LLVM vectorization:

### 1. Affine Tiling
Loop kernels are tiled (`tile-size=32`) to ensure data remains in L1 cache, significantly reducing memory latency in the 6 billion total iterations.

### 2. SSA Loop Carrying  
Reduction accumulators in training loops live in registers via `scf.for iter_args`, eliminating store-to-load forwarding penalties.

### 3. FMLA Vectorization
LLVM contracts multiply-add patterns into NEON **FMLA** (Fused Multiply-Add) instructions, processing 4 single-precision floats per cycle.

---

## Files

| File | Purpose |
|------|---------|
| `keuos_train.salt` | Salt implementation |
| `keuos_train.c` | C baseline |
| `keuos_train.py` | PyTorch reference |
| `ml_bridge.c` | FFI bridge (mmap, timing, printf) |
| `benchmark.sh` | Build and run script |
| `prepare_data.py` | MNIST download/preprocessing |

---

## Build Commands

**C:**
```bash
clang -O3 -ffast-math -march=native keuos_train.c -o keuos_train_c -lm
```

**Salt (Optimized Pipeline):**
```bash
# Compile Salt -> MLIR (with affine tiling) -> LLVM (with O3 vectorization)
./benchmark.sh --salt
```
