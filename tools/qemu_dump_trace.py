#!/usr/bin/env python3
"""
qemu_dump_trace.py — Lattice Flight Recorder Memory Dump Parser

Reads the TRACE_BUFFERS and TRACE_INDICES symbols from a QEMU core dump
or from a live QEMU monitor session and prints a chronological timeline
of context switches per core.

Usage:
  # From a QEMU memory dump (binary):
  python3 tools/qemu_dump_trace.py --elf qemu_build/kernel.elf --dump memory.bin

  # From symbol addresses (manual):
  python3 tools/qemu_dump_trace.py --elf qemu_build/kernel.elf --dump memory.bin \
      --buffers-addr 0x... --indices-addr 0x...

The script uses the ELF symbol table to find TRACE_BUFFERS and TRACE_INDICES
in the kernel binary, then reads the corresponding memory regions from the dump.

Output format:
  [Core 0] TSC=123456789  Fiber=3  DISPATCH
  [Core 0] TSC=123456812  Fiber=3  YIELD
  [Core 0] TSC=123456900  Fiber=5  DISPATCH
"""

import argparse
import struct
import subprocess
import sys
import os

# Constants matching flight_recorder.salt
TRACE_CAPACITY = 4096
TRACE_MASK = TRACE_CAPACITY - 1
MAX_CPUS = 16
TOTAL_ENTRIES = MAX_CPUS * TRACE_CAPACITY

# TraceEvent struct: 16 bytes
#   u64 tsc
#   u32 fiber_id
#   u32 event_type
EVENT_SIZE = 16
EVENT_FORMAT = '<QII'  # little-endian: u64, u32, u32

EVENT_NAMES = {
    0: 'DISPATCH',
    1: 'YIELD',
    2: 'COMPLETED',
    3: 'TIMER_ISR',
}

# Higher-half kernel base
KERNEL_VIRT_BASE = 0xFFFFFFFF80000000
KERNEL_PHYS_BASE = 0x100000


def find_symbol(elf_path, symbol_name):
    """Find a symbol's virtual address in the ELF using nm or objdump."""
    try:
        # Try llvm-nm first (works on macOS cross-compiled ELFs)
        result = subprocess.run(
            ['llvm-nm', elf_path],
            capture_output=True, text=True
        )
        if result.returncode != 0:
            result = subprocess.run(
                ['nm', elf_path],
                capture_output=True, text=True
            )

        for line in result.stdout.splitlines():
            parts = line.strip().split()
            if len(parts) >= 3 and parts[2] == symbol_name:
                return int(parts[0], 16)

    except FileNotFoundError:
        pass

    return None


def virt_to_phys(vaddr):
    """Convert higher-half virtual address to physical offset in dump."""
    if vaddr >= KERNEL_VIRT_BASE:
        return vaddr - KERNEL_VIRT_BASE
    return vaddr


def read_trace_from_dump(dump_path, buffers_addr, indices_addr):
    """Read trace data from a raw memory dump file."""
    with open(dump_path, 'rb') as f:
        dump_data = f.read()

    # Read per-core indices (16 × u64 = 128 bytes)
    indices_offset = virt_to_phys(indices_addr)
    indices = []
    for cpu in range(MAX_CPUS):
        offset = indices_offset + cpu * 8
        if offset + 8 <= len(dump_data):
            val = struct.unpack_from('<Q', dump_data, offset)[0]
            indices.append(val)
        else:
            indices.append(0)

    # Read trace events
    events_by_core = {}
    buffers_offset = virt_to_phys(buffers_addr)

    for cpu in range(MAX_CPUS):
        idx = indices[cpu]
        if idx == 0:
            continue  # No events recorded for this core

        events = []
        # Read events in chronological order (oldest first)
        count = min(idx, TRACE_CAPACITY)
        start = (idx - count) & TRACE_MASK if idx >= TRACE_CAPACITY else 0

        for i in range(count):
            event_idx = (start + i) & TRACE_MASK
            buf_idx = cpu * TRACE_CAPACITY + event_idx
            offset = buffers_offset + buf_idx * EVENT_SIZE

            if offset + EVENT_SIZE <= len(dump_data):
                tsc, fiber_id, event_type = struct.unpack_from(
                    EVENT_FORMAT, dump_data, offset
                )
                if tsc != 0:  # Skip uninitialized entries
                    events.append((tsc, fiber_id, event_type))

        if events:
            events_by_core[cpu] = events

    return events_by_core, indices


