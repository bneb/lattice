#!/bin/bash
# =============================================================================
# Run benchmarks on persistent instance. SCP kernel.elf + run QEMU with KVM.
# Supports both single-node and multi-node (fabric) modes.
#
# Usage:
#   ./tools/cloud/bench_run.sh              # Single-node (legacy)
#   ./tools/cloud/bench_run.sh --fabric     # Multi-node fabric (Router + Expert)
#
# Takes ~2 seconds per iteration.
#
# Expected Serial Output (v6.3.0-ExokernelPurity):
#   [KeuOS] Mapped VirtIO RX pool at 0x40000000 (24 pages)
#   [KeuOS] MoE Pipeline spawned
#   [DIAG] EtherType=<decimal>               # Kernel logs EtherType, nothing else
#   [RX] Notified Ring 3: buf=0x... len=...  # Kernel hands off raw buffer
#   [Ring 3] Distributed Convergence via Zero-Copy DMA   # Ring 3 parsed the frame
#   <16-char hex dispatch_tsc>               # e.g. 00000000314B8510
#   <16-char hex end_tsc>                    # e.g. 0000000033230010
#   ACPI shutdown
#
# Failure indicators:
#   - "[RX_MOE]" in output means kernel is still parsing (stale binary)
#   - No "Ring 3" line means frame never reached userspace
#   - No TSC lines means convergence was not detected
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$ROOT/tools/cloud/cloud_config.sh"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

STATE_FILE="$ROOT/.bench_instance"
KERNEL_ROUTER="$ROOT/qemu_build/kernel_router.elf"
KERNEL_EXPERT="$ROOT/qemu_build/kernel_expert.elf"
FABRIC_MODE=false

# Parse arguments
for arg in "$@"; do
    case "$arg" in
        --fabric) FABRIC_MODE=true ;;
    esac
done

if [ ! -f "$STATE_FILE" ]; then
    echo -e "${RED}No instance running. Run bench_launch.sh first.${NC}"
    exit 1
fi
source "$STATE_FILE"

if [ ! -f "$KERNEL_ROUTER" ] || [ ! -f "$KERNEL_EXPERT" ]; then
    echo -e "${RED}Split kernel binaries not found. Build locally first.${NC}"
    exit 1
fi

SSH_OPTS="-i $EC2_KEY_PATH -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"
BENCH_TIMEOUT=${BENCHMARK_TIMEOUT:-300}

echo -e "${CYAN}Uploading kernel_router.elf and kernel_expert.elf...${NC}"
scp $SSH_OPTS "$KERNEL_ROUTER" "${EC2_USER}@${INSTANCE_IP}:~/kernel_router.elf"
scp $SSH_OPTS "$KERNEL_EXPERT" "${EC2_USER}@${INSTANCE_IP}:~/kernel_expert.elf"

# =============================================================================
# Single-Node Mode (Legacy)
# =============================================================================
if [ "$FABRIC_MODE" = false ]; then
    echo -e "${CYAN}Running benchmarks with KVM on $INSTANCE_IP...${NC}"
    echo ""

    ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" "timeout $BENCH_TIMEOUT qemu-system-x86_64 \
        -kernel ~/kernel.elf \
        -nographic \
        -m 1G \
        -enable-kvm \
        -cpu host \
        -d guest_errors,cpu_reset \
        -D /tmp/qemu.log \
        -no-reboot \
        -serial stdio \
        -monitor none \
        -device virtio-net-pci,netdev=net0 \
        -netdev user,id=net0,hostfwd=udp::5555-:5555 \
        > ~/bench_output.txt 2>&1" || true

    echo ""
    echo -e "${CYAN}────────────────── BENCHMARK OUTPUT ──────────────────${NC}"
    ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" "cat ~/bench_output.txt" || echo "Failed to retrieve output"
    echo -e "${CYAN}──────────────────────────────────────────────────────${NC}"

    if ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" "grep -q 'BENCHMARK SUITE COMPLETE' ~/bench_output.txt" 2>/dev/null; then
        echo -e "${GREEN}BENCHMARK SUITE COMPLETE${NC}"
    else
        echo -e "${RED}BENCHMARK SUITE DID NOT COMPLETE${NC}"
        echo ""
        echo -e "${CYAN}QEMU Guest Error Log:${NC}"
        ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" 'cat /tmp/qemu.log 2>/dev/null | tail -50' || echo "  (no log)"
    fi
    exit 0
