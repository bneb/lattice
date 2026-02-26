#!/usr/bin/env python3
"""QEMU fault debugger for preemptive ABI wrapper development."""

import subprocess
import sys
import re
import signal
import os

def run_qemu_debug(kernel_elf, timeout_sec=5):
    """Run QEMU with -d int and capture interrupt log."""
    cmd = [
        "qemu-system-x86_64",
        "-kernel", kernel_elf,
        "-display", "none",
        "-serial", "stdio",
        "-smp", "4",
        "-m", "64M",
        "-no-reboot",
        "-no-shutdown",
        "-d", "int,cpu_reset",
    ]
    
    try:
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=False,  # binary mode to handle non-utf8
        )
        
        try:
            stdout, stderr = proc.communicate(timeout=timeout_sec)
        except subprocess.TimeoutExpired:
            proc.kill()
            stdout, stderr = proc.communicate()
        
        serial_output = stdout.decode('utf-8', errors='replace')
        debug_output = stderr.decode('utf-8', errors='replace')
        
        return serial_output, debug_output
    except Exception as e:
        print(f"Error: {e}")
        return "", ""

def analyze_exceptions(debug_output):
    """Parse and display exception events from QEMU -d int output."""
    # Find all exception lines
    exc_pattern = re.compile(
        r'(\d+): v=([0-9a-f]+) e=([0-9a-f]+) i=(\d+) cpl=(\d+) '
        r'IP=([0-9a-f]+):([0-9a-f]+) pc=([0-9a-f]+) '
        r'SP=([0-9a-f]+):([0-9a-f]+)'
    )
    
    VECTORS = {
        0: "#DE Divide Error",
        1: "#DB Debug",
        3: "#BP Breakpoint",
        6: "#UD Invalid Opcode",
        7: "#NM No Math Coprocessor",
        8: "#DF Double Fault",
        0xd: "#GP General Protection",
        0xe: "#PF Page Fault",
        0x20: "Timer IRQ (PIT)",
    }
    
    exceptions = []
    lines = debug_output.split('\n')
    for i, line in enumerate(lines):
        m = exc_pattern.search(line)
        if m:
            cpu = int(m.group(1))
            vec = int(m.group(2), 16)
            err = int(m.group(3), 16)
            cs = int(m.group(6), 16)
            rip = int(m.group(7), 16)
            ss = int(m.group(9), 16)
            rsp = int(m.group(10), 16)
            
            vec_name = VECTORS.get(vec, f"Vector 0x{vec:02x}")
            exceptions.append({
                'cpu': cpu, 'vec': vec, 'vec_name': vec_name,
                'err': err, 'cs': cs, 'rip': rip, 'ss': ss, 'rsp': rsp,
                'line_idx': i,
            })
    
    # Find register dumps near exceptions
    reg_pattern = re.compile(
        r'RAX=([0-9a-f]+) RBX=([0-9a-f]+) RCX=([0-9a-f]+) RDX=([0-9a-f]+)'
    )
    
    return exceptions, lines

def main():
    kernel = "qemu_build/kernel.elf"
    if len(sys.argv) > 1:
        kernel = sys.argv[1]
    
    print("=== QEMU Fault Debugger ===")
    print(f"Kernel: {kernel}")
    print(f"Running QEMU with -d int,cpu_reset ...\n")
    
    serial, debug = run_qemu_debug(kernel, timeout_sec=5)
    
    # Show serial tail
    serial_lines = serial.strip().split('\n')
    print("=== Last 15 lines of serial output ===")
    for line in serial_lines[-15:]:
        print(f"  {line}")
    print()
    
    # Analyze exceptions
    exceptions, debug_lines = analyze_exceptions(debug)
    
    # Filter: skip timer IRQs (v=0x20) and NM (v=7) 
    faults = [e for e in exceptions if e['vec'] not in (0x20, 7, 0x30)]
    
    print(f"=== Exception Analysis ({len(faults)} faults, {len(exceptions)} total events) ===")
    for i, exc in enumerate(faults[:10]):
        print(f"\n  Fault #{i}: CPU {exc['cpu']} — {exc['vec_name']}")
        print(f"    IP = {exc['cs']:04x}:{exc['rip']:016x}")
        print(f"    SP = {exc['ss']:04x}:{exc['rsp']:016x}")
        print(f"    Error Code = 0x{exc['err']:04x}")
        
        # Show register dump if available (lines after exception)
        line_idx = exc['line_idx']
        for j in range(line_idx + 1, min(line_idx + 5, len(debug_lines))):
            line = debug_lines[j]
            if 'RAX=' in line or 'R8 =' in line or 'RIP=' in line:
                print(f"    {line.strip()}")
    
    # Check for CPU resets
    resets = debug.count('CPU Reset')
    if resets:
        print(f"\n  ⚠️  {resets} CPU Reset(s) detected (triple fault)")
    
    # Key diagnostic checks
    print("\n=== Diagnostic Summary ===")
    if faults:
        f = faults[0]
        print(f"  FIRST FAULT: {f['vec_name']} on CPU {f['cpu']}")
        print(f"  CS = 0x{f['cs']:04x}", end="")
        if f['cs'] == 0x08:
            print(" ✓ (Kernel Code)")
        elif f['cs'] == 0x18:
            print(" ✗ (User Code DPL=3! — IRETQ loaded wrong CS)")
        else:
            print(f" ✗ (unexpected)")
        
        print(f"  RIP = 0x{f['rip']:016x}", end="")
        if f['rip'] < 0x100000:
            print(f" ✗ (too low — expected 0xFFFFFFFF80XXXXXX)")
        elif f['rip'] >= 0xFFFFFFFF80000000:
            print(f" ✓ (kernel virtual)")
        else:
            print()
        
        # Check if this looks like frame misalignment
        if f['cs'] == 0x18 and f['rip'] < 16:
            print(f"\n  ★ DIAGNOSIS: IRETQ frame is OFF BY 8 BYTES!")
            print(f"    The CPU popped RFLAGS into the CS slot and")
            print(f"    CS into the RFLAGS slot. This indicates the")
            print(f"    GPR block is one slot too large or too small.")
            print(f"    Expected RIP+CS at ctx_ptr+120, got them at ctx_ptr+128?")

if __name__ == "__main__":
    main()
