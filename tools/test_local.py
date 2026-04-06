#!/usr/bin/env python3
"""Quick QEMU test launcher with unbuffered output and heartbeat."""
import subprocess, sys, os, time

sys.stdout.reconfigure(line_buffering=True)  # Force line-buffered stdout

KERNEL = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "qemu_build", "kernel.elf")
QEMU = "/opt/homebrew/bin/qemu-system-x86_64"
LOG = "/tmp/qemu_test.log"

if not os.path.exists(KERNEL):
    print(f"ERROR: {KERNEL} not found", flush=True)
    sys.exit(1)

subprocess.run(['pkill', '-f', 'qemu-system'], capture_output=True)

cmd = [
    QEMU,
    '-kernel', KERNEL,
    '-nographic',
    '-m', '1G',
    '-cpu', 'qemu64,+fxsr,+mmx,+sse,+sse2,+xsave,+pcid,+invpcid',
    '-smp', '1',
    '-d', 'guest_errors',
    '-D', LOG,
    '-no-reboot',
    '-serial', 'mon:stdio',
    '-device', 'virtio-net-pci,netdev=net0',
    '-netdev', 'user,id=net0,hostfwd=udp::5555-:5555',
]

print(f"QEMU launching...", flush=True)
proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)

start = time.time()
lines = []
try:
    while True:
        line = proc.stdout.readline()
        if not line:
            break
        line = line.rstrip()
        lines.append(line)
        elapsed = int(time.time() - start)
        # Print every line that matters
        if any(k in line for k in ["TEST:", "BENCH:", "SUITE", "socket", "VERDICT", "KeuOS", "[TEST]"]):
            print(f"[{elapsed}s] {line}", flush=True)
        # Heartbeat every 50 lines
        elif len(lines) % 50 == 0:
            print(f"[{elapsed}s] ...{len(lines)} lines... last: {line[:60]}", flush=True)
        # Terminate when suite completes
        if "BENCHMARK SUITE COMPLETE" in line:
            proc.kill()
            break
except KeyboardInterrupt:
    proc.kill()

elapsed = int(time.time() - start)
print(f"\nDone in {elapsed}s, {len(lines)} total lines", flush=True)
