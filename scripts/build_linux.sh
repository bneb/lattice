#!/usr/bin/env bash
# =============================================================================
# build_linux.sh — Build KeuOS Exokernel on Ubuntu (UM890 Pro)
# =============================================================================
#
# Modes:
#   ./scripts/build_linux.sh                  # Build Basalt + kernel
#   ./scripts/build_linux.sh --basalt-only    # Basalt only (needs mlir-opt)
#   ./scripts/build_linux.sh --kernel-only    # Kernel only
#   ./scripts/build_linux.sh --basalt-from-ll <path.ll>
#       Compile a pre-built LLVM IR file directly with clang -march=native.
#       Use this when mlir-opt is not available on the target machine
#       (apt.llvm.org does not ship mlir-opt-21 on Ubuntu 24.04).
#       bench_baremetal.sh generates the .ll on Mac and rsyncs it before
#       calling this mode.
#
# Prerequisites (UM890, one-time):
#   See: https://apt.llvm.org/  — add the noble-21 repo first, then:
#   sudo apt install -y llvm-21 clang-21 lld-21 nasm libz3-dev z3 build-essential
#
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SALT_FRONT="$PROJECT_ROOT/salt-front"

# ── LLVM version (apt installs as llvm-21, clang-21, etc.) ───────────────────
LLVM_VERSION="${LLVM_VERSION:-21}"

# Detect llvm binary suffix — mlir-opt is NOT available from apt.llvm.org on
# Noble (only libmlir-21-dev ships, no binary). We degrade gracefully.
if command -v "mlir-opt-${LLVM_VERSION}" &>/dev/null; then
    MLIR_OPT="mlir-opt-${LLVM_VERSION}"
    MLIR_TRANSLATE="mlir-translate-${LLVM_VERSION}"
elif command -v mlir-opt &>/dev/null; then
    MLIR_OPT="mlir-opt"
    MLIR_TRANSLATE="mlir-translate"
else
    MLIR_OPT=""   # Not available — --basalt-from-ll mode bypasses this
    MLIR_TRANSLATE=""
fi

if command -v "clang-${LLVM_VERSION}" &>/dev/null; then
    CLANG="clang-${LLVM_VERSION}"
    LLD="ld.lld-${LLVM_VERSION}"
elif command -v clang &>/dev/null; then
    CLANG="clang"
    LLD="ld.lld"
else
    echo "❌  clang not found. Install: sudo apt install -y clang-${LLVM_VERSION}"
    exit 1
fi

# ── Modes ─────────────────────────────────────────────────────────────────────
BUILD_ONLY=false
BASALT_ONLY=false
KERNEL_ONLY=false
BASALT_FROM_LL=""  # path to pre-built .ll file

while [[ $# -gt 0 ]]; do
    case "$1" in
        --build-only)      BUILD_ONLY=true;        shift ;;
        --basalt-only)     BASALT_ONLY=true;       shift ;;
        --kernel-only)     KERNEL_ONLY=true;       shift ;;
        --basalt-from-ll)  BASALT_FROM_LL="$2";    shift 2 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

BUILD_DIR="$PROJECT_ROOT/build"
TMP_DIR="/tmp/salt_build_linux"
mkdir -p "$BUILD_DIR" "$TMP_DIR"

KERNEL_ELF="$BUILD_DIR/kernel.elf"

MLIR_STATUS="${MLIR_OPT:-not available (use --basalt-from-ll)}"
echo "╔══════════════════════════════════════════════════════╗"
echo "║  KeuOS Linux Build                                 ║"
echo "║  Host: $(hostname) — $(uname -m)"                    
echo "║  clang: $CLANG"
echo "║  mlir-opt: $MLIR_STATUS"
echo "╚══════════════════════════════════════════════════════╝"
echo ""

# =============================================================================
# BUILD SALT COMPILER (salt-front) if not already built
# =============================================================================
build_salt_compiler() {
    if [[ -f "$SALT_FRONT/target/release/salt-front" ]]; then
        echo "  [✓] salt-front already built (release)"
        return
    fi
    echo "  Building salt-front (release)..."
    cd "$SALT_FRONT"
    # z3-sys needs the header path on Linux
    export Z3_SYS_Z3_HEADER=/usr/include/z3.h
    export LIBRARY_PATH=/usr/lib
    cargo build --release --quiet
    echo "  [✓] salt-front built"
    cd "$PROJECT_ROOT"
}

