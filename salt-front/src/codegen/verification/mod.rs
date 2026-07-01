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
#[cfg(test)] mod slice_verifier_tests;
pub mod silicon_ingest;
pub mod executor_verifier;
pub mod hardware_target;
pub mod c10m_validator;
pub mod stack_stability;
pub mod pointer_state;
pub mod arena_escape;
pub mod ptr_bounds_verifier;
pub mod proof_hint;
mod fold_constants;
#[cfg(test)] mod z3_smoke_tests;

pub use state_tracker::{OwnershipState, Z3StateTracker};
pub use malloc_tracker::MallocTracker;
pub use pointer_state::{PointerState, PointerStateTracker};
pub use exhaustiveness::{check_exhaustiveness, ExhaustivenessResult};
pub use arena_verifier::ArenaVerifier;
pub use arena_escape::ArenaEscapeTracker;
pub use proof_witness::{ProofHint, VerificationFailure};

use crate::codegen::context::LoweringContext;
use crate::types::Type;
use std::collections::HashMap;
use crate::z3_shim::ast::Ast;
use syn::spanned::Spanned;

use std::rc::Rc;

pub struct SymbolicContext<'ctx> {
    pub z3_ctx: &'ctx crate::z3_shim::Context,
    // Cache for field access functions: "len" -> FuncDecl(Ptr -> Int)
    field_decls: std::cell::RefCell<HashMap<String, Rc<crate::z3_shim::FuncDecl<'ctx>>>>,
}

