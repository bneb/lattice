//! TDD tests for MLIR DataLayout & TargetTriple emission.
//!
//! Verifies that the emitted MLIR module includes:
//!   - `llvm.data_layout` with the x86-64 System V layout string
//!   - `llvm.target_triple` with the freestanding ELF target
//!
//! Without these attributes, MLIR's constant folder uses naive packing
//! (e.g., 20-byte stride for {i64, i64, i8}), while LLVM's runtime GEP
//! uses ABI-aligned 24-byte stride — causing a 4-byte-per-element drift
//! that corrupts struct arrays and triggers Triple Faults in the kernel.

#[cfg(test)]
mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    /// Helper: compile a Salt source string via drive_codegen() and return MLIR.
    fn compile_to_mlir(source: &str) -> String {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    // =========================================================================
    // Test 1: Normal program emits llvm.data_layout attribute
    // =========================================================================

    #[test]
    fn test_module_has_data_layout() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.data_layout"),
            "MLIR module must include llvm.data_layout attribute, got:\n{}",
            mlir
        );
        // Verify it contains the essential x86-64 alignment specs
        assert!(
            mlir.contains("i64:64"),
            "DataLayout must specify i64:64 alignment for x86-64, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 2: Normal program emits llvm.target_triple attribute
    // =========================================================================

    #[test]
    fn test_module_has_target_triple() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                return 0;
            }
        "#);

        assert!(
            mlir.contains("llvm.target_triple"),
            "MLIR module must include llvm.target_triple attribute, got:\n{}",
            mlir
        );
        assert!(
            mlir.contains("x86_64"),
            "TargetTriple must specify x86_64 architecture, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 3: Module attributes appear on the `module` keyword line
    //         (i.e., `module attributes {... } {` not as a separate op)
    // =========================================================================

    #[test]
    fn test_data_layout_on_module_line() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                return 0;
            }
        "#);

        // Find the line containing `module` and verify it has the attributes
        let module_line = mlir.lines()
            .find(|l| l.contains("module"))
            .expect("MLIR output must contain a 'module' line");

        assert!(
            module_line.contains("llvm.data_layout") && module_line.contains("llvm.target_triple"),
            "Both data_layout and target_triple must be on the module line.\nModule line: {}",
            module_line
        );
    }

    // =========================================================================
    // Test 4: Empty module (lib mode with no public functions) also gets attrs
    // =========================================================================

    #[test]
    fn test_empty_module_has_data_layout() {
        // A lib-mode compilation with no public/no_mangle functions
        // falls through to the `module {}` empty path.
        let file: SaltFile = syn::parse_str(r#"
            package test::empty
            fn private_fn() -> i32 {
                return 42;
            }
        "#).unwrap();
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.lib_mode = true;
        let mlir = ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e));

        assert!(
            mlir.contains("llvm.data_layout"),
            "Empty module must include llvm.data_layout, got:\n{}",
            mlir
        );
        assert!(
            mlir.contains("llvm.target_triple"),
            "Empty module must include llvm.target_triple, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 5: DataLayout string matches the exact x86-64 System V spec
    // =========================================================================

    #[test]
    fn test_data_layout_exact_string() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                return 0;
            }
        "#);

        let expected_dl = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128";
        assert!(
            mlir.contains(expected_dl),
            "DataLayout must be the exact x86-64 System V string.\nExpected: {}\nMLIR:\n{}",
            expected_dl, mlir
        );
    }

    // =========================================================================
    // Test 6: TargetTriple matches the freestanding kernel target
    // =========================================================================

    #[test]
    fn test_target_triple_exact_string() {
        let mlir = compile_to_mlir(r#"
            package main
            fn main() -> i32 {
                return 0;
            }
        "#);

        let expected_triple = "x86_64-unknown-none-elf";
        assert!(
            mlir.contains(expected_triple),
            "TargetTriple must be the freestanding ELF target.\nExpected: {}\nMLIR:\n{}",
            expected_triple, mlir
        );
    }

    // =========================================================================
    // Test 7: lib_mode emits x86-64 target-cpu, NOT apple-m4
    // =========================================================================
    // REGRESSION TEST: Hardcoded apple-m4 caused salt-opt to segfault when
    // the module triple was x86_64-unknown-none-elf (ARM backend + x86 triple
    // = crash). lib_mode MUST emit x86-64.

    #[test]
    fn test_lib_mode_emits_x86_target_cpu() {
        let file: SaltFile = syn::parse_str(r#"
            package test::crosscomp
            @no_mangle
            pub fn sip_entry() -> i32 {
                return 42;
            }
        "#).unwrap();
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.lib_mode = true;
        let mlir = ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e));

        assert!(
            mlir.contains("x86-64"),
            "lib_mode must emit target-cpu=x86-64 for kernel cross-compilation. \
             apple-m4 causes salt-opt to segfault. MLIR:\n{}",
            mlir
        );
        assert!(
            !mlir.contains("apple-m4"),
            "lib_mode must NOT emit apple-m4 (causes salt-opt ARM/x86 triple conflict). \
             MLIR:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 8: non-lib-mode (native) emits apple-m4 target-cpu
    // =========================================================================

    #[test]
    fn test_native_mode_emits_apple_m4_target_cpu() {
        let mlir = compile_to_mlir(r#"
            package main
            @no_mangle
            pub fn native_fn() -> i32 {
                return 42;
            }
            fn main() -> i32 {
                return native_fn();
            }
        "#);

        // Non-lib mode = native macOS compilation → apple-m4 is correct
        assert!(
            mlir.contains("apple-m4"),
            "Native mode must emit target-cpu=apple-m4 for host compilation. MLIR:\n{}",
            mlir
        );
    }
}
