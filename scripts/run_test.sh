#!/usr/bin/env zsh
# =============================================================================
# Salt Test Runner — Full MLIR Pipeline
# =============================================================================
# Compiles a .salt file through the full pipeline and runs it:
#   saltc → mlir-opt → mlir-translate → clang → execute
#
# Usage:
#   ./scripts/run_test.sh tests/test_thread.salt
#   ./scripts/run_test.sh tests/test_sync.salt
#   ./scripts/run_test.sh examples/http_server.salt    # compile only (server)
#
# Options:
#   --compile-only    Build but don't execute
#   --verbose         Show each pipeline stage
#   --bridge FILE     Include additional C bridge file(s)
# =============================================================================

set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"
SALT_FRONT="$PROJECT_ROOT/salt-front"

# LLVM tools — override with: LLVM_VERSION=19 ./scripts/run_test.sh ...
LLVM_VERSION="${LLVM_VERSION:-21}"
export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"
export DYLD_LIBRARY_PATH=/opt/homebrew/lib

# Defaults
COMPILE_ONLY=false
VERBOSE=false
NO_VERIFY=false
EXTRA_BRIDGES=()
SALT_FILE=""
LIB_MODE=false
BENCHMARK_MODE=false

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --compile-only) COMPILE_ONLY=true; shift ;;
        --benchmark) BENCHMARK_MODE=true; shift ;;
        --lib) LIB_MODE=true; shift ;;
        --no-verify) NO_VERIFY=true; shift ;;
        --verbose) VERBOSE=true; shift ;;
        --bridge) EXTRA_BRIDGES+=("$2"); shift 2 ;;
        *) SALT_FILE="$1"; shift ;;
    esac
done

if [[ -z "$SALT_FILE" ]]; then
    echo "Usage: $0 [--compile-only] [--verbose] [--bridge file.c] <file.salt>"
    exit 1
fi

# Derive output names from input
BASENAME=$(basename "$SALT_FILE" .salt)
TMP_DIR="/tmp/salt_build"
mkdir -p "$TMP_DIR"

is_ecs_test=false
is_standalone=false
is_basalt_test=false
is_lettuce_test=false
is_native=false
if [[ "$BASENAME" == *lettuce* ]] || [[ "$SALT_FILE" == *lettuce* ]]; then
    is_lettuce_test=true
fi
if [[ "$BASENAME" == *native* ]] || [[ "$SALT_FILE" == *native* ]]; then
    is_native=true
fi
if [[ "$BASENAME" == *ecs* ]] || [[ "$BASENAME" == *scheduler* ]] || [[ "$BASENAME" == *ipc* ]] || [[ "$BASENAME" == *epoch* ]] || [[ "$SALT_FILE" == *ecs* ]]; then
    is_ecs_test=true
fi
if [[ "$BASENAME" == *chase_lev* ]] || [[ "$BASENAME" == *sliding_window* ]] || [[ "$BASENAME" == "test_aof" ]]; then
    is_standalone=true
fi
if [[ "$BASENAME" == *basalt_kv* ]]; then
    is_basalt_test=true
fi

MLIR_OUT="$TMP_DIR/${BASENAME}.mlir"
OPT_OUT="$TMP_DIR/${BASENAME}.opt.mlir"
LL_OUT="$TMP_DIR/${BASENAME}.ll"
BIN_OUT="$TMP_DIR/${BASENAME}"

# Determine which C bridges to link
BRIDGES=("$SALT_FRONT/runtime.c")

# KeuOS stubs — needed for linking (StringMap, AOF pull in OS primitives)
BRIDGES+=("$PROJECT_ROOT/user/os/facet_os.c")
BRIDGES+=("$PROJECT_ROOT/tests/bridges/ipc_bridge.c")
BRIDGES+=("$PROJECT_ROOT/tests/bridges/mac_stubs.c")

if [[ "$is_native" == true ]]; then
    # macOS-native: override networking with real BSD sockets + kqueue
    BRIDGES+=("$SALT_FRONT/std/net/tcp_native_bridge.c")
elif [[ "$is_lettuce_test" == true ]]; then
    BRIDGES+=("$PROJECT_ROOT/lettuce/tests/dummy_ebr.c")
fi

if [[ "$is_ecs_test" == false ]] && [[ "$is_standalone" == false ]] && [[ "$is_basalt_test" == false ]] && [[ "$is_lettuce_test" == false ]] && [[ "$BENCHMARK_MODE" == false ]] && [[ "$BASENAME" != "test_e2e_integration" ]] && ! grep -q 'sys_exec_capture_stdout' "$SALT_FILE" 2>/dev/null; then
    # Removed JSC bridge logic
fi

BRIDGES+=("$PROJECT_ROOT/vendor/openlibm/libopenlibm.a")

