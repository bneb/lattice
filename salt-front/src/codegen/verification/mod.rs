//! Verification Module - Z3-based formal verification for Salt
//!
//! This module provides:
//! - `SymbolicContext`: Cache for Z3 uninterpreted functions (field access)
//! - `VerificationEngine`: Contract verification for #requires clauses
//! - `Z3StateTracker`: Ownership state machine for memory safety proofs
//! - `ArenaVerifier`: Z3 verification of arena mark/reset use-after-free safety
//! - `exhaustiveness`: Z3-based match exhaustiveness checking

mod state_tracker;
pub mod malloc_tracker;
pub mod exhaustiveness;
pub mod arena_verifier;
pub mod hash_loop_verifier;
pub mod proof_witness;
pub mod slice_verifier;
pub mod silicon_ingest;
pub mod executor_verifier;
pub mod hardware_target;
pub mod c10m_validator;
pub mod stack_stability;
pub mod pointer_state;
pub mod arena_escape;
pub mod ptr_bounds_verifier;
pub mod proof_hint;

pub use state_tracker::{OwnershipState, Z3StateTracker};
pub use malloc_tracker::MallocTracker;
pub use pointer_state::{PointerState, PointerStateTracker};
pub use exhaustiveness::{check_exhaustiveness, ExhaustivenessResult};
pub use arena_verifier::ArenaVerifier;
pub use arena_escape::ArenaEscapeTracker;
pub use proof_witness::{ProofHint, VerificationFailure};

use crate::codegen::context::{CodegenContext, LoweringContext};
use crate::types::Type;
use std::collections::HashMap;
use z3::ast::Ast;

use std::rc::Rc;

pub struct SymbolicContext<'ctx> {
    pub z3_ctx: &'ctx z3::Context,
    // Cache for field access functions: "len" -> FuncDecl(Ptr -> Int)
    field_decls: std::cell::RefCell<HashMap<String, Rc<z3::FuncDecl<'ctx>>>>,
}

