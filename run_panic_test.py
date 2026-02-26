#!/usr/bin/env python3
import sys, subprocess, time

elf = sys.argv[1]

cmd = [
    "qemu-system-x86_64", "-kernel", elf,
    "-nographic", "-m", "512M",
    "-cpu", "qemu64,+fxsr,+mmx,+sse,+sse2,+xsave",
    "-no-reboot", "-serial", "mon:stdio",
]

proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, errors="replace", bufsize=1)
start = time.time()
lines = []
success = False

try:
    while time.time() - start < 5:
        c = proc.stdout.read(1)
        if c:
            sys.stdout.write(c)
            sys.stdout.flush()
            lines.append(c)
            line_so_far = "".join(lines)
            if "[FATAL] DOUBLE FAULT (#DF)" in line_so_far:
                success = True
                break
            if "#DF!" in line_so_far:
                success = True
                break
        if proc.poll() is not None:
            break
except KeyboardInterrupt:
    pass

proc.terminate()
try:
    proc.wait(timeout=3)
except:
    proc.kill()

if success:
    print("\nTEST PASSED: Verified isolated Double Fault Panic.")
    sys.exit(0)
else:
    print("\nTEST FAILED: Did not see terminal #DF trace.")
    sys.exit(1)
