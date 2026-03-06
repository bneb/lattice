#!/bin/bash
# Local Fabric Test — QEMU multicast socket backend (no tap/bridge)
# Bypasses Linux kernel networking entirely to isolate VirtIO RX behavior.
set -e

echo "=== Local Fabric Test (Socket Backend) ==="
echo "Cleaning up old logs..."
rm -f /tmp/node_a_local.log /tmp/node_b_local.log

echo "Launching Node B (Expert: 52:54:00:12:34:BB)..."
qemu-system-x86_64 \
    -kernel qemu_build/kernel_expert.elf \
    -display none -m 1G \
    -netdev socket,id=n1,mcast=230.0.0.1:1234 \
    -device virtio-net-pci,netdev=n1,mac=52:54:00:12:34:BB \
    -no-reboot \
    -serial file:/tmp/node_b_local.log &
PID_B=$!
echo "  Node B PID: $PID_B"

# Give Expert a second to boot and enter wait state
sleep 1

echo "Launching Node A (Router: 52:54:00:12:34:AA)..."
qemu-system-x86_64 \
    -kernel qemu_build/kernel_router.elf \
    -display none -m 1G \
    -netdev socket,id=n1,mcast=230.0.0.1:1234 \
    -device virtio-net-pci,netdev=n1,mac=52:54:00:12:34:AA \
    -no-reboot \
    -serial file:/tmp/node_a_local.log &
PID_A=$!
echo "  Node A PID: $PID_A"

echo "Waiting for convergence (10 seconds)..."
sleep 10

echo "Killing QEMU instances..."
kill $PID_A $PID_B 2>/dev/null || true
wait $PID_A $PID_B 2>/dev/null || true

echo ""
echo "=== NODE A (Router) ==="
grep -E "Booting Node|MAC byte|DIAG|RX_MOE|MOE_TX|DEADBEEF|Convergence|Sentinel|reclaimed|6775" /tmp/node_a_local.log 2>/dev/null | tail -20 || echo "(no output)"

echo ""
echo "=== NODE B (Expert) ==="
grep -E "Booting Node|MAC byte|DIAG|RX_MOE|MOE_TX|DEADBEEF|Expert|Convergence|Sentinel|6775" /tmp/node_b_local.log 2>/dev/null | tail -20 || echo "(no output)"

echo ""
echo "=== FULL NODE A (last 30 lines) ==="
tail -30 /tmp/node_a_local.log 2>/dev/null || echo "(no output)"

echo ""
echo "=== FULL NODE B (last 30 lines) ==="
tail -30 /tmp/node_b_local.log 2>/dev/null || echo "(no output)"
