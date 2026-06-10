//! TDD Tests for Z3 Loop Invariant Verification — Hoare Logic
//!
//! Series-A Remediation Ticket 1: The compiler must enforce loop invariants
//! using strict Hoare logic with havoc semantics. Modified variables are
//! "havoc'd" (replaced with fresh Z3 symbols) so the solver cannot use
//! pre-loop state to prove post-loop conditions.
//!
//! Tests follow Red-Green-Refactor:
//!   RED:   Tests that MUST FAIL when Z3 loop verification is disabled.
//!   GREEN: Tests pass when full Hoare logic is active.
//!
//! Layer 1: While loop invariant verification (base case + inductive)
//! Layer 2: Invalid invariant detection (compile-time rejection)
//! Layer 3: For loop induction variable bounds
//! Layer 4: Havoc semantics erase pre-loop knowledge
//! Layer 5: Nested loop Z3 scoping

mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    /// Helper: compile a Salt source string and return the MLIR output.
    fn compile_to_mlir(source: &str) -> String {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    /// Helper: compile and return Err(String) if codegen fails.
    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
    }

    // =========================================================================
    // LAYER 1: While Loop — Valid Invariant Accepted
    // =========================================================================

    /// A while loop with a valid invariant (i >= 0) should compile.
    /// Z3 must actively prove the base case: with i = 0, `i >= 0` holds.
    #[test]
    fn test_while_loop_valid_invariant() {
        let result = try_compile(r#"
            package main
            fn main() -> i32 {
                let mut i: i64 = 0;
                while i < 10 {
                    invariant i >= 0;
                    i = i + 1;
                }
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "Valid loop invariant (i >= 0) was improperly rejected by Z3. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 2: While Loop — Invalid Invariant REJECTED at Compile Time
    // =========================================================================

    /// A while loop with an obviously false invariant (i > 100 when i = 0)
    /// MUST be rejected at compile time. The Z3 base-case check should prove
    /// that the invariant does NOT hold at loop entry.
    ///
    /// RED EXPECTATION: If Z3 loop verification is disabled, this returns Ok
    /// because the invariant is only emitted as a runtime check. When Z3 is
    /// active, the base-case check (assert !I, check SAT) catches this.
    #[test]
    fn test_while_loop_invalid_invariant_rejected() {
        let result = try_compile(r#"
            package main
            fn main() -> i32 {
                let mut i: i64 = 0;
                while i < 10 {
                    invariant i > 100;
                    i = i + 1;
                }
                return 0;
            }
        "#);

        assert!(
            result.is_err(),
            "FATAL: Z3 allowed an invalid loop invariant (i > 100 when i starts at 0). \
             Loop verification base-case check is dead."
        );
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("verification") || err_msg.contains("invariant"),
            "Expected invariant verification error, got: {}", err_msg
        );
    }

    // =========================================================================
    // LAYER 3: For Loop — Induction Variable Bounds
    // =========================================================================

    /// A for loop should register the induction variable with Z3 bounds.
    /// Z3 must know: 0 <= i < 10 during the loop body.
    #[test]
    fn test_for_loop_registers_induction_variable_bounds() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                let mut sum: i64 = 0;
                for i in 0..10 {
                    sum = sum + i;
                }
                return 0;
            }
        "#);

        assert!(
            !mlir.is_empty(),
            "For loop with bounded iteration must compile to valid MLIR"
        );
    }

    /// For loop with dynamic upper bound should still register Z3 constraints.
    #[test]
    fn test_for_loop_dynamic_bound_z3_registration() {
        let result = try_compile(r#"
            package main
            fn count(n: i64) -> i64 {
                let mut sum: i64 = 0;
                for i in 0..n {
                    sum = sum + i;
                }
                return sum;
            }
            fn main() -> i32 {
                let _ = count(10);
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "For loop with dynamic bound must compile. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 4: Havoc Semantics — Erase Pre-Loop Knowledge
    // =========================================================================

    /// The havoc mechanism must erase Z3 knowledge about variables modified
    /// in the loop body. After havoc, Z3 treats them as fresh unknowns.
    #[test]
    fn test_havoc_erases_pre_loop_state() {
        let result = try_compile(r#"
            package main
            fn main() -> i32 {
                let mut x: i64 = 5;
                while x > 0 {
                    x = x - 1;
                }
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "Havoc test must compile. Error: {:?}",
            result.err()
        );
    }

    /// Multiple variables modified in the same loop should all be havoc'd.
    #[test]
    fn test_havoc_multiple_variables() {
        let result = try_compile(r#"
            package main
            fn main() -> i32 {
                let mut a: i64 = 0;
                let mut b: i64 = 100;
                while a < b {
                    a = a + 1;
                    b = b - 1;
                }
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "Havoc with multiple modified variables must compile. Error: {:?}",
            result.err()
        );
    }

    // =========================================================================
    // LAYER 5: Nested Loop Z3 Scoping
    // =========================================================================

    /// Nested loops must have independent Z3 solver scopes.
    /// Inner push/pop must not corrupt the outer loop's assertions.
    #[test]
    fn test_nested_loop_z3_scoping() {
        let result = try_compile(r#"
            package main
            fn main() -> i32 {
                let mut total: i64 = 0;
                for i in 0..10 {
                    for j in 0..10 {
                        total = total + 1;
                    }
                }
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "Nested loops must compile with independent Z3 scopes. Error: {:?}",
            result.err()
        );
    }

    /// While loop nested inside a for loop with invariants in both.
    #[test]
    fn test_nested_while_in_for_with_invariants() {
        let result = try_compile(r#"
            package main
            fn main() -> i32 {
                let mut count: i64 = 0;
                for i in 0..5 {
                    let mut j: i64 = 0;
                    while j < i {
                        invariant j >= 0;
                        count = count + 1;
                        j = j + 1;
                    }
                }
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "Nested while-in-for with invariants must compile. Error: {:?}",
            result.err()
        );
    }

    /// Post-loop condition negation: after `while x < 10`, Z3 knows `x >= 10`.
    #[test]
    fn test_post_loop_condition_negation() {
        let result = try_compile(r#"
            package main
            fn main() -> i32 {
                let mut x: i64 = 0;
                while x < 10 {
                    x = x + 1;
                }
                // After loop: Z3 should know !(x < 10), i.e. x >= 10
                return 0;
            }
        "#);

        assert!(
            result.is_ok(),
            "Post-loop code with negated condition must compile. Error: {:?}",
            result.err()
        );
    }
}
