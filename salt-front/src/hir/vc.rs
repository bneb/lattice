//! Verification Context: Z3-backed symbolic reasoning for Salt contracts.
//!
//! This module maps Salt's HIR variables to Z3 symbolic integers and uses
//! the UNSAT negation technique to prove pre/post-conditions at compile time.
//!
//! If Z3 returns UNSAT (the negation of the safety condition is unsatisfiable),
//! then the condition holds under all possible executions, and the compiler
//! can elide the runtime check entirely.

use crate::z3_shim::{Context, Solver, SatResult, ast::{Int, Ast}};
use std::collections::HashMap;
use crate::hir::ids::VarId;

/// Z3-backed verification context for proving compile-time safety contracts.
pub struct VerificationContext<'ctx> {
    z3_ctx: &'ctx Context,
    solver: Solver<'ctx>,
    /// Maps a Salt VarId to a Z3 symbolic integer.
    symbolic_vars: HashMap<VarId, Int<'ctx>>,
}

impl<'ctx> VerificationContext<'ctx> {
    /// Create a new verification context backed by the given Z3 context.
    pub fn new(z3_ctx: &'ctx Context) -> Self {
        Self {
            z3_ctx,
            solver: Solver::new(z3_ctx),
            symbolic_vars: HashMap::new(),
        }
    }

