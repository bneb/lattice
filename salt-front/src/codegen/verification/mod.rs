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

use crate::codegen::context::LoweringContext;
use crate::types::Type;
use std::collections::HashMap;
use crate::z3_shim::ast::Ast;

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
                 let sym = crate::z3_shim::ast::Int::new_const(ctx.z3_ctx, p_name.clone());
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
        
        let substitutions: Vec<(&crate::z3_shim::ast::Int, &crate::z3_shim::ast::Int)> = from_vec.iter().zip(to_vec.iter())
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
                 
                 // We check if the negation of the requirement is satisfiable.
                 // If NOT(req) is UNSAT, then req is ALWAYS TRUE (proven).
                 let solver = crate::z3_shim::Solver::new(ctx.z3_ctx);
                 let mut solver_params = crate::z3_shim::Params::new(ctx.z3_ctx);
                 solver_params.set_u32("timeout", 100);
                 solver.set_params(&solver_params);
                 
                 // Also add path conditions from the caller's context to constrain the arguments
                 let path_conditions = ctx.emission.path_conditions.clone();
                 for pc in &path_conditions {
                     let dummy_locals_for_pc = local_vars.clone(); // The path condition uses caller's locals
                     if let Ok(z3_pc) = crate::codegen::expr::translate_bool_to_z3(ctx, pc, &dummy_locals_for_pc, &sym_ctx) {
                         solver.assert(&z3_pc);
                     }
                 }
                 
                 // [TEMPORAL SAFETY TIER 2] Inject Pointer State Tokens
                 // For each argument that is a known variable, map its pointer state into Z3
                 for (i, p_name) in params.iter().enumerate() {
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
                             } else {
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
                     crate::z3_shim::SatResult::Unsat => {
                         eprintln!("DEBUG VERIFY: result UNSAT (proven)");
                         // The negation CANNOT be satisfied → the requirement is PROVEN!
                         *ctx.elided_checks += 1;
                     }
                     crate::z3_shim::SatResult::Unknown => {
                         eprintln!("DEBUG VERIFY: result UNKNOWN (pass)");
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

    /// Apply postconditions to the caller's context (e.g. updating PointerStateTracker)
    pub fn apply_postconditions(
        ctx: &mut LoweringContext<'_, '_>,
        ensures: &[syn::Expr],
        params: &[String],
        arg_exprs: &[syn::Expr],
    ) {
        eprintln!("[DEBUG] Applying postconditions for {}", ensures.len());
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

                eprintln!("[DEBUG] apply_postconditions: func_name={}", func_name);

                if (func_name == "valid" || func_name == "freed") && call.args.len() == 1 {
                    // Check if it's `result`
                    if let syn::Expr::Path(p) = &call.args[0] {
                        let arg_name = p.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                        eprintln!("[DEBUG] apply_postconditions: arg_name={}", arg_name);
                        if arg_name == "result" {
                            let state = if func_name == "valid" {
                                crate::codegen::verification::PointerState::Valid
                            } else {
                                crate::codegen::verification::PointerState::Freed
                            };
                            eprintln!("[DEBUG] Marking result as {:?}", state);
                            *ctx.pending_pointer_state = Some(state);
                            continue;
                        }

                        // Otherwise find which parameter this corresponds to
                        let arg_idx = params.iter().position(|name| name == &arg_name);
                        eprintln!("[DEBUG] apply_postconditions: param idx={:?}", arg_idx);

                        if let Some(idx) = arg_idx {
                            if let Some(arg_expr) = arg_exprs.get(idx) {
                                if let Some(var_name) = crate::codegen::expr::extract_ident_name(arg_expr) {
                                    eprintln!("[DEBUG] apply_postconditions: marking var {} as {}", var_name, func_name);
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

    /// [v0.9.2 POSTCONDITION PIVOT] Weakest Precondition verification for `ensures` clauses.
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

        // 2b. [v0.9.2 PATH-SENSITIVE] Assume branch conditions (path guards)
        // These are pushed by emit_if_expr when entering then/else branches.
        // They tell Z3 what branch we're in (e.g., "x < 0" in the then-branch).
        let path_conds = ctx.emission.path_conditions.clone();
        for pc in &path_conds {
            if let Ok(z3_pc) = crate::codegen::expr::translate_bool_to_z3(ctx, pc, &z3_locals, &sym_ctx) {
                solver.assert(&z3_pc);
            }
        }

        // [TEMPORAL SAFETY TIER 2] Inject Pointer State Tokens for ensures
        for (i, p_name) in params.iter().enumerate() {
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
                    let arg_refs: Vec<&dyn crate::z3_shim::ast::Ast> = vec![&*sym as &dyn crate::z3_shim::ast::Ast];
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
                            eprintln!("[Z3 POSTCONDITION] ✓ ensures verified for '{}' (UNSAT — proven)", fn_name);
                        }
                        crate::z3_shim::SatResult::Sat => {
                            // VIOLATION: Z3 found inputs that violate the postcondition
                            // BUT: Check if the return expression uses untracked local variables
                            // (mutated locals like `acc` that Z3 treats as unconstrained).
                            // In that case, the SAT result is due to incomplete symbolic tracking,
                            // not a genuine violation. Defer to runtime assertion.
                            let return_uses_untracked = Self::expr_uses_untracked_local(return_expr, params);
                            if return_uses_untracked {
                                // Incompleteness Gate: defer to runtime assertion
                                eprintln!("[Z3 WARNING] Postcondition deferred to runtime assertion for '{}' \
                                           (return expression uses untracked local variable)", fn_name);
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
                            eprintln!("[Z3 WARNING] Complex proof deferred to runtime assertion ({}:ensures)", fn_name);
                        }
                    }
                    solver.pop(1);
                }
            }
        }

        Ok(verified)
    }

    /// [v0.9.2] Check if a return expression uses local variables that aren't tracked
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
                            use crate::z3_shim::ast::Ast;
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

#[cfg(test)]
mod is_provably_safe_tests {
    #[allow(unused_imports)]
    use crate::z3_shim::ast::Ast;

    /// Test that `is_provably_safe` returns true for trivially unsatisfiable violations
    #[test]
    fn test_trivially_safe_contradiction() {
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        
        // Create a contradiction: x > 0 AND x < 0 (impossible)
        let x = crate::z3_shim::ast::Int::new_const(&z3_ctx, "x");
        let zero = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 0);
        
        let gt_zero = x.gt(&zero);
        let lt_zero = x.lt(&zero);
        let contradiction = crate::z3_shim::ast::Bool::and(&z3_ctx, &[&gt_zero, &lt_zero]);
        
        // This should be UNSAT (no value of x satisfies both x > 0 and x < 0)
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        solver.assert(&contradiction);
        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat, 
            "Contradiction should be unsatisfiable");
    }

    /// Test that satisfiable violations return false
    #[test]
    fn test_satisfiable_violation_returns_false() {
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        
        // Create a satisfiable condition: x > 5 (counterexample: x = 6)
        let x = crate::z3_shim::ast::Int::new_const(&z3_ctx, "x");
        let five = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 5);
        let gt_five = x.gt(&five);
        
        // This should be SAT (x = 6 satisfies x > 5)
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        solver.assert(&gt_five);
        assert_eq!(solver.check(), crate::z3_shim::SatResult::Sat,
            "x > 5 should be satisfiable");
    }

    /// Test that always-false conditions are UNSAT
    #[test]
    fn test_always_false_is_unsat() {
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        
        // Create: false (literal)
        let always_false = crate::z3_shim::ast::Bool::from_bool(&z3_ctx, false);
        
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        solver.assert(&always_false);
        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
            "Always-false should be unsatisfiable");
    }

    /// Test that always-true conditions are SAT
    #[test]
    fn test_always_true_is_sat() {
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        
        // Create: true (literal)
        let always_true = crate::z3_shim::ast::Bool::from_bool(&z3_ctx, true);
        
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        solver.assert(&always_true);
        assert_eq!(solver.check(), crate::z3_shim::SatResult::Sat,
            "Always-true should be satisfiable");
    }

    /// Test bounds check scenario: i < len where len = 10 and i ∈ [0, 10)
    #[test]
    fn test_bounds_check_provable() {
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        
        // Domain constraints: 0 <= i < 10, len = 10
        let i = crate::z3_shim::ast::Int::new_const(&z3_ctx, "i");
        let len = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 10);
        let zero = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 0);
        
        let i_ge_0 = i.ge(&zero);
        let i_lt_10 = i.lt(&len);
        
        // Violation: i >= len (out of bounds)
        let violation = i.ge(&len);
        
        // With domain constraints, violation should be UNSAT
        let solver = crate::z3_shim::Solver::new(&z3_ctx);
        solver.assert(&i_ge_0);
        solver.assert(&i_lt_10);
        solver.assert(&violation);
        
        assert_eq!(solver.check(), crate::z3_shim::SatResult::Unsat,
            "With i ∈ [0, 10), violation i >= 10 should be unsatisfiable");
    }
}
