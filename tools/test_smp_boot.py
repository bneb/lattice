#!/usr/bin/env python3
"""SMP AP Boot Test - PASS if serial output contains 'AP ALIVE!'"""
import subprocess, sys, os, signal, time

TIMEOUT = 30
cmd = ['qemu-system-x86_64', '-kernel', 'qemu_build/kernel.elf', '-nographic',
       '-m', '1G', '-cpu', 'qemu64,+fxsr,+mmx,+sse,+sse2,+xsave',
       '-smp', '4', '-no-reboot', '-serial', 'mon:stdio',
       '-machine', 'q35']

proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                        preexec_fn=os.setsid)
start = time.time()
try:
    out, _ = proc.communicate(timeout=TIMEOUT)
except subprocess.TimeoutExpired:
    os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
    out, _ = proc.communicate(timeout=5)

text = out.decode('utf-8', errors='replace')

diag = {}
for c in 'ABCDE123FGHI!?':
    diag[c] = sum(1 for b in out if b == ord(c))

alive = text.count('ALIVE! GS_BASE loaded.')
sched_entries = text.count('entering scheduler...')
marker_7f = sum(1 for b in out if b == 0x7F)
marker_7e = sum(1 for b in out if b == 0x7E)
marker_7d = sum(1 for b in out if b == 0x7D)
elapsed = time.time() - start

print(f'=== SMP AP BOOT TEST ({elapsed:.1f}s) ===')
print(f'Diagnostics: {diag}')
m = {0x7F: '16-bit', 0x01: '32-bit', 0x02: '64-bit',
     0x03: 'stack OK', 0x04: 'entry OK', 0x0A: 'entry valid 0xFF',
     0x05: 'pre-lidt', 0x06: 'post-lidt', 0x07: 'post-lgdt', 
     0x08: 'pre-jump', 0x7E: 'ap_entry', 0x7D: 'push rbp',
     0xFE: 'BAD STACK', 0xFD: 'BAD ENTRY', 0xFC: 'ENTRY PTR GARBAGE'}
for byte_val, label in m.items():
    cnt = sum(1 for b in out if b == byte_val)
    if cnt > 0:
        print(f'  0x{byte_val:02X} ({label}): {cnt}')
print(f'AP ALIVE count: {alive} (expected 3)')
print(f'AP scheduler entries: {sched_entries} (expected 3)')
passed = alive == 3 and sched_entries == 3
print(f'RESULT: {"PASS" if passed else "FAIL"}')
print()
for line in text.split('\n'):
    for k in ['SMP', 'AP ', 'ALIVE', 'TEST', 'PASS', 'FAIL', 'Layer', 'online', 'TIMEOUT', '0x8000']:
        if k in line:
            print(line[:120].encode('ascii', 'replace').decode('ascii'))
            break

sys.exit(0 if passed else 1)