fi

# =============================================================================
# Multi-Node Fabric Mode (--fabric)
# =============================================================================
MAC_ROUTER="52:54:00:12:34:AA"
MAC_EXPERT="52:54:00:12:34:BB"

echo -e "${CYAN}════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}  Multi-Node Fabric — KVM on $INSTANCE_IP${NC}"
echo -e "${CYAN}  Router: ${MAC_ROUTER}  Expert: ${MAC_EXPERT}${NC}"
echo -e "${CYAN}════════════════════════════════════════════════════════${NC}"
echo ""

# ── Phase 1: Ruthless cleanup + bridge provisioning (single SSH) ────────────
echo -e "${CYAN}[1/4] Setting up Layer 2 bridge...${NC}"
ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" << 'CLEANUP'
set -e

# 1. Ruthless cleanup of previous iteration state
sudo pkill -9 qemu-system-x86_64 || true
sudo ip link set br_keuos down 2>/dev/null || true
sudo ip link delete br_keuos type bridge 2>/dev/null || true
sudo ip link delete tap_a 2>/dev/null || true
sudo ip link delete tap_b 2>/dev/null || true
rm -f /tmp/qemu_node_a.pid /tmp/qemu_node_b.pid
sudo chmod 666 /dev/kvm 2>/dev/null || true
echo "  Cleanup complete (socket backend — no bridge needed)"
CLEANUP

# ── Phase 2: Launch Node B (Expert) in background, then Node A (Router) ────
echo -e "${CYAN}[2/4] Launching Node B (Expert: ${MAC_EXPERT}) in background...${NC}"
ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" \
    "nohup timeout $BENCH_TIMEOUT qemu-system-x86_64 -kernel /home/ubuntu/kernel_expert.elf -display none -m 1G -enable-kvm -cpu host -d guest_errors,cpu_reset -D /tmp/qemu_node_b.log -no-reboot -serial file:/home/ubuntu/bench_output_node_b.txt -monitor none -netdev socket,id=net1,mcast=230.0.0.1:1234 -device virtio-net-pci,netdev=net1,mac=${MAC_EXPERT} > /home/ubuntu/qemu_b_error.log 2>&1 < /dev/null &"
echo "  Node B launched"

# Wait for Node B's VirtIO device to initialize before Router sends
echo "  Waiting for Node B VirtIO initialization..."
ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" 'timeout 10 bash -c "until grep -q \"VirtIO-Net initialized\" /home/ubuntu/bench_output_node_b.txt 2>/dev/null; do sleep 0.1; done"' || echo "  Node B init timeout"

echo -e "${CYAN}[3/4] Launching Node A (Router: ${MAC_ROUTER}) in foreground...${NC}"
ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" "timeout $BENCH_TIMEOUT qemu-system-x86_64 \
    -kernel /home/ubuntu/kernel_router.elf \
    -nographic \
    -m 1G \
    -enable-kvm \
    -cpu host \
    -d guest_errors,cpu_reset \
    -D /tmp/qemu_node_a.log \
    -no-reboot \
    -serial stdio \
    -monitor none \
    -netdev socket,id=net0,mcast=230.0.0.1:1234 \
    -device virtio-net-pci,netdev=net0,mac=${MAC_ROUTER} \
    > ~/bench_output_node_a.txt 2>&1" || true

