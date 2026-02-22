#!/bin/bash
# =============================================================================
# Run benchmarks on persistent instance. SCP kernel.elf + run QEMU with KVM.
# Takes ~2 seconds per iteration.
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$ROOT/tools/cloud/cloud_config.sh"

GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

STATE_FILE="$ROOT/.bench_instance"
KERNEL_ELF="$ROOT/qemu_build/kernel.elf"

if [ ! -f "$STATE_FILE" ]; then
    echo -e "${RED}No instance running. Run bench_launch.sh first.${NC}"
    exit 1
fi
source "$STATE_FILE"

if [ ! -f "$KERNEL_ELF" ]; then
    echo -e "${RED}$KERNEL_ELF not found. Build locally first.${NC}"
    exit 1
fi

SSH_OPTS="-i $EC2_KEY_PATH -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"
BENCH_TIMEOUT=${BENCHMARK_TIMEOUT:-300}

echo -e "${CYAN}Uploading kernel.elf ($(du -h "$KERNEL_ELF" | cut -f1))...${NC}"
scp $SSH_OPTS "$KERNEL_ELF" "${EC2_USER}@${INSTANCE_IP}:~/kernel.elf"

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
    -netdev user,id=net0,hostfwd=udp::5555-:7 \
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
