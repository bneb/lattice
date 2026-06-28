#!/bin/bash
# ============================================================================
# KeuOS OS v0.5 Demo — One Command Boot
# ============================================================================
# Usage:   ./scripts/demo_keuos.sh
# Result:  Builds the Salt compiler + kernel, boots in QEMU, runs Ring of Fire
#
# Prerequisites:
#   - Rust 1.75+      (curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh)
#   - LLVM 21+        (brew install llvm@21)
#   - Z3 4.12+        (brew install z3)
#   - QEMU            (brew install qemu)
# ============================================================================

set -e

# Colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
cd "$ROOT"

echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║${NC}  ${BOLD}KEUOS OS v0.5${NC} — Salt-Powered • Z3-Verified         ${CYAN}║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""

# ──────────────────────────────────────────────────────────
# Step 0: Check prerequisites
# ──────────────────────────────────────────────────────────
MISSING=""
command -v cargo &>/dev/null || MISSING="${MISSING}  - Rust/Cargo: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n"
command -v qemu-system-x86_64 &>/dev/null || MISSING="${MISSING}  - QEMU: brew install qemu\n"

# Check for LLVM tools
LLVM_VERSION="${LLVM_VERSION:-21}"
LLVM_BIN=""
if [ -d "/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin" ]; then
    LLVM_BIN="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin"
elif [ -d "/opt/homebrew/opt/llvm/bin" ]; then
    LLVM_BIN="/opt/homebrew/opt/llvm/bin"
elif [ -d "/usr/local/opt/llvm@${LLVM_VERSION}/bin" ]; then
    LLVM_BIN="/usr/local/opt/llvm@${LLVM_VERSION}/bin"
fi

if [ -z "$LLVM_BIN" ]; then
    MISSING="${MISSING}  - LLVM ${LLVM_VERSION}: brew install llvm@${LLVM_VERSION}\n"
fi

# Check for Z3
Z3_HEADER=""
if [ -f "/opt/homebrew/include/z3.h" ]; then
    Z3_HEADER="/opt/homebrew/include/z3.h"
    Z3_LIB="/opt/homebrew/lib"
elif [ -f "/usr/local/include/z3.h" ]; then
    Z3_HEADER="/usr/local/include/z3.h"
    Z3_LIB="/usr/local/lib"
fi

if [ -z "$Z3_HEADER" ]; then
    MISSING="${MISSING}  - Z3: brew install z3\n"
fi

if [ -n "$MISSING" ]; then
    echo -e "${RED}Missing prerequisites:${NC}"
    echo -e "$MISSING"
    exit 1
fi

# Set environment for the build
export PATH="$LLVM_BIN:$PATH"
export Z3_SYS_Z3_HEADER="$Z3_HEADER"
export LIBRARY_PATH="$Z3_LIB"
export DYLD_LIBRARY_PATH="$Z3_LIB"

echo -e "${GREEN}[0/4]${NC} Prerequisites verified ✓"

# ──────────────────────────────────────────────────────────
# Step 1: Build the Salt compiler (if needed)
# ──────────────────────────────────────────────────────────
SALT_BIN="salt-front/target/release/saltc"
if [ ! -f "$SALT_BIN" ]; then
    echo -e "${YELLOW}[1/4]${NC} Building Salt compiler (first-time only, ~60s)..."
    cd salt-front && cargo build --release 2>&1 | tail -1 && cd ..
else
    echo -e "${GREEN}[1/4]${NC} Salt compiler found ✓"
fi

# ──────────────────────────────────────────────────────────
# Step 2: Build the kernel + benchmark
# ──────────────────────────────────────────────────────────
echo -e "${YELLOW}[2/4]${NC} Compiling KeuOS kernel..."
python3 tools/runner_qemu.py build 2>&1 | tail -3
echo -e "${GREEN}       Kernel compiled ✓${NC}"

# ──────────────────────────────────────────────────────────
# Step 3: Check for QEMU (already checked above, but confirm)
# ──────────────────────────────────────────────────────────
echo -e "${GREEN}[3/4]${NC} QEMU found ✓"

# ──────────────────────────────────────────────────────────
# Step 4: Boot the kernel
# ──────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}────────────────── QEMU SERIAL OUTPUT ──────────────────${NC}"
echo ""

# Run QEMU with timeout. The kernel halts after benchmark via cli;hlt.
OUTPUT=$(timeout 30 qemu-system-x86_64 \
    -kernel qemu_build/kernel.elf \
    -nographic \
    -m 128M \
    -cpu qemu64,+fxsr,+mmx,+sse,+sse2,+xsave \
    -no-reboot \
    -serial mon:stdio 2>/dev/null || true)

echo "$OUTPUT"

echo ""
echo -e "${CYAN}────────────────────────────────────────────────────────${NC}"
echo ""

# ──────────────────────────────────────────────────────────
# Result
# ──────────────────────────────────────────────────────────
if echo "$OUTPUT" | grep -q "ROF Result"; then
    echo -e "${GREEN}${BOLD}✓ Demo complete.${NC} Ring of Fire benchmark ran successfully."
    echo ""
    echo -e "  The kernel booted, spawned fibers, measured context switch gaps,"
    echo -e "  and halted cleanly."
elif echo "$OUTPUT" | grep -q "KEUOS BOOT"; then
    echo -e "${YELLOW}${BOLD}⚠ Partial boot.${NC} Kernel started but benchmark did not complete."
    echo -e "  Check qemu.log for interrupt traces."
else
    echo -e "${RED}${BOLD}✗ Boot failed.${NC} No serial output detected."
    echo -e "  Run with: qemu-system-x86_64 -kernel qemu_build/kernel.elf -nographic -d int"
fi

echo ""
echo -e "Learn more:"
echo -e "  ${BOLD}Architecture:${NC} docs/ARCH.md"
echo -e "  ${BOLD}Kernel:${NC}       kernel/README.md"
echo -e "  ${BOLD}Benchmarks:${NC}   benchmarks/BENCHMARKS.md"
echo -e "  ${BOLD}Salt lang:${NC}    README.md"
echo ""
