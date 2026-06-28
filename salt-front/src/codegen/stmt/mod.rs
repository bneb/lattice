use crate::grammar::{Stmt, SaltBlock, SaltElse, SaltIf, LetElse};
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use crate::codegen::type_bridge::resolve_type;
use std::collections::HashMap;
use syn::spanned::Spanned;
pub mod analysis;
pub mod helpers;
pub(crate) use self::helpers::*;
pub mod match_stmt;
pub use self::match_stmt::*;
pub mod pattern;
pub(crate) use self::pattern::*;
pub mod for_loop;
use self::for_loop::*;
pub mod for_loop_reduction;
pub mod for_loop_emit;









// ============================================================================

// ============================================================================



fn hoist_allocas_in_else_branch(
    ctx: &mut LoweringContext,
    eb: &SaltElse,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<(), String> {
    match eb {
        SaltElse::Block(b) => hoist_allocas_in_block(ctx, &b.stmts, local_vars),
        SaltElse::If(nested) => hoist_allocas_in_block(ctx, &nested.then_branch.stmts, local_vars),
    }
}

fn salt_if_always_returns(f: &crate::grammar::SaltIf) -> bool {
    let Some(else_branch) = &f.else_branch else { return false; };
    salt_block_always_returns(&f.then_branch.stmts) && salt_else_always_returns(else_branch.as_ref())
}

pub fn salt_block_always_returns(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::Return(_) => return true,
            Stmt::Expr(syn::Expr::Return(_), _) => return true,
            Stmt::Syn(syn::Stmt::Expr(syn::Expr::Return(_), _)) => return true,
            Stmt::If(f) => { if salt_if_always_returns(f) { return true; } }
            _ => {}
        }
    }
    false
}



fn salt_else_always_returns(else_branch: &SaltElse) -> bool {
    match else_branch {
        SaltElse::Block(b) => salt_block_always_returns(&b.stmts),
        SaltElse::If(nested) => salt_block_always_returns(&nested.then_branch.stmts),
    }
}

