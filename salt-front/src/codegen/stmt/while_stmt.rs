use crate::grammar::{Stmt};
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;
use syn::spanned::Spanned;

/// Phase A: Prove loop invariants hold at entry (base case).
pub(crate) fn prove_while_loop_base_case(
    ctx: &mut LoweringContext,
    stmts: &[Stmt],
    bv: &HashMap<String, (Type, LocalKind)>,
) -> Result<Vec<syn::Expr>, String> {
    if ctx.config.no_verify { return Ok(vec![]); }
    let sc = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
    let mut inv: Vec<syn::Expr> = Vec::new();
    for s in stmts { if let Stmt::Invariant(e) = s { inv.push(e.clone()); } }
    // Check each invariant in an isolated sub-frame, then assert in parent context.
    // No outer push — surrounding constraints (loop bounds, let bindings) are visible.
    for e in &inv {
        if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, e, bv, &sc) {
            ctx.z3_solver.push(); ctx.z3_solver.assert(&z.not());
            let ck = ctx.z3_solver.check(); ctx.z3_solver.pop(1);
            if ck == crate::z3_shim::SatResult::Sat {
                return Err("Z3 verification failed: loop invariant does not hold at entry.                      The solver found a counterexample proving the invariant is false                      with current variable values.".to_string());
            }
            ctx.z3_solver.assert(&z);
        }
    }
    Ok(inv)
}

/// Phase B: Inductive step for while loop verification.
pub(crate) fn setup_while_loop_inductive_step(
    ctx: &mut LoweringContext,
    stmts: &[Stmt],
    bv: &mut HashMap<String, (Type, LocalKind)>,
    cond: &syn::Expr,
    inv: &[syn::Expr],
) -> Result<(), String> {
    if ctx.config.no_verify { return Ok(()); }
    let sc = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
    ctx.z3_solver.push();
    for n in &crate::codegen::stmt::helpers::collect_mutations(stmts) {
        if let Some((ty, _)) = bv.get(n) {
            if ty.is_integer() {
                let f = format!("{}_havoc_{}", n, ctx.next_id());
                ctx.symbolic_tracker.insert(n.clone(), ctx.mk_var(&f));
            }
        }
    }
    for e in inv {
        if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, e, bv, &sc) {
            ctx.z3_solver.assert(&z);
        }
    }
    if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, cond, bv, &sc) {
        ctx.z3_solver.assert(&z);
    }
    Ok(())
}

/// Try to auto-infer a loop invariant for simple monotonic while loops.
///
/// Supported patterns:
///   `while var < N { var = var + 1 }` → invariant `var >= 0 && var < N`
///   `while var <= N { var = var + 1 }` → invariant `var >= 0 && var <= N`
///   `while cond1 && cond2 { ... }` → tries each sub-condition independently
fn try_infer_while_invariant(
    cond: &syn::Expr,
    body: &[Stmt],
    local_vars: &HashMap<String, (Type, LocalKind)>,
) -> Option<syn::Expr> {
    // 0. Unwrap && conditions: try each side independently.
    if let syn::Expr::Binary(syn::ExprBinary {
        op: syn::BinOp::And(_), left, right, ..
    }) = cond
    {
        if let Some(inv) = try_infer_while_invariant(left, body, local_vars) {
            return Some(inv);
        }
        return try_infer_while_invariant(right, body, local_vars);
    }

    // 1. Parse condition: must be `var < N` or `var <= N` where N is a constant.
    let (var_name, bound, inclusive) = extract_monotonic_bound(cond)?;
    // 2. Find the loop variable's initial value from local_vars.
    // It must have been initialized to a compile-time constant before the loop.
    let (_, kind) = local_vars.get(&var_name)?;
    // Verify the variable has an SSA tracking entry (exists in local_vars).
    if !matches!(kind, LocalKind::SSA(_)) { return None; }
    // 3. Verify the body contains a simple increment: `var = var + K` (K > 0).
    if !has_monotonic_increment(body, &var_name) { return None; }
    // 4. Try to get the initial constant value. We can't easily extract it
    // from Z3 state at this point, so use 0 as the default lower bound.
    // The base case check will reject the invariant if it doesn't hold.
    let lower = 0i64;
    // Synthesize: `invariant var >= lower && var < bound` or `var <= bound`
    if inclusive {
        let inv_str = format!("{} >= {} && {} <= {}", var_name, lower, var_name, bound);
        syn::parse_str(&inv_str).ok()
    } else {
        let inv_str = format!("{} >= {} && {} < {}", var_name, lower, var_name, bound);
        syn::parse_str(&inv_str).ok()
    }
}

/// Extract `(var_name, bound, inclusive)` from a condition like `i < 5` or `i <= n`.
fn extract_monotonic_bound(cond: &syn::Expr) -> Option<(String, i64, bool)> {
    if let syn::Expr::Binary(b) = cond {
        let (var_name, bound, inclusive) = match &b.op {
            syn::BinOp::Lt(_) => {
                let var = extract_var_name(&b.left)?;
                let n = extract_const_i64(&b.right)?;
                (var, n, false)
            }
            syn::BinOp::Le(_) => {
                let var = extract_var_name(&b.left)?;
                let n = extract_const_i64(&b.right)?;
                (var, n, true)
            }
            _ => return None,
        };
        Some((var_name, bound, inclusive))
    } else {
        None
    }
}

