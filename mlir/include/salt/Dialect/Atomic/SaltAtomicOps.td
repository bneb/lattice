//===- SaltAtomicOps.td - salt.atomic Dialect Definition --*- tablegen -*-===//
//
// Part of the Salt Compiler Project (Pillar 1: Formal Methods Stream)
//
// Defines the salt.atomic dialect for modeling concurrency intrinsics at the
// MLIR level. These operations are consumed by the Z3 SMT verification pass
// to prove memory ordering correctness and ABA-freedom before lowering to
// LLVM IR (cmpxchg16b, PAUSE, MFENCE/DMB).
//
// Design Principles:
//   1. 128-bit CAS modeled as two i64 pairs (matches x86-64 RDX:RAX/RCX:RBX)
//   2. Memory orderings as first-class enum attributes (Z3-inspectable)
//   3. All ops carry MemoryEffects to prevent DCE/CSE across barriers
//
//===----------------------------------------------------------------------===//

#ifndef SALT_ATOMIC_OPS_TD
#define SALT_ATOMIC_OPS_TD

include "mlir/IR/OpBase.td"
include "mlir/IR/EnumAttr.td"
include "mlir/Interfaces/SideEffectInterfaces.td"

//===----------------------------------------------------------------------===//
// Dialect Definition
//===----------------------------------------------------------------------===//

def SaltAtomic_Dialect : Dialect {
  let name = "salt.atomic";
  let cppNamespace = "::salt::atomic";
  let summary = "Salt Atomic Operations for Verified Concurrency";
  let description = [{
    The salt.atomic dialect models hardware concurrency primitives at a level
    suitable for formal verification via Z3 SMT solving. Operations in this
    dialect represent atomic memory accesses, memory barriers, and spin-loop
    hints that the compiler's @atomic decorator produces.

    The Z3 verification pass consumes these operations to build constraint
    equations over memory layouts, proving:
      - Sequential consistency of critical sections
      - ABA-freedom for lock-free data structures (Treiber stacks, etc.)
      - Absence of data races across core boundaries

    After verification, a lowering pass converts these operations to their
    LLVM Dialect equivalents (llvm.cmpxchg, llvm.inline_asm, llvm.fence)
    with the appropriate target-specific encodings.
  }];
  let useDefaultTypePrinterParser = 0;
}

//===----------------------------------------------------------------------===//
// Memory Ordering Enum
//
// Integer values match LLVM's AtomicOrdering encoding, which is already
// used throughout salt-front/src/codegen/intrinsics.rs:
//   Monotonic=0, Acquire=2, Release=3, AcqRel=4, SeqCst=5
//===----------------------------------------------------------------------===//

def SaltAtomic_Monotonic : I64EnumAttrCase<"Monotonic", 0>;
def SaltAtomic_Acquire   : I64EnumAttrCase<"Acquire",   2>;
def SaltAtomic_Release   : I64EnumAttrCase<"Release",   3>;
def SaltAtomic_AcqRel    : I64EnumAttrCase<"AcqRel",    4>;
def SaltAtomic_SeqCst    : I64EnumAttrCase<"SeqCst",    5>;

def SaltAtomic_MemoryOrder : I64EnumAttr<
  "MemoryOrder",
  "Atomic memory ordering for concurrency operations",
  [SaltAtomic_Monotonic, SaltAtomic_Acquire, SaltAtomic_Release,
   SaltAtomic_AcqRel, SaltAtomic_SeqCst]> {
  let cppNamespace = "::salt::atomic";
}

//===----------------------------------------------------------------------===//
// Base Operation Class
//===----------------------------------------------------------------------===//

class SaltAtomic_Op<string mnemonic, list<Trait> traits = []> :
    Op<SaltAtomic_Dialect, mnemonic, traits>;

//===----------------------------------------------------------------------===//
// CmpXchg16bOp — 128-bit Double-Width Compare-and-Swap
//===----------------------------------------------------------------------===//
//
// Models the x86-64 CMPXCHG16B instruction as a first-class MLIR op.
// The 128-bit value is split into two i64 halves (lo/hi), matching the
// hardware register assignment: RDX:RAX (expected) / RCX:RBX (desired).
//
// The kernel's slab allocator (kernel/mem/slab.salt) avoids cmpxchg16b
// in the hot path via epoch-tagged pointers, but the Z3 verification
// layer needs this op to reason about ABA-freedom in the general case
// (e.g., lock-free queues, hazard pointer retirement).
//
// Returns a struct of {old_lo: i64, old_hi: i64, success: i1}.
//
// Lowering to LLVM:
//   %res = llvm.cmpxchg %addr, %expected_128, %desired_128 seq_cst monotonic
//          : !llvm.ptr, i128
//   (with i128 composed/decomposed via llvm.{insert,extract}value)
//
//===----------------------------------------------------------------------===//

