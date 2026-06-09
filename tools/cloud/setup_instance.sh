#!/bin/bash
# ============================================================================
# KeuOS Cloud Instance Setup
# ============================================================================
# Installs the complete toolchain on a fresh Ubuntu 24.04 instance.
# Idempotent — safe to re-run.
#
# Usage: ssh ubuntu@<instance-ip> 'bash -s' < tools/cloud/setup_instance.sh
# ============================================================================

set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  KeuOS Benchmark — Instance Setup${NC}"
echo -e "${GREEN}════════════════════════════════════════════════════════${NC}"

# ─── System packages ──────────────────────────────────────────────
echo -e "${YELLOW}[1/5]${NC} Installing system packages..."
if ! command -v clang-21 &>/dev/null; then
    sudo apt-get update -qq
    sudo apt-get install -y -qq software-properties-common wget gnupg
    echo "deb http://apt.llvm.org/noble/ llvm-toolchain-noble-21 main" | sudo tee /etc/apt/sources.list.d/llvm-21.list > /dev/null
    wget -qO- https://apt.llvm.org/llvm-snapshot.gpg.key | sudo tee /etc/apt/trusted.gpg.d/apt.llvm.org.asc > /dev/null
    sudo apt-get update -qq
    
    sudo apt-get install -y -qq \
    build-essential \
    cmake \
    ninja-build \
    python3 \
    git \
    qemu-system-x86 \
    llvm-21 \
    llvm-21-dev \
    libmlir-21-dev \
    mlir-21-tools \
    clang-21 \
    lld-21 \
    libz3-dev \
    z3 \
    zlib1g-dev \
    libzstd-dev \
    libcurl4-openssl-dev \
    libedit-dev \
    pkg-config \
    libssl-dev \
    curl \
    2>&1 | tail -5

# Create symlinks so tools are on PATH without version suffix
sudo ln -sf /usr/bin/llc-21 /usr/local/bin/llc 2>/dev/null || true
sudo ln -sf /usr/bin/clang-21 /usr/local/bin/clang 2>/dev/null || true
sudo ln -sf /usr/bin/lld-21 /usr/local/bin/lld 2>/dev/null || true
sudo ln -sf /usr/bin/clang++-21 /usr/local/bin/clang++ 2>/dev/null || true

    echo -e "${GREEN}  ✓ System packages installed${NC}"
else
    echo -e "${GREEN}  ✓ System packages already installed${NC}"
fi

# ─── Rust ──────────────────────────────────────────────────────────
echo -e "${YELLOW}[2/5]${NC} Installing Rust toolchain..."
if command -v cargo &>/dev/null; then
    echo -e "${GREEN}  ✓ Rust already installed$(cargo --version)${NC}"
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
    source "$HOME/.cargo/env"
    echo -e "${GREEN}  ✓ Rust installed $(cargo --version)${NC}"
fi
source "$HOME/.cargo/env" 2>/dev/null || true

# ─── Build salt-front ──────────────────────────────────────────────
echo -e "${YELLOW}[3/5]${NC} Building salt-front compiler..."
REPO_DIR="$HOME/keuos"
if [ ! -d "$REPO_DIR" ]; then
    echo -e "${RED}  Error: repo not synced to $REPO_DIR${NC}"
    echo "  Run: rsync -az --exclude target --exclude .git <local-repo>/ ubuntu@<ip>:keuos/"
    exit 1
fi

cd "$REPO_DIR"

# Set Z3 environment
export Z3_SYS_Z3_HEADER="/usr/include/z3.h"
export LIBRARY_PATH="/usr/lib/x86_64-linux-gnu"
export LD_LIBRARY_PATH="/usr/lib/x86_64-linux-gnu"

    cd salt-front
    cargo build --release 2>&1 | tail -3
    cd ..
    echo -e "${GREEN}  ✓ salt-front built${NC}"

# ─── Build salt-opt (MLIR optimizer) ────────────────────────────────
echo -e "${YELLOW}[4/6]${NC} Building salt-opt..."
SALT_OPT="salt/build/salt-opt"
if [ -f "$SALT_OPT" ]; then
    echo -e "${GREEN}  ✓ salt-opt already built${NC}"