    /// Declare a fresh symbolic integer for the given VarId.
    /// Returns a reference to the Z3 integer that can be used in assertions.
    pub fn declare_symbolic(&mut self, var_id: VarId) -> Int<'ctx> {
        let name = format!("v{}", var_id.0);
        let z3_var = Int::new_const(self.z3_ctx, name);
        self.symbolic_vars.insert(var_id, z3_var.clone());
        z3_var
    }

    /// Assert that a variable is bound to a concrete integer value.
    /// Generates the Z3 constraint: v_id == value
    pub fn assert_binding(&mut self, var_id: VarId, value: i64) {
        let z3_var = self.declare_symbolic(var_id);
        let z3_val = Int::from_i64(self.z3_ctx, value);
        self.solver.assert(&z3_var._eq(&z3_val));
    }

    /// Assert an arbitrary Z3 boolean constraint.
    pub fn assert_constraint(&self, constraint: &crate::z3_shim::ast::Bool<'ctx>) {
        self.solver.assert(constraint);
    }

    /// Look up the Z3 symbolic integer for a given VarId.
    pub fn get_symbolic(&self, var_id: VarId) -> Option<&Int<'ctx>> {
        self.symbolic_vars.get(&var_id)
    }

    /// Prove that a safety condition holds under all possible executions.
    ///
    /// Technique: assert the NEGATION of the condition and check satisfiability.
    /// - UNSAT => the negation is impossible => the condition always holds => SAFE
    /// - SAT   => the negation is satisfiable => the condition can be violated => UNSAFE
    /// - Unknown => Z3 timed out or couldn't decide
    pub fn prove_requires(&self, safety_condition: &crate::z3_shim::ast::Bool<'ctx>) -> Result<(), String> {
        self.solver.push(); // Save current state

        // Assert the NEGATION of what we want to prove
        self.solver.assert(&safety_condition.not());

        // Ask Z3 if this broken state is possible
        let result = self.solver.check();
        self.solver.pop(1); // Restore state

        match result {
            SatResult::Unsat => Ok(()), // Impossible to violate. Proved safe.
            SatResult::Sat => Err("Z3 proof failed: contract violation is possible".into()),
            SatResult::Unknown => Err("Z3 timeout: could not prove safety".into()),
        }
    }

    /// Create a Z3 integer constant from a raw i64 value.
    pub fn int_const(&self, value: i64) -> Int<'ctx> {
        Int::from_i64(self.z3_ctx, value)
    }

    /// Inject a mathematical fact as ground truth into the solver.
    ///
    /// Unlike `prove_requires` (which negates and checks UNSAT), this
    /// directly asserts the condition. No push/pop — the fact permanently
    /// restricts the state space for the remainder of this scope.
    pub fn assume_condition(&self, condition: &crate::z3_shim::ast::Bool<'ctx>) {
        self.solver.assert(condition);
    }

    /// Translate a HIR boolean expression into a Z3 Bool.
    ///
    /// Handles `Binary { op: Lt/Le/Gt/Ge/Eq/Ne, lhs, rhs }` where
    /// lhs/rhs are `Var(id)` or `Literal(Int(n))`.
    ///
    /// Returns `None` for expressions the translator cannot model
    /// (graceful degradation rather than a build panic).
    pub fn lower_assume_expr(
        &self,
        expr: &crate::hir::expr::Expr,
    ) -> Option<crate::z3_shim::ast::Bool<'ctx>> {
        use crate::hir::expr::{ExprKind, BinOp, Literal, UnOp};

        match &expr.kind {
            ExprKind::Binary { op, lhs, rhs } => {
                let z3_lhs = self.lower_int_expr(lhs)?;
                let z3_rhs = self.lower_int_expr(rhs)?;

                Some(match op {
                    BinOp::Lt => z3_lhs.lt(&z3_rhs),
                    BinOp::Le => z3_lhs.le(&z3_rhs),
                    BinOp::Gt => z3_lhs.gt(&z3_rhs),
                    BinOp::Ge => z3_lhs.ge(&z3_rhs),
                    BinOp::Eq => z3_lhs._eq(&z3_rhs),
                    BinOp::Ne => z3_lhs._eq(&z3_rhs).not(),
                    _ => return None, // arithmetic ops don't produce Bool
                })
            }
            ExprKind::Unary { op: UnOp::Not, expr: inner } => {
                let z3_inner = self.lower_assume_expr(inner)?;
                Some(z3_inner.not())
            }
            ExprKind::Literal(Literal::Bool(true)) => {
                Some(crate::z3_shim::ast::Bool::from_bool(self.z3_ctx, true))
            }
            ExprKind::Literal(Literal::Bool(false)) => {
                Some(crate::z3_shim::ast::Bool::from_bool(self.z3_ctx, false))
            }
            _ => None, // graceful degradation
        }
    }

    /// Translate a HIR integer expression into a Z3 Int.
    ///
    /// Handles `Var(id)` (looked up in symbolic_vars) and `Literal(Int(n))`.
    fn lower_int_expr(&self, expr: &crate::hir::expr::Expr) -> Option<Int<'ctx>> {
        use crate::hir::expr::{ExprKind, Literal};

        match &expr.kind {
            ExprKind::Var(var_id) => self.symbolic_vars.get(var_id).cloned(),
            ExprKind::Literal(Literal::Int(n)) => Some(Int::from_i64(self.z3_ctx, *n)),
            ExprKind::Field { .. } => {
                // Field access on ctx — treat as opaque for now.
                // Full support requires mapping ctx.__local_N → VarId.
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::z3_shim::Config;

    // ═════════════════════════════════════════════════════════════════════
    // Phase 6: Track B — Z3 Verification Engine
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_vc_trivial_proof() {
        // 5 < 10 is always true — Z3 should prove UNSAT on its negation
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let vc = VerificationContext::new(&ctx);

        let five = Int::from_i64(&ctx, 5);
        let ten = Int::from_i64(&ctx, 10);
        let condition = five.lt(&ten); // 5 < 10

        let result = vc.prove_requires(&condition);
        assert!(result.is_ok(), "Expected proof to succeed for 5 < 10");
    }

    #[test]
    fn test_vc_trivial_failure() {
        // 5 > 10 is always false — Z3 should find a counterexample (SAT on negation)
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let vc = VerificationContext::new(&ctx);

        let five = Int::from_i64(&ctx, 5);
        let ten = Int::from_i64(&ctx, 10);
        let condition = five.gt(&ten); // 5 > 10

        let result = vc.prove_requires(&condition);
        assert!(result.is_err(), "Expected proof to fail for 5 > 10");
        assert!(result.unwrap_err().contains("contract violation"));
    }

    #[test]
    fn test_vc_symbolic_binding_safe() {
        // let x = 5; requires(x < 10) — should be provably safe
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let mut vc = VerificationContext::new(&ctx);

        vc.assert_binding(VarId(0), 5); // x = 5
        let x = vc.get_symbolic(VarId(0)).unwrap().clone();
        let ten = Int::from_i64(&ctx, 10);
        let condition = x.lt(&ten); // x < 10

        let result = vc.prove_requires(&condition);
        assert!(result.is_ok(), "Expected proof: 5 < 10 is always true");
    }

    #[test]
    fn test_vc_symbolic_binding_unsafe() {
        // let x = 15; requires(x < 10) — should be provably unsafe
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let mut vc = VerificationContext::new(&ctx);

        vc.assert_binding(VarId(0), 15); // x = 15
        let x = vc.get_symbolic(VarId(0)).unwrap().clone();
        let ten = Int::from_i64(&ctx, 10);
        let condition = x.lt(&ten); // x < 10

        let result = vc.prove_requires(&condition);
        assert!(result.is_err(), "Expected proof failure: 15 < 10 is false");
    }

    #[test]
    fn test_vc_two_vars() {
        // let a = 3; let b = 7; requires(a + b == 10) — provably safe
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let mut vc = VerificationContext::new(&ctx);

        vc.assert_binding(VarId(0), 3); // a = 3
        vc.assert_binding(VarId(1), 7); // b = 7

        let a = vc.get_symbolic(VarId(0)).unwrap().clone();
        let b = vc.get_symbolic(VarId(1)).unwrap().clone();
        let sum = Int::add(&ctx, &[&a, &b]);
        let ten = Int::from_i64(&ctx, 10);
        let condition = sum._eq(&ten); // a + b == 10

        let result = vc.prove_requires(&condition);
        assert!(result.is_ok(), "Expected proof: 3 + 7 == 10");
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 10 (P2): State Invariant Engine — Solver-Isolation Tests
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_assume_constrains_state() {
        // let x; assume(x < 10); requires(x < 20) → SAFE
        // Z3 knows x < 10, so x < 20 is trivially true.
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let mut vc = VerificationContext::new(&ctx);

        let x = vc.declare_symbolic(VarId(0));
        let ten = Int::from_i64(&ctx, 10);
        let twenty = Int::from_i64(&ctx, 20);

        // Assume: x < 10
        let assume_cond = x.lt(&ten);
        vc.assume_condition(&assume_cond);

        // Requires: x < 20
        let require_cond = x.lt(&twenty);
        let result = vc.prove_requires(&require_cond);
        assert!(result.is_ok(), "x < 10 implies x < 20");
    }

    #[test]
    fn test_assume_does_not_mask_failures() {
        // let x; assume(x < 10); requires(x < 5) → UNSAFE
        // x could be 8, which is < 10 but not < 5.
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let mut vc = VerificationContext::new(&ctx);

        let x = vc.declare_symbolic(VarId(0));
        let ten = Int::from_i64(&ctx, 10);
        let five = Int::from_i64(&ctx, 5);

        vc.assume_condition(&x.lt(&ten)); // assume x < 10

        let result = vc.prove_requires(&x.lt(&five)); // requires x < 5
        assert!(result.is_err(), "x < 10 does NOT imply x < 5 (counterexample: x = 8)");
    }

    #[test]
    fn test_assume_dead_path_elimination() {
        // let x; assume(x > 10); assume(x < 5); requires(false) → SAFE
        // x > 10 AND x < 5 is contradictory. The path is unreachable.
        // Therefore requires(false) is vacuously true.
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let mut vc = VerificationContext::new(&ctx);

        let x = vc.declare_symbolic(VarId(0));
        let ten = Int::from_i64(&ctx, 10);
        let five = Int::from_i64(&ctx, 5);

        vc.assume_condition(&x.gt(&ten));  // assume x > 10
        vc.assume_condition(&x.lt(&five)); // assume x < 5 (contradicts!)

        // requires(false) — a guaranteed panic in reachable code
        let false_cond = crate::z3_shim::ast::Bool::from_bool(&ctx, false);
        let result = vc.prove_requires(&false_cond);
        assert!(result.is_ok(),
            "Contradictory assumes make path unreachable; requires(false) is vacuously safe");
    }

    #[test]
    fn test_assume_type_mismatch() {
        // assume(5 + 10) — the condition is not Bool, it's Int.
        // This tests the typeck layer, not Z3 directly.
        use crate::hir::stmt::{Stmt, StmtKind};
        use crate::hir::expr::{Expr, ExprKind, Literal, BinOp};
        use crate::hir::types::Type;

        let bad_assume = Stmt {
            kind: StmtKind::Assume(Expr {
                kind: ExprKind::Binary {
                    op: BinOp::Add,
                    lhs: Box::new(Expr {
                        kind: ExprKind::Literal(Literal::Int(5)),
                        ty: Type::I64,
                        span: proc_macro2::Span::call_site(),
                    }),
                    rhs: Box::new(Expr {
                        kind: ExprKind::Literal(Literal::Int(10)),
                        ty: Type::I64,
                        span: proc_macro2::Span::call_site(),
                    }),
                },
                ty: Type::I64,
                span: proc_macro2::Span::call_site(),
            }),
            span: proc_macro2::Span::call_site(),
        };

        let mut typeck = crate::hir::typeck::TypeckContext::new();
        let result = typeck.typeck_stmt(&mut bad_assume.clone());
        assert!(result.is_err(), "Assume with non-Bool condition must fail typeck");
        assert!(result.unwrap_err().contains("Compiler Bug"));
    }

    #[test]
    fn test_lower_assume_expr_binary() {
        // lower_assume_expr(x < 10) → Some(z3_bool)
        use crate::hir::expr::{Expr, ExprKind, Literal, BinOp};
        use crate::hir::types::Type;

        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let mut vc = VerificationContext::new(&ctx);

        let _x = vc.declare_symbolic(VarId(0));

        let hir_expr = Expr {
            kind: ExprKind::Binary {
                op: BinOp::Lt,
                lhs: Box::new(Expr {
                    kind: ExprKind::Var(VarId(0)),
                    ty: Type::I64,
                    span: proc_macro2::Span::call_site(),
                }),
                rhs: Box::new(Expr {
                    kind: ExprKind::Literal(Literal::Int(10)),
                    ty: Type::I64,
                    span: proc_macro2::Span::call_site(),
                }),
            },
            ty: Type::Bool,
            span: proc_macro2::Span::call_site(),
        };

        let z3_bool = vc.lower_assume_expr(&hir_expr);
        assert!(z3_bool.is_some(), "Binary Lt with Var and Literal should translate");

        // Now use it: assume x < 10, then prove x < 20
        vc.assume_condition(&z3_bool.unwrap());
        let x = vc.get_symbolic(VarId(0)).unwrap().clone();
        let twenty = Int::from_i64(&ctx, 20);
        let result = vc.prove_requires(&x.lt(&twenty));
        assert!(result.is_ok(), "HIR-lowered assume should constrain state");
    }
}