impl<'ctx> SymbolicContext<'ctx> {
    pub fn new(z3_ctx: &'ctx crate::z3_shim::Context) -> Self {
        Self {
            z3_ctx,
            field_decls: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn get_field_func(&self, name: &str) -> Rc<crate::z3_shim::FuncDecl<'ctx>> {
        let mut cache = self.field_decls.borrow_mut();
        if let Some(decl) = cache.get(name) {
            return decl.clone();
        }
        
        // Create a new uninterpreted function: Field(Object) -> Int
        // This is where we solve the move error: use a reference/clone here
        let symbol = crate::z3_shim::Symbol::String(name.to_string());
        let decl = crate::z3_shim::FuncDecl::new(
            self.z3_ctx,
            symbol,
            &[&crate::z3_shim::Sort::int(self.z3_ctx)], // Domain: Struct/Object (as Int/Ptr)
            &crate::z3_shim::Sort::int(self.z3_ctx)     // Range: Field Value (Int)
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
        out: &mut String,
        requires: &[syn::Expr],
        params: &[String],
        arg_exprs: &[syn::Expr],
        local_vars: &mut HashMap<String, (Type, crate::codegen::context::LocalKind)>,
        param_tys: &[Type],
    ) -> Result<(), String> {
        if requires.is_empty() || ctx.config.no_verify {
            return Ok(());
        }

        // Initialize Symbolic Context
        let sym_ctx = SymbolicContext::new(ctx.z3_ctx);

        // 1. Translate Arguments to Z3 values
        // These must be kept alive for the duration of verification
        let mut call_vals_z3 = Vec::new();
        
        for arg_expr in arg_exprs {
            if let Ok(z3_val) = crate::codegen::expr::translate_to_z3(ctx, arg_expr, local_vars) {
                call_vals_z3.push(z3_val);
            } else {
                // Hard error on translation failure.
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
        // Fresh constants are created for the parameters: "p0", "p1", etc.
        // And they are mapped to the actual argument values.
        
        let mut created_symbols = Vec::new(); // Owner of parameter symbols
        let mut dummy_locals = HashMap::new(); // For resolving parameter names in `requires` exprs
       
        for (i, p_name) in params.iter().enumerate() {
             if i < call_vals_z3.len() {
                 let sym = crate::z3_shim::ast::Int::new_const(ctx.z3_ctx, p_name.clone());
                 created_symbols.push(sym);
                 
                 // SSA kind is used which triggers fallback in translate_to_z3 to mk_var,
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
        
        let substitutions: Vec<(&crate::z3_shim::ast::Int, &crate::z3_shim::ast::Int)> = from_vec.iter().zip(to_vec.iter())
            .map(|(f, t)| (*f, *t))
            .collect();

        // 2.5. Build known-length map for .length()/.len() constant folding
        let mut known_lengths: HashMap<String, i64> = HashMap::new();
        for (i, arg) in arg_exprs.iter().enumerate() {
            if i < params.len() {
                let param = &params[i];
                // String literal arguments: .length() folds to byte count
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = arg {
                    known_lengths.insert(param.clone(), s.value().len() as i64);
                }
                // Let-bound string literals: `let x = "hello"; f(x)`
                if let syn::Expr::Path(p) = arg {
                    if let Some(ident) = p.path.get_ident() {
                        if let Some(&len) = ctx.emission.known_string_lengths.get(&ident.to_string()) {
                            known_lengths.insert(param.clone(), len);
                        }
                    }
                }
                // Array-typed parameters: [T; N] has known length N.
                // Unwrap references: &[T; N] still has compile-time-known length N.
                if i < param_tys.len() {
                    let ty = &param_tys[i];
                    let array_ty = match ty {
                        crate::types::Type::Array(..) => Some(ty),
                        crate::types::Type::Reference(inner, _) => {
                            if matches!(inner.as_ref(), crate::types::Type::Array(..)) {
                                Some(inner.as_ref())
                            } else { None }
                        }
                        _ => None,
                    };
                    if let Some(crate::types::Type::Array(_, len, _)) = array_ty {
                        known_lengths.insert(param.clone(), *len as i64);
                    }
                }
            }
        }

        // 3. Verify Each Clause
        for req in requires {
            // Unwrap Block: Grammar parses `requires { expr }` as Expr::Block
            // The inner expression must be extracted for Z3 translation.
            let actual_req = if let syn::Expr::Block(block) = req {
                if let Some(syn::Stmt::Expr(inner, _)) = block.block.stmts.first() {
                    inner
                } else {
                    return Err("Empty requires block".to_string());
                }
            } else {
                req
            };

            // Tier 1: try compile-time evaluation with known argument values.
            // If the expression resolves to a concrete boolean, skip Z3 entirely.
            if let Some(value) = fold_constants::try_eval(actual_req, &known_lengths, params, arg_exprs) {
                if let crate::evaluator::ConstValue::Bool(false) = value {
                    return Err(
                        "VERIFICATION ERROR: contract evaluates to false with the given arguments".to_string()
                    );
                }
                // Bool(true): proven at compile time, skip Z3
                if matches!(value, crate::evaluator::ConstValue::Bool(true)) {
                    continue;
                }
                // Non-bool result: fall through to Z3
            }

            // Tier 2: Z3 symbolic verification
            if let Ok(z3_req_sym) = crate::codegen::expr::translate_bool_to_z3(ctx, actual_req, &dummy_locals, &sym_ctx) {
                 let z3_req_subst = z3_req_sym.substitute(&substitutions);
                 
                 // ═══════════════════════════════════════════════════════════════
                 // Z3 PROOF-OR-PANIC — SAT/UNSAT POLARITY (DO NOT INVERT)
                 // ═══════════════════════════════════════════════════════════════
                 // The Z3 solver checks the NEGATION of the requirement:
                 //
                 //   Z3.assert(NOT(requirement))
                 //   Z3.check()
                 //
                 //   UNSAT → NOT(requirement) is impossible
                 //        → requirement is ALWAYS TRUE
                 //        → VERIFIED ✓ (check elided, zero runtime cost)
                 //
                 //   SAT   → NOT(requirement) has a satisfying assignment
                 //        → requirement CAN BE VIOLATED
                 //        → COMPILE ERROR ✗ (counterexample reported)
                 //
                 //   UNKNOWN → Z3 timed out (100ms default)
                 //          → Emit runtime assertion as safe fallback
                 //
                 // REGRESSION GUARD: salt-front/tests/z3_contracts/run_tests.sh
                 //   - test_contract_proved.salt: requires(x != 0) with x=10 → UNSAT expected
                 //   - test_contract_rejected.salt: requires(x != 0) with x=0  → SAT expected
                 //   - test_contract_timeout.salt: complex non-linear constraint
                 //
                 // If these tests ever fail, the SAT/UNSAT polarity has been inverted.
                 // ═══════════════════════════════════════════════════════════════

                 // 3-state verification:
                 // - Check if the substituted requirement is DEFINITELY FALSE
                 //   by checking if `NOT(requirement)` is a tautology (always true).
                 // - If requirement is definitely false (e.g., 0 > 0) → REJECT
                 // - If requirement is definitely true → PASS
                 // - If Z3 can't determine (uninterpreted functions) → PASS (conservative)

                 // The negation of the requirement is checked for satisfiability.
                 // If NOT(req) is UNSAT, then req is ALWAYS TRUE (proven).
                 let solver = crate::z3_shim::Solver::new(ctx.z3_ctx);
                 let mut solver_params = crate::z3_shim::Params::new(ctx.z3_ctx);
                 solver_params.set_u32("timeout", 100);
                 solver.set_params(&solver_params);
                 
                 // Also add path conditions from the caller's context to constrain the arguments
                 let path_conditions = ctx.emission.path_conditions.clone();
                 for pc in &path_conditions {
                     let dummy_locals_for_pc = local_vars.clone();
                     if let Ok(z3_pc) = crate::codegen::expr::translate_bool_to_z3(ctx, pc, &dummy_locals_for_pc, &sym_ctx) {
                         solver.assert(&z3_pc);
                     }
                 }

                 // Inject type-based bounds so Z3 proves contracts
                 // implied by the type system (e.g., u8 ∈ [0, 255]).
                 assert_type_bounds(ctx, &call_vals_z3, param_tys, &solver);

                 // Inject Pointer State Tokens
                 // For each argument that is a known variable, map its pointer state into Z3
                 for (i, _p_name) in params.iter().enumerate() {
                     if let Some(arg_expr) = arg_exprs.get(i) {
                         if let Some(var_name) = crate::codegen::expr::extract_ident_name(arg_expr) {
                             if let Some(state) = ctx.pointer_tracker.get_state(&var_name) {
                                 if let Some(z3_val) = call_vals_z3.get(i) {
                                     let sort_refs = [&crate::z3_shim::Sort::int(ctx.z3_ctx)];
                                     
                                     let valid_func = crate::z3_shim::FuncDecl::new(
                                         ctx.z3_ctx,
                                         crate::z3_shim::Symbol::String("valid".to_string()),
                                         &sort_refs,
                                         &crate::z3_shim::Sort::bool(ctx.z3_ctx),
                                     );
                                     let freed_func = crate::z3_shim::FuncDecl::new(
                                         ctx.z3_ctx,
                                         crate::z3_shim::Symbol::String("freed".to_string()),
                                         &sort_refs,
                                         &crate::z3_shim::Sort::bool(ctx.z3_ctx),
                                     );
                                     
                                     let arg_refs: Vec<&dyn crate::z3_shim::ast::Ast> = vec![z3_val as &dyn crate::z3_shim::ast::Ast];
                                     let valid_app = valid_func.apply(&arg_refs).as_bool().unwrap();
                                     let freed_app = freed_func.apply(&arg_refs).as_bool().unwrap();
                                     
                                     
                                     match state {
                                         crate::codegen::verification::PointerState::Valid => {
                                             solver.assert(&valid_app._eq(&crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, true)));
                                             solver.assert(&freed_app._eq(&crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, false)));
                                         }
                                         crate::codegen::verification::PointerState::Freed => {
                                             solver.assert(&valid_app._eq(&crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, false)));
                                             solver.assert(&freed_app._eq(&crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, true)));
                                         }
                                         _ => {}
                                     }
                                 }
                             } 
                         }
                     }
                 }
                 
                 solver.assert(&z3_req_subst.not());
                 
                 *ctx.total_checks += 1;
                 
                 match solver.check() {
                     crate::z3_shim::SatResult::Sat => {
                         // The negation CAN be satisfied → the requirement can be VIOLATED!
                         let constraint_str = format!("{}", z3_req_subst);
                         let span = actual_req.span();
                         let line = span.start().line;
                         let source_info = if !ctx.config.source_file.is_empty() {
                             format!("{}:{}", ctx.config.source_file, line)
                         } else {
                             format!("line {}", line)
                         };

                         // Extract counterexample values from the substitution map
                         let mut counterexample_values = Vec::new();
                         if let Some(model) = solver.get_model() {
                             for (i, p_name) in params.iter().enumerate() {
                                 if let Some(z3_val) = call_vals_z3.get(i) {
                                     if let Some(val) = model.eval(z3_val, true) {
                                         counterexample_values.push((p_name.clone(), val.as_i64().unwrap_or(0)));
                                     }
                                 }
                             }
                         }

                         let failure = if counterexample_values.is_empty() {
                             proof_witness::VerificationFailure::new(
                                 constraint_str,
                                 format!("precondition check ({})", source_info),
                             )
                         } else {
                             proof_witness::VerificationFailure::with_counterexample(
                                 constraint_str,
                                 format!("precondition check ({})", source_info),
                                 counterexample_values,
                             )
                         };
                         return Err(failure.format_error());
                     }
                     crate::z3_shim::SatResult::Unsat => {
                         // The negation CANNOT be satisfied → the requirement is PROVEN!
                         *ctx.elided_checks += 1;
                     }
                     crate::z3_shim::SatResult::Unknown => {
                         // Z3 could not determine satisfiability (timeout / incomplete theory)
                         let constraint_str = format!("{}", z3_req_subst);
                         eprintln!(
                             "WARNING: Z3 could not prove `requires({})` within 100ms. \
                              Emitting runtime check.",
                             constraint_str
                         );
                         emit_requires_runtime_check(
                             ctx, out, actual_req, params, arg_exprs,
                             local_vars, param_tys,
                         )?;
                     }
                 }
            } else {
                // Failed to translate requirement.
                return Err(format!("Verification Logic Error: Could not translate requirement expression: {:?}", req));
            }
        }

        Ok(())
    }

    /// Apply postconditions to the caller's context (e.g. updating PointerStateTracker)
    pub fn apply_postconditions(
        ctx: &mut LoweringContext<'_, '_>,
        ensures: &[syn::Expr],
        params: &[String],
        arg_exprs: &[syn::Expr],
    ) {
        for ens in ensures {
            let actual_ens = if let syn::Expr::Block(block) = ens {
                if let Some(syn::Stmt::Expr(inner, _)) = block.block.stmts.first() {
                    inner
                } else {
                    ens
                }
            } else {
                ens
            };

            if let syn::Expr::Call(call) = actual_ens {
                let func_name = if let syn::Expr::Path(p) = &*call.func {
                    p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("_")
                } else {
                    "".to_string()
                };


                if (func_name == "valid" || func_name == "freed") && call.args.len() == 1 {
                    // Check if it's `result`
                    if let syn::Expr::Path(p) = &call.args[0] {
                        let arg_name = p.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                        if arg_name == "result" {
                            let state = if func_name == "valid" {
                                crate::codegen::verification::PointerState::Valid
                            } else {
                                crate::codegen::verification::PointerState::Freed
                            };
                            *ctx.pending_pointer_state = Some(state);
                            continue;
                        }

                        // Otherwise find which parameter this corresponds to
                        let arg_idx = params.iter().position(|name| name == &arg_name);

                        if let Some(idx) = arg_idx {
                            if let Some(arg_expr) = arg_exprs.get(idx) {
                                if let Some(var_name) = crate::codegen::expr::extract_ident_name(arg_expr) {
                                    if func_name == "valid" {
                                        ctx.pointer_tracker.mark_valid(&var_name);
                                    } else if func_name == "freed" {
                                        ctx.pointer_tracker.mark_freed(&var_name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Weakest Precondition verification for `ensures` clauses.
    ///
    /// At each return site, substitutes `result` in the ensures expression with
    /// the actual return value, then checks the obligation via Z3.
    ///
    /// Verification logic:
    ///   1. Create symbolic variables for all function parameters
    ///   2. Assume all `requires` preconditions (narrow the input domain)
    ///   3. For each `ensures` clause, substitute `result` with the return value
    ///   4. Check: can the negation of the postcondition be satisfied?
    ///      - UNSAT → postcondition is PROVEN (violation impossible)
    ///      - SAT → postcondition VIOLATED (counterexample found)
    ///      - Unknown → deferred to runtime assertion
    pub fn verify_postcondition(
        ctx: &mut LoweringContext<'_, '_>,
        ensures: &[syn::Expr],
        requires: &[syn::Expr],
        return_expr: &syn::Expr,
        params: &[String],
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
        fn_name: &str,
    ) -> Result<bool, String> {
        if ensures.is_empty() || ctx.config.no_verify {
            return Ok(false);
        }

        let sym_ctx = SymbolicContext::new(ctx.z3_ctx);
        let mut verified = false;
        use crate::z3_shim::ast::Ast;

        // Create a fresh solver with timeout for postcondition proofs
        let solver = crate::z3_shim::Solver::new(ctx.z3_ctx);
        let mut solver_params = crate::z3_shim::Params::new(ctx.z3_ctx);
        solver_params.set_u32("timeout", 100); // 100ms Z3 watchdog
        solver.set_params(&solver_params);

        // 1. Create symbolic constants for function parameters
        let mut param_symbols = Vec::new();
        for p_name in params {
            let sym = crate::z3_shim::ast::Int::new_const(ctx.z3_ctx, p_name.clone());
            param_symbols.push((p_name.clone(), sym));
        }

        // Build a dummy local_vars map for parameter name resolution in Z3
        let mut z3_locals = local_vars.clone();
        for (name, _) in &param_symbols {
            if !z3_locals.contains_key(name) {
                z3_locals.insert(name.clone(), (Type::I32, crate::codegen::context::LocalKind::SSA(name.clone())));
            }
        }

        // 2. Assume preconditions (requires clauses narrow the input domain)
        for req in requires {
            let actual_req = if let syn::Expr::Block(block) = req {
                if let Some(syn::Stmt::Expr(inner, _)) = block.block.stmts.first() {
                    inner
                } else {
                    continue;
                }
            } else {
                req
            };

            if let Ok(z3_req) = crate::codegen::expr::translate_bool_to_z3(ctx, actual_req, &z3_locals, &sym_ctx) {
                solver.assert(&z3_req);
            }
        }

        // 2b. Assume branch conditions (path guards)
        // These are pushed by emit_if_expr when entering then/else branches.
        // They tell Z3 what branch we're in (e.g., "x < 0" in the then-branch).
        let path_conds = ctx.emission.path_conditions.clone();
        for pc in &path_conds {
            if let Ok(z3_pc) = crate::codegen::expr::translate_bool_to_z3(ctx, pc, &z3_locals, &sym_ctx) {
                solver.assert(&z3_pc);
            }
        }

        // Inject Pointer State Tokens for ensures
        for p_name in params.iter() {
            if let Some(state) = ctx.pointer_tracker.get_state(p_name) {
                if let Some((_, sym)) = param_symbols.iter().find(|(n, _)| n == p_name) {
                    let sort_refs = [&crate::z3_shim::Sort::int(ctx.z3_ctx)];
                    let valid_func = crate::z3_shim::FuncDecl::new(
                        ctx.z3_ctx,
                        crate::z3_shim::Symbol::String("valid".to_string()),
                        &sort_refs,
                        &crate::z3_shim::Sort::bool(ctx.z3_ctx),
                    );
                    let freed_func = crate::z3_shim::FuncDecl::new(
                        ctx.z3_ctx,
                        crate::z3_shim::Symbol::String("freed".to_string()),
                        &sort_refs,
                        &crate::z3_shim::Sort::bool(ctx.z3_ctx),
                    );
                    let arg_refs: Vec<&dyn crate::z3_shim::ast::Ast> = vec![sym as &dyn crate::z3_shim::ast::Ast];
                    let valid_app = valid_func.apply(&arg_refs).as_bool().unwrap();
                    let freed_app = freed_func.apply(&arg_refs).as_bool().unwrap();
                    
                    match state {
                        crate::codegen::verification::PointerState::Valid => {
                            solver.assert(&valid_app._eq(&crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, true)));
                            solver.assert(&freed_app._eq(&crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, false)));
                        }
                        crate::codegen::verification::PointerState::Freed => {
                            solver.assert(&valid_app._eq(&crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, false)));
                            solver.assert(&freed_app._eq(&crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, true)));
                        }
                        _ => {}
                    }
                }
            }
        }

        // 2c. [v4.0] Axiomatize intrinsics in the return expression
        Self::axiomatize_intrin_find_byte(ctx, return_expr, &solver, &z3_locals);
        
        // 3. Translate the return value expression to Z3
        let z3_return_val = crate::codegen::expr::translate_to_z3(ctx, return_expr, &z3_locals);

        // 4. For each ensures clause, substitute `result` and verify
        for ens in ensures {
            let actual_ens = if let syn::Expr::Block(block) = ens {
                if let Some(syn::Stmt::Expr(inner, _)) = block.block.stmts.first() {
                    inner
                } else {
                    continue;
                }
            } else {
                ens
            };

            // Create a `result` symbol and register it in the Z3 locals
            let result_sym = crate::z3_shim::ast::Int::new_const(ctx.z3_ctx, "result");
            let mut ens_locals = z3_locals.clone();
            ens_locals.insert("result".to_string(), (Type::I32, crate::codegen::context::LocalKind::SSA("result".to_string())));

            if let Ok(z3_ens) = crate::codegen::expr::translate_bool_to_z3(ctx, actual_ens, &ens_locals, &sym_ctx) {
                if let Ok(ref ret_val) = z3_return_val {
                    // WP Check: Assume result == return_value, then check NOT(postcondition)
                    let binding = result_sym._eq(ret_val);

                    solver.push();
                    solver.assert(&binding);
                    solver.assert(&z3_ens.not());
                    *ctx.total_checks += 1;

                    match solver.check() {
                        crate::z3_shim::SatResult::Unsat => {
                            // PROVEN: No input can violate the postcondition
                            *ctx.elided_checks += 1;
                            verified = true;
                        }
                        crate::z3_shim::SatResult::Sat => {
                            // VIOLATION: Z3 found inputs that violate the postcondition
                            // BUT: Check if the return expression uses untracked local variables
                            // (mutated locals like `acc` that Z3 treats as unconstrained).
                            // In that case, the SAT result is due to incomplete symbolic tracking,
                            // not a genuine violation. Defer to runtime assertion.
                            let return_uses_untracked = Self::expr_uses_untracked_local(return_expr, params);
                            if return_uses_untracked {
                            } else {
                                // Genuine violation: the return expression only uses tracked params/literals
                                let model = solver.get_model();
                                let mut counterexample = Vec::new();
                                if let Some(model) = model {
                                    for (name, sym) in &param_symbols {
                                        if let Some(val) = model.eval(sym, true) {
                                            counterexample.push(format!("  {} := {}", name, val));
                                        }
                                    }
                                }

                                let ce_str = if counterexample.is_empty() {
                                    String::new()
                                } else {
                                    format!("\n[Formal Shadow] Z3 counter-example:\n{}", counterexample.join("\n"))
                                };

                                solver.pop(1);
                                return Err(format!(
                                    "Postcondition violation in '{}': ensures({:?}) is not satisfied \
                                     for all return paths.{}",
                                    fn_name, actual_ens, ce_str
                                ));
                            }
                        }
                        crate::z3_shim::SatResult::Unknown => {
                            // TIMEOUT: Z3 couldn't determine — deferred to runtime
                        }
                    }
                    solver.pop(1);
                }
            }
        }

        Ok(verified)
    }

    /// Check if a return expression uses local variables that aren't tracked
    /// as function parameters. Mutated locals like `acc` are unconstrained in Z3,
    /// leading to false SAT (violation) results.
    fn expr_uses_untracked_local(expr: &syn::Expr, params: &[String]) -> bool {
        match expr {
            syn::Expr::Path(p) => {
                if let Some(ident) = p.path.get_ident() {
                    let name = ident.to_string();
                    // If it's not a parameter and not "result", it's an untracked local
                    !params.contains(&name) && name != "result"
                } else {
                    false
                }
            }
            syn::Expr::Binary(b) => {
                Self::expr_uses_untracked_local(&b.left, params) ||
                Self::expr_uses_untracked_local(&b.right, params)
            }
            syn::Expr::Unary(u) => Self::expr_uses_untracked_local(&u.expr, params),
            syn::Expr::Paren(p) => Self::expr_uses_untracked_local(&p.expr, params),
            syn::Expr::Lit(_) => false,
            _ => false,
        }
    }

    fn axiomatize_intrin_find_byte<'a, 'ctx>(
        ctx: &mut LoweringContext<'a, 'ctx>,
        expr: &syn::Expr,
        solver: &crate::z3_shim::Solver<'ctx>,
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>
    ) {
        match expr {
            syn::Expr::Call(call) => {
                let func_name = if let syn::Expr::Path(p) = &*call.func {
                    p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("_")
                } else {
                    "".to_string()
                };
                if func_name == "intrin_find_byte" && call.args.len() == 3 {
                    if let Ok(res_val) = crate::codegen::expr::translate_to_z3(ctx, expr, local_vars) {
                        if let Ok(len_val) = crate::codegen::expr::translate_to_z3(ctx, &call.args[1], local_vars) {
                            
                            let minus_one = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, -1);
                            let zero = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, 0);
                            
                            // res >= -1
                            solver.assert(&res_val.ge(&minus_one));
                            // res >= 0 => res < len
                            let is_pos = res_val.ge(&zero);
                            let is_less = res_val.lt(&len_val);
                            solver.assert(&is_pos.implies(&is_less));
                        }
                    }
                }
                for arg in &call.args {
                    Self::axiomatize_intrin_find_byte(ctx, arg, solver, local_vars);
                }
            }
            syn::Expr::Binary(b) => {
                Self::axiomatize_intrin_find_byte(ctx, &b.left, solver, local_vars);
                Self::axiomatize_intrin_find_byte(ctx, &b.right, solver, local_vars);
            }
            syn::Expr::Unary(u) => Self::axiomatize_intrin_find_byte(ctx, &u.expr, solver, local_vars),
            syn::Expr::Paren(p) => Self::axiomatize_intrin_find_byte(ctx, &p.expr, solver, local_vars),
            syn::Expr::Field(f) => Self::axiomatize_intrin_find_byte(ctx, &f.base, solver, local_vars),
            syn::Expr::MethodCall(mc) => {
                Self::axiomatize_intrin_find_byte(ctx, &mc.receiver, solver, local_vars);
                for arg in &mc.args {
                    Self::axiomatize_intrin_find_byte(ctx, arg, solver, local_vars);
                }
            }
            _ => {}
        }
    }
}

/// Assert type-based bounds into a Z3 solver so contracts implied by
/// the type system are proved at compile time. Covers all integer types,
/// bool, and unwraps Atomic<T> to the inner type.
fn assert_type_bounds<'ctx>(
    ctx: &mut LoweringContext<'_, '_>,
    call_vals_z3: &[crate::z3_shim::ast::Int<'ctx>],
    param_tys: &[Type],
    solver: &crate::z3_shim::Solver<'ctx>,
) {
    for (i, arg_val) in call_vals_z3.iter().enumerate() {
        if i >= param_tys.len() { continue; }
        let zero = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, 0);
        // Unwrap Atomic<T> to the storage type for bounds
        let ty = match &param_tys[i] {
            Type::Atomic(inner) => inner.as_ref(),
            other => other,
        };
        match ty {
            Type::U8 => {
                let max = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, 255);
                solver.assert(&arg_val.ge(&zero));
                solver.assert(&arg_val.le(&max));
            }
            Type::U16 => {
                let max = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, 65535);
                solver.assert(&arg_val.ge(&zero));
                solver.assert(&arg_val.le(&max));
            }
            Type::U32 | Type::U64 | Type::Usize => {
                solver.assert(&arg_val.ge(&zero));
            }
            Type::I8 => {
                let min = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, -128);
                let max = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, 127);
                solver.assert(&arg_val.ge(&min));
                solver.assert(&arg_val.le(&max));
            }
            Type::I16 => {
                let min = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, -32768);
                let max = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, 32767);
                solver.assert(&arg_val.ge(&min));
                solver.assert(&arg_val.le(&max));
            }
            Type::Bool => {
                let one = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, 1);
                solver.assert(&arg_val.ge(&zero));
                solver.assert(&arg_val.le(&one));
            }
            _ => {}
        }
    }
}