# Add C flags
C_FLAGS_ARR=(-I"$PROJECT_ROOT/vendor/openlibm/include" -I"$PROJECT_ROOT/vendor/openlibm/src" -Wno-implicit-fallthrough -Wno-int-conversion -D_GNU_SOURCE -ffast-math -march=native)

# Auto-detect bridges needed based on imports in the salt file
# Skip for native builds — tcp_native_bridge.c already linked above
if [[ "$is_native" == false ]]; then
    if grep -q 'std\.net\|std\.http\|std\.io\.reactor\|TcpListener\|TcpStream\|Poller\|KqueueReactor\|http_tcp_connect\|salt_http_get' "$SALT_FILE" 2>/dev/null; then
        BRIDGES+=("$PROJECT_ROOT/std/net/http_bridge.c")
    fi
fi

# Detect TLS pipeline bridge (BearSSL)
if grep -q 'netd_tls_' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/user/netd/tls_bridge.c")
    BRIDGES+=("$PROJECT_ROOT/vendor/bearssl/build/libbearssl.a")
    C_FLAGS_ARR+=(-I"$PROJECT_ROOT/vendor/bearssl/inc")
fi

if [[ "$is_ecs_test" == false ]] && [[ "$is_standalone" == false ]] && [[ "$is_basalt_test" == false ]] && [[ "$is_lettuce_test" == false ]] && [[ "$BENCHMARK_MODE" == false ]]; then
    # Removed font bridge logic
fi

# Detect Facet Window bridge
LD_FLAGS=(-lm -framework JavaScriptCore)
if [[ "$is_ecs_test" == false ]] && [[ "$is_standalone" == false ]] && [[ "$is_basalt_test" == false ]] && [[ "$is_lettuce_test" == false ]] && [[ "$BENCHMARK_MODE" == false ]] && grep -q 'facet_window_open' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/user/facet/window/facet_window.m")
    LD_FLAGS+=("-framework" "Cocoa" "-framework" "CoreGraphics" "-fobjc-arc")
fi

