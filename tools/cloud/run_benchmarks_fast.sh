#!/bin/bash
# =============================================================================
# Lightweight KVM Benchmark Deploy
# =============================================================================
# Ships only the pre-built kernel.elf (147KB) to an EC2 bare-metal instance.
# No toolchain needed — just QEMU + KVM.
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$ROOT/tools/cloud/cloud_config.sh"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

KERNEL_ELF="$ROOT/qemu_build/kernel.elf"
RUNNER_PY="$ROOT/tools/runner_qemu.py"

if [ ! -f "$KERNEL_ELF" ]; then
    echo -e "${RED}ERROR: $KERNEL_ELF not found. Build locally first.${NC}"
    exit 1
fi

# ─── Cleanup on exit ─────────────────────────────────────────────
cleanup() {
    if [ -n "${INSTANCE_ID:-}" ]; then
        echo ""
        echo "Terminating instance $INSTANCE_ID..."
        aws ec2 terminate-instances --instance-ids "$INSTANCE_ID" \
            --region "$AWS_REGION" --output text > /dev/null 2>&1 || true
        echo -e "✓ Instance terminated"
    fi
}
trap cleanup EXIT

ssh_cmd() {
    ssh -i "$EC2_KEY_PATH" -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
        -o ConnectTimeout=10 "${EC2_USER}@${INSTANCE_IP}" "$@"
}

echo -e "${CYAN}╔════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║  Lattice KVM Benchmarks — Lightweight Deploy (ELF only) ║${NC}"
echo -e "${CYAN}╚════════════════════════════════════════════════════════╝${NC}"
echo ""

# ─── Step 1: Launch instance ─────────────────────────────────────
USE_ON_DEMAND=false
if [[ "${1:-}" == "--on-demand" ]]; then
    USE_ON_DEMAND=true
fi

echo -e "${YELLOW}[1/5]${NC} Launching $EC2_INSTANCE_TYPE instance..."
if $USE_ON_DEMAND; then
    echo "  Using on-demand pricing"
    INSTANCE_ID=$(aws ec2 run-instances \
        --region "$AWS_REGION" \
        --image-id "$EC2_AMI" \
        --instance-type "$EC2_INSTANCE_TYPE" \
        --key-name "$EC2_KEY_NAME" \
        --security-groups "$EC2_SECURITY_GROUP" \
        --query 'Instances[0].InstanceId' \
        --output text)
else
    echo "  Using spot pricing (use --on-demand to skip)"
    INSTANCE_ID=$(aws ec2 run-instances \
        --region "$AWS_REGION" \
        --image-id "$EC2_AMI" \
        --instance-type "$EC2_INSTANCE_TYPE" \
        --key-name "$EC2_KEY_NAME" \
        --security-groups "$EC2_SECURITY_GROUP" \
        --instance-market-options "MarketType=spot,SpotOptions={MaxPrice=$EC2_MAX_SPOT_PRICE,SpotInstanceType=one-time}" \
        --query 'Instances[0].InstanceId' \
        --output text)
fi
echo -e "  ✓ Instance launched: ${INSTANCE_ID}"

# ─── Step 2: Wait for running ────────────────────────────────────
echo -e "${YELLOW}[2/5]${NC} Waiting for instance to be running..."
aws ec2 wait instance-running --instance-ids "$INSTANCE_ID" --region "$AWS_REGION"
INSTANCE_IP=$(aws ec2 describe-instances \
    --instance-ids "$INSTANCE_ID" --region "$AWS_REGION" \
    --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
echo -e "  ✓ Instance running at ${INSTANCE_IP}"

echo -n "  Waiting for SSH..."
for i in $(seq 1 60); do
    if ssh_cmd "true" 2>/dev/null; then
        echo ""
        echo -e "  ✓ SSH connected"
        break
    fi
    echo -n "."
    sleep 5
done

# ─── Step 3: Install QEMU only ───────────────────────────────────
echo -e "${YELLOW}[3/5]${NC} Installing QEMU..."
ssh_cmd "sudo apt-get update -qq && sudo apt-get install -y -qq qemu-system-x86 python3 > /dev/null 2>&1"
ssh_cmd "sudo usermod -aG kvm ubuntu && sudo chmod 666 /dev/kvm" 2>/dev/null || true
echo -e "  ✓ QEMU installed"

# ─── Step 4: Ship ELF + runner ───────────────────────────────────
echo -e "${YELLOW}[4/5]${NC} Uploading kernel.elf ($(du -h "$KERNEL_ELF" | cut -f1))..."
scp -i "$EC2_KEY_PATH" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o LogLevel=ERROR "$KERNEL_ELF" "${EC2_USER}@${INSTANCE_IP}:~/kernel.elf"
echo -e "  ✓ kernel.elf uploaded"

# ─── Step 5: Run benchmarks with KVM ─────────────────────────────
echo -e "${YELLOW}[5/5]${NC} Running benchmarks with KVM..."
echo ""
echo -e "${CYAN}────────────────── BENCHMARK OUTPUT ──────────────────${NC}"
echo ""

BENCH_TIMEOUT=${BENCHMARK_TIMEOUT:-300}

# Run QEMU on the remote, capturing all output to a file.
# SSH pipe buffering can swallow serial output, so we capture remotely and retrieve.
ssh_cmd "timeout $BENCH_TIMEOUT qemu-system-x86_64 \
    -kernel ~/kernel.elf \
    -nographic \
    -m 1G \
    -enable-kvm \
    -cpu host \
    -d guest_errors \
    -D /tmp/qemu.log \
    -no-reboot \
    -serial stdio \
    -monitor none \
    -device virtio-net-pci,netdev=net0 \
    -netdev user,id=net0,hostfwd=udp::5555-:7 \
    > ~/bench_output.txt 2>&1" || true

# Retrieve and display the output
echo -e "${GREEN}  QEMU exited, retrieving results...${NC}"
BENCH_OUTPUT=$(ssh_cmd "cat ~/bench_output.txt" 2>/dev/null || echo "Failed to retrieve output")
echo "$BENCH_OUTPUT"

echo ""
echo -e "${CYAN}──────────────────────────────────────────────────────${NC}"

if echo "$BENCH_OUTPUT" | grep -q "BENCHMARK SUITE COMPLETE"; then
    echo -e "${GREEN}BENCHMARK SUITE COMPLETE${NC}"
else
    echo -e "${RED}BENCHMARK SUITE DID NOT COMPLETE${NC}"
fi

echo ""
echo -e "${GREEN}Results saved. Instance will be terminated on exit.${NC}"
