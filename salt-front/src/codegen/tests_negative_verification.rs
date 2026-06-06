//! SEC-04: Negative Compilation Tests for Z3 Precondition Verification
//!
//! These tests prove that the Z3 verification engine correctly REJECTS
//! function calls that violate `requires` preconditions, and correctly
//! ACCEPTS calls that satisfy them.
//!
//! This is a critical security property: if a function declares
//! `requires(b > 0)`, then calling it with `b = 0` MUST produce a
//! compilation error. Silent acceptance would be a soundness hole.

#[cfg(test)]
mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    /// Helper: compile a Salt source string and return Ok(mlir) or Err(msg).
    fn try_compile(source: &str) -> Result<String, String> {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
    }

    /// Helper: compile and expect success, returning the MLIR string.
    fn compile_to_mlir(source: &str) -> String {
        try_compile(source)
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    // =========================================================================
    // TEST 1: Precondition violation MUST be rejected
    // =========================================================================
    // A function requires(b > 0), and the caller passes b = 0.
    // Z3 must detect that NOT(0 > 0) is SAT (i.e., 0 > 0 is false),
    // meaning the precondition is violated, and reject the call.

    /// Calling a function with `requires(b > 0)` and `b = 0` MUST fail compilation.
    #[test]
    fn test_precondition_violation_b_equals_zero_rejected() {
        let result = try_compile(r#"
            package main
            fn safe_div(a: i32, b: i32) -> i32
                requires(b > 0)
            {
                return a / b;
            }
            fn main() -> i32 {
                return safe_div(10, 0);
            }
        "#);

        assert!(
            result.is_err(),
            "Z3 must reject safe_div(10, 0): precondition requires(b > 0) is violated \
             when b = 0. Expected compilation error, but compilation succeeded:\n{}",
            result.unwrap_or_default().chars().take(500).collect::<String>()
        );
    }

    // =========================================================================
    // TEST 2: Valid precondition call MUST succeed
    // =========================================================================
    // Same function, but called with b = 5 which satisfies b > 0.

    /// Calling a function with `requires(b > 0)` and `b = 5` MUST succeed.
    #[test]
    fn test_precondition_satisfied_b_equals_five_accepted() {
        let mlir = compile_to_mlir(r#"
            package main
            fn safe_div(a: i32, b: i32) -> i32
                requires(b > 0)
            {
                return a / b;
            }
            fn main() -> i32 {
                return safe_div(10, 5);
            }
        "#);

        // The function should compile successfully — b = 5 satisfies b > 0.
        assert!(
            !mlir.is_empty(),
            "Compilation should succeed when precondition is satisfied (b = 5 > 0)"
        );
    }

    // =========================================================================
    // TEST 3: Negative literal also rejected
    // =========================================================================
    // Calling with b = -1 should also be rejected since -1 > 0 is false.

    /// Calling with `b = -1` must also be rejected (negative value violates b > 0).
    #[test]
    fn test_precondition_violation_negative_b_rejected() {
        let result = try_compile(r#"
            package main
            fn positive_only(x: i32) -> i32
                requires(x > 0)
            {
                return x;
            }
            fn main() -> i32 {
                return positive_only(-1);
            }
        "#);

        assert!(
            result.is_err(),
            "Z3 must reject positive_only(-1): precondition requires(x > 0) is violated \
             when x = -1. Expected compilation error, but compilation succeeded:\n{}",
            result.unwrap_or_default().chars().take(500).collect::<String>()
        );
    }
}