else
    # Discover MLIR/LLVM CMake config paths
    MLIR_CMAKE_DIR=$(find /usr/lib/llvm-21 -name "MLIRConfig.cmake" -printf '%h' 2>/dev/null | head -1)
    LLVM_CMAKE_DIR=$(find /usr/lib/llvm-21 -name "LLVMConfig.cmake" -printf '%h' 2>/dev/null | head -1)

    if [ -z "$MLIR_CMAKE_DIR" ]; then
        echo -e "${RED}  ✗ MLIRConfig.cmake not found. Is libmlir-21-dev installed?${NC}"
        echo "  Try: sudo apt-get install -y libmlir-21-dev"
        echo "  Search: find /usr -name 'MLIRConfig.cmake' 2>/dev/null"
        exit 1
    fi
    if [ -z "$LLVM_CMAKE_DIR" ]; then
        echo -e "${RED}  ✗ LLVMConfig.cmake not found. Is llvm-21-dev installed?${NC}"
        exit 1
    fi

    echo "  MLIR_DIR=$MLIR_CMAKE_DIR"
    echo "  LLVM_DIR=$LLVM_CMAKE_DIR"

    cd salt
    if [ ! -f "build/CMakeCache.txt" ]; then
        rm -rf build
        mkdir -p build && cd build
        CMAKE_OUTPUT=$(cmake -G Ninja .. \
            -DMLIR_DIR="$MLIR_CMAKE_DIR" \
            -DLLVM_DIR="$LLVM_CMAKE_DIR" \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_C_COMPILER=clang-21 \
            -DCMAKE_CXX_COMPILER=clang++-21 2>&1) || CMAKE_EXIT=$?
        CMAKE_EXIT=${CMAKE_EXIT:-0}
        
        if [ $CMAKE_EXIT -ne 0 ]; then
            echo -e "${RED}  ✗ CMake configuration failed (exit $CMAKE_EXIT):${NC}"
            echo "$CMAKE_OUTPUT"
            exit 1
        fi
        echo "$CMAKE_OUTPUT" | tail -3
    else
        cd build
        echo "  ✓ CMake configuration already exists"
    fi
    ninja clean && ninja salt-opt 2>&1 | tail -5
    cd ../..
    echo -e "${GREEN}  ✓ salt-opt built${NC}"
fi

# ─── Verify KVM ──────────────────────────────────────────────────
echo -e "${YELLOW}[5/6]${NC} Verifying KVM access..."
if [ -e /dev/kvm ]; then
    echo -e "${GREEN}  ✓ /dev/kvm available — hardware acceleration enabled${NC}"
    # Ensure current user can access KVM
    sudo chmod 666 /dev/kvm 2>/dev/null || true
else
    echo -e "${RED}  ✗ /dev/kvm not found — benchmark will use TCG (software emulation)${NC}"
    echo "  This instance may not support KVM. Use a .metal instance type."
fi

# ─── Pin CPU frequency (prevent rdtsc skew) ──────────────────────
echo -e "${YELLOW}[6/6]${NC} Pinning CPU to max performance..."

# Install cpupower if not present
sudo apt-get install -y -qq linux-tools-common linux-tools-$(uname -r) 2>/dev/null || true

if command -v cpupower &>/dev/null; then
    sudo cpupower frequency-set -g performance 2>/dev/null && \
        echo -e "${GREEN}  ✓ CPU governor set to 'performance' — no frequency scaling${NC}" || \
        echo -e "${YELLOW}  ⚠ cpupower set failed (may not affect .metal instances)${NC}"
else
    echo -e "${YELLOW}  ⚠ cpupower not available — rdtsc measurements may have ramp-up jitter${NC}"
fi

# Also disable turbo boost for consistent measurements (optional, best-effort)
if [ -f /sys/devices/system/cpu/intel_pstate/no_turbo ]; then
    echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo > /dev/null 2>&1 && \
        echo -e "${GREEN}  ✓ Turbo boost disabled for consistent cycle counts${NC}" || true
fi

echo ""
echo -e "${GREEN}════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  Setup complete. Ready to benchmark.${NC}"
echo -e "${GREEN}════════════════════════════════════════════════════════${NC}"
echo ""
echo "Run:  cd ~/keuos && python3 tools/runner_qemu.py run"
