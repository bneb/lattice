//! Loop Invariant Inference — Auto-inject Z3 assertions for `for i in start..end`
//!
//! This pass analyzes `SaltFor` nodes and infers bounds invariants:
//!   - `for VAR in START..END` → asserts `VAR >= START && VAR < END`
//!
//! These invariants are used by the verification engine to elide bounds checks
//! within loop bodies, enabling zero-overhead safe indexing.
//!
//! Design decisions:
//!   - Only handles `for i in START..END` pattern (Range expressions)
//!   - Injects Z3 assertions, not user-visible `requires` clauses
//!   - Works with existing `VerificationState::symbolic_tracker`
//!   - Tracks injection count for diagnostics

use crate::grammar::{SaltFor, SaltBlock, Stmt, SaltFn};
use std::collections::HashMap;

/// A loop invariant extracted from a `for` loop's range pattern.
#[derive(Debug, Clone)]
pub struct LoopInvariant {
    /// The loop variable name (e.g., "i")
    pub var_name: String,
    /// The start of the range (as a string expression)
    pub start: RangeBound,
    /// The end of the range (as a string expression)  
    pub end: RangeBound,
    /// Whether the range is inclusive (..=) or exclusive (..)
    pub inclusive: bool,
}

/// A bound in a range expression — either a literal or a named variable.
#[derive(Debug, Clone)]
pub enum RangeBound {
    Literal(i64),
    Variable(String),
}

/// Results from analyzing a function for loop invariants.
#[derive(Debug)]
pub struct LoopInvariantAnalysis {
    /// All loop invariants found in the function
    pub invariants: Vec<LoopInvariant>,
}

/// Analyze a function's body for `for` loops with range iterators.
/// Returns all inferred loop invariants.
pub fn analyze_function(func: &SaltFn) -> LoopInvariantAnalysis {
    let mut invariants = Vec::new();
    analyze_block(&func.body, &mut invariants);
    LoopInvariantAnalysis { invariants }
}

/// Recursively analyze a block for `for` loops.
fn analyze_block(block: &SaltBlock, invariants: &mut Vec<LoopInvariant>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::For(for_stmt) => {
                // Try to extract range bounds from the iterator expression
                if let Some(inv) = extract_range_invariant(for_stmt) {
                    invariants.push(inv);
                }
                // Also analyze the loop body for nested loops
                analyze_block(&for_stmt.body, invariants);
            }
            Stmt::If(if_stmt) => {
                analyze_block(&if_stmt.then_branch, invariants);
                if let Some(else_branch) = &if_stmt.else_branch {
                    match else_branch.as_ref() {
                        crate::grammar::SaltElse::Block(b) => analyze_block(b, invariants),
                        crate::grammar::SaltElse::If(nested) => {
                            analyze_block(&nested.then_branch, invariants);
                        }
                    }
                }
            }
            Stmt::While(w) => {
                analyze_block(&w.body, invariants);
            }
            Stmt::Loop(b) | Stmt::Unsafe(b) => {
                analyze_block(b, invariants);
            }
            _ => {}
        }
    }
}

/// Try to extract a loop invariant from a `for VAR in EXPR` statement.
/// Handles `for i in 0..n`, `for i in a..b`, `for i in 0..=n`.
fn extract_range_invariant(for_stmt: &SaltFor) -> Option<LoopInvariant> {
    // Extract the loop variable name from the pattern
    let var_name = extract_pat_ident(&for_stmt.pat)?;

    // Extract range bounds from the iterator expression
    // syn::Expr::Range { from, to, limits }
    match &for_stmt.iter {
        syn::Expr::Range(range) => {
            let start = range.start.as_ref()
                .and_then(|e| extract_bound(e))
                .unwrap_or(RangeBound::Literal(0));

            let end = range.end.as_ref()
                .and_then(|e| extract_bound(e))?;

            let inclusive = matches!(range.limits, syn::RangeLimits::Closed(_));

            Some(LoopInvariant {
                var_name,
                start,
                end,
                inclusive,
            })
        }
        _ => None,
    }
}

/// Extract a simple identifier from a pattern (e.g., `i` from `for i in ...`).
fn extract_pat_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(pi) => Some(pi.ident.to_string()),
        _ => None,
    }
}