def print_timeline(events_by_core, indices, last_n=None):
    """Print a chronological timeline of trace events."""
    print("=" * 72)
    print("  Lattice Flight Recorder — Context Switch Timeline")
    print("=" * 72)

    for cpu in sorted(events_by_core.keys()):
        events = events_by_core[cpu]
        idx = indices[cpu]
        total = min(idx, TRACE_CAPACITY)

        if last_n and len(events) > last_n:
            events = events[-last_n:]

        print(f"\n  Core {cpu} ({total} events recorded, showing {len(events)}):")
        print(f"  {'─' * 60}")
        print(f"  {'TSC':>16}  {'Fiber':>6}  {'Event':<12}  {'Delta':>10}")
        print(f"  {'─' * 60}")

        prev_tsc = None
        for tsc, fiber_id, event_type in events:
            event_name = EVENT_NAMES.get(event_type, f'UNKNOWN({event_type})')
            delta = ''
            if prev_tsc is not None and tsc > prev_tsc:
                delta = f'+{tsc - prev_tsc}'
            prev_tsc = tsc

            print(f"  {tsc:>16}  {fiber_id:>6}  {event_name:<12}  {delta:>10}")

        print(f"  {'─' * 60}")

    print(f"\n{'=' * 72}")


def main():
    parser = argparse.ArgumentParser(
        description='Lattice Flight Recorder Dump Parser'
    )
    parser.add_argument('--elf', required=True,
                       help='Path to kernel.elf')
    parser.add_argument('--dump', required=True,
                       help='Path to QEMU memory dump (binary)')
    parser.add_argument('--buffers-addr', type=lambda x: int(x, 0),
                       help='Override TRACE_BUFFERS symbol address')
    parser.add_argument('--indices-addr', type=lambda x: int(x, 0),
                       help='Override TRACE_INDICES symbol address')
    parser.add_argument('--last', type=int, default=50,
                       help='Show last N events per core (default: 50)')

    args = parser.parse_args()

    if not os.path.exists(args.elf):
        print(f"ERROR: ELF not found: {args.elf}", file=sys.stderr)
        sys.exit(1)

    if not os.path.exists(args.dump):
        print(f"ERROR: Memory dump not found: {args.dump}", file=sys.stderr)
        sys.exit(1)

    # Find symbol addresses
    buffers_addr = args.buffers_addr
    indices_addr = args.indices_addr

    if buffers_addr is None:
        buffers_addr = find_symbol(args.elf,
            'kernel__core__flight_recorder__TRACE_BUFFERS')
        if buffers_addr is None:
            print("ERROR: Could not find TRACE_BUFFERS symbol in ELF",
                  file=sys.stderr)
            print("  Use --buffers-addr to specify manually", file=sys.stderr)
            sys.exit(1)

    if indices_addr is None:
        indices_addr = find_symbol(args.elf,
            'kernel__core__flight_recorder__TRACE_INDICES')
        if indices_addr is None:
            print("ERROR: Could not find TRACE_INDICES symbol in ELF",
                  file=sys.stderr)
            print("  Use --indices-addr to specify manually", file=sys.stderr)
            sys.exit(1)

    print(f"  TRACE_BUFFERS @ 0x{buffers_addr:016x}")
    print(f"  TRACE_INDICES @ 0x{indices_addr:016x}")
    print(f"  Buffer size: {TRACE_CAPACITY} events/core × {MAX_CPUS} cores "
          f"= {TOTAL_ENTRIES * EVENT_SIZE // 1024}KB")

    events_by_core, indices = read_trace_from_dump(
        args.dump, buffers_addr, indices_addr
    )

    if not events_by_core:
        print("\n  No trace events found in dump.")
        sys.exit(0)

    print_timeline(events_by_core, indices, last_n=args.last)


if __name__ == '__main__':
    main()
