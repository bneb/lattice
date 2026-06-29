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
    ctx.z3_solver.push();
    for e in &inv {
        if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, e, bv, &sc) {
            ctx.z3_solver.push(); ctx.z3_solver.assert(&z.not());
            let ck = ctx.z3_solver.check(); ctx.z3_solver.pop(1);
            if ck == crate::z3_shim::SatResult::Sat {
                ctx.z3_solver.pop(1);
                return Err("Z3 verification failed: loop invariant does not hold at entry.                      The solver found a counterexample proving the invariant is false                      with current variable values.".to_string());
            }
            ctx.z3_solver.assert(&z);
        }
    }
    ctx.z3_solver.pop(1);
    for e in &inv {
        if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, e, bv, &sc) {
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
            let invariant_exprs = prove_while_loop_base_case(ctx, &w.body.stmts, &body_vars)?;
            setup_while_loop_inductive_step(ctx, &w.body.stmts, &mut body_vars, &w.cond, &invariant_exprs)?;

            // `while p.addr() != 0 { ... }` — p is Valid inside body (push/pop isolates body state)
            let ptr_narrowing = super::get_narrowing_target(&w.cond);
            if let Some((ref var, true)) = ptr_narrowing { ctx.pointer_tracker.push_scope(); ctx.pointer_tracker.mark_valid(var); }
            let body_diverges = super::emit_block(ctx, out, &w.body.stmts, &mut body_vars)?;
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