pub fn emit_block(ctx: &mut LoweringContext, out: &mut String, stmts: &[Stmt], local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String> {
    // 1. Preamble Pass: Hoist all allocas to function entry
    hoist_allocas_in_block(ctx, stmts, local_vars)?;

    let mut emitted_terminator = false;
    let mut pushed_guards: usize = 0;
    for stmt in stmts {
        if emit_stmt(ctx, out, stmt, local_vars)? {
            emitted_terminator = true;
            break;
        }

        // Implicit Guard Negation for path-sensitive postcondition verification.
        // After `if cond { return ...; }` (no else), remaining code runs under `!cond`.
        if let Stmt::If(f) = stmt {
            if f.else_branch.is_none() && salt_block_always_returns(&f.then_branch.stmts) {
                let negated_cond = syn::Expr::Unary(syn::ExprUnary {
                    attrs: vec![],
                    op: syn::UnOp::Not(syn::token::Not::default()),
                    expr: Box::new(f.cond.clone()),
                });
                ctx.emission.path_conditions.push(negated_cond);
                pushed_guards += 1;
            }
        }
    }

    // Clean up implicit guards when exiting block scope
    for _ in 0..pushed_guards {
        ctx.emission.path_conditions.pop();
    }

    // If block is empty and not terminated, it must have at least one instruction
    // or a branch to merge to be MLIR-valid.
    Ok(emitted_terminator)
}

fn hoist_allocas_if_block(ctx: &mut LoweringContext, f: &SaltIf, local_vars: &HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    let mut then_vars = local_vars.clone();
    hoist_allocas_in_block(ctx, &f.then_branch.stmts, &mut then_vars)?;
    let Some(eb) = &f.else_branch else { return Ok(()); };
    let mut else_vars = local_vars.clone();
    hoist_allocas_in_else_branch(ctx, eb.as_ref(), &mut else_vars)
}

fn hoist_allocas_in_block(ctx: &mut LoweringContext, stmts: &[Stmt], local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    for stmt in stmts {
        match stmt {
            Stmt::Syn(syn::Stmt::Local(local)) => {
                let pat = match &local.pat {
                    syn::Pat::Type(pt) => &pt.pat,
                    p => p,
                };
                if let syn::Pat::Ident(id) = pat {
                    let name = id.ident.to_string();
                    
                    if let std::collections::hash_map::Entry::Vacant(e) = local_vars.entry(name.clone()) {
                        let ty = if let syn::Pat::Type(pt) = &local.pat {
                            resolve_type(ctx, &crate::grammar::SynType::from_std(*pt.ty.clone()).map_err(|e| e.to_string())?)
                        } else if let Some(_init) = &local.init {
                            // HEURISTIC: Try to infer type from init expression ONLY if it's a simple literal or known variable.
                            // In a real compiler, a full type inference pass would be done.
                            // Salt prefers explicit types or well-behaved inference.
                            // emit_stmt handles inferring and hoisting if this is skipped here.
                            continue;
                        } else {
                            Type::I32
                        };
                        
                        let alloca = format!("%local_{}_{}", name, ctx.next_id());
                        let mlir_ty = ty.to_mlir_storage_type(ctx)?;
                        ctx.emit_alloca(&mut String::new(), &alloca, &mlir_ty);
                        e.insert((ty, LocalKind::Ptr(alloca)));
                    }
                }
            }
            Stmt::While(w) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &w.body.stmts, &mut inner_vars)?;
            }
            Stmt::Loop(body) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &body.stmts, &mut inner_vars)?;
            }
            Stmt::If(f) => hoist_allocas_if_block(ctx, f, local_vars)?,
            Stmt::For(f) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &f.body.stmts, &mut inner_vars)?;
            }
            Stmt::Unsafe(b) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &b.stmts, &mut inner_vars)?;
            }
            Stmt::DynamicCheck(b) => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &b.stmts, &mut inner_vars)?;
            }
            Stmt::WithRegion { region: _, body } => {
                let mut inner_vars = local_vars.clone();
                hoist_allocas_in_block(ctx, &body.stmts, &mut inner_vars)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn emit_stmt(ctx: &mut LoweringContext, out: &mut String, stmt: &Stmt, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String> {
    match stmt {
        Stmt::Syn(s) => match s {
            syn::Stmt::Local(local) => emit_local_stmt(ctx, out, local, local_vars),
            syn::Stmt::Expr(e, semi) => {
                let (val, _) = emit_expr(ctx, out, e, local_vars, None)?;
                let is_return = matches!(e, syn::Expr::Return(_));
                Ok((semi.is_none() && val == "%unreachable") || is_return)
            }
            // Handle macro statements
            // syn parses `macro!(...);` at statement position as Stmt::Macro,
            // not Stmt::Expr(Expr::Macro). Route through emit_expr for handling
            // by the macro dispatch logic (e.g., __fstring_append_expr!).
            syn::Stmt::Macro(ref sm) => {
                let expr_macro = syn::ExprMacro {
                    attrs: sm.attrs.clone(),
                    mac: sm.mac.clone(),
                };
                let (_, _) = emit_expr(ctx, out, &syn::Expr::Macro(expr_macro), local_vars, None)?;
                Ok(false)
            }
            _ => Ok(false),
        },
        Stmt::While(w) => emit_while_stmt(ctx, out, w, local_vars),
        Stmt::If(f) => {
            emit_salt_if(ctx, out, &f.cond, &f.then_branch, &f.else_branch, local_vars)
        }
        Stmt::For(f) => emit_for_stmt(ctx, out, f, local_vars),
        Stmt::MapWindow { addr, size: _, region, body } => {
            let (_addr_val, _addr_ty) = emit_expr(ctx, out, addr, local_vars, None)?;
            let packed_win_var = format!("%packed_win_{}", ctx.next_id());
            
            let mut inner_vars = local_vars.clone();
            let win_ty = Type::Window(Box::new(Type::U8), region.to_string());
            inner_vars.insert(region.to_string(), (win_ty, LocalKind::SSA(packed_win_var)));

            ctx.region_stack_mut().push(region.to_string());
            emit_block(ctx, out, &body.stmts, &mut inner_vars)?;
            ctx.region_stack_mut().pop();
            Ok(false)
        }
        Stmt::Move(expr) => {
             if let syn::Expr::Path(p) = expr {
                 let name = p.path.get_ident().map(|id| id.to_string()).unwrap_or_default();
                 ctx.consumed_vars_mut().insert(name.clone());
                 ctx.consumption_locs_mut().insert(name, "explicit move".to_string());
             }
             Ok(false)
        }
        Stmt::Return(opt_expr) => emit_return_stmt(ctx, out, opt_expr, local_vars),
        Stmt::Expr(expr, _) => {
            let (val, _) = emit_expr(ctx, out, expr, local_vars, None)?;
            Ok(val == "%unreachable")
        }
        Stmt::Invariant(e) => {
            let (cond, _) = emit_expr(ctx, out, e, local_vars, None)?;
            // Lower loop invariant to standard MLIR runtime assertion.
            // Uses scf.if (not cf.cond_br) because invariants live inside
            // loop bodies that may use affine.for or scf.for.
            let true_const = format!("%inv_true_{}", ctx.next_id());
            let violated = format!("%inv_violated_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant true\n", true_const));
            out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", violated, cond, true_const));
            ctx.ensure_external_declaration("__salt_contract_violation", &[], &Type::Unit)?;
            out.push_str(&format!("    scf.if {} {{\n", violated));
            out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
            out.push_str("      scf.yield\n");
            out.push_str("    }\n");
            Ok(false)
        }
        Stmt::Unsafe(block) => emit_unsafe_stmt(ctx, out, block, local_vars),
        Stmt::DynamicCheck(block) => emit_dynamic_check_stmt(ctx, out, block, local_vars),
        Stmt::WithRegion { region, body } => {
            ctx.region_stack_mut().push(region.to_string());
            let mut inner_vars = local_vars.clone();
            let res = emit_block(ctx, out, &body.stmts, &mut inner_vars)?;
            ctx.region_stack_mut().pop();
            Ok(res)
        }
        Stmt::Break => {
            let label = ctx.break_labels().last().ok_or("Break outside of loop")?.clone();
            out.push_str(&format!("    cf.br ^{}\n", label));
            Ok(true)
        }
        Stmt::Continue => {
            let label = ctx.continue_labels().last().ok_or("Continue outside of loop")?.clone();
            out.push_str(&format!("    cf.br ^{}\n", label));
            Ok(true)
        }
        Stmt::Match(match_expr) => {
            emit_match(ctx, out, match_expr, local_vars)
        }
        Stmt::LetElse(let_else) => {
            emit_let_else(ctx, out, let_else, local_vars)
        }
        Stmt::Loop(body) => emit_loop_stmt(ctx, out, body, local_vars),
    }
}

fn emit_local_stmt(ctx: &mut LoweringContext, out: &mut String, local: &syn::Local, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
    let pat = match &local.pat {
        syn::Pat::Type(pt) => &pt.pat,
        p => p,
    };
    let name = if let syn::Pat::Ident(id) = pat { id.ident.to_string() } else { "".to_string() };
    
    if !name.is_empty() && local_vars.contains_key(&name) {
        emit_hoisted_local_init(ctx, out, local, &name, local_vars)?;
    } else {
        emit_unhoisted_local_init(ctx, out, local, &name, local_vars)?;
    }
    
    if !name.is_empty() {
        emit_local_malloc_tracking(ctx, local, &name);
        emit_local_pointer_tracking(ctx, local, &name, local_vars);
        emit_local_arena_tracking(ctx, local, &name);
    }
    
    Ok(false)
}

fn emit_hoisted_local_init(ctx: &mut LoweringContext, out: &mut String, local: &syn::Local, name: &str, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    let (ty, kind) = local_vars.get(name).ok_or_else(|| format!("Local variable {} lost during emission", name))?.clone();
    if let Some(init) = &local.init {
        let hint = if ty.k_is_ptr_type() { None } else { Some(&ty) };
        let (val, init_ty) = emit_expr(ctx, out, &init.expr, local_vars, hint)?;
        
        if ty.is_affine() {
            if let Some(rhs_var_name) = crate::codegen::expr::extract_ident_name(&init.expr) {
                ctx.consumed_vars_mut().insert(rhs_var_name);
            }
        }
        
        let val_prom = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &init_ty, &ty)?;
        if let LocalKind::Ptr(ptr) = kind {
             ctx.emit_store_logical(out, &val_prom, &ptr, &ty)?;
        }

        if !ctx.config.no_verify && ty.is_integer() {
            if let Ok(z3_val) = crate::codegen::expr::translate_to_z3(ctx, &init.expr, local_vars) {
                use crate::z3_shim::ast::Ast;
                let z3_var = ctx.mk_var(name);
                ctx.z3_solver.assert(&z3_var._eq(&z3_val));
            }
        }
    }
    Ok(())
}



