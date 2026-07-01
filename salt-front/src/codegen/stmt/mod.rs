use crate::grammar::{Stmt, SaltBlock, SaltElse, LetElse};
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use crate::codegen::type_bridge::resolve_type;
use std::collections::{HashMap, HashSet};
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
pub mod hoist;
pub(crate) use self::hoist::*;
pub mod while_stmt;
pub(crate) use self::while_stmt::*;
pub mod return_stmt;
pub(crate) use self::return_stmt::*;

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
        // Track string literal lengths for Z3 constant folding in requires/ensures.
        // Enables `let x = "hello"; f(x)` where f has `requires(x.length() > 0)`.
        if let Some(len) = string_lit_length(&init.expr) {
            ctx.emission.known_string_lengths.insert(name.to_string(), len);
        }
    }
    Ok(())
}

/// Extract the byte length of a string literal expression, including
/// through `as` casts ("hello" as StringView → 5).
fn string_lit_length(expr: &syn::Expr) -> Option<i64> {
    match expr {
        syn::Expr::Lit(lit) => {
            if let syn::Lit::Str(s) = &lit.lit { Some(s.value().len() as i64) } else { None }
        }
        syn::Expr::Cast(cast) => string_lit_length(&cast.expr),
        _ => None,
    }
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
    // Track string literal lengths for Z3 constant folding
    if !ctx.config.no_verify && !name.is_empty() {
        if let Some(init) = &local.init {
            if let Some(len) = string_lit_length(&init.expr) {
                ctx.emission.known_string_lengths.insert(name.to_string(), len);
            }
        }
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
pub(crate) fn get_narrowing_target(cond: &syn::Expr) -> Option<(String, bool)> {
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

fn emit_cond_to_bool(ctx: &mut LoweringContext, out: &mut String, cond_val: String, cond_ty: Type) -> Result<String, String> {
    if cond_ty.k_is_ptr_type() {
        let id = ctx.next_id();
        let int_val = format!("%ptrtoint_{}", id);
        let zero_val = format!("%ptr_zero_{}", ctx.next_id());
        let cmp_val = format!("%ptr_nonnull_{}", id);
        out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", int_val, cond_val));
        out.push_str(&format!("    {} = arith.constant 0 : i64\n", zero_val));
        out.push_str(&format!("    {} = arith.cmpi ne, {}, {} : i64\n", cmp_val, int_val, zero_val));
        Ok(cmp_val)
    } else if cond_ty != Type::Bool {
        Err(format!("If condition must be boolean, found {:?}", cond_ty))
    } else {
        Ok(cond_val)
    }
}

fn apply_ptr_narrowing(ctx: &mut LoweringContext, narrowing: &Option<(String, bool)>, invert: bool) {
    if let Some((var, is_neq)) = narrowing {
        let mark_valid = if invert { !*is_neq } else { *is_neq };
        if mark_valid {
            ctx.pointer_tracker.mark_valid(var);
        } else {
            ctx.pointer_tracker.mark_empty(var);
        }
    }
}

fn merge_branch_consumed_vars(
    base: &HashSet<String>,
    base_locs: &HashMap<String, String>,
    then_consumed: &HashSet<String>,
    then_locs: &HashMap<String, String>,
    else_consumed: &HashSet<String>,
    else_locs: &HashMap<String, String>,
    local_vars: &HashMap<String, (Type, LocalKind)>,
) -> (HashSet<String>, HashMap<String, String>) {
    let mut final_consumed = base.clone();
    let mut final_locs = base_locs.clone();
    for v in then_consumed.iter().chain(else_consumed.iter()) {
        if local_vars.contains_key(v) {
            final_consumed.insert(v.clone());
            if let Some(l) = then_locs.get(v).or_else(|| else_locs.get(v)) {
                final_locs.insert(v.clone(), l.clone());
            }
        }
    }
    (final_consumed, final_locs)
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
    let cond_val = emit_cond_to_bool(ctx, out, cond_val, cond_ty)?;
    let narrowing = get_narrowing_target(cond);
    let has_else = else_branch.is_some();
    let loc = ctx.loc_tag(cond.span());

    // Emit Then branch
    ctx.pointer_tracker.push_scope();
    apply_ptr_narrowing(ctx, &narrowing, false);
    let dest_label = if has_else { &label_else } else { &label_merge };
    out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}{}\n", cond_val, label_then, dest_label, loc));

    let state_before = ctx.consumed_vars().clone();
    let locs_before = ctx.consumption_locs().clone();
    ctx.emission.global_lvn.push_snapshot();

    out.push_str(&format!("  ^{}:\n", label_then));
    let mut then_vars = local_vars.clone();
    ctx.emission.path_conditions.push(cond.clone());
    let then_diverges = emit_block(ctx, out, &then_branch.stmts, &mut then_vars)?;
    ctx.emission.path_conditions.pop();
    if !then_diverges {
        out.push_str(&format!("    cf.br ^{}\n", label_merge));
    }

    ctx.emission.global_lvn.pop_snapshot();
    let pre_if_state_opt = ctx.pointer_tracker.pop_scope();
    if let Some(pre_if_state) = pre_if_state_opt {
        ctx.pointer_tracker.restore_state(pre_if_state);
    }
    let state_after_then = ctx.consumed_vars().clone();
    let locs_after_then = ctx.consumption_locs().clone();

    // Emit Else branch
    *ctx.consumed_vars_mut() = state_before.clone();
    *ctx.consumption_locs_mut() = locs_before.clone();

    let mut else_diverges = false;
    if has_else {
        ctx.pointer_tracker.push_scope();
        apply_ptr_narrowing(ctx, &narrowing, true);
        ctx.emission.global_lvn.push_snapshot();

        out.push_str(&format!("  ^{}:\n", label_else));
        let mut else_vars = local_vars.clone();
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

        ctx.emission.global_lvn.pop_snapshot();
        let pre_if_state_opt = ctx.pointer_tracker.pop_scope();
        if let Some(pre_if_state) = pre_if_state_opt {
            ctx.pointer_tracker.restore_state(pre_if_state);
        }
    }

    let state_after_else = ctx.consumed_vars().clone();
    let locs_after_else = ctx.consumption_locs().clone();

    let (final_consumed, final_locs) = merge_branch_consumed_vars(
        &state_before, &locs_before,
        &state_after_then, &locs_after_then,
        &state_after_else, &locs_after_else,
        local_vars,
    );
    *ctx.consumed_vars_mut() = final_consumed;
    *ctx.consumption_locs_mut() = final_locs;

    if !then_diverges || !else_diverges || !has_else {
        out.push_str(&format!("  ^{}:\n", label_merge));
        Ok(false)
    } else {
        Ok(true)
    }
}

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