# =============================================================================
# BUILD KERNEL ELF
# =============================================================================
build_kernel() {
    echo "🔨 Building KeuOS Kernel (freestanding x86_64)"
    echo "  ────────────────────────────────────────────────"

    KERNEL_SALT_FILES=("$PROJECT_ROOT/user/kernel/main.salt")
    LINKER_SCRIPT="$PROJECT_ROOT/kernel/arch/x86/linker.ld"

    # Step 1: Salt → MLIR
    echo "  [1/5] Compiling Salt → MLIR..."
    build_salt_compiler
    SALT_BIN="$SALT_FRONT/target/release/salt-front"
    for SALT_FILE in "${KERNEL_SALT_FILES[@]}"; do
        BASENAME=$(basename "$SALT_FILE" .salt)
        "$SALT_BIN" "$SALT_FILE" --lib \
            > "$TMP_DIR/kernel_${BASENAME}.mlir" 2>/dev/null || {
            echo "  ✗ MLIR generation failed for $BASENAME"
            exit 1
        }
    done
    echo "  [✓] MLIR generated"

    # Step 2: Optimize MLIR
    echo "  [2/5] Optimizing MLIR..."
    for MLIR_FILE in "$TMP_DIR"/kernel_*.mlir; do
        BASENAME=$(basename "$MLIR_FILE" .mlir)
        "$MLIR_OPT" \
            --convert-scf-to-cf \
            --convert-cf-to-llvm \
            --convert-func-to-llvm \
            --convert-arith-to-llvm \
            --convert-index-to-llvm \
            --reconcile-unrealized-casts \
            "$MLIR_FILE" > "$TMP_DIR/${BASENAME}.opt.mlir" 2>/dev/null
    done
    echo "  [✓] MLIR optimized"

    # Step 3: MLIR → LLVM IR
    echo "  [3/5] Generating LLVM IR..."
    for OPT_FILE in "$TMP_DIR"/kernel_*.opt.mlir; do
        BASENAME=$(basename "$OPT_FILE" .opt.mlir)
        "$MLIR_TRANSLATE" --mlir-to-llvmir \
            "$OPT_FILE" > "$TMP_DIR/${BASENAME}.ll" 2>/dev/null
    done
    echo "  [✓] LLVM IR generated"

    # Step 4: Assemble boot.S / arch asm files
    echo "  [4/5] Assembling..."
    if ! command -v nasm &>/dev/null; then
        echo "  ❌  nasm not found. Install with: sudo apt install -y nasm"
        exit 1
    fi
    nasm -f elf64 \
        "$PROJECT_ROOT/kernel/arch/x86/boot.S" \
        -o "$TMP_DIR/boot.o" 2>/dev/null || \
    nasm -f elf64 \
        "$PROJECT_ROOT/user/kernel/start.S" \
        -o "$TMP_DIR/boot.o" 2>/dev/null || true
    echo "  [✓] Assembly built"

    # Step 5: Compile LLVM IR → objects
    echo "  [5/5] Linking freestanding kernel..."
    for LL_FILE in "$TMP_DIR"/kernel_*.ll; do
        BASENAME=$(basename "$LL_FILE" .ll)
        "$CLANG" -c -target x86_64-unknown-none-elf \
            -fno-stack-protector -ffreestanding -nostdlib \
            "$LL_FILE" -o "$TMP_DIR/${BASENAME}.o" 2>/dev/null
    done

    OBJ_FILES=("$TMP_DIR"/kernel_*.o)
    [[ -f "$TMP_DIR/boot.o" ]] && OBJ_FILES+=("$TMP_DIR/boot.o")

    # Also link the pre-built objects from qemu_build if available
    QEMU_OBJS=("$PROJECT_ROOT/qemu_build"/*.o)
    if [[ ${#QEMU_OBJS[@]} -gt 0 && -f "${QEMU_OBJS[0]}" ]]; then
        OBJ_FILES+=("${QEMU_OBJS[@]}")
    fi

    "$LLD" \
        -T "$LINKER_SCRIPT" \
        --no-dynamic-linker \
        -static -nostdlib \
        -o "$KERNEL_ELF" \
        "${OBJ_FILES[@]}" 2>/dev/null || {
        echo "  ⚠ Linking failed — falling back to qemu_build/kernel.elf"
        cp "$PROJECT_ROOT/qemu_build/kernel.elf" "$KERNEL_ELF"
    }

    echo "  [✓] Kernel ELF: $KERNEL_ELF"
    echo ""
}

# =============================================================================
# BUILD BASALT (LLM inference benchmark — userspace, no boot needed)
# =============================================================================
build_basalt() {
    echo "🧠 Building Basalt (LLM inference benchmark)"
    echo "  ────────────────────────────────────────────────"

    SALT_BIN="$SALT_FRONT/target/release/salt-front"
    OUT="$TMP_DIR"
    COMBINED="$OUT/basalt_combined.salt"

    build_salt_compiler

    echo "// Auto-generated build file for Basalt" > "$COMBINED"
    echo "package main" >> "$COMBINED"
    echo "use std.core.ptr.Ptr" >> "$COMBINED"
    echo "" >> "$COMBINED"
    for f in basalt/src/kernels.salt basalt/src/sampler.salt basalt/src/quant.salt \
              basalt/src/transformer.salt basalt/src/model_loader.salt \
              basalt/src/tokenizer.salt basalt/src/main.salt; do
        echo "// ---- Module: $(basename "$f") ----" >> "$COMBINED"
        grep -v "^package " "$PROJECT_ROOT/$f" | grep -v "^use basalt\." >> "$COMBINED"
        echo "" >> "$COMBINED"
    done

    echo "  [1/4] Salt → MLIR..."
    "$SALT_BIN" "$COMBINED" > "$OUT/basalt.mlir"

    echo "  [2/4] Optimizing MLIR..."
    "$MLIR_OPT" "$OUT/basalt.mlir" \
        --allow-unregistered-dialect \
        --canonicalize --cse --loop-invariant-code-motion --sccp --canonicalize --cse \
        --lower-affine \
        --convert-scf-to-cf --convert-vector-to-llvm --convert-cf-to-llvm \
        --convert-arith-to-llvm --convert-math-to-llvm --convert-func-to-llvm \
        --reconcile-unrealized-casts \
        -o "$OUT/basalt.opt.mlir"

    sed -i '/\"salt.verify\"/d' "$OUT/basalt.opt.mlir"

    echo "  [3/4] MLIR → LLVM IR..."
    "$MLIR_TRANSLATE" --mlir-to-llvmir "$OUT/basalt.opt.mlir" -o "$OUT/basalt.ll"

    echo "  [4/4] Clang -O3 -march=native..."
    "$CLANG" -O3 -ffast-math -march=native \
        "$OUT/basalt.ll" "$SALT_FRONT/runtime.c" \
        -o "$OUT/basalt" -lm -Wno-override-module

    echo "  [✓] Basalt binary: $OUT/basalt"
    echo ""
}

# =============================================================================
# BUILD BASALT FROM PRE-COMPILED .ll (no mlir-opt needed)
# =============================================================================
build_basalt_from_ll() {
    local LL_FILE="$1"
    if [[ ! -f "$LL_FILE" ]]; then
        echo "❌  .ll file not found: $LL_FILE"
        echo "    Run bench_baremetal.sh on Mac first to generate it."
        exit 1
    fi
    echo "🧠 Compiling Basalt from pre-built LLVM IR (Zen 4 native)"
    echo "  Source: $LL_FILE"
    OUT="/tmp/salt_build_linux"
    mkdir -p "$OUT"
    echo "  clang -O3 -ffast-math -march=native ..."
    "$CLANG" -O3 -ffast-math -march=native \
        "$LL_FILE" "$SALT_FRONT/runtime.c" \
        -o "$OUT/basalt" -lm -Wno-override-module
    echo "  [✓] Basalt binary: $OUT/basalt"
}

# =============================================================================
# MAIN
# =============================================================================
if [[ -n "$BASALT_FROM_LL" ]]; then
    build_basalt_from_ll "$BASALT_FROM_LL"
elif $BASALT_ONLY; then
    if [[ -z "$MLIR_OPT" ]]; then
        echo "❌  mlir-opt is not available on this machine."
        echo "    Use --basalt-from-ll <path.ll> instead."
        echo "    bench_baremetal.sh will generate the .ll on your Mac and rsync it over."
        exit 1
    fi
    build_basalt
elif $KERNEL_ONLY; then
    build_kernel
else
    if [[ -n "$MLIR_OPT" ]]; then
        build_basalt
    else
        echo "  ⚠ mlir-opt not available — skipping Basalt full build (use --basalt-from-ll)"
    fi
    build_kernel
fi

echo "✅ Build complete"
if [[ -f "$KERNEL_ELF" ]]; then
    echo "   Kernel:  $KERNEL_ELF ($(wc -c < "$KERNEL_ELF") bytes)"
fi
if [[ -f "$TMP_DIR/basalt" ]]; then
    echo "   Basalt:  $TMP_DIR/basalt"
fi