/// If the local init is an integer literal, assert equality in Z3 solver.
fn assert_local_lit_int_in_z3(ctx: &mut LoweringContext, name: &str, init: &Option<syn::LocalInit>) {
    let init_expr = match init { Some(i) => &i.expr, None => return };
    let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(li), .. }) = &**init_expr else { return; };
    let Ok(int_val) = li.base10_parse::<i64>() else { return; };
    use crate::z3_shim::ast::Ast;
    let z3_var = ctx.mk_var(name);
    let z3_val = ctx.mk_int(int_val);
    ctx.z3_solver.assert(&z3_var._eq(&z3_val));
}

fn emit_unhoisted_local_init(ctx: &mut LoweringContext, out: &mut String, local: &syn::Local, name: &str, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    let type_hint: Option<Type> = match &local.pat {
        syn::Pat::Type(pt) => Some(resolve_type(ctx, &crate::grammar::SynType::from_std(*pt.ty.clone()).map_err(|e| e.to_string())?)),
        _ => None,
    };
    
    let (val, actual_ty) = if let Some(init) = &local.init {
        let (v, t) = emit_expr(ctx, out, &init.expr, local_vars, type_hint.as_ref())?;
        
        if t.is_affine() {
            if let Some(rhs_var_name) = crate::codegen::expr::extract_ident_name(&init.expr) {
                ctx.consumed_vars_mut().insert(rhs_var_name);
            }
        }
        (v, t)
    } else {
        ("%c0".to_string(), Type::I32)
    };
    
    let target_ty = type_hint.unwrap_or_else(|| actual_ty.clone());
    emit_pattern(ctx, out, &local.pat, val, actual_ty, target_ty.clone(), local_vars)?;

    if !ctx.config.no_verify && !name.is_empty() && target_ty.is_integer() {
        assert_local_lit_int_in_z3(ctx, name, &local.init);
    }
    Ok(())
}