/// Extract a variable name from an expression like `i`.
fn extract_var_name(expr: &syn::Expr) -> Option<String> {
    if let syn::Expr::Path(p) = expr {
        p.path.get_ident().map(|id| id.to_string())
    } else { None }
}

/// Extract a compile-time integer constant from an expression like `5`.
fn extract_const_i64(expr: &syn::Expr) -> Option<i64> {
    if let syn::Expr::Lit(lit) = expr {
        if let syn::Lit::Int(i) = &lit.lit {
            i.base10_parse::<i64>().ok()
        } else { None }
    } else { None }
}

/// Check if the loop body monotonically increments `var` by a positive constant.
fn has_monotonic_increment(body: &[Stmt], var_name: &str) -> bool {
    for stmt in body {
        if let Stmt::Syn(s) = stmt {
            let expr = match s {
                syn::Stmt::Expr(e, _) => e,
                _ => continue,
            };
            if let syn::Expr::Binary(syn::ExprBinary {
                ref left, op: syn::BinOp::AddAssign(_), ref right, ..
            }) = expr
            {
                if extract_var_name(left) == Some(var_name.to_string())
                    && extract_const_i64(right).is_some_and(|n| n > 0)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Phase C: Pop inductive scope and assert not(cond) for post-loop.
pub(crate) fn verify_while_loop_post_body(
    ctx: &mut LoweringContext,
    cond: &syn::Expr,
    lv: &HashMap<String, (Type, LocalKind)>,
) {
    if ctx.config.no_verify { return; }
    let sc = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
    ctx.z3_solver.pop(1);
    if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, cond, lv, &sc) {
        ctx.z3_solver.assert(&z.not());
    }
}

pub(crate) fn emit_while_stmt(ctx: &mut LoweringContext, out: &mut String, w: &crate::grammar::SaltWhile, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            let label_header = format!("while_header_{}", ctx.next_id());
            let label_body = format!("while_body_{}", ctx.next_id());
            let label_exit = format!("while_exit_{}", ctx.next_id());

            out.push_str(&format!("    cf.br ^{}\n", label_header));
            out.push_str(&format!("  ^{}:\n", label_header));

            let (cond_val, cond_ty) = emit_expr(ctx, out, &w.cond, local_vars, None)?;
            // Accept Pointer types as while conditions
            let cond_val = if cond_ty.k_is_ptr_type() {
                let id = ctx.next_id();
                let int_val = format!("%ptrtoint_{}", id);
                let zero_val = format!("%ptr_zero_{}", ctx.next_id());
                let cmp_val = format!("%ptr_nonnull_{}", id);
                out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", int_val, cond_val));
                out.push_str(&format!("    {} = arith.constant 0 : i64\n", zero_val));
                out.push_str(&format!("    {} = arith.cmpi ne, {}, {} : i64\n", cmp_val, int_val, zero_val));
                cmp_val
            } else if cond_ty != Type::Bool {
                return Err(format!("While condition must be boolean, found {:?}", cond_ty));
            } else {
                cond_val
            };

            let loc = ctx.loc_tag(w.cond.span());
            out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}{}\n", cond_val, label_body, label_exit, loc));
            out.push_str(&format!("  ^{}:\n", label_body));

            // Heartbeat Injection (simplified, uses @yielding at function level)
            if !*ctx.no_yield() {
                ctx.emit_lto_hook(out, "__salt_yield_check", &[], local_vars, None)?;
            }
            ctx.break_labels_mut().push(label_exit.clone());
            ctx.continue_labels_mut().push(label_header.clone());
            let mut body_vars = local_vars.clone();

            // === Z3 HOARE LOGIC: While Loop Verification ===
            // Auto-infer simple invariants before checking explicit ones.
            // If the loop is `let mut i = K; while i < N { ... i = i + 1; }`,
            // synthesize `invariant i >= K && i < N` automatically.
            let auto_inv = try_infer_while_invariant(&w.cond, &w.body.stmts, local_vars);
            let all_stmts: Vec<Stmt> = if let Some(ref ai) = auto_inv {
                let mut s = w.body.stmts.clone();
                s.insert(0, Stmt::Invariant(ai.clone()));
                s
            } else {
                w.body.stmts.clone()
            };
            let invariant_exprs = prove_while_loop_base_case(ctx, &all_stmts, &body_vars)?;
            let body_to_emit = if auto_inv.is_some() { &all_stmts } else { &w.body.stmts };
            setup_while_loop_inductive_step(ctx, body_to_emit, &mut body_vars, &w.cond, &invariant_exprs)?;

            let ptr_narrowing = super::get_narrowing_target(&w.cond);
            if let Some((ref var, true)) = ptr_narrowing { ctx.pointer_tracker.push_scope(); ctx.pointer_tracker.mark_valid(var); }
            let body_diverges = super::emit_block(ctx, out, body_to_emit, &mut body_vars)?;
            if ptr_narrowing.is_some() { ctx.pointer_tracker.pop_scope(); }

            verify_while_loop_post_body(ctx, &w.cond, local_vars);
            ctx.break_labels_mut().pop();
            ctx.continue_labels_mut().pop();

            if !body_diverges {
                out.push_str(&format!("    cf.br ^{}\n", label_header));
            }
            out.push_str(&format!("  ^{}:\n", label_exit));
            Ok(false)
        }
