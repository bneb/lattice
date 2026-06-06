#!/usr/bin/env zsh
set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"
TMP_DIR="/tmp/salt_build_cdm"
mkdir -p "$TMP_DIR"

LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

# Only link essential dependencies for the sandboxed CDM
DEPS=(
    "std/core/str.salt"
    "std/time.salt"
    "user/os/process.salt"
    "user/browser/ipc_shared.salt"
    "user/browser/constants.salt"
)

LL_FILES=()
for mod in "${DEPS[@]}"; do
    dep_path="$PROJECT_ROOT/$mod"
    dep_base=$(basename "$mod" .salt)
    dep_ll="$TMP_DIR/${dep_base}.ll"
    echo "🔧 [CDM] Compiling ${mod}..."
    "$SALT_FRONT/target/release/salt-front" "$dep_path" --lib --release > "${dep_ll}.mlir"
    mlir-opt "${dep_ll}.mlir" --allow-unregistered-dialect \
        --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
        --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
        --convert-func-to-llvm --reconcile-unrealized-casts -o "${dep_ll}.opt"
    mlir-translate --mlir-to-llvmir "${dep_ll}.opt" -o "$dep_ll"
    sed -i '' 's/internal global/linkonce_odr global/g' "$dep_ll"
    LL_FILES+=("$dep_ll")
done

echo "🔧 [CDM] Compiling user/browser/cdm_main.salt..."
"$SALT_FRONT/target/release/salt-front" "user/browser/cdm_main.salt" --release > "$TMP_DIR/cdm_main.mlir"
mlir-opt "$TMP_DIR/cdm_main.mlir" --allow-unregistered-dialect \
    --canonicalize --cse --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
    --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
    --convert-func-to-llvm --reconcile-unrealized-casts -o "$TMP_DIR/cdm_main.opt"
mlir-translate --mlir-to-llvmir "$TMP_DIR/cdm_main.opt" -o "$TMP_DIR/cdm_main.ll"

echo "🔧 [CDM] Linking prisimi_cdm..."
llvm-link -o "$TMP_DIR/cdm_merged.ll" "$TMP_DIR/cdm_main.ll" "${LL_FILES[@]}"

# Link with runtime and minimal OS bridges
clang -O3 "$TMP_DIR/cdm_merged.ll" \
    "$SALT_FRONT/runtime.c" \
    "$PROJECT_ROOT/user/os/facet_os.c" \
    "$PROJECT_ROOT/user/browser/cdm_bridge.c" \
    -o "./cdm_main"

echo "✅ cdm_main built successfully."