fn emit_local_malloc_tracking(ctx: &mut LoweringContext, local: &syn::Local, name: &str) {
    let pending = ctx.pending_malloc_result.take();
    if pending.is_some() {
        let alloc_id = format!("malloc:{}", name);
        ctx.malloc_tracker.track(alloc_id, format!("malloc at {}", name));
    }

    if let Some(init) = &local.init {
        if let syn::Expr::Cast(c) = &*init.expr {
            if let syn::Expr::Path(p) = &*c.expr {
                if p.path.segments.len() == 1 {
                    let src = p.path.segments[0].ident.to_string();
                    let src_alloc_id = format!("malloc:{}", src);
                    if ctx.malloc_tracker.contains_alloc(&src_alloc_id) {
                        ctx.malloc_tracker.link_dependency(name.to_string(), src_alloc_id);
                    }
                }
            }
        }
    }

    ctx.malloc_tracker.drain_pending_to(name);
}

fn emit_local_pointer_tracking(ctx: &mut LoweringContext, local: &syn::Local, name: &str, local_vars: &HashMap<String, (Type, LocalKind)>) {
    let pending_state = ctx.pending_pointer_state.take();
    if let Some(state) = pending_state {
        match state {
            crate::codegen::verification::PointerState::Empty => ctx.pointer_tracker.mark_empty(name),
            crate::codegen::verification::PointerState::Valid => ctx.pointer_tracker.mark_valid(name),
            crate::codegen::verification::PointerState::Optional => ctx.pointer_tracker.mark_optional(name),
            crate::codegen::verification::PointerState::Freed => ctx.pointer_tracker.mark_freed(name),
            crate::codegen::verification::PointerState::Uninitialized => ctx.pointer_tracker.mark_uninitialized(name),
        }
    } else if local.init.is_none() {
        if let Some((ty, _)) = local_vars.get(name) {
            if ty.k_is_ptr_type() {
                ctx.pointer_tracker.mark_uninitialized(name);
            }
        }
    }
}

fn emit_local_arena_tracking(ctx: &mut LoweringContext, local: &syn::Local, name: &str) {
    if let Some(init) = &local.init {
        if is_arena_constructor(&init.expr) {
            ctx.arena_escape_tracker.register_arena(name);
        }
        if let Some(arena_name) = extract_arena_alloc_receiver(&init.expr) {
            ctx.arena_escape_tracker.register_alloc(name, &arena_name);
        }
        if let Some(arena_name) = extract_arena_allocator_source(&init.expr) {
            ctx.arena_escape_tracker.register_arena_allocator(name, &arena_name);
        }
        if let Some(alloc_name) = extract_vec_new_allocator(&init.expr) {
            ctx.arena_escape_tracker.register_vec_from_allocator(name, &alloc_name);
        }
    }
}





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

fn emit_while_stmt(ctx: &mut LoweringContext, out: &mut String, w: &crate::grammar::SaltWhile, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
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
            let ptr_narrowing = get_narrowing_target(&w.cond);
            if let Some((ref var, true)) = ptr_narrowing { ctx.pointer_tracker.push_scope(); ctx.pointer_tracker.mark_valid(var); }
            let body_diverges = emit_block(ctx, out, &w.body.stmts, &mut body_vars)?;
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






/// Verify the ensures (postcondition) clause at a return site using Z3.
fn verify_return_ensures_clause(
    ctx: &mut LoweringContext,
    out: &mut String,
    ret_expr: &syn::Expr,
    local_vars: &HashMap<String, (Type, LocalKind)>,
) -> Result<(), String> {
    let ensures = ctx.current_ensures().clone();
    if ensures.is_empty() { return Ok(()); }
    let fn_name = ctx.current_fn_name().clone();
    let file = ctx.config.file;
    let (requires, param_names) = file.items.iter()
        .filter_map(|item| {
            if let crate::grammar::Item::Fn(f) = item {
                if f.name == fn_name || ctx.expansion.current_fn_name.ends_with(&f.name.to_string()) {
                    let params: Vec<String> = f.args.iter().map(|a| a.name.to_string()).collect();
                    return Some((f.requires.clone(), params));
                }
            }
            None
        })
        .next()
        .unwrap_or((vec![], vec![]));
    match crate::codegen::verification::VerificationEngine::verify_postcondition(
        ctx, &ensures, &requires, ret_expr, &param_names, local_vars, &fn_name,
    ) {
        Ok(true) => {
            out.push_str(&format!("    // z3_postcondition_verified: ensures proven for '{}'\n", fn_name));
        }
        Ok(false) => {}
        Err(err) => { return Err(err); }
    }
    Ok(())
}
fn emit_return_stmt(ctx: &mut LoweringContext, out: &mut String, opt_expr: &Option<syn::Expr>, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            emit_cleanup_for_return(ctx, out, local_vars)?;
            if let Some(e) = opt_expr {
                // Substitute generics in return type (T -> u8 etc.)
                let expected_ret = ctx.current_ret_ty().clone().map(|t| t.substitute(ctx.current_type_map()));
                let (val_raw, ty) = emit_expr(ctx, out, e, local_vars, expected_ret.as_ref())?;

                // Recursive escape marking.
                crate::codegen::expr::mark_expression_escaped(ctx, e);

                // Arena escape analysis: enforce the return rule
                // return x is valid iff depth(x) <= 1.
                // A pointer from a local arena (depth >= 2) cannot escape.
                if let Some(var_name) = extract_return_var_name(e) {
                    ctx.arena_escape_tracker.check_return_escape(&var_name)?
                }

                verify_return_ensures_clause(ctx, out, e, local_vars)?;
                
                let loc = ctx.loc_tag(e.span());
                if ty == Type::Unit {
                    out.push_str(&format!("    func.return{}\n", loc));
                } else {
                    let mut val = val_raw;
                    if let Some(expected) = &expected_ret {
                        val = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &ty, expected)?;
                    }
                    
                    let mlir_ty = if let Some(expected) = &expected_ret {
                        let e_ty: Type = expected.clone();
                        e_ty.to_mlir_type(ctx)?
                    } else {
                        ty.to_mlir_type(ctx)?
                    };
                    out.push_str(&format!("    func.return {} : {}{}\n", val, mlir_ty, loc));
                }
            } else {
                out.push_str("    func.return\n");
            }
            Ok(true)
        }

