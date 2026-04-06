#!/usr/bin/env zsh
# =============================================================================
# bench_baremetal.sh — Run KeuOS Benchmarks on UM890 Pro (Bare Metal)
# =============================================================================
#
# Drives benchmark execution on the UM890 Pro over SSH. No hypervisor noise.
# Results are saved locally to .bench_basalt/results_um890.txt
#
# Usage:
#   ./scripts/bench_baremetal.sh --host 192.168.1.x
#   ./scripts/bench_baremetal.sh --host um890.local --user kevin --runs 40
#   ./scripts/bench_baremetal.sh --host um890.local --basalt-only
#
# Benchmark modes:
#   --basalt-only   LLM inference (Basalt vs llama2.c) — runs in Linux userspace,
#                   no bare metal boot needed. Run this FIRST.
#   --kernel-only   Kernel benchmarks (syscall, IPC, ctx-switch, slab, SMP, NetD)
#                   Requires KeuOS to be booted on bare metal.
#
# =============================================================================

set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"

# ── Defaults ──────────────────────────────────────────────────────────────────
HOST=""
USER="ubuntu"
RUNS=40
BASALT_ONLY=false
KERNEL_ONLY=false
REMOTE_REPO="~/keuos"

# ── Arg parsing ───────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)         HOST="$2";        shift 2 ;;
        --user)         USER="$2";        shift 2 ;;
        --runs)         RUNS="$2";        shift 2 ;;
        --remote-repo)  REMOTE_REPO="$2"; shift 2 ;;
        --basalt-only)  BASALT_ONLY=true; shift ;;
        --kernel-only)  KERNEL_ONLY=true; shift ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

if [[ -z "$HOST" ]]; then
    echo "❌  --host is required"
    echo "    Usage: $0 --host <ip-or-hostname> [--user ubuntu] [--runs 40]"
    exit 1
fi

SSH_TARGET="$USER@$HOST"
SSH_OPTS=(-o StrictHostKeyChecking=accept-new -o ConnectTimeout=10)
RESULTS_DIR="$PROJECT_ROOT/.bench_basalt"
RESULTS_FILE="$RESULTS_DIR/results_um890.txt"
mkdir -p "$RESULTS_DIR"

echo "╔══════════════════════════════════════════════════════╗"
echo "║   KeuOS Bare Metal Benchmark — UM890 Pro           ║"
echo "║   AMD Ryzen 9 8945HS (Zen 4, x86_64)                ║"
echo "║   Target: $SSH_TARGET"
echo "║   Runs:   $RUNS"
echo "╚══════════════════════════════════════════════════════╝"
echo ""

# ── Verify SSH ────────────────────────────────────────────────────────────────
echo "  Checking SSH connectivity..."
if ! ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "echo ok" &>/dev/null; then
    echo "❌  Cannot reach $SSH_TARGET"
    exit 1
fi
echo "  [✓] SSH OK"

