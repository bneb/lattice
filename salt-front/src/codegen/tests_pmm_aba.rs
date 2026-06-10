//! TDD Tests for PMM ABA Resolution — Versioned Pointer Protection
//!
//! Series-A Remediation Ticket 2: Verify the versioned pointer (ABA counter)
//! pattern and atomic operations compile correctly in Salt.
//!
//! Layer 1: Versioned pointer pack/unpack & bit shift operations
//! Layer 2: CAS + atomic_add_i64 emit correct MLIR (using extern ptr pattern)
//! Layer 3: Full versioned CAS steal pattern

mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    fn compile_to_mlir(source: &str) -> String {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
    }

    // =========================================================================
    // LAYER 1: Versioned Pointer Bit Packing
    // =========================================================================

    #[test]
    fn test_versioned_pointer_pack_unpack_compiles() {
        let mlir = compile_to_mlir(r#"
            package main
            fn pack_versioned(aba: u64, phys: u64) -> u64 {
                let packed = (aba << 48) | (phys & 0x0000FFFFFFFFFFFF);
                return packed;
            }
            fn unpack_aba(packed: u64) -> u64 {
                return packed >> 48;
            }
            fn unpack_phys(packed: u64) -> u64 {
                return packed & 0x0000FFFFFFFFFFFF;
            }
            fn main() -> i32 { return 0; }
        "#);

        assert!(!mlir.is_empty(), "Versioned pointer pack/unpack must compile");
    }

    #[test]
    fn test_aba_counter_shift_emits_correct_mlir() {
        let mlir = compile_to_mlir(r#"
            package main
            fn extract_aba(packed: u64) -> u64 {
                let aba = packed >> 48;
                let new_aba = (aba + 1) & 0xFFFF;
                return new_aba;
            }
            fn main() -> i32 {
                let _ = extract_aba(0);
                return 0;
            }
        "#);

        assert!(mlir.contains("shrui") || mlir.contains("shru"),
            "MLIR must contain right-shift for ABA counter. MLIR:\n{}", mlir);
    }

    // =========================================================================
    // LAYER 2: CAS + Atomic Add with extern ptr pattern
    // =========================================================================

    /// atomic_cas_i64 must emit cmpxchg — the core of ABA-protected steal.
    #[test]
    fn test_cas_with_versioned_value_emits_cmpxchg() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_head_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let head = get_head_ptr();
                let packed_old: i64 = 42;
                let packed_new: i64 = 99;
                let result = atomic_cas_i64(head, packed_old, packed_new);
                return 0;
            }
        "#);

        assert!(mlir.contains("cmpxchg"),
            "CAS must emit llvm.cmpxchg instruction. MLIR:\n{}", mlir);
    }

    /// atomic_add_i64 with negative delta (-1) emits atomicrmw add.
    /// This is the fix for the non-atomic count decrement bug.
    #[test]
    fn test_atomic_add_negative_emits_atomicrmw() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_count_ptr() -> Ptr<u8>;
            fn main() -> i32 {
                let count_ptr = get_count_ptr();
                let old = atomic_add_i64(count_ptr, -1 as i64);
                return 0;
            }
        "#);

        assert!(mlir.contains("atomicrmw"),
            "atomic_add_i64(-1) must emit atomicrmw for atomic decrement. MLIR:\n{}", mlir);
    }

    // =========================================================================
    // LAYER 3: Full Versioned CAS Steal Pattern
    // =========================================================================

    /// Complete ABA-protected steal pattern: read packed head, extract ABA
    /// counter and physical address, increment ABA, CAS with versioned value.
    #[test]
    fn test_full_versioned_steal_pattern_compiles() {
        let result = try_compile(r#"
            package main
            extern fn read_head() -> i64;
            extern fn get_head_ptr() -> Ptr<u8>;
            extern fn get_count_ptr() -> Ptr<u8>;

            fn steal_with_aba() -> i64 {
                let packed_head = read_head();
                if packed_head == 0 {
                    return 0;
                }

                // Unpack: top 16 bits = ABA counter, bottom 48 = physical address
                let packed = packed_head as u64;
                let aba = packed >> 48;
                let phys_addr = packed & 0x0000FFFFFFFFFFFF;

                // Read next pointer (simulated)
                let next: u64 = 0;

                // Pack new head with incremented ABA counter
                let new_aba = (aba + 1) & 0xFFFF;
                let new_packed = (new_aba << 48) | (next & 0x0000FFFFFFFFFFFF);

                // CAS with full 64-bit comparison — ABA mismatch causes failure
                let head_ptr = get_head_ptr();
                let old_val = atomic_cas_i64(head_ptr, packed_head, new_packed as i64);

                if old_val == packed_head {
                    // Steal successful! Atomic count decrement
                    let count_ptr = get_count_ptr();
                    let _ = atomic_add_i64(count_ptr, -1 as i64);
                    return phys_addr as i64;
                }
                return 0;
            }

            fn main() -> i32 { return 0; }
        "#);

        assert!(result.is_ok(),
            "Full versioned CAS steal pattern must compile. Error: {:?}", result.err());
    }

    /// The full pattern emits both cmpxchg AND atomicrmw in MLIR.
    #[test]
    fn test_full_pattern_emits_cmpxchg_and_atomicrmw() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn get_head_ptr() -> Ptr<u8>;
            extern fn get_count_ptr() -> Ptr<u8>;

            fn do_steal() -> i32 {
                let head_ptr = get_head_ptr();
                let packed_old: i64 = 100;
                let packed_new: i64 = 200;
                let result = atomic_cas_i64(head_ptr, packed_old, packed_new);

                let count_ptr = get_count_ptr();
                let old_count = atomic_add_i64(count_ptr, -1 as i64);
                return 0;
            }

            fn main() -> i32 {
                let _ = do_steal();
                return 0;
            }
        "#);

        assert!(mlir.contains("cmpxchg"),
            "Pattern must contain cmpxchg. MLIR:\n{}", mlir);
        assert!(mlir.contains("atomicrmw"),
            "Pattern must contain atomicrmw for atomic count decrement. MLIR:\n{}", mlir);
    }
}