fn emit_loop_stmt(ctx: &mut LoweringContext, out: &mut String, body: &crate::grammar::SaltBlock, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            let label_body = format!("loop_body_{}", ctx.next_id());
            let label_exit = format!("loop_exit_{}", ctx.next_id());
            
            out.push_str(&format!("    cf.br ^{}\n", label_body));
            out.push_str(&format!("  ^{}:\n", label_body));
            
            // Heartbeat Injection
            if !*ctx.no_yield() {
                ctx.emit_lto_hook(out, "__salt_yield_check", &[], local_vars, None)?;
            }
            ctx.break_labels_mut().push(label_exit.clone());
            ctx.continue_labels_mut().push(label_body.clone());
            let mut body_vars = local_vars.clone();
            let body_diverges = emit_block(ctx, out, &body.stmts, &mut body_vars)?;
            ctx.break_labels_mut().pop();
            ctx.continue_labels_mut().pop();
            
            if !body_diverges {
                out.push_str(&format!("    cf.br ^{}\n", label_body));
            }

            // Only emit the exit block if a break
            // actually targets it. An infinite `loop { }` with no break
            // produces an exit block with zero predecessors, which crashes
            // MLIR's dominance tree computation in salt-opt.
            let break_target = format!("cf.br ^{}", label_exit);
            let break_was_used = out.contains(&break_target);
            if break_was_used {
                out.push_str(&format!("  ^{}:\n", label_exit));
                Ok(false)
            } else {
                // Infinite loop — no exit path exists. Signal divergence.
                Ok(true)
            }
        }

fn emit_unsafe_stmt(ctx: &mut LoweringContext, out: &mut String, block: &crate::grammar::SaltBlock, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            // Only allow unsafe blocks in privileged packages
            // (std.* and kernel.*). All other packages are rejected.
            // Uses config.file.package as fallback when current_package is None.
            let first_seg = ctx.current_package.as_ref()
                .or(ctx.config.file.package.as_ref())
                .and_then(|pkg| pkg.name.iter().next().map(|id| id.to_string()));

            let fn_name = ctx.current_fn_name();
            let is_privileged = matches!(first_seg.as_deref(), Some("std") | Some("kernel") | Some("basalt"))
                || fn_name.starts_with("std__") || fn_name.starts_with("kernel__") || fn_name.starts_with("basalt__");

            if !is_privileged {
                return Err("unsafe blocks are not allowed in user code. All unsafe operations must go through the standard library's safe abstractions or be placed in kernel.* or basalt packages. See docs/UNSAFE.md.".to_string());
            }

            let was_unsafe = *ctx.is_unsafe_block();
            *ctx.is_unsafe_block_mut() = true;
            let mut inner_vars = local_vars.clone();
            let res = emit_block(ctx, out, &block.stmts, &mut inner_vars)?;
            *ctx.is_unsafe_block_mut() = was_unsafe;
            Ok(res)
        }

