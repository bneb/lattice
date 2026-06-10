//! TDD tests for kernel halt codegen.
//!
//! Verifies that:
//!   1. `extern fn kernel_halt()` is declared as a function in MLIR
//!   2. A function calling `kernel_halt()` emits `func.call @kernel_halt`
//!   3. `extern fn disable_interrupts()` is also declared correctly
//!
//! These invariants ensure that `panic.halt()` actually halts the CPU
//! by calling the `cli; hlt` assembly stub in boot.S.

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

    // =========================================================================
    // Test 1: extern fn kernel_halt() generates a declaration in MLIR
    // =========================================================================

    #[test]
    fn test_extern_kernel_halt_declared() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn kernel_halt();
            fn main() -> i32 {
                kernel_halt();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("@kernel_halt"),
            "extern fn kernel_halt() should be declared in MLIR, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 2: main() calls kernel_halt
    // =========================================================================

    #[test]
    fn test_main_calls_kernel_halt() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn kernel_halt();
            fn main() -> i32 {
                kernel_halt();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("call @kernel_halt()"),
            "main() should emit call to @kernel_halt, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 3: disable_interrupts + halt pattern (the safe halt sequence)
    // =========================================================================

    #[test]
    fn test_disable_then_halt_pattern() {
        let mlir = compile_to_mlir(r#"
            package main
            extern fn disable_interrupts();
            extern fn kernel_halt();
            fn main() -> i32 {
                disable_interrupts();
                kernel_halt();
                return 0;
            }
        "#);

        // Both calls must be present in the function body
        assert!(
            mlir.contains("call @disable_interrupts()"),
            "main should call disable_interrupts, got:\n{}",
            mlir
        );
        assert!(
            mlir.contains("call @kernel_halt()"),
            "main should call kernel_halt, got:\n{}",
            mlir
        );
    }
}
