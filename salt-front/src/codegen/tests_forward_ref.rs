//! TDD tests for @no_mangle forward reference codegen (Forward Reference Trap).
//!
//! Verifies that @no_mangle functions defined AFTER their call site in the same
//! file are correctly emitted as full function definitions (not just declarations).
//!
//! Bug: In lib mode, the `is_cross_module` check in calls.rs uses
//! `!mangled_name.starts_with(pkg_prefix__)` to detect cross-module calls.
//! For @no_mangle functions, the mangled name is bare (e.g., "sched_yield"),
//! which never starts with the package prefix, causing same-file functions to be
//! misclassified as cross-module. The compiler emits only a forward declaration
//! instead of hydrating the body, and the definition is silently dropped.

#[cfg(test)]
mod tests {
    use crate::grammar::SaltFile;
    use crate::codegen::context::CodegenContext;

    /// Helper: compile a Salt source in lib mode and return MLIR.
    fn compile_lib_to_mlir(source: &str) -> String {
        let file: SaltFile = syn::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}", e));
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        ctx.lib_mode = true;
        ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    // =========================================================================
    // Test 1: @no_mangle function defined AFTER call site emits definition
    // =========================================================================
    // This is THE Forward Reference Trap test.
    // foo() calls bar() at line 3, bar() is defined at line 8.
    // Both are @no_mangle. bar's body MUST appear in MLIR output.

    #[test]
    fn test_forward_ref_no_mangle_emits_definition() {
        let mlir = compile_lib_to_mlir(r#"
            package test.forward

            @no_mangle
            pub fn foo() -> u64 {
                bar();
                return 0;
            }

            @no_mangle
            pub fn bar() {
                return;
            }
        "#);

        // bar must have a DEFINITION (func.func ... { ... }), not just a declaration
        assert!(
            mlir.contains("func.func public @bar()"),
            "@no_mangle pub fn bar() must be defined in MLIR output.\n\
             Expected: func.func public @bar()\n\
             Got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 2: Forward-referenced @no_mangle function's body is emitted
    // =========================================================================
    // Stronger test: bar() has a side effect (returns a value). Verify the
    // function body is present, not just the signature.

    #[test]
    fn test_forward_ref_no_mangle_body_emitted() {
        let mlir = compile_lib_to_mlir(r#"
            package test.forward_body

            @no_mangle
            pub fn caller() -> i64 {
                let x = callee();
                return x;
            }

            @no_mangle
            pub fn callee() -> i64 {
                return 42;
            }
        "#);

        // callee must be defined (not just declared)
        assert!(
            mlir.contains("func.func public @callee()"),
            "callee() must appear as a public definition.\nMLIR:\n{}",
            mlir
        );

        // The constant 42 must appear in the output (proves body was emitted)
        assert!(
            mlir.contains("42"),
            "callee()'s body containing return 42 must be emitted.\nMLIR:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 3: Reverse order works (control test — should always pass)
    // =========================================================================

    #[test]
    fn test_backward_ref_no_mangle_works() {
        let mlir = compile_lib_to_mlir(r#"
            package test.backward

            @no_mangle
            pub fn bar() {
                return;
            }

            @no_mangle
            pub fn foo() -> u64 {
                bar();
                return 0;
            }
        "#);

        assert!(
            mlir.contains("func.func public @bar()"),
            "bar() defined before call site must always work.\nMLIR:\n{}",
            mlir
        );

        assert!(
            mlir.contains("func.func public @foo()"),
            "foo() must be defined.\nMLIR:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 4: Syscall pattern — the real-world case that triggered the bug
    // =========================================================================
    // handle_syscall (line 1) calls sched_yield (line 2).
    // Both are @no_mangle pub fn in the same file.

    #[test]
    fn test_syscall_pattern_forward_ref() {
        let mlir = compile_lib_to_mlir(r#"
            package kernel.core.syscall

            @no_mangle
            pub fn handle_syscall(num: u64) -> u64 {
                if num == 119 {
                    sched_yield();
                    return 0;
                }
                return 0;
            }

            @no_mangle
            pub fn sched_yield() {
                return;
            }
        "#);

        // Both functions must be defined (not just declared)
        assert!(
            mlir.contains("func.func public @handle_syscall"),
            "handle_syscall must be defined.\nMLIR:\n{}",
            mlir
        );
        assert!(
            mlir.contains("func.func public @sched_yield()"),
            "sched_yield must be defined (not just forward-declared).\nMLIR:\n{}",
            mlir
        );

        // The call must reference sched_yield
        assert!(
            mlir.contains("call @sched_yield()"),
            "handle_syscall must call @sched_yield.\nMLIR:\n{}",
            mlir
        );
    }
}