# ── Phase 3: Kill Node B, tear down bridge, collect telemetry (single SSH) ──
echo ""
echo -e "${CYAN}[4/4] Collecting telemetry and cleaning up...${NC}"
RESULTS=$(ssh $SSH_OPTS "${EC2_USER}@${INSTANCE_IP}" << 'COLLECT'
set -e

# Kill Node B if still running
if [ -f /tmp/qemu_node_b.pid ]; then
    PID=$(cat /tmp/qemu_node_b.pid)
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    rm -f /tmp/qemu_node_b.pid
fi

# Kill any stale QEMU processes
sudo pkill -9 qemu-system-x86_64 2>/dev/null || true

# Emit all telemetry in one shot
echo "===NODE_A_START==="
cat ~/bench_output_node_a.txt 2>/dev/null || echo "(no output)"
echo "===NODE_A_END==="
echo "===NODE_B_START==="
cat ~/bench_output_node_b.txt 2>/dev/null || echo "(no output)"
echo "===NODE_B_END==="

# Verification markers
grep -q 'BENCHMARK SUITE COMPLETE' ~/bench_output_node_a.txt 2>/dev/null && echo "NODE_A_PASS" || echo "NODE_A_FAIL"
grep -q 'BENCHMARK SUITE COMPLETE' ~/bench_output_node_b.txt 2>/dev/null && echo "NODE_B_PASS" || echo "NODE_B_NONE"

# Error logs (only on failure)
if ! grep -q 'BENCHMARK SUITE COMPLETE' ~/bench_output_node_a.txt 2>/dev/null; then
    echo "===LOGS_A_START==="
    cat /tmp/qemu_node_a.log 2>/dev/null | tail -30 || echo "(no log)"
    echo "===LOGS_A_END==="
    echo "===LOGS_B_START==="
    cat /tmp/qemu_node_b.log 2>/dev/null | tail -30 || echo "(no log)"
    echo "===LOGS_B_END==="
fi
COLLECT
)

# ── Display Results (parsed from single SSH response) ───────────────────────
echo ""
echo -e "${CYAN}───────────── NODE A (Router) OUTPUT ─────────────${NC}"
echo "$RESULTS" | sed -n '/===NODE_A_START===/,/===NODE_A_END===/{ /===NODE_/d; p; }'
echo -e "${CYAN}──────────────────────────────────────────────────${NC}"
echo ""
echo -e "${CYAN}───────────── NODE B (Expert) OUTPUT ─────────────${NC}"
echo "$RESULTS" | sed -n '/===NODE_B_START===/,/===NODE_B_END===/{ /===NODE_/d; p; }'
echo -e "${CYAN}──────────────────────────────────────────────────${NC}"

# ── Verify Convergence ──────────────────────────────────────────────────────
echo ""
PASS=true

if echo "$RESULTS" | grep -q "NODE_A_PASS"; then
    echo -e "${GREEN}  ✓ Node A: BENCHMARK SUITE COMPLETE${NC}"
else
    echo -e "${RED}  ✗ Node A: BENCHMARK SUITE DID NOT COMPLETE${NC}"
    PASS=false
fi

if echo "$RESULTS" | grep -q "NODE_B_PASS"; then
    echo -e "${GREEN}  ✓ Node B: BENCHMARK SUITE COMPLETE${NC}"
else
    echo -e "${YELLOW}  ⚠ Node B: No suite marker (Expert may not emit one)${NC}"
fi

echo ""
if [ "$PASS" = true ]; then
    echo -e "${GREEN}════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  DISTRIBUTED CONVERGENCE: PASS${NC}"
    echo -e "${GREEN}════════════════════════════════════════════════════════${NC}"
else
    echo -e "${RED}════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}  DISTRIBUTED CONVERGENCE: FAIL${NC}"
    echo -e "${RED}════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${CYAN}QEMU Guest Error Logs:${NC}"
    echo -e "${CYAN}── Node A ──${NC}"
    echo "$RESULTS" | sed -n '/===LOGS_A_START===/,/===LOGS_A_END===/{ /===LOGS_/d; p; }'
    echo -e "${CYAN}── Node B ──${NC}"
    echo "$RESULTS" | sed -n '/===LOGS_B_START===/,/===LOGS_B_END===/{ /===LOGS_/d; p; }'
fi