fn emit_dynamic_check_stmt(ctx: &mut LoweringContext, out: &mut String, block: &crate::grammar::SaltBlock, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<bool, String>  {
            let was_dynamic = *ctx.is_dynamic_check_block();
            *ctx.is_dynamic_check_block_mut() = true;
            let mut inner_vars = local_vars.clone();
            let res = emit_block(ctx, out, &block.stmts, &mut inner_vars)?;
            *ctx.is_dynamic_check_block_mut() = was_dynamic;
            Ok(res)
        }




// Helper to detect `p.addr != 0` or `p.addr == 0` check
fn get_narrowing_target(cond: &syn::Expr) -> Option<(String, bool)> {
    // Bare pointer: `if ptr { ... }` => narrowing target = ptr, is_neq=true
    if let syn::Expr::Path(p) = cond {
        if let Some(ident) = p.path.get_ident() {

            return Some((ident.to_string(), true));
        }
    }
    
    if let syn::Expr::Binary(bin) = cond {
        // Check if RHS is 0
        let is_zero = if let syn::Expr::Lit(l) = &*bin.right {
             if let syn::Lit::Int(vals) = &l.lit { vals.base10_parse::<u64>().unwrap_or(1) == 0 } else { false }
        } else { false };
        
        if is_zero {
             // Check if LHS is p.addr
             if let syn::Expr::Field(f) = &*bin.left {
                 if let syn::Member::Named(id) = &f.member {
                     if id == "addr" {
                         if let syn::Expr::Path(p) = &*f.base {
                             if let Some(ident) = p.path.get_ident() {
                                 let var_name = ident.to_string();
                                 // != 0 (is_neq=true) or == 0 (is_neq=false)
                                 if let syn::BinOp::Ne(_) = bin.op { return Some((var_name, true)); }
                                 if let syn::BinOp::Eq(_) = bin.op { return Some((var_name, false)); }
                             }
                         }
                     }
                 }
             }
        }
    }
    None
}

pub fn emit_salt_if(
    ctx: &mut LoweringContext,
    out: &mut String,
    cond: &syn::Expr,
    then_branch: &SaltBlock,
    else_branch: &Option<Box<SaltElse>>,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    let label_then = format!("then_{}", ctx.next_id());
    let label_else = format!("else_{}", ctx.next_id());
    let label_merge = format!("merge_{}", ctx.next_id());

    let (cond_val, cond_ty) = emit_expr(ctx, out, cond, local_vars, None)?;
    // Accept Pointer types as if conditions
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
        return Err(format!("If condition must be boolean, found {:?}", cond_ty));
    } else {
        cond_val
    };

    // Flow-Sensitive Narrowing
    let narrowing = get_narrowing_target(cond);
    
    // Save state (Push Scope) for Then branch
    ctx.pointer_tracker.push_scope();

    // Apply narrowing for Then
    if let Some((var, is_neq)) = &narrowing {

        if *is_neq { 
            // p != 0 -> Valid in Then
            ctx.pointer_tracker.mark_valid(var); 
        } else { 
            // p == 0 -> Empty in Then
            ctx.pointer_tracker.mark_empty(var); 
        }
    } 

    let loc = ctx.loc_tag(cond.span());
    let has_else = else_branch.is_some();
    if has_else {
         out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}{}\n", cond_val, label_then, label_else, loc));
    } else {
         out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}{}\n", cond_val, label_then, label_merge, loc));
    }

    let state_before = ctx.consumed_vars().clone();
    let locs_before = ctx.consumption_locs().clone();

    // Save LVN cache before then-branch
    ctx.emission.global_lvn.push_snapshot();

    out.push_str(&format!("  ^{}:\n", label_then));
    let mut then_vars = local_vars.clone();
    // Push branch condition for Z3 postcondition verification
    ctx.emission.path_conditions.push(cond.clone());
    let then_diverges = emit_block(ctx, out, &then_branch.stmts, &mut then_vars)?;
    ctx.emission.path_conditions.pop();
    if !then_diverges {
        out.push_str(&format!("    cf.br ^{}\n", label_merge));
    }

    // Restore LVN cache after then-branch — discard branch-local values
    ctx.emission.global_lvn.pop_snapshot();

    // Restore Pre-If state for Else/Merge (pop "Then" scope)
    let pre_if_state_opt = ctx.pointer_tracker.pop_scope();
    if let Some(pre_if_state) = pre_if_state_opt {
        ctx.pointer_tracker.restore_state(pre_if_state);
    }

    let state_after_then = ctx.consumed_vars().clone();
    let locs_after_then = ctx.consumption_locs().clone();

    // Restore state for Else branch
    *ctx.consumed_vars_mut() = state_before.clone();
    *ctx.consumption_locs_mut() = locs_before.clone();

    let mut else_diverges = false;
    if has_else {
        // Save state (Push Scope) for Else branch (which is Pre-If currently)
        ctx.pointer_tracker.push_scope();

        // Apply narrowing for Else
        if let Some((var, is_neq)) = &narrowing {
            if *is_neq { 
                 // Else of != 0 (== 0) -> Empty
                ctx.pointer_tracker.mark_empty(var); 
            } else { 
                 // Else of == 0 (!= 0) -> Valid
                ctx.pointer_tracker.mark_valid(var); 
            }
        }

        // Save LVN cache before else-branch
        ctx.emission.global_lvn.push_snapshot();

        out.push_str(&format!("  ^{}:\n", label_else));
        let mut else_vars = local_vars.clone();
        // Push negated condition for else branch
        let negated_cond = syn::Expr::Unary(syn::ExprUnary {
            attrs: vec![],
            op: syn::UnOp::Not(syn::token::Not::default()),
            expr: Box::new(cond.clone()),
        });
        ctx.emission.path_conditions.push(negated_cond);
        else_diverges = if let Some(eb) = else_branch {
            match eb.as_ref() {
                SaltElse::Block(b) => emit_block(ctx, out, &b.stmts, &mut else_vars)?,
                SaltElse::If(nested) => {
                     emit_salt_if(ctx, out, &nested.cond, &nested.then_branch, &nested.else_branch, &mut else_vars)?
                }
            }
        } else {
            false
        };
        ctx.emission.path_conditions.pop();
        if !else_diverges {
            out.push_str(&format!("    cf.br ^{}\n", label_merge));
        }

        // Restore LVN cache after else-branch
        ctx.emission.global_lvn.pop_snapshot();

        // Restore Pre-If state for Merge (pop "Else" scope)
        let pre_if_state_opt = ctx.pointer_tracker.pop_scope();
        if let Some(pre_if_state) = pre_if_state_opt {
            ctx.pointer_tracker.restore_state(pre_if_state);
        }
    }
    
    let state_after_else = ctx.consumed_vars().clone();
    let locs_after_else = ctx.consumption_locs().clone();

    // MERGE: Union of consumed vars, but filtered to outer scope
    // We only care about variables that existed BEFORE the if (in local_vars)
    // Local vars defined inside branches are out of scope, so their consumption status is irrelevant
    // UNLESS preventing reuse of names? No, reuse is fine if new definition.
    
    // Safety: If a variable is consumed in ANY branch executed, it is consumed.
    // Since the taken branch is unknown, consumption must be assumed if used in EITHER (for safety).
    // But logically, if I check `if x { move y } else { keep y }`. After: y is maybe moved.
    // Salt requires definitive move? Or partial move tracking?
    // For now, Union is safe (over-conservative).
    // Filtering by `local_vars` prevents leaking inner names.
    
    let mut final_consumed = state_before.clone();
    let mut final_locs = locs_before.clone();
    
    // Add Then-consumed outer vars
    for v in state_after_then.iter() {
        if local_vars.contains_key(v) {
             final_consumed.insert(v.clone());
             if let Some(l) = locs_after_then.get(v) { final_locs.insert(v.clone(), l.clone()); }
        }
    }
    // Add Else-consumed outer vars
    for v in state_after_else.iter() {
        if local_vars.contains_key(v) {
             final_consumed.insert(v.clone());
             if let Some(l) = locs_after_else.get(v) { final_locs.insert(v.clone(), l.clone()); }
        }
    }
    
    *ctx.consumed_vars_mut() = final_consumed;
    *ctx.consumption_locs_mut() = final_locs;

    if !then_diverges || !else_diverges || !has_else {
        out.push_str(&format!("  ^{}:\n", label_merge));
        Ok(false)
    } else {
        Ok(true)
    }
}


