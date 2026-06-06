# The KeuOS Application Binary Interface (ABI)

## 1. Overview

This document outlines the architectural specifications of the KeuOS Application Binary Interface (ABI). The ABI is designed for heterogeneous computing environments requiring microsecond-latency deterministic execution, specifically targeting high-frequency trading (HFT), high-performance computer graphics, and distributed execution.

The legacy POSIX standard introduces non-deterministic latency spikes due to virtual memory management, interrupt handling, and monolithic kernel scheduling. KeuOS addresses these bottlenecks through hardware-fenced determinism, zero-allocation tear downs, and lock-free operations.

## 2. Resource Architecture and Teardown

Traditional kernels utilize recursive memory freeing mechanisms that can induce latency spikes. KeuOS replaces this with the Hardware-Fenced Reclaim protocol, which reclaims state in an ordered sequence without thread locks.

### 2.1. Hardware-Fenced Reclaim Protocol

The teardown process operates in the following phases:

1. **SPSC Ring Drain**: The kernel drains and clears pending descriptors in the process's Single Producer Single Consumer (SPSC) shared memory rings.
2. **Peripheral Ring Disable**: The kernel signals hardware peripherals (e.g., NICs via PCIe) to halt Direct Memory Access (DMA) polling on the process's TX and RX rings. This prevents DMA race conditions during page reclamation.
3. **MMU Unmap and Sweep**: A flat, non-recursive bitmap-backed sweep tears down user page tables and triggers Translation Lookaside Buffer (TLB) shootdowns, freeing 4KB and 2MB huge-pages in bulk.
4. **Frame Release**: Physical pages are returned to the Physical Memory Manager (PMM). The kernel stack is freed, and the process table slot is zeroed.

The P99 reclamation time constraint for this protocol is < 1,000 microseconds. Violations trigger telemetry alerts.

## 3. Lock-Free Memory and Task Scheduling

To minimize synchronization latency in Symmetric Multi-Processing (SMP) systems, KeuOS utilizes per-core sharding.

### 3.1. Per-Core Sharding

Each CPU core maintains an independent scheduler state. KeuOS uses the `IA32_GS_BASE` Model Specific Register (MSR) on x86 architectures to point to a core's local data block. 

The Physical Memory Manager (PMM) is implemented as a per-core sharded, lock-free Treiber stack. It uses 64-byte cacheline padding to prevent MESI (Modified, Exclusive, Shared, Invalid) protocol false-sharing between cores. Interrupts are masked locally during linked list mutations. If a core exhausts its local free list, it uses the atomic `cmpxchg` instruction to steal pages from remote cores.

### 3.2. Context Switching

The scheduler utilizes a hierarchical two-level bitmap and the hardware TZCNT (Trailing Zero Count) instruction to achieve O(1) scheduling complexity.

Transitions from Ring 0 to Ring 3 populate the process's kernel stack with an IRETQ frame, pre-loading the Code Segment (0x2B), Stack Segment (0x23), and entry pointers from the ELF header. KeuOS enforces 16-byte stack alignment to support advanced hardware instructions (`FXSAVE`, `FXRSTOR`) without generating KVM General Protection Faults.

## 4. Inter-Process Communication (IPC)

KeuOS bypasses traditional buffer copying to reduce IPC latency.

### 4.1. Register-Level IPC

For small payloads, KeuOS implements a fast-path register-level IPC mechanism (`sys_ipc_send`, `sys_ipc_recv`). Payloads are delivered directly via CPU registers (RDI, RSI, RDX). The kernel stages the payload in global variables and injects them into the receiving application's registers during the SYSRET transition.

### 4.2. Zero-Copy Shared Memory Grants

For bulk data transfers, KeuOS maps physical memory frames of the sending process directly into the page tables of the receiving process. 

This enables native shared-memory abstractions:
*   **Futures**: Cross-process computations with timeout support.
*   **Lazy Evaluation**: Shared and cached computations.
*   **Reactive Streams**: Continuous data flows with built-in backpressure using SPSC rings.
*   **Channels**: Structured Communication Sequential Processes (CSP) for worker pools.

Data structures are defined by Name, Offset, and Size metadata to allow cross-language compatibility without Foreign Function Interfaces (FFI) or serialization overhead.

## 5. Domain Applications

### 5.1. High-Frequency Trading (HFT)

KeuOS provides an alternative to traditional kernel bypass methodologies (e.g., DPDK, AF_XDP). It separates Kernel Page Table Isolation (KPTI) logic via a higher-half direct memory map. Register-level IPC allows market data handlers to pass decoded protocols directly to execution nodes without traversing network sockets.

### 5.2. Computer Graphics

KeuOS implements a Unified Memory Architecture (UMA), eliminating explicit data transfers across the PCIe bus between CPU RAM and GPU VRAM. It provides peer-to-peer inter-GPU synchronization and collective communication primitives to minimize CPU overhead in draw call dispatching.

### 5.3. Distributed Hardware Interconnects

KeuOS serves as an orchestration layer for open consortium standards like UALink 1.0 and Compute Express Link (CXL 4.0), mapping network-attached accelerators into the local virtual memory hierarchy to abstract physical topology.

## 6. Formal Verification

The ABI uses SMT (Satisfiability Modulo Theories) solvers, such as Z3, to verify the correctness of system calls and trap handlers at compile time. Constraints are explicitly modeled as pre-conditions and post-conditions to eliminate undefined behavior in lock-free concurrency operations.
