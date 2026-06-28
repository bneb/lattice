#!/usr/bin/env bash
# =============================================================================
# Salt + KeuOS — Bootstrap Development Environment
# =============================================================================
# One command to go from zero to kernel booting in QEMU.
#
# Usage:
#   curl -fsSL https://.../bootstrap.sh | bash       # via curl
#   ./scripts/bootstrap.sh                            # from repo root
#   make setup                                        # via Makefile
#
# What it does:
#   1. Detects OS (macOS / Linux)
#   2. Checks prerequisites (LLVM 21, Z3, Rust, QEMU)
#   3. Prints actionable install instructions for missing deps
#   4. Builds the Salt compiler (salt-front)
#   5. Builds the KeuOS kernel ISO
#   6. Optionally boots in QEMU for smoke test
#
# Time target: <15 minutes on a warm cache, <30 minutes from clean
# =============================================================================

set -euo pipefail

# ── Resolve project root ──────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*"; }
header(){ echo ""; echo -e "${BOLD}═══ $* ═══${NC}"; echo ""; }

# ── OS Detection ──────────────────────────────────────────────────
OS="unknown"
PKG_MANAGER=""
if [[ "$(uname -s)" == "Darwin" ]]; then
    OS="macos"
    PKG_MANAGER="brew"
elif [[ "$(uname -s)" == "Linux" ]]; then
    OS="linux"
    if command -v apt-get &>/dev/null; then
        PKG_MANAGER="apt"
    else
        PKG_MANAGER="unknown"
    fi
fi

header "Salt + KeuOS Bootstrap"
info "Detected OS: $OS ($PKG_MANAGER)"
info "Project root: $PROJECT_ROOT"

# ── Prerequisites ─────────────────────────────────────────────────
header "Step 1: Checking Prerequisites"

MISSING=()

# Rust 1.75+
if command -v rustc &>/dev/null; then
    RUST_VER=$(rustc --version | grep -oE '[0-9]+\.[0-9]+' | head -1)
    if [[ "$(printf '%s\n' "1.75" "$RUST_VER" | sort -V | head -1)" == "1.75" ]]; then
        ok "Rust $RUST_VER (>=1.75 required)"
    else
        warn "Rust $RUST_VER is too old (need 1.75+)"
        MISSING+=("rust>=1.75")
    fi
else
    warn "Rust not found"
    MISSING+=("rust")
fi

# LLVM 21 (check for mlir-opt)
MLIR_OPT=""
if command -v mlir-opt &>/dev/null; then
    MLIR_OPT="mlir-opt"
    ok "mlir-opt found on PATH"
elif [[ -f "/opt/homebrew/opt/llvm@21/bin/mlir-opt" ]]; then
    MLIR_OPT="/opt/homebrew/opt/llvm@21/bin/mlir-opt"
    ok "mlir-opt found at /opt/homebrew/opt/llvm@21"
    export PATH="/opt/homebrew/opt/llvm@21/bin:$PATH"
elif [[ -f "/usr/lib/llvm-21/bin/mlir-opt" ]]; then
    MLIR_OPT="/usr/lib/llvm-21/bin/mlir-opt"
    ok "mlir-opt found at /usr/lib/llvm-21"
    export PATH="/usr/lib/llvm-21/bin:$PATH"
else
    warn "mlir-opt (LLVM 21) not found"
    MISSING+=("llvm@21")
fi

# Z3 4.12+
if [[ -f "/opt/homebrew/lib/libz3.dylib" ]] || [[ -f "/usr/lib/libz3.so" ]] || [[ -f "/usr/lib/x86_64-linux-gnu/libz3.so" ]]; then
    ok "libz3 found"
    if [[ "$OS" == "macos" ]]; then
        export DYLD_LIBRARY_PATH="/opt/homebrew/lib:${DYLD_LIBRARY_PATH:-}"
    fi
else
    warn "Z3 library not found (libz3.dylib / libz3.so)"
    MISSING+=("z3>=4.12")
fi

# QEMU (for kernel boot testing)
if command -v qemu-system-x86_64 &>/dev/null; then
    ok "qemu-system-x86_64 found"
else
    warn "qemu-system-x86_64 not found (optional, needed for kernel boot test)"
    MISSING+=("qemu")
fi

# Python 3 (for test runners)
if command -v python3 &>/dev/null; then
    ok "python3 found"
else
    warn "python3 not found (optional, needed for test runners)"
    MISSING+=("python3")
fi