// ============================================================================
// PHASE 6: Let-Else Codegen
// ============================================================================

/// Emit let-else statement
pub fn emit_let_else(
    ctx: &mut LoweringContext,
    out: &mut String,
    let_else: &LetElse,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<bool, String> {
    let (init_val, init_ty) = emit_expr(ctx, out, &let_else.init, local_vars, None)?;
    
    let bind_label = format!("let_else_bind_{}", ctx.next_id());
    let else_label = format!("let_else_else_{}", ctx.next_id());
    let continue_label = format!("let_else_continue_{}", ctx.next_id());
    
    if let_else.pattern.is_irrefutable() {
        emit_pattern_bindings(ctx, out, &let_else.pattern, &init_val, &init_ty, local_vars)?;
        return Ok(false);
    }
    
    let cond = emit_pattern_condition(ctx, out, &let_else.pattern, &init_val, &init_ty)?;
    
    out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}\n", cond, bind_label, else_label));
    
    out.push_str(&format!("  ^{}:\n", bind_label));
    emit_pattern_bindings(ctx, out, &let_else.pattern, &init_val, &init_ty, local_vars)?;
    out.push_str(&format!("    cf.br ^{}\n", continue_label));
    
    out.push_str(&format!("  ^{}:\n", else_label));
    let mut else_vars = local_vars.clone();
    let else_diverges = emit_block(ctx, out, &let_else.else_block.stmts, &mut else_vars)?;
    
    if !else_diverges {
        out.push_str("    // WARNING: let-else else block must diverge\n");
        out.push_str("    llvm.unreachable\n");
    }
    
    out.push_str(&format!("  ^{}:\n", continue_label));
    
    Ok(false)
}

