//! Tests for codegen spill patterns.
//!
//! These tests document the current spill behavior and verify that
//! the spills don't break correctness. Note that Ptr<T> spills in MLIR
//! are eliminated by LLVM's mem2reg during `clang -O3`, so they don't
//! affect final binary performance.

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
        ctx.no_verify = true;
        ctx.register_builtins();
        crate::codegen::register_templates(&ctx, &file);
        crate::codegen::register_signatures(&ctx, &file);
        ctx.init_registry_definitions();
        ctx.scan_defs_from_file(&file, true).unwrap();
        ctx.drive_codegen()
            .unwrap_or_else(|e| panic!("Codegen failed: {}", e))
    }

    // =========================================================================
    // Test 1: Ptr<T> spills exist but reduction detection still works
    // =========================================================================

    #[test]
    fn test_ptr_reduction_correctness() {
        let mlir = compile_to_mlir(r#"
            package main

            fn sum_sq(x: Ptr<f32>, n: i64) -> f32
                requires n > 0
            {
                let mut ss: f32 = 0.0f32;
                for i in 0..n {
                    let v: f32 = x[i];
                    ss = ss + v * v;
                }
                return ss;
            }

            fn main() -> i32 {
                let p: Ptr<f32> = 0 as Ptr<f32>;
                let r: f32 = sum_sq(p, 10);
                return 0;
            }
        "#);

        // Reduction still works with scf.for + iter_args
        assert!(
            mlir.contains("scf.for") && mlir.contains("iter_args"),
            "Reduction detection must work correctly:\n{}",
            mlir
        );

        // Fast-math still present
        assert!(
            mlir.contains("fastmath"),
            "Fast-math flags must be emitted on reduction ops:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 2: Multi-Ptr kernel compiles correctly
    // =========================================================================

    #[test]
    fn test_multi_ptr_kernel_compiles() {
        let mlir = compile_to_mlir(r#"
            package main

            fn scale_vec(out: Ptr<f32>, x: Ptr<f32>, w: Ptr<f32>, n: i64)
                requires n > 0
            {
                for i in 0..n {
                    let xi: f32 = x[i];
                    let wi: f32 = w[i];
                    out[i] = wi * xi;
                }
            }

            fn main() -> i32 {
                let p: Ptr<f32> = 0 as Ptr<f32>;
                scale_vec(p, p, p, 10);
                return 0;
            }
        "#);

        // All three Ptr args should generate correct GEP patterns
        assert!(
            mlir.contains("llvm.getelementptr"),
            "Ptr indexing must generate GEP instructions:\n{}",
            mlir
        );

        // The function should compile without errors
        assert!(
            mlir.contains("func.func") && mlir.contains("main__scale_vec"),
            "Function must be emitted correctly:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 3: Runtime-bound non-reduction loops use scf.for (not cf.br)
    // =========================================================================
    // rmsnorm's second loop (`for i in 0..size { out[i] = w*(ss*v) }`) has
    // runtime bounds and no reduction. It should still get scf.for instead
    // of falling back to cf.br basic-block loops.

    #[test]
    fn test_non_reduction_loop_uses_scf_for() {
        let mlir = compile_to_mlir(r#"
            package main

            fn write_loop(out: Ptr<f32>, x: Ptr<f32>, scale: f32, n: i64)
                requires n > 0
            {
                for i in 0..n {
                    let v: f32 = x[i];
                    out[i] = scale * v;
                }
            }

            fn main() -> i32 {
                let p: Ptr<f32> = 0 as Ptr<f32>;
                write_loop(p, p, 1.0f32, 10);
                return 0;
            }
        "#);

        // The non-reduction loop must use scf.for, NOT cf.br
        assert!(
            mlir.contains("scf.for"),
            "Runtime non-reduction loop must use scf.for:\n{}",
            mlir
        );
        // Should NOT fall back to cf.br basic-block loop
        let write_loop_section: &str = mlir.split("main__write_loop").nth(1).unwrap_or("");
        let next_fn = write_loop_section.find("func.func").unwrap_or(write_loop_section.len());
        let write_loop_body = &write_loop_section[..next_fn];
        assert!(
            !write_loop_body.contains("cf.br") && !write_loop_body.contains("cf.cond_br"),
            "Non-reduction loop must NOT use cf.br fallback:\n{}",
            write_loop_body
        );
    }

    // =========================================================================
    // Test 4: @fast_math puts fastmath on ALL float ops (not just reductions)
    // =========================================================================

    #[test]
    fn test_fast_math_attr_all_ops() {
        let mlir = compile_to_mlir(r#"
            package main

            @fast_math
            fn scale_add(out: Ptr<f32>, x: Ptr<f32>, s: f32, n: i64)
                requires n > 0
            {
                for i in 0..n {
                    let v: f32 = x[i];
                    out[i] = s * v;
                }
            }

            fn main() -> i32 {
                let p: Ptr<f32> = 0 as Ptr<f32>;
                scale_add(p, p, 2.0f32, 10);
                return 0;
            }
        "#);

        // The mulf inside the non-reduction loop must have fastmath
        assert!(
            mlir.contains("arith.mulf") && mlir.contains("fastmath"),
            "@fast_math must emit fastmath on ALL float ops, not just reductions:\n{}",
            mlir
        );

        // Specifically verify a non-reduction mulf has the flag
        // (the mulf in `s * v` is NOT a reduction accumulator, it's a plain multiply)
        let mulf_lines: Vec<&str> = mlir.lines()
            .filter(|l| l.contains("arith.mulf"))
            .collect();
        for line in &mulf_lines {
            assert!(
                line.contains("fastmath"),
                "Every arith.mulf in @fast_math fn must have fastmath flag: {}",
                line
            );
        }
    }
}
