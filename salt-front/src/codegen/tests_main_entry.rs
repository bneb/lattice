//! TDD tests for main entry point codegen.
//!
//! Verifies that `fn main` in `package main` is emitted as:
//!   1. Unmangled symbol name `@main` (not `@main__main`)
//!   2. Public visibility (not private)
//!
//! These invariants are required for the C linker to find `_main`.

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
    // Test 1: fn main in package main → @main (unmangled)
    // =========================================================================

    #[test]
    fn test_main_symbol_is_unmangled() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                return 0;
            }
        "#);

        // Must contain @main — the C entry point symbol
        assert!(
            mlir.contains("@main("),
            "fn main should emit as @main, but got:\n{}",
            mlir
        );

        // Must NOT contain @main__main — that would break linking
        assert!(
            !mlir.contains("@main__main"),
            "fn main must not be mangled to @main__main, but got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 2: fn main is public (even without pub keyword)
    // =========================================================================

    #[test]
    fn test_main_is_public() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                return 0;
            }
        "#);

        // The main function must be public for the linker to see it.
        // In MLIR: `func.func public @main(...)`
        assert!(
            mlir.contains("func.func public @main("),
            "fn main should have public visibility, but got:\n{}",
            mlir
        );

        // Must NOT be private
        assert!(
            !mlir.contains("func.func private @main("),
            "fn main must not be private, but got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 3: Non-main functions in package main ARE mangled
    // =========================================================================

    #[test]
    fn test_non_main_functions_are_mangled() {
        let mlir = compile_to_mlir(r#"
            package main
            fn helper() -> i32 {
                return 42;
            }
            fn main() -> i32 {
                return helper();
            }
        "#);

        // helper should be mangled as main__helper
        assert!(
            mlir.contains("@main__helper"),
            "Non-main functions should be mangled with package prefix, but got:\n{}",
            mlir
        );

        // main should still be @main (unmangled)
        assert!(
            mlir.contains("func.func public @main("),
            "fn main should remain @main public, but got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 4: fn main without package declaration
    // =========================================================================

    #[test]
    fn test_main_without_package() {
        let mlir = compile_to_mlir(r#"
            fn main() -> i32 {
                return 0;
            }
        "#);

        assert!(
            mlir.contains("@main("),
            "fn main without package should still be @main, but got:\n{}",
            mlir
        );
        assert!(
            !mlir.contains("@main__main"),
            "fn main without package must not be double-mangled, but got:\n{}",
            mlir
        );
    }
}