# ── Detect HW info on UM890 ───────────────────────────────────────────────────
REMOTE_HW=$(ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "
    CPU=\$(grep 'model name' /proc/cpuinfo | head -1 | cut -d: -f2 | xargs)
    CORES=\$(nproc)
    MEM=\$(free -h | awk '/Mem:/{print \$2}')
    OS=\$(lsb_release -ds 2>/dev/null || cat /etc/os-release | grep PRETTY | cut -d= -f2 | tr -d '\"')
    KERNEL_VER=\$(uname -r)
    echo \"CPU: \$CPU\"
    echo \"Cores: \$CORES\"
    echo \"RAM: \$MEM\"
    echo \"OS: \$OS\"
    echo \"Kernel: \$KERNEL_VER\"
")
echo ""
echo "  Remote hardware:"
echo "$REMOTE_HW" | sed 's/^/    /'
echo ""

# =============================================================================
# BASALT BENCHMARK (LLM inference — userspace, no kernel boot needed)
# =============================================================================
run_basalt_benchmark() {
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  🧠 Basalt LLM Inference Benchmark (Salt vs llama2.c)"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    # ── Step 1: Build basalt.ll on the Mac using Homebrew mlir-opt ───────────
    # mlir-opt is not available from apt.llvm.org on Ubuntu 24.04, so we
    # generate the LLVM IR here on the Mac and let clang-21 on Zen 4 do the
    # final -march=native compile. This gives genuine native code without
    # needing mlir-opt on the remote machine.
    LLVM_VERSION="${LLVM_VERSION:-21}"
    export PATH="/opt/homebrew/opt/llvm@${LLVM_VERSION}/bin:$PATH"

    LOCAL_TMP="/tmp/salt_build_baremetal"
    mkdir -p "$LOCAL_TMP"
    LOCAL_LL="$LOCAL_TMP/basalt.ll"
    SALT_FRONT="$PROJECT_ROOT/salt-front"

    if [[ ! -f "$LOCAL_LL" ]]; then
        echo "  Building basalt.ll on Mac (mlir pipeline)..."
        COMBINED="$LOCAL_TMP/basalt_combined.salt"
        echo "// Auto-generated" > "$COMBINED"
        echo "package main" >> "$COMBINED"
        echo "use std.core.ptr.Ptr" >> "$COMBINED"
        echo "" >> "$COMBINED"
        for f in basalt/src/kernels.salt basalt/src/sampler.salt basalt/src/quant.salt \
                  basalt/src/transformer.salt basalt/src/model_loader.salt \
                  basalt/src/tokenizer.salt basalt/src/main.salt; do
            echo "// ---- Module: $(basename "$f") ----" >> "$COMBINED"
            grep -v "^package " "$PROJECT_ROOT/$f" | grep -v "^use basalt\." >> "$COMBINED"
        done

        "$SALT_FRONT/target/release/salt-front" "$COMBINED" > "$LOCAL_TMP/basalt.mlir"

        mlir-opt "$LOCAL_TMP/basalt.mlir" \
            --allow-unregistered-dialect \
            --canonicalize --cse --loop-invariant-code-motion --sccp --canonicalize --cse \
            --lower-affine \
            --convert-scf-to-cf --convert-vector-to-llvm --convert-cf-to-llvm \
            --convert-arith-to-llvm --convert-math-to-llvm --convert-func-to-llvm \
            --reconcile-unrealized-casts \
            -o "$LOCAL_TMP/basalt.opt.mlir"
        sed -i '' '/"salt.verify"/d' "$LOCAL_TMP/basalt.opt.mlir"
        mlir-translate --mlir-to-llvmir "$LOCAL_TMP/basalt.opt.mlir" -o "$LOCAL_LL"
        echo "  [✓] basalt.ll generated ($(wc -c < "$LOCAL_LL" | tr -d ' ') bytes)"
    else
        echo "  [✓] basalt.ll already cached at $LOCAL_LL"
    fi
    echo ""

    # ── Step 2: Rsync essential files to remote ───────────────────────────────
    echo "  Syncing to $SSH_TARGET:$REMOTE_REPO..."
    ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "mkdir -p $REMOTE_REPO/salt-front $REMOTE_REPO/.bench_basalt $REMOTE_REPO/scripts"
    rsync -az -e "ssh ${SSH_OPTS[*]}" \
        "$PROJECT_ROOT/salt-front/runtime.c" \
        "$SSH_TARGET:$REMOTE_REPO/salt-front/"
    rsync -az -e "ssh ${SSH_OPTS[*]}" \
        "$PROJECT_ROOT/.bench_basalt/" \
        --exclude='results*' \
        "$SSH_TARGET:$REMOTE_REPO/.bench_basalt/"
    ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "mkdir -p /tmp/salt_build_linux"
    rsync -az -e "ssh ${SSH_OPTS[*]}" \
        "$LOCAL_LL" \
        "$SSH_TARGET:/tmp/salt_build_linux/basalt.ll"
    rsync -az -e "ssh ${SSH_OPTS[*]}" \
        "$PROJECT_ROOT/scripts/build_linux.sh" \
        "$SSH_TARGET:$REMOTE_REPO/scripts/build_linux.sh"
    echo "  [✓] Synced"
    echo ""

    # ── Step 3: Native compile on UM890 (Zen 4, AVX-512) ─────────────────────
    echo "  Compiling Basalt on UM890 (clang-21 -O3 -march=native)..."
    ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "
        set -e
        cd $REMOTE_REPO
        chmod +x scripts/build_linux.sh
        ./scripts/build_linux.sh --basalt-from-ll /tmp/salt_build_linux/basalt.ll
    "
    echo "  [✓] UM890 native binary ready"
    echo ""

    # ── Step 4: Run benchmark ─────────────────────────────────────────────────
    echo "  Running $RUNS-iteration benchmark on UM890..."
    echo ""

    BENCH_OUTPUT=$(ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "
        set -e
        cd $REMOTE_REPO
        MODEL=.bench_basalt/models/stories15M.bin
        TOK=.bench_basalt/models/tokenizer.bin
        OUT=/tmp/salt_build_linux
        C_SRC=.bench_basalt/llama2.c/run.c

        CLANG=clang-21
        command -v \$CLANG >/dev/null 2>&1 || CLANG=clang
        \$CLANG -O3 -ffast-math -march=native \"\$C_SRC\" -o /tmp/llama2c_um890 -lm

        C_BEST=0
        printf 'C:    '
        for i in \$(seq 1 $RUNS); do
            OUT_LINE=\$(/tmp/llama2c_um890 \"\$MODEL\" -z \"\$TOK\" -n 256 2>&1)
            TOKS=\$(echo \"\$OUT_LINE\" | grep -oE 'achieved tok/s: [0-9.]+' | awk '{print \$NF}')
            TOKS_INT=\${TOKS%.*}
            [ \"\$TOKS_INT\" -gt \"\$C_BEST\" ] && C_BEST=\$TOKS_INT
            printf '%4d ' \$TOKS_INT
        done
        echo ''
        echo \"      Best: \$C_BEST tok/s\"

        SALT_BEST=0
        printf 'Salt: '
        for i in \$(seq 1 $RUNS); do
            OUT_LINE=\$(\$OUT/basalt \"\$MODEL\" \"\$TOK\" 2>&1)
            TOKS=\$(echo \"\$OUT_LINE\" | grep 'tok/s:' | awk '{print \$2}')
            TOKS_INT=\${TOKS%.*}
            [ \"\$TOKS_INT\" -gt \"\$SALT_BEST\" ] && SALT_BEST=\$TOKS_INT
            printf '%4d ' \$TOKS_INT
        done
        echo ''
        echo \"      Best: \$SALT_BEST tok/s\"

        if [ \"\$C_BEST\" -gt 0 ]; then
            R100=\$(( SALT_BEST * 100 / C_BEST ))
            printf 'Ratio: Salt/C = %d.%02dx\n' \$(( R100 / 100 )) \$(( R100 % 100 ))
        fi
        echo \"__RESULTS__:\$C_BEST:\$SALT_BEST\"
    ")

    echo "$BENCH_OUTPUT" | grep -v "^__RESULTS__" || true
    echo ""

    RESULT_LINE=$(echo "$BENCH_OUTPUT" | grep "^__RESULTS__:" || echo "__RESULTS__:0:0")
    C_BEST=$(echo "$RESULT_LINE" | cut -d: -f2)
    SALT_BEST=$(echo "$RESULT_LINE" | cut -d: -f3)

    cat > "$RESULTS_FILE" << RESULTS_EOF
Basalt Bare Metal Benchmark Results
$(date -u '+%Y-%m-%d %H:%M:%S') UTC

Hardware: AMD Ryzen 9 8945HS (Zen 4, 8C/16T)
Platform: Minisforum UM890 Pro
OS: Ubuntu (bare metal, no hypervisor)
$REMOTE_HW

Model: stories15M.bin
Tokens: 256
Runs: $RUNS

llama2.c  (-O3 -ffast-math -march=native): ${C_BEST} tok/s
Basalt    (-O3 -ffast-math -march=native): ${SALT_BEST} tok/s
RESULTS_EOF

    if [[ "$C_BEST" -gt 0 ]]; then
        R100=$(( SALT_BEST * 100 / C_BEST ))
        echo "Ratio:    $(( R100 / 100 )).$(printf '%02d' $(( R100 % 100 )))x" >> "$RESULTS_FILE"
    fi

    echo "  Results saved → $RESULTS_FILE"
}

# =============================================================================
# KERNEL BENCHMARK (requires KeuOS booted on bare metal)
# =============================================================================
run_kernel_benchmark() {
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  ⚡ Kernel Benchmarks (syscall / IPC / slab / SMP)"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "  NOTE: Kernel benchmarks require KeuOS to be booted bare metal."
    echo "  The kernel emits all benchmark results on the serial port (COM1)."
    echo ""
    echo "  ── How to capture kernel benchmark output ────────────────"
    echo "  Option A — USB serial adapter (connect Mac to UM890 COM port):"
    echo "    screen /dev/cu.usbserial-* 115200"
    echo ""
    echo "  Option B — From UM890 itself (via SSH while Ubuntu is running):"
    echo "    ssh $SSH_TARGET 'sudo minicom -b 115200 -D /dev/ttyS0'"
    echo ""
    echo "  Option C — socat redirect over SSH:"
    echo "    ssh $SSH_TARGET 'sudo socat /dev/ttyS0,raw,b115200,crnl -' | tee kernel_bench_output.txt"
    echo ""
    echo "  After capturing, look for benchmark sections:"
    echo "    [BENCH] syscall_bench:  <cycles> cycles/call"
    echo "    [BENCH] ipc_fastpath:   <ns> ns/message"
    echo "    [BENCH] ctx_switch:     <cycles> cycles"
    echo "    [BENCH] slab_alloc:     <ns> ns/alloc"
    echo "    [BENCH] smp_bench:      <throughput> ops/s"
    echo "    [BENCH] netd_bench:     <throughput> pps"
    echo ""
    echo "  TODO: Once serial output format is confirmed, automate parsing here."
}

# =============================================================================
# MAIN
# =============================================================================
if $KERNEL_ONLY; then
    run_kernel_benchmark
elif $BASALT_ONLY; then
    run_basalt_benchmark
else
    run_basalt_benchmark
    echo ""
    run_kernel_benchmark
fi

echo ""
echo "════════════════════════════════════════════════════════"
echo "  ✅ Benchmark session complete"
[[ -f "$RESULTS_FILE" ]] && echo "  Results: $RESULTS_FILE"