pub fn emit_cleanup_for_return(ctx: &mut LoweringContext, out: &mut String, local_vars: &HashMap<String, (Type, LocalKind)>) -> Result<(), String> {
    // RAII-Lite: Emit cleanup for all owned resources in the cleanup_stack
    // This handles Vec and other container types registered via register_owned_resource
    {
        let tasks: Vec<_> = ctx.cleanup_stack()
            .last()
            .map(|t| t.iter().rev().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        for task in &tasks {

                // Z3 Ownership Ledger: Register DEATH event for each resource (DISABLED)
                /*
                ctx.ownership_tracker.mark_released(
                    &task.var_name,
                    &ctx.z3_solver
                )?;
                */
                
                let mlir_ty = task.ty.to_mlir_type(ctx)?;
                out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", 
                    task.drop_fn, task.value, mlir_ty));
        }
    }

    // Drop Trait RAII: Auto-call drop() on locals implementing Drop
    // Iterate in reverse insertion order for proper cleanup ordering (LIFO)
    {
        let mut drop_fns: Vec<(String, String)> = Vec::new();
        
        for (name, (ty, kind)) in local_vars.iter() {
            // Skip internal/synthetic variables
            if name.starts_with("__") { continue; }
            
            let type_key = crate::codegen::type_bridge::type_to_type_key(ty);
            if ctx.trait_registry().contains_method(&type_key, "drop") {
                if let LocalKind::Ptr(ptr) = kind {
                    // Construct the mangled drop function name
                    let type_name = match ty {
                        Type::Struct(n) | Type::Concrete(n, _) => n.clone(),
                        _ => continue,
                    };
                    let mangled = format!("{}__drop", type_name);
                    
                    // Demand-driven hydration: ensure drop() is emitted
                    // Same pattern as Display::fmt hydration (intrinsics.rs:3580-3596)
                    let drop_impl_data = {
                        ctx.generic_impls().get(&mangled).cloned()
                    };
                    if let Some((func_def, func_imports)) = drop_impl_data {
                        let task = crate::codegen::collector::MonomorphizationTask {
                            identity: crate::types::TypeKey { 
                                path: vec![], 
                                name: mangled.clone(), 
                                specialization: None 
                            },
                            mangled_name: mangled.clone(),
                            func: func_def,
                            concrete_tys: vec![],
                            self_ty: Some(ty.clone()),
                            imports: func_imports,
                            type_map: std::collections::BTreeMap::new(),
                        };
                        ctx.entity_registry_mut().request_specialization(task.clone());
                    }
                    
                    drop_fns.push((mangled, ptr.clone()));
                }
            }
        }
        
        // Emit drop calls in reverse order
        for (mangled, ptr) in drop_fns.iter().rev() {
            out.push_str(&format!("    func.call @{}({}) : (!llvm.ptr) -> ()\n", mangled, ptr));
        }
    }

    // Legacy cleanup for Type::Owned
    // Note: salt.drop was removed as MLIR doesn't recognize the salt dialect.
    // Owned types that need cleanup should use explicit drop() calls or
    // register with the CleanupStack for RAII-Lite handling.
    for (name, (ty, kind)) in local_vars {
        if let Type::Owned(inner) = ty {
            if !ctx.consumed_vars().contains(name) {
                 if let LocalKind::Ptr(ptr) = kind {
                     let loaded_ptr = format!("%owned_load_{}", ctx.next_id());
                     out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !llvm.ptr\n", loaded_ptr, ptr));
                     
                     let type_key = crate::codegen::type_bridge::type_to_type_key(inner);
                     if ctx.trait_registry().contains_method(&type_key, "drop") {
                         let type_name = match &**inner {
                             Type::Struct(n) | Type::Concrete(n, _) => n.clone(),
                             _ => String::new(),
                         };
                         if !type_name.is_empty() {
                             let mangled = format!("{}__drop", type_name);
                             let drop_impl_data = ctx.generic_impls().get(&mangled).cloned();
                             if let Some((func_def, func_imports)) = drop_impl_data {
                                 let task = crate::codegen::collector::MonomorphizationTask {
                                     identity: crate::types::TypeKey { path: vec![], name: mangled.clone(), specialization: None },
                                     mangled_name: mangled.clone(),
                                     func: func_def,
                                     concrete_tys: vec![],
                                     self_ty: Some((**inner).clone()),
                                     imports: func_imports,
                                     type_map: std::collections::BTreeMap::new(),
                                 };
                                 ctx.entity_registry_mut().request_specialization(task.clone());
                                 ctx.pending_generations_mut().push_back(task);
                             }
                             out.push_str(&format!("    func.call @{}({}) : (!llvm.ptr) -> ()\n", mangled, loaded_ptr));
                         }
                     }
                     out.push_str(&format!("    func.call @free({}) : (!llvm.ptr) -> ()\n", loaded_ptr));
                 }
            }
        }
    }
    Ok(())
}


