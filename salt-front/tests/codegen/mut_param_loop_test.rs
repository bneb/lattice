// ============================================================================
// Regression Test: mut Parameter Codegen (While Loop Mutation)
//
// Guards against the codegen bug where `mut` function parameters are registered
// as SSA values (immutable registers) instead of alloca'd memory. This causes
// assignments to the parameter inside while loops to be invisible — the loop
// condition always reads the original argument value, making the loop infinite
// or logically wrong.
//
// Root Cause: emit_fn registered all parameters as LocalKind::SSA. When the
// body assigned to a `mut` parameter, emit_lvalue created a temporary spill
// alloca but never updated local_vars, so subsequent reads used the stale SSA.
//
// Fix: Parameters that are `mut` or appear in mutated_vars are promoted to
// LocalKind::Ptr (alloca) at function entry, with the initial SSA arg value
// stored into the alloca.
// ============================================================================

use saltc::compile;

// ============================================================================
// Core Regression: mut parameter in while loop
// ============================================================================

#[test]
fn test_mut_param_promoted_to_alloca() {
    // This is the exact pattern from the KeuOS engine's
    // invalidate_layout function that triggered the bug.
    // `mut node_id` must be stored in an alloca so that `node_id = sentinel`
    // is visible to the while loop condition on the next iteration.
    let code = r#"
        fn walk_up(mut node_id: i64, sentinel: i64) -> i64 {
            let mut count: i64 = 0;
            while node_id != sentinel {
                count = count + 1;
                node_id = sentinel;
            }
            return count;
        }
        fn main() -> i32 {
            let r = walk_up(5, 0);
            return r as i32;
        }
    "#;

    let mlir = compile(code, false, None, true)
        .expect("Failed to compile Salt code");

    // CRITICAL ASSERTION: The mut parameter must be promoted to an alloca.
    // Without the fix, node_id stays as %arg_node_id (SSA) and never changes.
    assert!(
        mlir.contains("%mut_arg_node_id"),
        "mut parameter 'node_id' was NOT promoted to alloca. \
         The codegen will produce incorrect code for while loops. \
         MLIR output:\n{}",
        mlir
    );

    // The alloca must be initialized with the SSA argument value
    assert!(
        mlir.contains("llvm.store %arg_node_id, %mut_arg_node_id"),
        "mut parameter alloca was not initialized with the argument value. \
         MLIR output:\n{}",
        mlir
    );
}

#[test]
fn test_mut_param_loaded_in_loop_condition() {
    // Verify that the while loop condition reads from the alloca,
    // not from the original SSA register.
    let code = r#"
        fn count_down(mut n: i64) -> i64 {
            let mut sum: i64 = 0;
            while n > 0 {
                sum = sum + n;
                n = n - 1;
            }
            return sum;
        }
        fn main() -> i32 {
            let r = count_down(10);
            return r as i32;
        }
    "#;

    let mlir = compile(code, false, None, true)
        .expect("Failed to compile Salt code");

    // The parameter must be promoted
    assert!(
        mlir.contains("%mut_arg_n"),
        "mut parameter 'n' not promoted to alloca.\nMLIR:\n{}",
        mlir
    );

    // The while loop head must load from the alloca (not use %arg_n directly)
    // After the fix, the loop condition will llvm.load from %mut_arg_n
    assert!(
        mlir.contains("llvm.load %mut_arg_n"),
        "While loop condition does not load from mut param alloca.\nMLIR:\n{}",
        mlir
    );
}

#[test]
fn test_immutable_param_stays_ssa() {
    // Non-mut parameters that are never assigned should remain SSA
    // (no unnecessary alloca overhead).
    let code = r#"
        fn identity(x: i64) -> i64 {
            return x;
        }
        fn main() -> i32 {
            let r = identity(42);
            return r as i32;
        }
    "#;

    let mlir = compile(code, false, None, true)
        .expect("Failed to compile Salt code");

    // Should NOT have an alloca for x
    assert!(
        !mlir.contains("%mut_arg_x"),
        "Immutable parameter 'x' was unnecessarily promoted to alloca.\nMLIR:\n{}",
        mlir
    );
}

#[test]
fn test_implicitly_mutated_param_promoted() {
    // Even without the `mut` keyword, if the body assigns to a parameter,
    // mutated_vars detection should promote it.
    let code = r#"
        fn implicit_mut(x: i64) -> i64 {
            x = x + 1;
            return x;
        }
        fn main() -> i32 {
            let r = implicit_mut(5);
            return r as i32;
        }
    "#;

    let mlir = compile(code, false, None, true)
        .expect("Failed to compile Salt code");

    // Should be promoted because it's assigned in the body
    assert!(
        mlir.contains("%mut_arg_x"),
        "Implicitly mutated parameter 'x' was NOT promoted to alloca.\nMLIR:\n{}",
        mlir
    );
}