# ── Install instructions ──────────────────────────────────────────
if [[ ${#MISSING[@]} -gt 0 ]]; then
    echo ""
    header "Missing Dependencies"
    echo "The following are required to build Salt + KeuOS:"
    for dep in "${MISSING[@]}"; do
        echo "  - $dep"
    done
    echo ""
    echo -e "${BOLD}Install instructions:${NC}"
    echo ""

    if [[ "$OS" == "macos" ]]; then
        echo "  # Install Homebrew if needed:"
        echo "  /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
        echo ""
        echo "  # Install all dependencies:"
        echo "  brew install rust llvm@21 z3 qemu python@3"
        echo ""
        echo "  # Add LLVM 21 to PATH (add to ~/.zshrc for persistence):"
        echo "  export PATH=\"/opt/homebrew/opt/llvm@21/bin:\$PATH\""
        echo "  export DYLD_LIBRARY_PATH=\"/opt/homebrew/lib:\$DYLD_LIBRARY_PATH\""
    elif [[ "$OS" == "linux" && "$PKG_MANAGER" == "apt" ]]; then
        echo "  # Install LLVM 21:"
        echo "  wget -O - https://apt.llvm.org/llvm-snapshot.gpg.key | sudo apt-key add -"
        echo "  sudo add-apt-repository 'deb http://apt.llvm.org/$(lsb_release -cs)/ llvm-toolchain-$(lsb_release -cs)-21 main'"
        echo "  sudo apt-get update"
        echo ""
        echo "  # Install all dependencies:"
        echo "  sudo apt-get install -y build-essential curl rustc cargo \\"
        echo "    llvm-21 llvm-21-dev llvm-21-tools clang-21 \\"
        echo "    libz3-dev z3 qemu-system-x86 python3"
        echo ""
        echo "  # Add LLVM 21 to PATH (add to ~/.bashrc for persistence):"
        echo "  export PATH=\"/usr/lib/llvm-21/bin:\$PATH\""
    else
        echo "  Unsupported OS/package manager. Please install manually:"
        echo "    - Rust 1.75+ (https://rustup.rs)"
        echo "    - LLVM 21 (https://llvm.org)"
        echo "    - Z3 4.12+ (https://github.com/Z3Prover/z3)"
        echo "    - QEMU (https://www.qemu.org)"
        echo "    - Python 3"
    fi
    echo ""
    echo -e "${YELLOW}Rerun this script after installing dependencies.${NC}"
    exit 1
fi

header "All Prerequisites Met"

# ── Build Salt Compiler ───────────────────────────────────────────
header "Step 2: Building Salt Compiler (salt-front)"

cd "$PROJECT_ROOT/salt-front"
info "Running: cargo build --release"

if cargo build --release 2>&1 | tail -20; then
    ok "Salt compiler built successfully"
    SALT_FRONT="$PROJECT_ROOT/salt-front/target/release/saltc"
    ok "Binary: $SALT_FRONT"
else
    err "Salt compiler build failed. Check the output above for errors."
    exit 1
fi

# ── Smoke Test: Compile hello_world ───────────────────────────────
header "Step 3: Smoke Test (Hello World)"

HELLO="$PROJECT_ROOT/examples/hello_world.salt"
if [[ -f "$HELLO" ]]; then
    info "Compiling: $HELLO"
    cd "$PROJECT_ROOT"
    if "$SALT_FRONT" "$HELLO" -o /tmp/salt_hello 2>&1; then
        ok "Hello world compiled to /tmp/salt_hello"
        if [[ -f /tmp/salt_hello ]]; then
            chmod +x /tmp/salt_hello
            OUTPUT=$(/tmp/salt_hello 2>&1 || true)
            ok "Execution output: $OUTPUT"
        fi
    else
        warn "Hello world compilation failed (check LLVM tools on PATH)"
    fi
else
    warn "examples/hello_world.salt not found — skipping smoke test"
fi

# ── Build Kernel ISO ──────────────────────────────────────────────
header "Step 4: Building KeuOS Kernel ISO"

if [[ -f "$PROJECT_ROOT/tools/build_iso.sh" ]]; then
    info "Running: tools/build_iso.sh"
    cd "$PROJECT_ROOT"
    if bash tools/build_iso.sh 2>&1 | tail -10; then
        ok "Kernel ISO built: keuos.iso"
    else
        warn "Kernel ISO build failed (this is expected if not on x86_64 or if cross-compilation tools are missing)"
    fi
else
    warn "tools/build_iso.sh not found — skipping kernel build"
fi

# ── Optional: Boot in QEMU ────────────────────────────────────────
header "Step 5: QEMU Boot Test (Optional)"

if command -v qemu-system-x86_64 &>/dev/null && [[ -f "$PROJECT_ROOT/keuos.iso" ]]; then
    echo ""
    echo -e "${YELLOW}Would you like to boot KeuOS in QEMU for a smoke test?${NC}"
    echo "  This will start QEMU with the kernel ISO. Press Ctrl+A then X to quit."
    echo ""
    read -p "  Boot in QEMU? [y/N] " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        info "Booting KeuOS in QEMU (Ctrl+A X to quit)..."
        qemu-system-x86_64 \
            -cdrom "$PROJECT_ROOT/keuos.iso" \
            -m 512M \
            -serial stdio \
            -no-reboot \
            -display none \
            2>&1 | head -80 || true
        ok "QEMU boot test completed"
    else
        info "Skipping QEMU boot test"
    fi
else
    info "QEMU or kernel ISO not available — skipping boot test"
fi

# ── Done ──────────────────────────────────────────────────────────
header "Setup Complete"

echo "What's ready:"
echo "  ✓ Salt compiler:  $SALT_FRONT"
echo "  ✓ Hello world:    /tmp/salt_hello"
if [[ -f "$PROJECT_ROOT/keuos.iso" ]]; then
    echo "  ✓ Kernel ISO:     $PROJECT_ROOT/keuos.iso"
fi
echo ""
echo "Quick start:"
echo "  # Compile a Salt program:"
echo "  $SALT_FRONT examples/hello_world.salt -o my_program"
echo ""
echo "  # Run tests:"
echo "  cd salt-front && cargo test"
echo ""
echo "  # Build the kernel:"
echo "  bash tools/build_iso.sh"
echo ""
echo "  # Boot in QEMU:"
echo "  qemu-system-x86_64 -cdrom keuos.iso -m 512M -serial stdio"
echo ""
echo -e "${GREEN}${BOLD}Happy hacking! 🧂${NC}"
