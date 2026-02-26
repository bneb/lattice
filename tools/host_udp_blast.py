#!/usr/bin/env python3
# =============================================================================
# tools/host_udp_blast.py
# Lattice C10M Live Traffic Smoke Test — Host-Side UDP Blaster
# =============================================================================
#
# Sends exactly 1,000 UDP datagrams to the guest VM via QEMU's SLIRP port
# forwarding (host:5555 → guest:5555).
#
# The guest-side VirtIO driver receives these frames, pushes them into
# NetD's SPSC ring, and NetD increments a counter. When the counter
# hits 1,000, the guest prints a success banner.
#
# Usage:
#   1. Start the kernel: python3 tools/runner_qemu.py bench
#   2. In another terminal: python3 tools/host_udp_blast.py
#
# "Aerospace Brutalism": no dependencies, no heap allocation, no guessing.
# =============================================================================

import socket
import time
import sys

HOST = "127.0.0.1"
PORT = 5555
TOTAL_FRAMES = 1000
PAYLOAD = b"LATTICE_C10M_TEST"

def main():
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

    print("=" * 60)
    print("  LATTICE C10M LIVE TRAFFIC SMOKE TEST")
    print(f"  Target: {HOST}:{PORT}")
    print(f"  Frames: {TOTAL_FRAMES}")
    print(f"  Payload: {PAYLOAD.decode()} ({len(PAYLOAD)} bytes)")
    print("=" * 60)

    start = time.monotonic()

    for i in range(TOTAL_FRAMES):
        sock.sendto(PAYLOAD, (HOST, PORT))
        # 1ms pacing to prevent host-side kernel UDP buffer drops
        # before QEMU SLIRP can process them
        if i % 100 == 99:
            time.sleep(0.01)

    elapsed = time.monotonic() - start
    pps = TOTAL_FRAMES / elapsed if elapsed > 0 else 0

    print()
    print(f"  Sent {TOTAL_FRAMES} UDP datagrams in {elapsed:.3f}s")
    print(f"  Host-side PPS: {pps:,.0f}")
    print()
    print("  Check guest serial output for:")
    print("    [NetD] LIVE_TRAFFIC_TEST: 1000/1000 RECEIVED")
    print("=" * 60)

    sock.close()

if __name__ == "__main__":
    main()
