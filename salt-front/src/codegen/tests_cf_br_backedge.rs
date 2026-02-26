//! TDD Tests — cf.br Back-Edge Regression Guard
//!
//! Guards for the salt-opt / mlir-opt SIGSEGV that occurred when compiling
//! infinite `loop {}` constructs.  The root cause was dead exit blocks
//! with zero predecessors, crashing MLIR's dominance tree computation.
//!
//! Fixed in `Stmt::Loop` (stmt.rs): the exit block is only emitted when
//! a `break` statement targets it.  Otherwise, the loop is marked divergent
//! and no exit block or trailing `func.return` is produced.
//!
//! These tests ensure the fix does not regress.

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

    // ======================================================================
    // Layer 1: Codegen — Infinite Loop (No Break)
    // ======================================================================

    /// Infinite loop in main: must have cf.br back-edge, no dead exit block,
    /// no trailing func.return.
    #[test]
    fn test_infinite_loop_codegen_correctness() {
        let src = r#"
            package main
            extern fn idle_halt();

            fn main() -> i32 {
                loop {
                    idle_halt();
                }
            }
        "#;
        let mlir = compile_to_mlir(src);

        // Must have a cf.br back-edge to the loop body
        assert!(mlir.contains("cf.br ^loop_body"),
            "Infinite loop must have cf.br back-edge:\n{}", mlir);

        // MUST NOT have a dead exit block
        assert!(!mlir.contains("^loop_exit_"),
            "Infinite loop with no break must NOT emit ^loop_exit_ block:\n{}", mlir);

        // MUST NOT have a func.return (body diverges via infinite loop)
        // Main function ends with the cf.br back-edge
        let fn_start = mlir.find("@main").unwrap();
        let fn_body = &mlir[fn_start..];
        assert!(!fn_body.contains("func.return"),
            "Divergent main (infinite loop) must NOT have func.return:\n{}", fn_body);
    }

    /// Code after an infinite loop is unreachable and must not be emitted.
    #[test]
    fn test_infinite_loop_subsequent_code_not_emitted() {
        let src = r#"
            package main
            extern fn idle_halt();
            extern fn should_not_appear();

            fn main() -> i32 {
                loop {
                    idle_halt();
                }
                should_not_appear();
                return 0;
            }
        "#;
        let mlir = compile_to_mlir(src);

        // The call to should_not_appear() must NOT be in the MLIR
        // because emit_block stops after a terminator (the divergent loop).
        let fn_start = mlir.find("@main").unwrap();
        let fn_body = &mlir[fn_start..];
        assert!(!fn_body.contains("@should_not_appear"),
            "Unreachable code after infinite loop must not be emitted:\n{}", fn_body);
    }

    // ======================================================================
    // Layer 2: Codegen — Loop with Break (Finite Exit)
    // ======================================================================

    /// A loop with break must produce an exit block and a func.return.
    #[test]
    fn test_loop_with_break_has_exit_block() {
        let src = r#"
            package main
            extern fn check_done() -> bool;
            extern fn idle_halt();

            fn main() -> i32 {
                loop {
                    let done: bool = check_done();
                    if done {
                        break;
                    }
                    idle_halt();
                }
                return 0;
            }
        "#;
        let mlir = compile_to_mlir(src);

        // Must have an exit block (break was used)
        assert!(mlir.contains("^loop_exit_"),
            "Loop with break MUST have an exit block:\n{}", mlir);

        // Must have a func.return (non-divergent due to break)
        assert!(mlir.contains("func.return"),
            "Non-divergent loop must allow func.return:\n{}", mlir);
    }

    // ======================================================================
    // Layer 3: Pipeline — MLIR Validity (mlir-opt must not crash)
    // ======================================================================

    /// The MLIR for an infinite loop must survive mlir-opt without SIGSEGV.
    /// This is the key regression test for the cf.br back-edge crash.
    ///
    /// We need to add `func.func private` declarations for extern functions
    /// so mlir-opt can validate the call targets.
    #[test]
    fn test_infinite_loop_mlir_survives_opt() {
        let src = r#"
            package main
            extern fn idle_halt();

            fn main() -> i32 {
                loop {
                    idle_halt();
                }
            }
        "#;
        let mlir = compile_to_mlir(src);

        // Inject extern declarations that mlir-opt needs to validate
        let mlir_with_decls = mlir.replace(
            "  func.func",
            "  func.func private @idle_halt() -> ()\n  func.func"
        ).replacen(
            "  func.func private @idle_halt() -> ()\n  func.func private @idle_halt",
            "  func.func private @idle_halt",
            1
        );

        let mlir_opt = std::path::Path::new("/opt/homebrew/opt/llvm@18/bin/mlir-opt");
        if !mlir_opt.exists() {
            eprintln!("SKIP: mlir-opt not found at {:?}", mlir_opt);
            return;
        }

        let tmp_dir = std::env::temp_dir().join("salt_cf_br_test");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let mlir_path = tmp_dir.join("infinite_loop.mlir");
        let out_path = tmp_dir.join("infinite_loop_opt.mlir");
        std::fs::write(&mlir_path, &mlir_with_decls).expect("write MLIR");

        let output = std::process::Command::new(mlir_opt)
            .args(&[
                "--convert-scf-to-cf",
                "--convert-cf-to-llvm",
                "--convert-arith-to-llvm",
                "--finalize-memref-to-llvm",
                "--convert-func-to-llvm",
                "--reconcile-unrealized-casts",
            ])
            .arg(mlir_path.to_str().unwrap())
            .arg("-o")
            .arg(out_path.to_str().unwrap())
            .output()
            .expect("failed to run mlir-opt");

        assert!(output.status.success(),
            "mlir-opt CRASHED on infinite loop MLIR (exit {:?}):\nSTDERR: {}\nMLIR:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
            mlir_with_decls);
    }

    /// Full kmain pattern: void function ending with infinite loop.
    /// Simulates the exact kernel pattern that caused the original crash.
    #[test]
    fn test_kmain_pattern_mlir_survives_opt() {
        let src = r#"
            package main
            extern fn serial_init();
            extern fn idle_halt();

            fn main() -> i32 {
                serial_init();
                loop {
                    idle_halt();
                }
            }
        "#;
        let mlir = compile_to_mlir(src);

        // Inject extern declarations
        let mlir_with_decls = mlir.replace(
            "  func.func",
            "  func.func private @serial_init() -> ()\n  func.func private @idle_halt() -> ()\n  func.func"
        ).replacen(
            "  func.func private @serial_init() -> ()\n  func.func private @idle_halt() -> ()\n  func.func private @serial_init",
            "  func.func private @serial_init",
            1
        ).replacen(
            "  func.func private @idle_halt() -> ()\n  func.func private @idle_halt",
            "  func.func private @idle_halt",
            1
        );

        let mlir_opt = std::path::Path::new("/opt/homebrew/opt/llvm@18/bin/mlir-opt");
        if !mlir_opt.exists() {
            eprintln!("SKIP: mlir-opt not found");
            return;
        }

        let tmp_dir = std::env::temp_dir().join("salt_cf_br_test");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let mlir_path = tmp_dir.join("kmain_loop.mlir");
        let out_path = tmp_dir.join("kmain_loop_opt.mlir");
        std::fs::write(&mlir_path, &mlir_with_decls).expect("write MLIR");

        let output = std::process::Command::new(mlir_opt)
            .args(&[
                "--convert-scf-to-cf",
                "--convert-cf-to-llvm",
                "--convert-arith-to-llvm",
                "--finalize-memref-to-llvm",
                "--convert-func-to-llvm",
                "--reconcile-unrealized-casts",
            ])
            .arg(mlir_path.to_str().unwrap())
            .arg("-o")
            .arg(out_path.to_str().unwrap())
            .output()
            .expect("failed to run mlir-opt");

        assert!(output.status.success(),
            "mlir-opt CRASHED on kmain pattern (exit {:?}):\nSTDERR: {}\nMLIR:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
            mlir_with_decls);
    }
}