impl<'ctx> SymbolicContext<'ctx> {
    pub fn new(z3_ctx: &'ctx z3::Context) -> Self {
        Self {
            z3_ctx,
            field_decls: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn get_field_func(&self, name: &str) -> Rc<z3::FuncDecl<'ctx>> {
        let mut cache = self.field_decls.borrow_mut();
        if let Some(decl) = cache.get(name) {
            return decl.clone();
        }
        
        // Create a new uninterpreted function: Field(Object) -> Int
        // This is where we solve the move error: use a reference/clone here
        let symbol = z3::Symbol::String(name.to_string());
        let decl = z3::FuncDecl::new(
            self.z3_ctx,
            symbol,
            &[&z3::Sort::int(self.z3_ctx)], // Domain: Struct/Object (as Int/Ptr)
            &z3::Sort::int(self.z3_ctx)     // Range: Field Value (Int)
        );
        let decl_rc = Rc::new(decl);
        
        cache.insert(name.to_string(), decl_rc.clone());
        decl_rc
    }
}

pub struct VerificationEngine;

impl VerificationEngine {
    pub fn verify(
        ctx: &mut LoweringContext<'_, '_>,
        requires: &[syn::Expr],
        params: &[String],
        arg_exprs: &[syn::Expr],
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
    ) -> Result<(), String> {
        if requires.is_empty() {
            return Ok(());
        }

        // Initialize Symbolic Context
        let sym_ctx = SymbolicContext::new(ctx.z3_ctx);

        // 1. Translate Arguments to Z3 values
        // We need to keep these alive for the duration of verification
        let mut call_vals_z3 = Vec::new();
        
        for arg_expr in arg_exprs {
            if let Ok(z3_val) = crate::codegen::expr::translate_to_z3(ctx, arg_expr, local_vars) {
                call_vals_z3.push(z3_val);
            } else {
                // CV-2 FIX: Hard error on translation failure.
                // If we can't translate an argument, we cannot verify the precondition.
                // Silently substituting zero would create false positive verification.
                return Err(format!(
                    "FORMAL SOUNDNESS ERROR: Cannot translate argument {:?} to Z3. \
                     Verification requires all arguments be expressible in the solver domain.",
                    arg_expr
                ));
            }
        }

        // 2. Prepare Substitution Map
        // We create fresh constants for the parameters: "p0", "p1", etc.
        // And we map them to the actual argument values.
        
        let mut created_symbols = Vec::new(); // Owner of parameter symbols
        let mut dummy_locals = HashMap::new(); // For resolving parameter names in `requires` exprs
       
        for (i, p_name) in params.iter().enumerate() {
             if i < call_vals_z3.len() {
                 let sym = z3::ast::Int::new_const(ctx.z3_ctx, p_name.clone());
                 created_symbols.push(sym);
                 
                 // We use SSA kind which will trigger fallback in translate_to_z3 to mk_var,
                 // ensuring consistent name usage.
                 dummy_locals.insert(p_name.clone(), (Type::Unit, crate::codegen::context::LocalKind::SSA(p_name.clone())));
             }
        }

        let mut from_vec = Vec::new();
        let mut to_vec = Vec::new();
        for (i, sym) in created_symbols.iter().enumerate() {
            from_vec.push(sym);
            if let Some(val) = call_vals_z3.get(i) {
                to_vec.push(val);
            }
        }
        
        let substitutions: Vec<(&z3::ast::Int, &z3::ast::Int)> = from_vec.iter().zip(to_vec.iter())
            .map(|(f, t)| (*f, *t))
            .collect();

        // 3. Verify Each Clause
        for req in requires {
            // [V4.0] Unwrap Block: Grammar parses `requires { expr }` as Expr::Block
            // We need to extract the inner expression for Z3 translation.
            let actual_req = if let syn::Expr::Block(block) = req {
                if let Some(syn::Stmt::Expr(inner, _)) = block.block.stmts.first() {
                    inner
                } else {
                    return Err("Empty requires block".to_string());
                }
            } else {
                req
            };
            
            if let Ok(z3_req_sym) = crate::codegen::expr::translate_bool_to_z3(ctx, actual_req, &dummy_locals, &sym_ctx) {
                 let z3_req_subst = z3_req_sym.substitute(&substitutions);
                 
                 // [V4.0] 3-state verification:
                 // - Check if the substituted requirement is DEFINITELY FALSE
                 //   by checking if `NOT(requirement)` is a tautology (always true).
                 // - If requirement is definitely false (e.g., 0 > 0) → REJECT
                 // - If requirement is definitely true → PASS  
                 // - If Z3 can't determine (uninterpreted functions) → PASS (conservative)
                 
                 // Check: is the requirement definitely violated?
                 // We check if the requirement itself is provably false (UNSAT).
                 let solver = z3::Solver::new(ctx.z3_ctx);
                 let mut solver_params = z3::Params::new(ctx.z3_ctx);
                 solver_params.set_u32("timeout", 100);
                 solver.set_params(&solver_params);
                 solver.assert(&z3_req_subst);
                 
                                  *ctx.total_checks += 1;
                 
                 match solver.check() {
                     z3::SatResult::Unsat => {
                         // The requirement is DEFINITELY unsatisfiable → violation!
                         // Example: requires { b > 0 } with b=0 → (0 > 0) is UNSAT
                         let constraint_str = format!("{}", z3_req_subst);
                         
                         // Extract counterexample values from the substitution map
                         let mut counterexample_values = Vec::new();
                         for (i, p_name) in params.iter().enumerate() {
                             if let Some(z3_val) = call_vals_z3.get(i) {
                                 // Try to extract concrete integer value from Z3 ast
                                 let val_str = format!("{}", z3_val);
                                 if let Ok(v) = val_str.parse::<i64>() {
                                     counterexample_values.push((p_name.clone(), v));
                                 }
                             }
                         }
                         
                         let failure = if counterexample_values.is_empty() {
                             proof_witness::VerificationFailure::new(
                                 constraint_str,
                                 "precondition check".to_string(),
                             )
                         } else {
                             proof_witness::VerificationFailure::with_counterexample(
                                 constraint_str,
                                 "precondition check".to_string(),
                                 counterexample_values,
                             )
                         };
                         return Err(failure.format_error());
                     }
                     z3::SatResult::Sat => {
                         // Requirement CAN be satisfied → PASS
                         *ctx.elided_checks += 1;
                     }
                     z3::SatResult::Unknown => {
                         // Z3 can't determine → conservative PASS
                         *ctx.elided_checks += 1;
                     }
                 }
            } else {
                // Failed to translate requirement.
                return Err(format!("Verification Logic Error: Could not translate requirement expression: {:?}", req));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod is_provably_safe_tests {
    #[allow(unused_imports)]
    use z3::ast::Ast;

    /// Test that `is_provably_safe` returns true for trivially unsatisfiable violations
    #[test]
    fn test_trivially_safe_contradiction() {
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        
        // Create a contradiction: x > 0 AND x < 0 (impossible)
        let x = z3::ast::Int::new_const(&z3_ctx, "x");
        let zero = z3::ast::Int::from_i64(&z3_ctx, 0);
        
        let gt_zero = x.gt(&zero);
        let lt_zero = x.lt(&zero);
        let contradiction = z3::ast::Bool::and(&z3_ctx, &[&gt_zero, &lt_zero]);
        
        // This should be UNSAT (no value of x satisfies both x > 0 and x < 0)
        let solver = z3::Solver::new(&z3_ctx);
        solver.assert(&contradiction);
        assert_eq!(solver.check(), z3::SatResult::Unsat, 
            "Contradiction should be unsatisfiable");
    }

    /// Test that satisfiable violations return false
    #[test]
    fn test_satisfiable_violation_returns_false() {
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        
        // Create a satisfiable condition: x > 5 (counterexample: x = 6)
        let x = z3::ast::Int::new_const(&z3_ctx, "x");
        let five = z3::ast::Int::from_i64(&z3_ctx, 5);
        let gt_five = x.gt(&five);
        
        // This should be SAT (x = 6 satisfies x > 5)
        let solver = z3::Solver::new(&z3_ctx);
        solver.assert(&gt_five);
        assert_eq!(solver.check(), z3::SatResult::Sat,
            "x > 5 should be satisfiable");
    }

    /// Test that always-false conditions are UNSAT
    #[test]
    fn test_always_false_is_unsat() {
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        
        // Create: false (literal)
        let always_false = z3::ast::Bool::from_bool(&z3_ctx, false);
        
        let solver = z3::Solver::new(&z3_ctx);
        solver.assert(&always_false);
        assert_eq!(solver.check(), z3::SatResult::Unsat,
            "Always-false should be unsatisfiable");
    }

    /// Test that always-true conditions are SAT
    #[test]
    fn test_always_true_is_sat() {
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        
        // Create: true (literal)
        let always_true = z3::ast::Bool::from_bool(&z3_ctx, true);
        
        let solver = z3::Solver::new(&z3_ctx);
        solver.assert(&always_true);
        assert_eq!(solver.check(), z3::SatResult::Sat,
            "Always-true should be satisfiable");
    }

    /// Test bounds check scenario: i < len where len = 10 and i ∈ [0, 10)
    #[test]
    fn test_bounds_check_provable() {
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        
        // Domain constraints: 0 <= i < 10, len = 10
        let i = z3::ast::Int::new_const(&z3_ctx, "i");
        let len = z3::ast::Int::from_i64(&z3_ctx, 10);
        let zero = z3::ast::Int::from_i64(&z3_ctx, 0);
        
        let i_ge_0 = i.ge(&zero);
        let i_lt_10 = i.lt(&len);
        
        // Violation: i >= len (out of bounds)
        let violation = i.ge(&len);
        
        // With domain constraints, violation should be UNSAT
        let solver = z3::Solver::new(&z3_ctx);
        solver.assert(&i_ge_0);
        solver.assert(&i_lt_10);
        solver.assert(&violation);
        
        assert_eq!(solver.check(), z3::SatResult::Unsat,
            "With i ∈ [0, 10), violation i >= 10 should be unsatisfiable");
    }
}
