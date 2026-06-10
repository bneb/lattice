//! TDD tests for fast-math reduction detection.
//!
//! Verifies that multi-statement loop bodies with reduction patterns
//! (e.g. rmsnorm's `let v = x[i]; ss = ss + v * v;`) emit fast-math
//! flags on the floating-point arithmetic, enabling LLVM to vectorize
//! the reduction with reassociated accumulation.
//!
//! Background: The reduction detector previously required `stmts.len() == 1`.
//! This excluded rmsnorm-style patterns where a load precedes the accumulator
//! update, causing LLVM to emit scalar lane extraction instead of vectorized
//! fmla.4s (NEON) or vfmadd (x86) instructions.

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
    // Test 1: Multi-statement reduction body gets fast-math flags
    // =========================================================================
    //
    // Pattern: `for i in 0..n { let v = x[i]; ss = ss + v * v; }`
    // This is the rmsnorm sum-of-squares pattern. The reduction detector
    // must recognize `ss = ss + expr` even when preceded by let bindings.

    #[test]
    fn test_multi_stmt_reduction_emits_fast_math() {
        let mlir = compile_to_mlir(r#"
            package main

            fn sum_of_squares(x: Ptr<f32>, n: i64) -> f32
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
                let r: f32 = sum_of_squares(p, 10);
                return 0;
            }
        "#);

        // The inner loop should use scf.for with iter_args (register reduction)
        assert!(
            mlir.contains("scf.for") && mlir.contains("iter_args"),
            "Multi-statement reduction should emit scf.for with iter_args, got:\n{}",
            mlir
        );

        // Fast-math flags should be present on the floating-point ops
        // in the reduction body (reassoc + contract enable vectorization)
        assert!(
            mlir.contains("fastmath") || mlir.contains("fastmathFlags"),
            "Reduction body should have fast-math flags for vectorization, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 2: Single-statement reduction still works (regression)
    // =========================================================================

    #[test]
    fn test_single_stmt_reduction_still_works() {
        let mlir = compile_to_mlir(r#"
            package main

            fn dot_product(a: Ptr<f32>, b: Ptr<f32>, n: i64) -> f32
                requires n > 0
            {
                let mut sum: f32 = 0.0f32;
                for i in 0..n {
                    sum = sum + a[i] * b[i];
                }
                return sum;
            }

            fn main() -> i32 {
                let p: Ptr<f32> = 0 as Ptr<f32>;
                let r: f32 = dot_product(p, p, 10);
                return 0;
            }
        "#);

        // Single-statement reduction should still emit scf.for with iter_args
        assert!(
            mlir.contains("scf.for") && mlir.contains("iter_args"),
            "Single-statement reduction should emit scf.for with iter_args, got:\n{}",
            mlir
        );

        assert!(
            mlir.contains("fastmath") || mlir.contains("fastmathFlags"),
            "Single-statement reduction should have fast-math flags, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 3: Three-statement body (two loads + accumulate) 
    // =========================================================================
    //
    // Pattern: `for i in 0..n { let a = x[i]; let b = y[i]; sum = sum + a * b; }`
    // This tests deeper multi-statement support.

    #[test]
    fn test_three_stmt_reduction_emits_fast_math() {
        let mlir = compile_to_mlir(r#"
            package main

            fn weighted_sum(x: Ptr<f32>, w: Ptr<f32>, n: i64) -> f32
                requires n > 0
            {
                let mut sum: f32 = 0.0f32;
                for i in 0..n {
                    let xi: f32 = x[i];
                    let wi: f32 = w[i];
                    sum = sum + xi * wi;
                }
                return sum;
            }

            fn main() -> i32 {
                let p: Ptr<f32> = 0 as Ptr<f32>;
                let r: f32 = weighted_sum(p, p, 10);
                return 0;
            }
        "#);

        assert!(
            mlir.contains("scf.for") && mlir.contains("iter_args"),
            "Three-statement reduction should emit scf.for with iter_args, got:\n{}",
            mlir
        );

        assert!(
            mlir.contains("fastmath") || mlir.contains("fastmathFlags"),
            "Three-statement reduction should have fast-math flags, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 4: Constant-bound reduction emits fast-math (V8 fix)
    // =========================================================================
    //
    // Before V8, only runtime-bound reductions (emit_scf_for_runtime_reduction) 
    // set in_fast_math_reduction = true. Constant-bound reductions 
    // (emit_affine_for_reduction) silently omitted fast-math flags, causing
    // LLVM to emit scalar code instead of vectorized fmla/vfmadd.

    #[test]
    fn test_constant_bound_reduction_emits_fast_math() {
        let mlir = compile_to_mlir(r#"
            package main

            fn sum_128(x: Ptr<f32>) -> f32 {
                let mut ss: f32 = 0.0f32;
                for i in 0..128 {
                    ss = ss + x[i];
                }
                return ss;
            }

            fn main() -> i32 {
                let p: Ptr<f32> = 0 as Ptr<f32>;
                let r: f32 = sum_128(p);
                return 0;
            }
        "#);

        // Should use scf.for with iter_args (constant-bound reduction)
        assert!(
            mlir.contains("scf.for") && mlir.contains("iter_args"),
            "Constant-bound reduction should emit scf.for with iter_args, got:\n{}",
            mlir
        );

        // V8 fix: fast-math flags must now be present
        assert!(
            mlir.contains("fastmath"),
            "Constant-bound reduction should now emit fast-math flags (V8 fix), got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 5: @fast_math attribute enables fast-math on all FP ops
    // =========================================================================
    //
    // A function marked @fast_math should emit fast-math flags on ALL 
    // floating-point operations, not just reductions.

    #[test]
    fn test_fast_math_attr_enables_all_fp_ops() {
        let mlir = compile_to_mlir(r#"
            package main

            @fast_math
            fn fp_scale(x: f32, y: f32) -> f32 {
                let a: f32 = x * y;
                let b: f32 = a + x;
                return b;
            }

            fn main() -> i32 {
                let r: f32 = fp_scale(1.0f32, 2.0f32);
                return 0;
            }
        "#);

        // @fast_math function should have fast-math flags on FP arithmetic
        assert!(
            mlir.contains("fastmath"),
            "@fast_math function should emit fast-math flags on all FP ops, got:\n{}",
            mlir
        );
    }

    // =========================================================================
    // Test 6: Non-@fast_math function does NOT get fast-math (isolation)
    // =========================================================================
    //
    // Ensures the @fast_math flag is properly scoped — a function without the
    // attribute should NOT inherit fast-math from a sibling function.

    #[test]
    fn test_no_fast_math_attr_no_flags() {
        let mlir = compile_to_mlir(r#"
            package main

            fn precise_add(x: f32, y: f32) -> f32 {
                return x + y;
            }

            fn main() -> i32 {
                let r: f32 = precise_add(1.0f32, 2.0f32);
                return 0;
            }
        "#);

        // Function without @fast_math should NOT have fast-math flags
        // (the only arith ops should be standard, not fast-math)
        // We check that the precise_add function's FP add doesn't have fastmath
        let lines: Vec<&str> = mlir.lines().collect();
        let in_precise_fn = lines.iter().any(|l| l.contains("precise_add") && l.contains("func.func"));
        assert!(in_precise_fn, "precise_add function should exist in MLIR output");
        
        // Count fastmath occurrences — should be zero for a file with no reductions
        // and no @fast_math attribute
        let fastmath_count = mlir.matches("fastmath").count();
        assert_eq!(
            fastmath_count, 0,
            "Non-@fast_math function should NOT emit fast-math flags, found {} occurrences:\n{}",
            fastmath_count, mlir
        );
    }

    // =========================================================================
    // Test 7: @fast_math covers both reduction AND non-reduction FP ops
    // =========================================================================
    //
    // This is the Basalt rmsnorm pattern: a reduction (sum of squares) followed
    // by a normalization loop (weight * scale * value). With @fast_math, ALL
    // floating-point ops — not just the reduction accumulator — must get
    // fast-math flags. This is what C's -ffast-math does globally.

    #[test]
    fn test_fast_math_rmsnorm_pattern() {
        let mlir = compile_to_mlir(r#"
            package main

            extern fn sqrtf(x: f32) -> f32;

            @fast_math
            fn my_rmsnorm(out: Ptr<f32>, x: Ptr<f32>, weight: Ptr<f32>, size: i64)
                requires size > 0
            {
                let mut ss: f32 = 0.0f32;
                for i in 0..size {
                    let v: f32 = x[i];
                    ss = ss + v * v;
                }
                ss = ss / (size as f32);
                ss = ss + 0.00001f32;
                ss = 1.0f32 / sqrtf(ss);
                for i in 0..size {
                    let w: f32 = weight[i];
                    let v: f32 = x[i];
                    out[i] = w * (ss * v);
                }
            }

            fn main() -> i32 {
                let p: Ptr<f32> = 0 as Ptr<f32>;
                my_rmsnorm(p, p, p, 10);
                return 0;
            }
        "#);

        // The reduction loop should have fast-math (from both reduction context AND @fast_math)
        assert!(
            mlir.contains("fastmath"),
            "@fast_math rmsnorm should have fast-math flags on FP arithmetic, got:\n{}",
            mlir
        );

        // Count fastmath occurrences — should be more than just the reduction ops
        // (the normalization multiply `w * (ss * v)` should also have fast-math)
        let fastmath_count = mlir.matches("fastmath").count();
        assert!(
            fastmath_count >= 3,
            "@fast_math rmsnorm should have fast-math on reduction AND normalization ops, found only {} occurrences:\n{}",
            fastmath_count, mlir
        );
    }
}
