# =============================================================================
# KeuOS Microkernel + Salt Compiler — Linux Build Environment
# =============================================================================
# Ubuntu 24.04 (Noble) + LLVM 21 + Z3 + Rust
#
# Usage:
#   python3 tools/docker_build.py image    # Build Docker image
#   python3 tools/docker_build.py build    # Build salt-front + salt-opt
#   python3 tools/docker_build.py test     # Build + run tests
#   python3 tools/docker_build.py shell    # Interactive shell
# =============================================================================

FROM ubuntu:24.04

LABEL maintainer="KeuOS Project"
LABEL description="Reproducible Linux build environment for Salt + KeuOS"

ENV DEBIAN_FRONTEND=noninteractive

# =============================================================================
# 1. Add LLVM 21 repository (apt.llvm.org/noble) + install deps
# =============================================================================
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl gnupg wget \
    && wget -qO- https://apt.llvm.org/llvm-snapshot.gpg.key | tee /etc/apt/trusted.gpg.d/apt.llvm.org.asc \
    && echo "deb http://apt.llvm.org/noble/ llvm-toolchain-noble-21 main" > /etc/apt/sources.list.d/llvm-21.list \
    && apt-get update && apt-get install -y --no-install-recommends \
    # Build essentials
    build-essential \
    cmake \
    ninja-build \
    pkg-config \
    git \
    # LLVM 21 + MLIR
    llvm-21-dev \
    libmlir-21-dev \
    mlir-21-tools \
    libclang-21-dev \
    clang-21 \
    lld-21 \
    # Z3 solver
    libz3-dev \
    # LLVM link deps
    libzstd-dev \
    zlib1g-dev \
    # Build runner
    python3 \
    && rm -rf /var/lib/apt/lists/*

# QEMU — optional, only available on x86_64 hosts
RUN apt-get update && \
    (apt-get install -y --no-install-recommends qemu-system-x86 2>/dev/null || \
    echo "NOTE: qemu-system-x86 not available (expected on ARM64)") && \
    rm -rf /var/lib/apt/lists/*

# Symlink versioned LLVM tools to unversioned names
RUN for tool in llc clang clang++ llvm-ar llvm-objdump llvm-nm opt mlir-opt; do \
    if [ -f "/usr/bin/${tool}-21" ]; then \
    ln -sf "/usr/bin/${tool}-21" "/usr/bin/${tool}"; \
    fi; \
    done && \
    ln -sf /usr/bin/lld-21 /usr/bin/lld && \
    ln -sf /usr/bin/ld.lld-21 /usr/bin/ld.lld

# =============================================================================
# 2. Rust toolchain (stable)
# =============================================================================
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH="/usr/local/cargo/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable --profile minimal && \
    rustup component add rust-src

# =============================================================================
# 3. Verify core toolchain
# =============================================================================
RUN echo "=== Toolchain Verification ===" && \
    clang --version | head -2 && \
    llc --version | head -2 && \
    cmake --version | head -1 && \
    rustc --version && \
    dpkg -l libz3-dev | grep -q libz3 && echo "Z3: $(dpkg -s libz3-dev | grep Version)" && \
    echo "=== All tools present ==="

# =============================================================================
# 4. MLIR/LLVM CMake paths
# =============================================================================
ENV MLIR_DIR=/usr/lib/llvm-21/lib/cmake/mlir
ENV LLVM_DIR=/usr/lib/llvm-21/lib/cmake/llvm
ENV Z3_DIR=/usr

WORKDIR /workspace
CMD ["bash"]