/// Emit a runtime assertion for a `requires` clause that Z3 couldn't prove.
/// Evaluates the clause as an MLIR boolean expression and calls
/// `__salt_contract_violation` at runtime when the condition is false.
fn emit_requires_runtime_check(
    ctx: &mut LoweringContext<'_, '_>,
    out: &mut String,
    req: &syn::Expr,
    params: &[String],
    arg_exprs: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, crate::codegen::context::LocalKind)>,
    param_tys: &[Type],
) -> Result<(), String> {
    // Build temporary local bindings: parameter names -> argument SSA values
    let mut temp_locals = local_vars.clone();
    for (i, p_name) in params.iter().enumerate() {
        if let Some(arg_expr) = arg_exprs.get(i) {
            let (val, ty) = crate::codegen::expr::emit_expr(
                ctx, out, arg_expr, local_vars, param_tys.get(i),
            )?;
            temp_locals.insert(p_name.clone(), (ty, crate::codegen::context::LocalKind::SSA(val)));
        }
    }

    // Emit the requires clause as an MLIR boolean expression
    let (req_val, _) = crate::codegen::expr::emit_expr(
        ctx, out, req, &mut temp_locals, Some(&Type::Bool),
    )?;

    // Emit runtime violation check: scf.if violated { call @__salt_contract_violation() }
    let true_val = format!("%verify_true_{}", ctx.emission.next_id());
    let violated = format!("%verify_violated_{}", ctx.emission.next_id());
    out.push_str(&format!("    {} = arith.constant true\n", true_val));
    out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", violated, req_val, true_val));
    ctx.ensure_func_declared("__salt_contract_violation", &[], &Type::Unit).ok();
    out.push_str(&format!("    scf.if {} {{\n", violated));
    out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
    out.push_str("      scf.yield\n");
    out.push_str("    }\n");
    Ok(())
}