# Detect Facet GPU bridge
if [[ "$is_ecs_test" == false ]] && [[ "$is_standalone" == false ]] && [[ "$is_basalt_test" == false ]] && [[ "$is_lettuce_test" == false ]] && [[ "$BENCHMARK_MODE" == false ]] && [[ "$BASENAME" != "test_e2e_integration" ]] && ! grep -q 'sys_exec_capture_stdout' "$SALT_FILE" 2>/dev/null; then
    if grep -q 'facet_gpu' "$SALT_FILE" 2>/dev/null || grep -q 'facet_gpu' $(dirname "$SALT_FILE")/*.salt 2>/dev/null || grep -q 'facet_gpu' $(dirname "$SALT_FILE")/../*/*.salt 2>/dev/null || grep -q 'facet_window' $(dirname "$SALT_FILE")/../*/*.salt 2>/dev/null; then
        BRIDGES+=("$PROJECT_ROOT/user/facet/gpu/facet_gpu.m")
        BRIDGES+=("$PROJECT_ROOT/user/facet/gpu/facet_window.m")
        BRIDGES+=("$PROJECT_ROOT/user/facet/gpu/facet_image.c")
        LD_FLAGS+=("-framework" "Metal" "-framework" "QuartzCore" "-framework" "Cocoa" "-framework" "VideoToolbox" "-framework" "CoreMedia" "-framework" "CoreVideo" "-framework" "IOSurface" "-fobjc-arc")
    fi
fi

# Detect SPSC/kernel stub bridge (provides volatile_read_i64, cpu_pause, idle_halt)
if [[ "$is_ecs_test" == true ]] || grep -q 'volatile_read_i64\|volatile_write_i64\|cpu_pause' "$SALT_FILE" 2>/dev/null; then
    if [[ -f "$PROJECT_ROOT/tests/bridges/spsc_bridge.c" ]]; then
        BRIDGES+=("$PROJECT_ROOT/tests/bridges/spsc_bridge.c")
    fi
fi

if grep -q 'e2e_execute_pipeline' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/e2e_bridge.c")
fi

if grep -q 'gc_stress_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/gc_bridge.c")
fi

if grep -q 'event_routing_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/event_bridge.c")
fi

if grep -q 'async_fetch_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/async_fetch_bridge.c")
fi

if grep -q 'chronos_e2e_test' "$SALT_FILE" 2>/dev/null; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/chronos_bridge.c")
fi

















# Detect Image decode bridge (stb_image)


if grep -q 'jit_test_bridge_init\|test_e2e_jit_tier' "$SALT_FILE" 2>/dev/null || [[ "$BASENAME" == "test_e2e_jit_tier" ]]; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/jit_bridge.c")
fi








# Block removed

if [[ "$BASENAME" == "test_script_fsm" ]]; then
    BRIDGES+=("$PROJECT_ROOT/tests/test_script_fsm.c")
    LIB_MODE=true
fi

if [[ "$BASENAME" == "test_image_pipeline" ]]; then
    BRIDGES+=("$PROJECT_ROOT/tests/test_image_pipeline.c")
    LIB_MODE=true
fi

if [[ "$BASENAME" == "test_lexer_tree" ]]; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/lexer_tree_bridge.c")
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/integration_bridge.c")
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/mac_stubs.c")
fi

if [[ "$BASENAME" == "test_html_lexer" ]]; then
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/lexer_tree_bridge.c")
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/integration_bridge.c")
    BRIDGES+=("$PROJECT_ROOT/tests/bridges/mac_stubs.c")
fi

# Add explicit bridges
BRIDGES+=("${EXTRA_BRIDGES[@]}")


log() { [[ "$VERBOSE" == true ]] && echo "  → $1" || true; }

# Locate LLVM linker
LLVM_LINK="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin/llvm-link"
LL_FILES=()

if [[ "$BASENAME" == "test_e2e_integration" ]] || grep -q 'sys_exec_capture_stdout' "$SALT_FILE" 2>/dev/null; then
    TEST_DEPS=()
elif [[ "$is_standalone" == true ]]; then
    TEST_DEPS=()
elif [[ "$BENCHMARK_MODE" == true ]]; then
    TEST_DEPS=("std/core/str.salt" "std/time.salt" "std/thread/thread.salt" "user/os/process.salt" "user/os/ipc_ring.salt" "user/netd/virtio_bridge.salt")
elif [[ "$is_basalt_test" == true ]]; then
    TEST_DEPS=("std/core/str.salt" "std/time.salt" "basalt/src/transformer.salt" "basalt/src/kernels.salt" "basalt/src/quant.salt")
elif [[ "$is_lettuce_test" == true ]]; then
    TEST_DEPS=("std/core/str.salt" "std/time.salt" "std/thread/thread.salt" "user/os/process.salt" "user/os/ipc_ring.salt" "user/netd/virtio_bridge.salt" "lettuce/store.salt" "lettuce/resp.salt" "lettuce/aof.salt" "std/fs/fs.salt" "std/simd/mod.salt" "std/collections/string_map.salt")
elif [[ "$is_ecs_test" == true ]]; then
    TEST_DEPS=("std/core/str.salt" "std/time.salt" "std/thread/thread.salt" "kernel/ecs/entity.salt" "kernel/ecs/components.salt" "kernel/ecs/sparse_set.salt" "kernel/ecs/world.salt" "kernel/ecs/ecs_bridge.salt" "kernel/ecs/commands.salt" "kernel/ecs/events.salt" "kernel/ecs/ecs_scheduler.salt" "kernel/ecs/ecs_ipc.salt" "kernel/ecs/ecs_epoch.salt")
else
    TEST_DEPS=("std/core/str.salt" "std/time.salt" "std/thread/thread.salt" "user/os/process.salt" "user/os/ipc_ring.salt" "user/os/worker_ring.salt" "user/netd/virtio_bridge.salt" "user/alloc/arena.salt")
fi

for mod in "${TEST_DEPS[@]}"; do
    dep_path="$PROJECT_ROOT/$mod"
    if [ -f "$dep_path" ]; then
        dep_base=$(basename "$mod" .salt)
        dep_ll="$TMP_DIR/${dep_base}.ll"
        echo "🔧 [LLVM] Compiling ${mod}..."
        if [[ "$NO_VERIFY" == true ]]; then
            "$SALT_FRONT/target/release/saltc" "$dep_path" --danger-no-verify --lib --release -o "${dep_ll}.mlir"
        else
            "$SALT_FRONT/target/release/saltc" "$dep_path" --lib --release -o "${dep_ll}.mlir"
        fi
        # Fix MLIR f32 literal emission: (0 : f32) -> (0. : f32)
        sed -i '' 's/(0 : f32)/(0. : f32)/g' "${dep_ll}.mlir"
        mlir-opt "${dep_ll}.mlir" --allow-unregistered-dialect \
            --canonicalize --cse --loop-invariant-code-motion --sccp --canonicalize --cse \
            --convert-linalg-to-loops \
            --lower-affine --convert-scf-to-cf --convert-vector-to-llvm \
            --expand-strided-metadata --finalize-memref-to-llvm \
            --convert-cf-to-llvm --convert-arith-to-llvm --convert-math-to-llvm \
            --convert-func-to-llvm --reconcile-unrealized-casts -o "${dep_ll}.opt"
        sed -i '' '/"salt.verify"/d' "${dep_ll}.opt"
        mlir-translate --mlir-to-llvmir "${dep_ll}.opt" -o "$dep_ll"
        
        # Patch MLIR-generated globals to weak_odr for multi-file linking
        sed -i '' 's/internal global/weak_odr global/g' "$dep_ll"
        sed -i '' 's/define internal/define weak_odr/g' "$dep_ll"
        sed -i '' 's/define ptr/define weak_odr ptr/g' "$dep_ll"
        sed -i '' 's/define void/define weak_odr void/g' "$dep_ll"
        sed -i '' 's/define i64/define weak_odr i64/g' "$dep_ll"
        sed -i '' 's/define i32/define weak_odr i32/g' "$dep_ll"
        sed -i '' 's/define i16/define weak_odr i16/g' "$dep_ll"
        sed -i '' 's/define i8/define weak_odr i8/g' "$dep_ll"
        sed -i '' 's/define i1/define weak_odr i1/g' "$dep_ll"
        sed -i '' 's/define %/define weak_odr %/g' "$dep_ll"
        sed -i '' 's/= global /= weak_odr global /g' "$dep_ll"
        sed -i '' '/target triple =/d' "$dep_ll"
        sed -i '' '/target datalayout =/d' "$dep_ll"
        
        LL_FILES+=("$dep_ll")
    fi
done

# Step 1: saltc → MLIR
log "saltc → MLIR"
if [[ "$LIB_MODE" == true ]]; then
    if [[ "$NO_VERIFY" == true ]]; then
        "$SALT_FRONT/target/release/saltc" "$SALT_FILE" --danger-no-verify --lib --release -o "$MLIR_OUT"
    else
        "$SALT_FRONT/target/release/saltc" "$SALT_FILE" --lib --release -o "$MLIR_OUT"
    fi
else
    if [[ "$NO_VERIFY" == true ]]; then
        "$SALT_FRONT/target/release/saltc" "$SALT_FILE" --danger-no-verify --release -o "$MLIR_OUT"
    else
        "$SALT_FRONT/target/release/saltc" "$SALT_FILE" --release -o "$MLIR_OUT"
    fi
fi
echo "  ✓ MLIR generated"

# Fix MLIR f32 literal emission: (0 : f32) -> (0. : f32)
sed -i '' 's/(0 : f32)/(0. : f32)/g' "$MLIR_OUT"

# Step 2: mlir-opt (lowering passes)
log "mlir-opt → optimized MLIR"
mlir-opt "$MLIR_OUT" \
    --allow-unregistered-dialect \
    --canonicalize --cse --loop-invariant-code-motion --sccp --canonicalize --cse \
    --convert-linalg-to-loops \
	    --lower-affine \
    --convert-scf-to-cf \
    --convert-vector-to-llvm \
    --expand-strided-metadata \
    --finalize-memref-to-llvm \
    --convert-cf-to-llvm \
    --convert-arith-to-llvm \
    --convert-math-to-llvm \
    --convert-func-to-llvm \
    --reconcile-unrealized-casts \
    -o "$OPT_OUT"
echo "  ✓ MLIR optimized"

# Step 3: Strip salt.verify ops (no LLVM lowering for verification dialect)
sed -i '' '/"salt.verify"/d' "$OPT_OUT"

# Step 4: mlir-translate → LLVM IR
log "mlir-translate → LLVM IR"
mlir-translate --mlir-to-llvmir "$OPT_OUT" -o "$LL_OUT"
sed -i '' 's/internal global/weak_odr global/g' "$LL_OUT"
sed -i '' 's/define internal/define weak_odr/g' "$LL_OUT"
sed -i '' 's/= global \[/= weak_odr global \[/g' "$LL_OUT"
sed -i '' 's/= global i/= weak_odr global i/g' "$LL_OUT"
sed -i '' '/target triple =/d' "$LL_OUT"
sed -i '' '/target datalayout =/d' "$LL_OUT"
echo "  ✓ LLVM IR generated"

# Step 4.5: llvm-link
log "llvm-link → merging dependencies"
echo "MERGING ${#LL_FILES[@]} FILES"
MERGED_LL="$TMP_DIR/${BASENAME}_merged.ll"
"$LLVM_LINK" -o "$MERGED_LL" "$LL_OUT" "${LL_FILES[@]}"

# Step 5: clang → native binary
log "clang → binary"
# Note: ${LD_FLAGS[@]} splits correctly in zsh/bash
/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin/clang -O3 "${C_FLAGS_ARR[@]}" "$MERGED_LL" "${BRIDGES[@]}" -o "$BIN_OUT" "${LD_FLAGS[@]}"
echo "  ✓ Binary linked: $BIN_OUT"

# Step 5: Execute
if [[ "$COMPILE_ONLY" == false ]]; then
    echo ""
    echo "--- Running $BASENAME ---"
    "$BIN_OUT"
    EXIT_CODE=$?
    echo ""
    echo "--- Exit code: $EXIT_CODE ---"
    exit $EXIT_CODE
fi