def CmpXchg16bOp : SaltAtomic_Op<"cmpxchg16b", [
    MemoryEffects<[MemRead, MemWrite]>
  ]> {
  let summary = "128-bit atomic compare-and-swap (x86-64 CMPXCHG16B)";
  let description = [{
    Performs an atomic 128-bit compare-and-swap on the memory location
    pointed to by `addr`. The expected value is given as two i64 halves
    (`expected_lo`, `expected_hi`), and the desired replacement as
    (`desired_lo`, `desired_hi`).

    If the value at `addr` equals {expected_lo, expected_hi}, it is
    atomically replaced with {desired_lo, desired_hi} and `success`
    is set to true. Otherwise, the memory is unchanged and `success`
    is false. In both cases, the original value is returned as
    {old_lo, old_hi}.

    The `success_ordering` applies when the comparison succeeds;
    `failure_ordering` applies when it fails. The failure ordering
    must be no stronger than the success ordering and cannot be
    Release or AcqRel.

    Example:
    ```mlir
    %old_lo, %old_hi, %ok = salt.atomic.cmpxchg16b %ptr,
        %exp_lo, %exp_hi, %des_lo, %des_hi
        { success_ordering = #salt.atomic.memory_order<SeqCst>,
          failure_ordering = #salt.atomic.memory_order<Monotonic> }
        : !llvm.ptr -> (i64, i64, i1)
    ```
  }];

  let arguments = (ins
    // The memory address to operate on (must be 16-byte aligned)
    LLVM_AnyPointer:$addr,
    // Expected value, low 64 bits (RAX on x86-64)
    I64:$expected_lo,
    // Expected value, high 64 bits (RDX on x86-64)
    I64:$expected_hi,
    // Desired value, low 64 bits (RBX on x86-64)
    I64:$desired_lo,
    // Desired value, high 64 bits (RCX on x86-64)
    I64:$desired_hi,
    // Memory ordering on successful comparison
    SaltAtomic_MemoryOrder:$success_ordering,
    // Memory ordering on failed comparison
    SaltAtomic_MemoryOrder:$failure_ordering
  );

  let results = (outs
    // Original value, low 64 bits
    I64:$old_lo,
    // Original value, high 64 bits
    I64:$old_hi,
    // Whether the swap succeeded
    I1:$success
  );

  let assemblyFormat = [{
    $addr `,` $expected_lo `,` $expected_hi `,` $desired_lo `,` $desired_hi
    attr-dict `:` type($addr) `->` `(` type($old_lo) `,` type($old_hi) `,` type($success) `)`
  }];

  let hasVerifier = 1;
}

//===----------------------------------------------------------------------===//
// PauseOp — Spin-Loop Hint
//===----------------------------------------------------------------------===//
//
// First-class representation of the x86 PAUSE instruction (or ARM YIELD).
// Currently emitted as llvm.inline_asm in intrinsics.rs (line ~1871).
// Promoting it to an MLIR op lets Z3 reason about spin-wait loops:
//   - Prove bounded retry counts for CAS loops
//   - Verify that PAUSE appears inside every spin-wait back-edge
//   - Estimate worst-case spin duration for the Pulse scheduler
//
//===----------------------------------------------------------------------===//

def PauseOp : SaltAtomic_Op<"pause", [
    MemoryEffects<[MemRead, MemWrite]>
  ]> {
  let summary = "Spin-loop hint (x86 PAUSE / ARM YIELD)";
  let description = [{
    Emits a processor hint that the current thread is in a spin-wait loop.
    On x86-64, this lowers to the PAUSE instruction, which:
      1. Avoids memory-order violations on speculative loads
      2. Reduces power consumption during busy-wait
      3. Yields pipeline resources to the sibling hyperthread

    On ARM, this lowers to YIELD (WFE in deep-sleep variant).

    The Z3 verification pass uses this op to:
      - Assert that every CAS retry loop contains a PAUSE
      - Bound the worst-case iteration count for deadline estimation
      - Verify absence of unbounded spin-waits in @pulse functions

    Example:
    ```mlir
    salt.atomic.pause
    ```
  }];

  let arguments = (ins);
  let results = (outs);
  let assemblyFormat = "attr-dict";
}

//===----------------------------------------------------------------------===//
// FenceOp — Memory Barrier
//===----------------------------------------------------------------------===//
//
// Models explicit memory barriers (x86 MFENCE, ARM DMB ISH).
// The existing codegen emits these via llvm.intr.aarch64.dmb (intrinsics.rs
// line ~425). This op makes the barrier Z3-visible so the solver can
// verify that stores before the fence are visible to loads after it
// on all cores in the inner-shareable domain.
//
//===----------------------------------------------------------------------===//

def FenceOp : SaltAtomic_Op<"fence", [
    MemoryEffects<[MemRead, MemWrite]>
  ]> {
  let summary = "Memory barrier with configurable ordering";
  let description = [{
    Emits a memory fence that enforces ordering constraints on memory
    accesses. The fence guarantees that all memory operations before it
    (in program order) are visible to all cores before any memory
    operation after it.

    The `ordering` attribute specifies the strength of the barrier:
      - `AcqRel`: Acquire-release fence (most common for lock-free code)
      - `SeqCst`: Sequentially-consistent fence (full barrier)

    On x86-64, both lower to MFENCE (x86 has a strong memory model).
    On ARM, AcqRel lowers to `DMB ISH` and SeqCst to `DMB ISH` + `DSB ISH`.

    Example:
    ```mlir
    salt.atomic.fence { ordering = #salt.atomic.memory_order<SeqCst> }
    ```
  }];

  let arguments = (ins
    SaltAtomic_MemoryOrder:$ordering
  );

  let results = (outs);
  let assemblyFormat = "attr-dict";
}

#endif // SALT_ATOMIC_OPS_TD
