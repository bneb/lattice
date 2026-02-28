//! TDD Tests for Z3 Postcondition Verification — v0.9.2 Postcondition Pivot
//!
//! Phase 1 RED: These tests assert that `ensures` postconditions are verified
//! by Z3 at every return site via Weakest Precondition (WP) generation.
//!
//! The compiler must:
//!   1. Extract the `ensures` expression from the function signature
//!   2. At each `return` site, substitute `result` with the actual return value
//!   3. Check the substituted expression via Z3
//!   4. UNSAT (violation proven) → compile error with counterexample
//!   5. SAT (condition satisfiable) → postcondition holds, elide check
//!   6. Unknown → emit runtime assertion fallback
//!
//! Tests follow Red-Green-Refactor:
//!   RED:   These tests assert behavior that does not yet exist.
//!   GREEN: Tests pass after WP generation is implemented.
//!
//! Layer 1: Basic postcondition (single return path)
//! Layer 2: Multi-branch postcondition (all paths must satisfy)
//! Layer 3: Postcondition violation (Z3 must reject with counterexample)
//! Layer 4: Z3 timeout watchdog (deferred to runtime on complex proofs)

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

    /// Helper: compile and return Err(String) if codegen fails.
    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
    }

    // =========================================================================
    // LAYER 1: Basic Postcondition — Identity Function [RED]
    // =========================================================================
    // The simplest ensures test: a function that returns the absolute value
    // of its input. Z3 must prove that `result >= 0` holds for all paths.
    //
    // WP obligation:
    //   Path 1: x < 0 => -x >= 0  (TRUE for all x in i32)
    //   Path 2: x >= 0 => x >= 0  (TRUE trivially)

    /// Z3 must verify that absolute_value always returns a non-negative value.
    /// Both return paths (negated and identity) must satisfy `ensures(result >= 0)`.
    #[test]
    fn test_postcondition_basic_absolute_value() {
        let mlir = compile_to_mlir(r#"
            package main
            fn absolute_value(x: i32) -> i32
                ensures(result >= 0)
            {
                if x < 0 {
                    return -x;
                }
                return x;
            }
            fn main() -> i32 {
                return absolute_value(42);
            }
        "#);

        // The function should compile successfully — Z3 proves the postcondition.
        // The MLIR should contain evidence of postcondition verification.
        assert!(
            mlir.contains("z3_postcondition_verified") || mlir.contains("ensures_verified"),
            "Z3 must verify ensures(result >= 0) for absolute_value. \
             Expected postcondition verification marker in MLIR output.\n\
             MLIR:\n{}",
            &mlir[..mlir.len().min(500)]
        );
    }

    // =========================================================================
    // LAYER 2: Multi-Branch Postcondition — Clamp Function [RED]
    // =========================================================================
    // Forces Z3 to evaluate multiple return paths with different constant values.
    // The "middle" path (return val) is safe because preceding guards logically
    // constrain val to [0, 100].
    //
    // WP obligation:
    //   Path 1: val < 0 => 0 >= 0 && 0 <= 100   (TRUE)
    //   Path 2: val > 100 => 100 >= 0 && 100 <= 100  (TRUE)
    //   Path 3: val >= 0 && val <= 100 => val >= 0 && val <= 100  (TRUE)

    /// Z3 must verify that clamp_to_unit returns a value in [0, 100] for all inputs.
    /// Three return paths must all satisfy the postcondition.
    #[test]
    fn test_postcondition_branches_clamp() {
        let mlir = compile_to_mlir(r#"
            package main
            fn clamp_to_unit(val: i32) -> i32
                ensures(result >= 0 && result <= 100)
            {
                if val < 0 {
                    return 0;
                }
                if val > 100 {
                    return 100;
                }
                return val;
            }
            fn main() -> i32 {
                return clamp_to_unit(50);
            }
        "#);

        // Should compile — all three paths satisfy ensures(result >= 0 && result <= 100).
        assert!(
            mlir.contains("z3_postcondition_verified") || mlir.contains("ensures_verified"),
            "Z3 must verify ensures(result >= 0 && result <= 100) for clamp_to_unit. \
             All three return paths must satisfy the postcondition.\n\
             MLIR:\n{}",
            &mlir[..mlir.len().min(500)]
        );
    }

    // =========================================================================
    // LAYER 3: Postcondition Violation — Flawed Logic [RED]
    // =========================================================================
    // This function has a bug: when x > 1000, it returns x (not x + 1),
    // violating the contract `ensures(result > x)`.
    //
    // Z3 must find the counterexample: x = 1001, result = 1001, 1001 > 1001 is FALSE.

    /// Compilation MUST FAIL. The function violates `ensures(result > x)` when x > 1000.
    /// Z3 should provide a counterexample showing the violation.
    #[test]
    fn test_postcondition_violation_rejected() {
        let result = try_compile(r#"
            package main
            fn flawed_increment(x: i32) -> i32
                ensures(result > x)
            {
                if x > 1000 {
                    return x;
                }
                return x + 1;
            }
            fn main() -> i32 {
                return flawed_increment(5);
            }
        "#);

        assert!(
            result.is_err(),
            "Z3 must reject flawed_increment: when x > 1000, return x violates \
             ensures(result > x). Expected compilation error with counterexample, \
             but compilation succeeded:\n{}",
            result.unwrap_or_default().chars().take(500).collect::<String>()
        );
    }

    // =========================================================================
    // LAYER 4: Postcondition with Requires — Combined Contracts [RED]
    // =========================================================================
    // Tests that requires preconditions are used as assumptions when verifying
    // postconditions. The function is only valid for positive inputs, and the
    // postcondition relies on that assumption.

    /// Z3 must verify ensures using requires as assumptions.
    /// safe_divide requires b > 0, and ensures result >= 0 when a >= 0.
    #[test]
    fn test_postcondition_with_requires_assumption() {
        let mlir = compile_to_mlir(r#"
            package main
            fn safe_divide(a: i32, b: i32) -> i32
                requires(b > 0)
                requires(a >= 0)
                ensures(result >= 0)
            {
                return a / b;
            }
            fn main() -> i32 {
                return safe_divide(10, 3);
            }
        "#);

        // Should compile — under the assumption b > 0 and a >= 0,
        // integer division a / b >= 0 holds.
        assert!(
            mlir.contains("z3_postcondition_verified") || mlir.contains("ensures_verified"),
            "Z3 must verify ensures(result >= 0) for safe_divide under \
             requires(b > 0) and requires(a >= 0) assumptions.\n\
             MLIR:\n{}",
            &mlir[..mlir.len().min(500)]
        );
    }

    // =========================================================================
    // LAYER 5: Postcondition — Pure Arithmetic Identity [RED]
    // =========================================================================
    // The simplest possible postcondition: result equals a known value.
    // This validates the `result` keyword substitution works correctly.

    /// Z3 must prove that a function returning a constant satisfies ensures(result == 42).
    #[test]
    fn test_postcondition_constant_return() {
        let mlir = compile_to_mlir(r#"
            package main
            fn always_42() -> i32
                ensures(result == 42)
            {
                return 42;
            }
            fn main() -> i32 {
                return always_42();
            }
        "#);

        assert!(
            mlir.contains("z3_postcondition_verified") || mlir.contains("ensures_verified"),
            "Z3 must verify ensures(result == 42) for constant return.\n\
             MLIR:\n{}",
            &mlir[..mlir.len().min(500)]
        );
    }

    // =========================================================================
    // LAYER 6: Z3 Timeout Watchdog — Complex Proof Deferred [RED]
    // =========================================================================
    // If Z3 cannot discharge the postcondition within 100ms, the compiler
    // should emit a runtime assertion fallback instead of hanging.
    // This tests the watchdog mechanism by creating an intentionally
    // complex postcondition that Z3 may timeout on.
    //
    // Note: This test validates graceful degradation, not a specific timeout.

    /// When Z3 times out on a complex postcondition, the compiler should emit
    /// a runtime assertion fallback rather than hanging indefinitely.
    #[test]
    fn test_postcondition_timeout_deferred_to_runtime() {
        let result = try_compile(r#"
            package main
            fn complex_computation(x: i32) -> i32
                ensures(result >= 0)
            {
                let mut acc: i32 = 0;
                let mut i: i32 = 0;
                if x > 0 {
                    acc = x * x;
                }
                if acc > 10000 {
                    acc = 10000;
                }
                return acc;
            }
            fn main() -> i32 {
                return complex_computation(5);
            }
        "#);

        // This should either:
        // 1. Compile successfully with postcondition verified (Z3 solves it fast), OR
        // 2. Compile successfully with a runtime assertion fallback (Z3 timed out)
        // It should NOT hang or fail with a hard error on timeout.
        assert!(
            result.is_ok(),
            "Z3 timeout on complex postcondition should result in graceful \
             degradation (runtime assertion), not a compilation failure.\n\
             Error: {}",
            result.unwrap_err()
        );
    }
}
