#!/usr/bin/env bash
# =============================================================================
# scripts/run_fabric.sh
# Multi-Node Fabric — TAP Substrate Orchestration
# =============================================================================
#
# Launches two freestanding KeuOS instances on a shared TAP bridge:
#   Node A (Router): MAC 52:54:00:12:34:AA
#   Node B (Expert): MAC 52:54:00:12:34:BB
#
# Prerequisites:
#   - Linux host with tun/tap support
#   - sudo privileges for TAP creation
#   - keuos.bin built in build/ directory
#
# Usage:
#   sudo ./scripts/run_fabric.sh
#
# =============================================================================

set -euo pipefail

KERNEL="${1:-build/keuos.bin}"
TAP_A="ltap0"
TAP_B="ltap1"
BRIDGE="lbr0"

MAC_ROUTER="52:54:00:12:34:AA"
MAC_EXPERT="52:54:00:12:34:BB"

PID_A=""
PID_B=""

cleanup() {
    echo ""
    echo "=== Tearing down fabric ==="
    [ -n "$PID_A" ] && kill "$PID_A" 2>/dev/null || true
    [ -n "$PID_B" ] && kill "$PID_B" 2>/dev/null || true
    sleep 0.5
    ip link set "$TAP_A" down 2>/dev/null || true
    ip link set "$TAP_B" down 2>/dev/null || true
    ip link set "$BRIDGE" down 2>/dev/null || true
    brctl delif "$BRIDGE" "$TAP_A" 2>/dev/null || true
    brctl delif "$BRIDGE" "$TAP_B" 2>/dev/null || true
    ip tuntap del dev "$TAP_A" mode tap 2>/dev/null || true
    ip tuntap del dev "$TAP_B" mode tap 2>/dev/null || true
    brctl delbr "$BRIDGE" 2>/dev/null || true
    echo "=== Fabric destroyed ==="
}

trap cleanup EXIT INT TERM

echo "=== Creating Layer 2 fabric ==="

# Create bridge
brctl addbr "$BRIDGE" 2>/dev/null || true
ip link set "$BRIDGE" up

# Create TAP interfaces
ip tuntap add dev "$TAP_A" mode tap
ip tuntap add dev "$TAP_B" mode tap
ip link set "$TAP_A" up
ip link set "$TAP_B" up

# Attach TAPs to bridge
brctl addif "$BRIDGE" "$TAP_A"
brctl addif "$BRIDGE" "$TAP_B"

echo "=== Bridge $BRIDGE ready: $TAP_A <-> $TAP_B ==="

# Verify kernel exists
if [ ! -f "$KERNEL" ]; then
    echo "ERROR: Kernel binary not found: $KERNEL"
    echo "Run './scripts/run_qemu.sh' to build first."
    exit 1
fi

echo "=== Launching Node A (Router: $MAC_ROUTER) ==="
qemu-system-x86_64 \
    -kernel "$KERNEL" \
    -nographic \
    -no-reboot \
    -serial mon:stdio \
    -netdev tap,id=net0,ifname="$TAP_A",script=no,downscript=no \
    -device virtio-net-pci,netdev=net0,mac="$MAC_ROUTER" \
    -m 128M \
    &
PID_A=$!

echo "=== Launching Node B (Expert: $MAC_EXPERT) ==="
qemu-system-x86_64 \
    -kernel "$KERNEL" \
    -nographic \
    -no-reboot \
    -serial stdio \
    -netdev tap,id=net1,ifname="$TAP_B",script=no,downscript=no \
    -device virtio-net-pci,netdev=net1,mac="$MAC_EXPERT" \
    -m 128M

echo "=== Node B exited ==="
wait
