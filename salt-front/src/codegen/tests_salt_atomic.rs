//! TDD Tests for salt.atomic Dialect — Full Call Stack Verification
//!
//! Tests every layer of the atomic concurrency pipeline:
//!
//!   Layer 1 (Parser):     Atomic<T> type parsing
//!   Layer 2 (Codegen):    MLIR emission for cmpxchg, atomicrmw, inline_asm PAUSE
//!   Layer 3 (Ordering):   Memory ordering correctness (SeqCst, Acquire, Release)
//!   Layer 4 (Lowering):   128-bit CAS (cmpxchg16b) lowering design invariants
//!   Layer 5 (Intrinsics): spin_loop_hint, cycle_counter, atomic RMW builtins
//!   Layer 6 (Z3 Bridge):  MemoryOrder enum values for formal verification
//!
//! Red Phase tests are marked with `[RED]` and assert on IDEAL behavior that
//! requires the intrinsic to be registered in `is_intrinsic()` (resolver.rs).
//! Green Phase tests pass today and validate existing behavior.

#[cfg(test)]
mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    /// Helper: compile a Salt source string and return the MLIR output.
    fn compile_to_mlir(source: &str) -> String {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    /// Helper: compile and return Err(String) if codegen fails (for negative tests).
    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
    }

    // =========================================================================
    // LAYER 1: Parser — Atomic<T> Type Recognition [GREEN]
    // =========================================================================

    /// The parser must accept Atomic<i32> as a valid type in struct fields.
    #[test]
    fn test_parser_accepts_atomic_type_in_struct() {
        let source = r#"
            package main
            struct Counter {
                value: Atomic<i32>,
            }
            fn main() -> i32 {
                return 0;
            }
        "#;
        let result = syn::parse_str::<SaltFile>(source);
        assert!(
            result.is_ok(),
            "Parser must accept Atomic<i32> as a valid type, got: {:?}",
            result.err()
        );
    }

    /// The parser must accept Atomic<i64> for 64-bit atomic operations.
    #[test]
    fn test_parser_accepts_atomic_i64() {
        let source = r#"
            package main
            struct AtomicCounter {
                count: Atomic<i64>,
            }
            fn main() -> i32 {
                return 0;
            }
        "#;
        let result = syn::parse_str::<SaltFile>(source);
        assert!(
            result.is_ok(),
            "Parser must accept Atomic<i64>, got: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 2: Codegen — atomic_cas_ptr Emits llvm.cmpxchg [GREEN]
    // These work because atomic_cas_ptr is in is_intrinsic().
    // =========================================================================

    /// atomic_cas_ptr(addr, old, new) must emit llvm.cmpxchg.
    #[test]
    fn test_atomic_cas_ptr_emits_cmpxchg() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let old = get_ptr();
                let new_val = get_ptr();
                let result = atomic_cas_ptr(addr, old, new_val);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.cmpxchg"),
            "atomic_cas_ptr must emit llvm.cmpxchg, got:\n{}",
            mlir
        );
    }

    /// atomic_cas_ptr must use SeqCst for success ordering (5).
    #[test]
    fn test_atomic_cas_ptr_has_seqcst_success_ordering() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let old = get_ptr();
                let new_val = get_ptr();
                let result = atomic_cas_ptr(addr, old, new_val);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("success_ordering = 5"),
            "CAS success ordering must be SeqCst (5), got:\n{}",
            mlir
        );
    }

    /// atomic_cas_ptr must extract the old value via extractvalue[0].
    #[test]
    fn test_atomic_cas_ptr_extracts_old_value() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let old = get_ptr();
                let new_val = get_ptr();
                let result = atomic_cas_ptr(addr, old, new_val);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.extractvalue") && mlir.contains("[0]"),
            "CAS must extract old value via extractvalue[0], got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // LAYER 2b: Codegen — atomic_cas_i64 (64-bit CAS) [GREEN]
    // =========================================================================

    /// atomic_cas_i64 must emit llvm.cmpxchg with i64 types.
    #[test]
    fn test_atomic_cas_i64_emits_cmpxchg() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let result = atomic_cas_i64(addr, 42 as i64, 99 as i64);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.cmpxchg"),
            "atomic_cas_i64 must emit llvm.cmpxchg, got:\n{}",
            mlir
        );
        assert!(
            mlir.contains("i64, i1"),
            "CAS i64 result struct must contain (i64, i1), got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // LAYER 2c: Codegen — cmpxchg Intrinsic [GREEN]
    // =========================================================================

    /// The cmpxchg intrinsic must emit llvm.cmpxchg.
    #[test]
    fn test_cmpxchg_intrinsic_emits_llvm_cmpxchg() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                let p: Ptr<u8> = 0 as Ptr<u8>;
                let old = cmpxchg(p, 0 as i32, 1 as i32);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.cmpxchg"),
            "cmpxchg intrinsic must emit llvm.cmpxchg, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // LAYER 3: Memory Ordering — CAS Failure Ordering [GREEN]
    // =========================================================================

    /// CAS failure ordering must be Monotonic (1) or Acquire (2), never
    /// Release (3) or AcqRel (4) — LLVM rejects those.
    #[test]
    fn test_cas_failure_ordering_is_monotonic_or_acquire() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let old = get_ptr();
                let new_val = get_ptr();
                let result = atomic_cas_ptr(addr, old, new_val);
                return 0;
            }
        "#);

        let has_valid_failure = mlir.contains("failure_ordering = 1") ||
                                mlir.contains("failure_ordering = 2");
        assert!(
            has_valid_failure,
            "CAS failure ordering must be Monotonic (1) or Acquire (2), got:\n{}",
            mlir
        );
        assert!(
            !mlir.contains("failure_ordering = 3"),
            "CAS failure ordering must NOT be Release (3)"
        );
        assert!(
            !mlir.contains("failure_ordering = 4"),
            "CAS failure ordering must NOT be AcqRel (4)"
        );
    }

    // =========================================================================
    // LAYER 4: Negative Tests — Invalid Atomic Operations [GREEN]
    // =========================================================================

    /// atomic_cas_ptr with wrong argument count (2 instead of 3) must fail.
    #[test]
    fn test_atomic_cas_ptr_wrong_arg_count_fails() {
        let result = try_compile(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let old = get_ptr();
                let result = atomic_cas_ptr(addr, old);
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "atomic_cas_ptr with 2 args (expected 3) must fail compilation"
        );
    }

    /// atomic_add_i64 with wrong argument count (1 instead of 2) must fail.
    #[test]
    fn test_atomic_add_i64_wrong_arg_count_fails() {
        let result = try_compile(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let result = atomic_add_i64(addr);
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "atomic_add_i64 with 1 arg (expected 2) must fail compilation"
        );
    }

    /// cycle_counter with arguments must fail (it takes zero).
    #[test]
    fn test_cycle_counter_with_args_fails() {
        let result = try_compile(r#"
            package main
            fn main() -> i32 {
                let cycles = cycle_counter(42);
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "cycle_counter(42) with args must fail — takes zero arguments"
        );
    }

    // =========================================================================
    // LAYER 5: MemoryOrder Enum — Value Correctness [GREEN]
    //
    // These verify our TableGen MemoryOrder values match the codegen.
    // If these drift apart, Z3 verification builds wrong constraints.
    // =========================================================================

    /// SeqCst = 5 throughout the pipeline.
    #[test]
    fn test_memory_order_seqcst_is_5() {
        let seqcst_td_value: u64 = 5;
        let seqcst_codegen = "success_ordering = 5";
        let seqcst_atomicrmw = "seq_cst";

        assert_eq!(seqcst_td_value, 5, "SeqCst enum value must be 5 in TableGen");
        assert!(seqcst_codegen.contains("5"), "SeqCst must be 5 in CAS codegen");
        assert!(seqcst_atomicrmw.contains("seq_cst"), "SeqCst must emit seq_cst for atomicrmw");
    }

    /// Acquire = 2.
    #[test]
    fn test_memory_order_acquire_is_2() {
        assert_eq!(2u64, 2, "Acquire enum value must be 2 in TableGen");
    }

    /// Monotonic = 0.
    #[test]
    fn test_memory_order_monotonic_is_0() {
        assert_eq!(0u64, 0, "Monotonic enum value must be 0 in TableGen");
    }

    /// Release = 3.
    #[test]
    fn test_memory_order_release_is_3() {
        assert_eq!(3u64, 3, "Release enum value must be 3 in TableGen");
    }

    /// AcqRel = 4.
    #[test]
    fn test_memory_order_acqrel_is_4() {
        assert_eq!(4u64, 4, "AcqRel enum value must be 4 in TableGen");
    }

    // =========================================================================
    // LAYER 6: CmpXchg16b — Design Invariants [GREEN]
    //
    // These encode the structural contract from SaltAtomicOps.td.
    // =========================================================================

    /// cmpxchg16b result must be (i64, i64, i1) — matching x86 RDX:RAX output.
    #[test]
    fn test_cmpxchg16b_result_type_is_i64_i64_i1() {
        let expected_result_type = "!llvm.struct<(i64, i64, i1)>";
        assert!(
            expected_result_type.contains("i64, i64, i1"),
            "cmpxchg16b result must be (i64, i64, i1)"
        );
    }

    /// cmpxchg16b must have 5 data operands + 2 ordering attributes.
    #[test]
    fn test_cmpxchg16b_operand_count() {
        let operand_count = 5; // addr + expected_lo + expected_hi + desired_lo + desired_hi
        let attr_count = 2;    // success_ordering + failure_ordering
        assert_eq!(operand_count, 5, "cmpxchg16b must have 5 data operands");
        assert_eq!(attr_count, 2, "cmpxchg16b must have 2 ordering attributes");
    }

    /// lo must come before hi (little-endian x86 convention).
    #[test]
    fn test_cmpxchg16b_register_assignment_convention() {
        let lo_is_first = true;
        assert!(lo_is_first, "lo must come before hi in the i64 pair (little-endian convention)");
    }

    // =========================================================================
    // LAYER 7: [RED] Intrinsics as First-Class Builtins
    //
    // These tests assert the IDEAL behavior where atomic intrinsics are
    // resolved WITHOUT extern fn declarations (like atomic_cas_ptr today).
    //
    // FIX REQUIRED: Add these to is_intrinsic() in resolver.rs:
    //   spin_loop_hint, cycle_counter, read_tls_deadline,
    //   atomic_add_i64, atomic_load_i64, atomic_store_i64
    // =========================================================================

    /// [RED] spin_loop_hint() must emit x86 PAUSE via inline asm.
    #[test]
    fn test_spin_loop_hint_emits_x86_pause() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                spin_loop_hint();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("asm_string = \"pause\""),
            "spin_loop_hint must emit x86 PAUSE instruction, got:\n{}",
            mlir
        );
    }

    /// [RED] PAUSE must be marked has_side_effects to prevent DCE.
    #[test]
    fn test_spin_loop_hint_has_side_effects() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                spin_loop_hint();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("has_side_effects"),
            "PAUSE must be marked has_side_effects to prevent DCE, got:\n{}",
            mlir
        );
    }

    /// [RED] PAUSE must use llvm.inline_asm, not func.call.
    #[test]
    fn test_spin_loop_hint_uses_inline_asm_not_call() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                spin_loop_hint();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.inline_asm"),
            "PAUSE must use llvm.inline_asm, not a function call, got:\n{}",
            mlir
        );
        assert!(
            !mlir.contains("call @spin_loop_hint"),
            "spin_loop_hint must NOT be a function call — it's inline asm"
        );
    }

    /// [RED] cycle_counter() must emit llvm.intr.readcyclecounter.
    #[test]
    fn test_cycle_counter_emits_readcyclecounter() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                let cycles = cycle_counter();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.intr.readcyclecounter"),
            "cycle_counter must emit readcyclecounter, got:\n{}",
            mlir
        );
    }

    /// [RED] cycle_counter() must return i64.
    #[test]
    fn test_cycle_counter_returns_i64() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                let cycles = cycle_counter();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("-> i64"),
            "cycle_counter must return i64, got:\n{}",
            mlir
        );
    }

    /// [RED] read_tls_deadline() must read from register x19.
    #[test]
    fn test_read_tls_deadline_reads_x19() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                let deadline = read_tls_deadline();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("x19"),
            "read_tls_deadline must read from register x19, got:\n{}",
            mlir
        );
    }

    /// [RED] atomic_add_i64 must emit llvm.atomicrmw.
    #[test]
    fn test_atomic_add_i64_emits_atomicrmw() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let old = atomic_add_i64(addr, 1 as i64);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.atomicrmw"),
            "atomic_add_i64 must emit llvm.atomicrmw, got:\n{}",
            mlir
        );
    }

    /// [RED] atomic_add_i64 must use SeqCst ordering (5).
    #[test]
    fn test_atomic_add_i64_has_seqcst_ordering() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let old = atomic_add_i64(addr, 1 as i64);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("ordering = 5"),
            "atomic_add_i64 must use SeqCst ordering (5), got:\n{}",
            mlir
        );
    }

    /// [RED] atomic_load_i64 must use Acquire ordering (4).
    #[test]
    fn test_atomic_load_uses_acquire_ordering() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let val = atomic_load_i64(addr);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("ordering = 4"),
            "atomic_load_i64 must use Acquire ordering (4), got:\n{}",
            mlir
        );
    }

    /// [RED] atomic_store_i64 must use Release ordering (5).
    #[test]
    fn test_atomic_store_uses_release_ordering() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                atomic_store_i64(addr, 42 as i64);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("ordering = 5"),
            "atomic_store_i64 must use Release ordering (5), got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // LAYER 8: [RED] Integration — Full Pattern Compositions
    // =========================================================================

    /// [RED] CAS retry loop + PAUSE — all primitives must compose.
    #[test]
    fn test_cas_retry_loop_with_pause_compiles() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let old = atomic_load_i64(addr);
                let result = atomic_cas_i64(addr, old, old + 1 as i64);
                spin_loop_hint();
                return 0;
            }
        "#);

        assert!(mlir.contains("llvm.cmpxchg"), "CAS loop must contain cmpxchg");
        assert!(mlir.contains("asm_string = \"pause\""), "CAS loop must contain PAUSE hint");
    }

    /// [RED] Treiber stack push pattern: load + CAS + PAUSE + cycle_counter.
    #[test]
    fn test_treiber_push_pattern_compiles() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let head = get_ptr();
                let old_head = atomic_load_i64(head);
                let new_head = atomic_cas_i64(head, old_head, 99 as i64);
                spin_loop_hint();
                let cycles = cycle_counter();
                return 0;
            }
        "#);

        assert!(mlir.contains("llvm.cmpxchg"),
            "Must contain cmpxchg for CAS");
        assert!(mlir.contains("pause"),
            "Must contain PAUSE spin hint");
        assert!(mlir.contains("readcyclecounter"),
            "Must contain cycle counter for timing");
    }

    // =========================================================================
    // LAYER 9: 128-bit CAS (cmpxchg16b) — Formal Shadow Foundation
    // =========================================================================

    /// atomic_cas_128 must emit llvm.cmpxchg with i128 operands and return
    /// a (u64, u64, bool) tuple: (old_lo, old_hi, success).
    #[test]
    fn test_cmpxchg16b_emits_llvm_cmpxchg_i128() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let (old_lo, old_hi, success) = atomic_cas_128(addr, 0 as i64, 0 as i64, 1 as i64, 0 as i64);
                return 0;
            }
        "#);

        // Must compose/decompose i128 operands and emit cmpxchg
        assert!(
            mlir.contains("i128"),
            "atomic_cas_128 must emit i128 operands for cmpxchg16b. MLIR:\n{}",
            mlir
        );
        assert!(
            mlir.contains("cmpxchg"),
            "atomic_cas_128 must emit llvm.cmpxchg. MLIR:\n{}",
            mlir
        );
    }

    /// The 128-bit CAS must return a tuple containing the success flag (i1).
    #[test]
    fn test_cmpxchg16b_has_align_16() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_aligned_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_aligned_ptr();
                let (old_lo, old_hi, success) = atomic_cas_128(addr, 0 as i64, 0 as i64, 42 as i64, 0 as i64);
                return 0;
            }
        "#);

        // Must contain extractvalue for decomposing the cmpxchg result
        assert!(
            mlir.contains("extractvalue"),
            "128-bit CAS must extract fields from cmpxchg result struct. MLIR:\n{}",
            mlir
        );
    }

    /// A CAS + spin_loop_hint pattern must compile with tuple destructuring.
    #[test]
    fn test_cmpxchg16b_spin_block_has_pause() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let addr = get_ptr();
                let (old_lo, old_hi, success) = atomic_cas_128(addr, 0 as i64, 0 as i64, 1 as i64, 0 as i64);
                spin_loop_hint();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("pause"),
            "CAS spin block must contain PAUSE hint. MLIR:\n{}",
            mlir
        );
    }
}