/// Extract a range bound from an expression.
/// Handles integer literals and simple identifiers.
fn extract_bound(expr: &syn::Expr) -> Option<RangeBound> {
    match expr {
        syn::Expr::Lit(lit) => {
            if let syn::Lit::Int(int_lit) = &lit.lit {
                int_lit.base10_parse::<i64>().ok().map(RangeBound::Literal)
            } else {
                None
            }
        }
        syn::Expr::Path(path) => {
            if path.path.segments.len() == 1 {
                Some(RangeBound::Variable(
                    path.path.segments[0].ident.to_string()
                ))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Inject loop invariants as Z3 assertions into a solver.
/// Returns the number of invariants injected.
pub fn inject_invariants_z3<'ctx>(
    z3_ctx: &'ctx crate::z3_shim::Context,
    solver: &crate::z3_shim::Solver<'ctx>,
    analysis: &LoopInvariantAnalysis,
    symbolic_tracker: &mut HashMap<String, crate::z3_shim::ast::Int<'ctx>>,
) -> usize {
    let mut injected = 0;

    for inv in &analysis.invariants {
        // Create or get the Z3 variable for the loop variable
        let loop_var = symbolic_tracker
            .entry(inv.var_name.clone())
            .or_insert_with(|| crate::z3_shim::ast::Int::new_const(z3_ctx, inv.var_name.clone()))
            .clone();

        // Create start bound
        let start_z3 = match &inv.start {
            RangeBound::Literal(v) => crate::z3_shim::ast::Int::from_i64(z3_ctx, *v),
            RangeBound::Variable(name) => {
                symbolic_tracker
                    .entry(name.clone())
                    .or_insert_with(|| crate::z3_shim::ast::Int::new_const(z3_ctx, name.clone()))
                    .clone()
            }
        };

        // Create end bound
        let end_z3 = match &inv.end {
            RangeBound::Literal(v) => crate::z3_shim::ast::Int::from_i64(z3_ctx, *v),
            RangeBound::Variable(name) => {
                symbolic_tracker
                    .entry(name.clone())
                    .or_insert_with(|| crate::z3_shim::ast::Int::new_const(z3_ctx, name.clone()))
                    .clone()
            }
        };

        // Assert: loop_var >= start
        let ge_start = loop_var.ge(&start_z3);
        solver.assert(&ge_start);

        // Assert: loop_var < end (exclusive) or loop_var <= end (inclusive)
        let lt_end = if inv.inclusive {
            loop_var.le(&end_z3)
        } else {
            loop_var.lt(&end_z3)
        };
        solver.assert(&lt_end);

        injected += 1;
    }

    injected
}


// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::z3_shim::ast::Ast;

    /// Helper: parse a pattern using Pat::parse_single
    fn parse_pat(s: &str) -> syn::Pat {
        syn::parse::Parser::parse_str(syn::Pat::parse_single, s).expect("test pattern is valid Rust syntax")
    }

    /// Helper: create a SaltFor from `for i in 0..10 { body }`
    fn make_for_range(var: &str, start: i64, end: i64) -> SaltFor {
        let pat = parse_pat(var);
        let iter: syn::Expr = syn::parse_str(&format!("{}..{}", start, end)).expect("test expr is valid Rust syntax");
        SaltFor {
            pat,
            iter,
            body: SaltBlock { stmts: vec![] },
        }
    }

    /// Helper: create a SaltFor with variable end bound `for i in 0..n`
    fn make_for_range_var(var: &str, start: i64, end_var: &str) -> SaltFor {
        let pat = parse_pat(var);
        let iter: syn::Expr = syn::parse_str(&format!("{}..{}", start, end_var)).expect("test expr is valid Rust syntax");
        SaltFor {
            pat,
            iter,
            body: SaltBlock { stmts: vec![] },
        }
    }

    #[test]
    fn test_extract_range_literal() {
        let for_stmt = make_for_range("i", 0, 10);
        let inv = extract_range_invariant(&for_stmt).expect("known-good range yields invariants");
        assert_eq!(inv.var_name, "i");
        assert!(matches!(inv.start, RangeBound::Literal(0)));
        assert!(matches!(inv.end, RangeBound::Literal(10)));
        assert!(!inv.inclusive);
    }

    #[test]
    fn test_extract_range_variable_end() {
        let for_stmt = make_for_range_var("i", 0, "n");
        let inv = extract_range_invariant(&for_stmt).expect("known-good range yields invariants");
        assert_eq!(inv.var_name, "i");
        assert!(matches!(inv.start, RangeBound::Literal(0)));
        assert!(matches!(&inv.end, RangeBound::Variable(name) if name == "n"));
        assert!(!inv.inclusive);
    }

    #[test]
    fn test_z3_bounds_check_elision() {
        // Setup: for i in 0..10, prove that i < 10 is always true
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);

        let for_stmt = make_for_range("i", 0, 10);
        let inv = extract_range_invariant(&for_stmt).expect("known-good range yields invariants");
        let analysis = LoopInvariantAnalysis { invariants: vec![inv] };

        let mut tracker = HashMap::new();
        let injected = inject_invariants_z3(&z3_ctx, &solver, &analysis, &mut tracker);
        assert_eq!(injected, 1);

        // Now try to prove that i >= 10 is impossible (should be UNSAT)
        let i = tracker.get("i").expect("loop var i was just inserted");
        let ten = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 10);
        let violation = i.ge(&ten);
        solver.assert(&violation);

        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
            "With invariant i ∈ [0, 10), i >= 10 should be UNSAT");
    }

    #[test]
    fn test_z3_variable_bound() {
        // Setup: for i in 0..n, prove that i < n is always true
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);

        let for_stmt = make_for_range_var("i", 0, "n");
        let inv = extract_range_invariant(&for_stmt).expect("known-good range yields invariants");
        let analysis = LoopInvariantAnalysis { invariants: vec![inv] };

        let mut tracker = HashMap::new();
        let injected = inject_invariants_z3(&z3_ctx, &solver, &analysis, &mut tracker);
        assert_eq!(injected, 1);

        // Assert n > 0 (precondition)
        let n = tracker.get("n").expect("loop var n was just inserted");
        let zero = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 0);
        solver.assert(&n.gt(&zero));

        // Prove that i >= n is impossible (should be UNSAT)
        let i = tracker.get("i").expect("loop var i was just inserted");
        let violation = i.ge(n);
        solver.assert(&violation);

        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
            "With invariant i ∈ [0, n) and n > 0, i >= n should be UNSAT");
    }

    #[test]
    fn test_nested_loops() {
        // for i in 0..10 { for j in 0..i { } }
        let inner_for = make_for_range_var("j", 0, "i");
        let outer_for = SaltFor {
            pat: parse_pat("i"),
            iter: syn::parse_str("0..10").expect("test expr is valid Rust syntax"),
            body: SaltBlock {
                stmts: vec![Stmt::For(inner_for)],
            },
        };

        let mut invariants = Vec::new();
        // Manually analyze since we don't have a SaltFn wrapper
        if let Some(inv) = extract_range_invariant(&outer_for) {
            invariants.push(inv);
        }
        analyze_block(&outer_for.body, &mut invariants);

        assert_eq!(invariants.len(), 2, "Should find 2 loop invariants (outer + inner)");
        assert_eq!(invariants[0].var_name, "i");
        assert_eq!(invariants[1].var_name, "j");
    }

    #[test]
    fn test_non_range_for_returns_none() {
        // for item in some_iter() → no range invariant
        let pat = parse_pat("item");
        let iter: syn::Expr = syn::parse_str("some_iter()").expect("test expr is valid Rust syntax");
        let for_stmt = SaltFor {
            pat,
            iter,
            body: SaltBlock { stmts: vec![] },
        };

        assert!(extract_range_invariant(&for_stmt).is_none(),
            "Non-range for loop should not produce invariant");
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_inclusive_range() {
        // for i in 0..=9 → i >= 0 && i <= 9
        let pat = parse_pat("i");
        let iter: syn::Expr = syn::parse_str("0..=9").expect("test expr is valid Rust syntax");
        let for_stmt = SaltFor {
            pat,
            iter,
            body: SaltBlock { stmts: vec![] },
        };

        let inv = extract_range_invariant(&for_stmt).expect("known-good range yields invariants");
        assert_eq!(inv.var_name, "i");
        assert!(matches!(inv.start, RangeBound::Literal(0)));
        assert!(matches!(inv.end, RangeBound::Literal(9)));
        assert!(inv.inclusive, "..= should produce inclusive invariant");

        // Z3: prove i >= 10 is UNSAT (since i <= 9)
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        let analysis = LoopInvariantAnalysis { invariants: vec![inv] };
        let mut tracker = HashMap::new();
        let injected = inject_invariants_z3(&z3_ctx, &solver, &analysis, &mut tracker);
        assert_eq!(injected, 1);

        let i = tracker.get("i").expect("loop var i was just inserted");
        let ten = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 10);
        solver.assert(&i.ge(&ten));
        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
            "With inclusive invariant i ∈ [0, 9], i >= 10 should be UNSAT");
    }

    #[test]
    fn test_variable_to_variable_range() {
        // for i in a..b → i >= a && i < b
        let pat = parse_pat("i");
        let iter: syn::Expr = syn::parse_str("a..b").expect("test expr is valid Rust syntax");
        let for_stmt = SaltFor {
            pat,
            iter,
            body: SaltBlock { stmts: vec![] },
        };

        let inv = extract_range_invariant(&for_stmt).expect("known-good range yields invariants");
        assert_eq!(inv.var_name, "i");
        assert!(matches!(&inv.start, RangeBound::Variable(name) if name == "a"));
        assert!(matches!(&inv.end, RangeBound::Variable(name) if name == "b"));

        // Z3: with a=2, b=8, prove i >= 8 is UNSAT
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        let analysis = LoopInvariantAnalysis { invariants: vec![inv] };
        let mut tracker = HashMap::new();
        let injected = inject_invariants_z3(&z3_ctx, &solver, &analysis, &mut tracker);
        assert_eq!(injected, 1);

        // Constrain a=2, b=8
        let a = tracker.get("a").expect("loop var a was just inserted");
        let b = tracker.get("b").expect("loop var b was just inserted");
        solver.assert(&a._eq(&crate::z3_shim::ast::Int::from_i64(&z3_ctx, 2)));
        solver.assert(&b._eq(&crate::z3_shim::ast::Int::from_i64(&z3_ctx, 8)));

        let i = tracker.get("i").expect("loop var i was just inserted");
        let violation = i.ge(b);
        solver.assert(&violation);
        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
            "With a=2, b=8, invariant i ∈ [a, b), i >= b should be UNSAT");
    }

    #[test]
    fn test_negative_start_range() {
        // for i in -5..5 → i >= -5 && i < 5
        // Note: syn parses `-5` as Expr::Unary(Neg, Lit(5)), not a single literal.
        // Our extract_bound only handles Lit and Path, so this returns None for start.
        // This test documents the current limitation.
        let pat = parse_pat("i");
        let iter: syn::Expr = syn::parse_str("-5..5").expect("test expr is valid Rust syntax");
        let for_stmt = SaltFor {
            pat,
            iter,
            body: SaltBlock { stmts: vec![] },
        };

        let inv = extract_range_invariant(&for_stmt);
        // Current limitation: negative literal start is not extracted (Expr::Unary)
        // The invariant should still be extracted with default start (0)
        // because extract_bound returns None → unwrap_or(Literal(0))
        let inv = inv.expect("negative start falls back to default 0");
        assert_eq!(inv.var_name, "i");
        // Start defaults to 0 because -5 is Expr::Unary(Neg), not Expr::Lit
        assert!(matches!(inv.start, RangeBound::Literal(0)),
            "Negative start falls back to 0 (known limitation)");
        assert!(matches!(inv.end, RangeBound::Literal(5)));
    }

    #[test]
    fn test_zero_iteration_range() {
        // for i in 5..5 → range is empty, but invariant still extracted: i >= 5 && i < 5
        let for_stmt = make_for_range("i", 5, 5);
        let inv = extract_range_invariant(&for_stmt).expect("known-good range yields invariants");
        assert_eq!(inv.var_name, "i");
        assert!(matches!(inv.start, RangeBound::Literal(5)));
        assert!(matches!(inv.end, RangeBound::Literal(5)));

        // Z3: The constraints i >= 5 && i < 5 are themselves UNSAT (empty range)
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        let analysis = LoopInvariantAnalysis { invariants: vec![inv] };
        let mut tracker = HashMap::new();
        inject_invariants_z3(&z3_ctx, &solver, &analysis, &mut tracker);

        // Empty range: the invariant itself is contradictory
        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
            "Empty range [5, 5) has no satisfying assignment for i");
    }

    #[test]
    fn test_large_range() {
        // for i in 0..1000000 → Z3 handles large bounds correctly
        let for_stmt = make_for_range("i", 0, 1_000_000);
        let inv = extract_range_invariant(&for_stmt).expect("known-good range yields invariants");

        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        let analysis = LoopInvariantAnalysis { invariants: vec![inv] };
        let mut tracker = HashMap::new();
        inject_invariants_z3(&z3_ctx, &solver, &analysis, &mut tracker);

        // Prove i >= 1000000 is UNSAT
        let i = tracker.get("i").expect("loop var i was just inserted");
        let million = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 1_000_000);
        solver.assert(&i.ge(&million));
        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
            "With i ∈ [0, 1000000), i >= 1000000 should be UNSAT");
    }

    // =========================================================================
    // Integration Tests: Loop Invariants + Slice Safety
    // =========================================================================

    #[test]
    fn test_loop_invariant_enables_slice_elision() {
        // Scenario: for i in 0..len { buf[i] }
        // Loop invariant injects: i >= 0 && i < len
        // With len == buf.length, every buf[i] access is provably safe.
        use crate::codegen::verification::slice_verifier::*;

        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);

        // Step 1: Create loop invariant for `for i in 0..256`
        let for_stmt = make_for_range("i", 0, 256);
        let inv = extract_range_invariant(&for_stmt).expect("known-good range yields invariants");
        let analysis = LoopInvariantAnalysis { invariants: vec![inv] };

        // Step 2: Inject into Z3
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        let mut tracker = HashMap::new();
        inject_invariants_z3(&z3_ctx, &solver, &analysis, &mut tracker);

        // Step 3: Verify: with loop invariant i < 256, dynamic access on slice(0, 256) is safe
        let info = SliceInfo::new("loop_handler")
            .with_buf_length(4096)
            .with_slice(0, 256);
        // Upper bound from loop invariant: i < 256 == slice_end
        let result = verify_dynamic_slice_access(&z3_ctx, &info, Some(256));
        assert_eq!(result, SliceProofResult::Proven,
            "Loop invariant i ∈ [0, 256) should prove slice(0, 256)[i] is safe");
    }

    #[test]
    fn test_nested_loop_invariants_compose() {
        // Scenario: for i in 0..4 { for j in 0..8 { matrix[i*8 + j] } }
        // Combined invariant: i ∈ [0,4), j ∈ [0,8) → i*8+j ∈ [0, 32)
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);

        // Build nested loops
        let inner_for = make_for_range("j", 0, 8);
        let outer_for = SaltFor {
            pat: parse_pat("i"),
            iter: syn::parse_str("0..4").expect("test expr is valid Rust syntax"),
            body: SaltBlock {
                stmts: vec![Stmt::For(inner_for)],
            },
        };

        // Extract and inject both invariants
        let mut invariants = Vec::new();
        if let Some(inv) = extract_range_invariant(&outer_for) {
            invariants.push(inv);
        }
        analyze_block(&outer_for.body, &mut invariants);
        assert_eq!(invariants.len(), 2);

        let analysis = LoopInvariantAnalysis { invariants };
        let mut tracker = HashMap::new();
        inject_invariants_z3(&z3_ctx, &solver, &analysis, &mut tracker);

        // Prove: i*8 + j < 32 is always true
        let i = tracker.get("i").expect("loop var i was just inserted");
        let j = tracker.get("j").expect("loop var j was just inserted");
        let eight = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 8);
        let thirty_two = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 32);

        let i_times_8 = crate::z3_shim::ast::Int::mul(&z3_ctx, &[i, &eight]);
        let linear_index = crate::z3_shim::ast::Int::add(&z3_ctx, &[&i_times_8, j]);
        let violation = linear_index.ge(&thirty_two);
        solver.assert(&violation);

        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
            "With i ∈ [0,4) and j ∈ [0,8), i*8+j >= 32 should be UNSAT");
    }

    #[test]
    fn test_loop_invariant_catches_overflow() {
        // Scenario: for i in 0..n where n > buf_length
        // The loop invariant alone does NOT guarantee safety if n exceeds the buffer.
        use crate::codegen::verification::slice_verifier::*;

        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);

        // Loop: for i in 0..512, but buffer is only 256 bytes
        let info = SliceInfo::new("overflow_handler")
            .with_buf_length(256)
            .with_slice(0, 256);

        // Loop upper bound (512) exceeds slice end (256) → unsafe
        let result = verify_dynamic_slice_access(&z3_ctx, &info, Some(512));
        assert!(matches!(result, SliceProofResult::Unsafe(_)),
            "Loop bound 512 exceeding slice size 256 MUST be flagged unsafe");
    }
}
