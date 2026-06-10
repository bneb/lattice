// =============================================================================
// TDD Tests: Epoch-Based Reclamation — Z3 Contract Verification
// =============================================================================
//
// Verifies that the EBR arena's Z3 contracts compile and emit correct MLIR:
//
//   Layer 1: requires(core_id >= 0) emits Z3 precondition verification
//   Layer 2: requires(core_id >= 0 && core_id < max_cores) bounds check
//   Layer 3: Linked-list traversal pattern compiles
//   Layer 4: Epoch comparison (u64) compiles to correct comparisons
//
// =============================================================================

mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.lib_mode = true;
        ctx.drive_codegen()
    }

    fn compile_to_mlir(source: &str) -> String {
        try_compile(source).unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    // =========================================================================
    // LAYER 1: Precondition — requires(core_id >= 0)
    // =========================================================================

    #[test]
    fn test_requires_nonneg_core_id() {
        let result = try_compile(r#"
            package main

            pub fn enter_epoch(core_id: i32) -> i64
                requires(core_id >= 0)
            {
                return core_id as i64;
            }
        "#);

        assert!(
            result.is_ok(),
            "requires(core_id >= 0) must compile. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 2: Combined precondition — bounds check
    // =========================================================================

    #[test]
    fn test_requires_core_id_bounded() {
        let result = try_compile(r#"
            package main

            pub fn safe_reclaim(core_id: i32, max_cores: i32) -> i64
                requires(core_id >= 0 && core_id < max_cores)
            {
                return core_id as i64;
            }
        "#);

        assert!(
            result.is_ok(),
            "requires(core_id >= 0 && core_id < max_cores) must compile. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 3: Linked-list traversal with pointer comparison
    // =========================================================================

    #[test]
    fn test_pointer_linked_list_walk_compiles() {
        let mlir = compile_to_mlir(r#"
            package main

            extern fn free(ptr: Ptr<u8>);

            pub fn count_list(head_ptr: Ptr<u8>) -> i64 {
                let mut head = head_ptr;
                let mut count: i64 = 0;
                while head as i64 != 0 {
                    count = count + 1;
                    // Simulate next pointer load
                    let next_addr = (head as i64) + 16;
                    head = next_addr as Ptr<u8>;
                    // Break after 10 to avoid infinite loop in test
                    if count > 10 {
                        return count;
                    }
                }
                return count;
            }
        "#);

        assert!(
            !mlir.is_empty(),
            "Linked-list walk with Ptr<u8> must compile to valid MLIR"
        );
    }

    // =========================================================================
    // LAYER 4: Epoch comparison (u64 arithmetic)
    // =========================================================================

    #[test]
    fn test_epoch_comparison_pattern() {
        let mlir = compile_to_mlir(r#"
            package main

            pub fn should_reclaim(retire_epoch: u64, min_epoch: u64) -> bool {
                return retire_epoch < min_epoch;
            }
        "#);

        assert!(
            mlir.contains("arith.cmpi") || mlir.contains("icmp"),
            "Epoch comparison must emit comparison instruction. Got:\n{}",
            &mlir[..mlir.len().min(500)]
        );
    }

    /// The full reclaim pattern: epoch scan + conditional free
    #[test]
    fn test_reclaim_pattern_compiles() {
        let result = try_compile(r#"
            package main

            extern fn free(ptr: Ptr<u8>);

            pub fn try_reclaim(retire_epoch: u64, global_epoch: u64, ptr: Ptr<u8>) -> bool {
                if retire_epoch < global_epoch {
                    free(ptr);
                    return true;
                }
                return false;
            }
        "#);

        assert!(
            result.is_ok(),
            "Reclaim pattern (epoch check + free) must compile. Error: {:?}",
            result.err()
        );
    }
}
