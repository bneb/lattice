//! Z3 array state tracker — models arrays as Z3 native Array<Int, Int>.
//!
//! `select(arr, i)` reads element i. `store(arr, i, v)` returns a new array.
//! Body scanner records store expressions; translate_to_z3 applies them lazily.

use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone)]
#[allow(dead_code)] // Fields used when array store emission is enabled
pub(crate) struct StoreRecord {
    pub index_expr: Box<syn::Expr>,
    pub value_expr: Box<syn::Expr>,
}

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static STORE_RECORDS: RefCell<HashMap<String, Vec<StoreRecord>>> = RefCell::new(HashMap::new());
    #[allow(clippy::missing_const_for_thread_local)]
    static STORES_APPLIED: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
    #[allow(clippy::missing_const_for_thread_local)]
    static VERSIONS: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
}

fn record_store(name: &str, index_expr: Box<syn::Expr>, value_expr: Box<syn::Expr>) {
    STORE_RECORDS.with(|c| {
        c.borrow_mut().entry(name.to_string()).or_default()
            .push(StoreRecord { index_expr, value_expr });
    });
    // Bump version: subsequent reads use arr_v{new_version} (fresh UF)
    VERSIONS.with(|c| {
        let mut map = c.borrow_mut();
        let v = map.get(name).copied().unwrap_or(0) + 1;
        map.insert(name.to_string(), v);
    });
}

pub(crate) fn get_version(name: &str) -> usize {
    VERSIONS.with(|c| c.borrow().get(name).copied().unwrap_or(0))
}

#[allow(dead_code)] // Used when array store emission is enabled
pub(crate) fn get_stores(name: &str) -> Vec<StoreRecord> {
    STORE_RECORDS.with(|c| c.borrow().get(name).cloned().unwrap_or_default())
}

#[allow(dead_code)]
pub(crate) fn stores_applied(name: &str) -> usize {
    STORES_APPLIED.with(|c| c.borrow().get(name).copied().unwrap_or(0))
}

#[allow(dead_code)]
pub(crate) fn mark_stores_applied(name: &str, count: usize) {
    STORES_APPLIED.with(|c| { c.borrow_mut().insert(name.to_string(), count); });
}


/// Scan loop body for indexed assignments. Re-exported for the lazy emission path.
pub(crate) fn process_array_stores_in_body(stmts: &[crate::grammar::Stmt]) {
    process_stores_depth(stmts, 0);
}

fn process_stores_depth(stmts: &[crate::grammar::Stmt], depth: usize) {
    if depth > 32 { return; }
    use crate::grammar::Stmt;
    for stmt in stmts {
        match stmt {
            Stmt::Syn(s) => scan_syn_depth(s, depth + 1),
            Stmt::Expr(e, _) => scan_expr_depth(e, depth + 1),
            Stmt::Unsafe(block) => process_stores_depth(&block.stmts, depth + 1),
            Stmt::While(w) => process_stores_depth(&w.body.stmts, depth + 1),
            Stmt::For(f) => process_stores_depth(&f.body.stmts, depth + 1),
            Stmt::If(salt_if) => {
                process_stores_depth(&salt_if.then_branch.stmts, depth + 1);
                if let Some(else_branch) = &salt_if.else_branch {
                    if let crate::grammar::SaltElse::Block(b) = else_branch.as_ref() {
                        process_stores_depth(&b.stmts, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
}

fn scan_syn_depth(stmt: &syn::Stmt, depth: usize) {
    if depth > 32 { return; }
    if let syn::Stmt::Expr(expr, _) = stmt { scan_expr_depth(expr, depth + 1); }
}

fn scan_expr_depth(expr: &syn::Expr, depth: usize) {
    if depth > 32 { return; }
    match expr {
        syn::Expr::Assign(assign) => {
            if let syn::Expr::Index(idx) = &*assign.left {
                if let syn::Expr::Path(p) = &*idx.expr {
                    if let Some(arr_name) = p.path.get_ident().map(|i| i.to_string()) {
                        record_store(&arr_name, idx.index.clone(), assign.right.clone());
                    }
                }
            }
        }
        syn::Expr::Unsafe(u) => { for s in &u.block.stmts { scan_syn_depth(s, depth + 1); } }
        syn::Expr::While(w) => { for s in &w.body.stmts { scan_syn_depth(s, depth + 1); } }
        syn::Expr::Block(b) => { for s in &b.block.stmts { scan_syn_depth(s, depth + 1); } }
        syn::Expr::If(if_expr) => {
            for s in &if_expr.then_branch.stmts { scan_syn_depth(s, depth + 1); }
            if let Some((_, else_expr)) = &if_expr.else_branch { scan_expr_depth(else_expr, depth + 1); }
        }
        syn::Expr::ForLoop(f) => { for s in &f.body.stmts { scan_syn_depth(s, depth + 1); } }
        syn::Expr::Loop(l) => { for s in &l.body.stmts { scan_syn_depth(s, depth + 1); } }
        _ => {}
    }
}

/// When for-loop bounds are compile-time constants, unroll the loop at the Z3 level.
pub(crate) fn prove_for_loop_concrete(
    ctx: &mut crate::codegen::context::LoweringContext,
    stmts: &[crate::grammar::Stmt],
    bv: &HashMap<String, (crate::types::Type, crate::codegen::context::LocalKind)>,
    iv_ssa: &str,
    start_val: i64,
    end_val: i64,
    var_name: &str,
) -> Result<Vec<syn::Expr>, String> {
    use crate::z3_shim::ast::Ast;
    if ctx.config.no_verify { return Ok(vec![]); }
    let sc = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
    let mut inv: Vec<syn::Expr> = Vec::new();
    for s in stmts { if let crate::grammar::Stmt::Invariant(e) = s { inv.push(e.clone()); } }
    if inv.is_empty() { return Ok(vec![]); }
    // Set concrete loop bound for frame axiom expansion in translate_to_z3
    crate::codegen::verification::loop_bounds::set_concrete_bound(Some(end_val));
    let var_ident = syn::Ident::new(var_name, proc_macro2::Span::call_site());
    for i_val in start_val..end_val {
        if let Some(z3_i) = ctx.symbolic_tracker.get(iv_ssa).cloned() {
            let z3_val = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, i_val);
            ctx.z3_solver.push();
            ctx.z3_solver.assert(&z3_i._eq(&z3_val));
            // Check invariant at i (base case)
            for e in &inv {
                if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, e, bv, &sc) {
                    *ctx.total_checks += 1;
                    ctx.z3_solver.push(); ctx.z3_solver.assert(&z.not());
                    if ctx.z3_solver.check() == crate::z3_shim::SatResult::Sat {
                        ctx.z3_solver.pop(1); ctx.z3_solver.pop(1);
                        return Err(format!("Z3: invariant fails at i={}", i_val));
                    }
                    ctx.z3_solver.pop(1);
                    *ctx.elided_checks += 1;
                    ctx.z3_solver.assert(&z);
                }
            }
            // Apply array stores from the body to model its effects
            process_array_stores_in_body(stmts);
            // Assert while-loop exit conditions to constrain store indices
            assert_while_exit_conditions(ctx, stmts, bv);
            // Check invariant at i+1 (inductive step)
            let next_val: syn::Expr = syn::parse_quote! { #var_ident + 1 };
            for e in &inv {
                let next_inv = crate::grammar::expr_utils::substitute_ident(e, &var_ident, &next_val);
                if let Ok(z3_next) = crate::codegen::expr::translate_bool_to_z3(ctx, &next_inv, bv, &sc) {
                    *ctx.total_checks += 1;
                    ctx.z3_solver.push(); ctx.z3_solver.assert(&z3_next.not());
                    if ctx.z3_solver.check() == crate::z3_shim::SatResult::Sat {
                        ctx.z3_solver.pop(1); ctx.z3_solver.pop(1);
                        return Err(format!("Z3: invariant not preserved at i={}", i_val + 1));
                    }
                    ctx.z3_solver.pop(1);
                    *ctx.elided_checks += 1;
                }
            }
            ctx.z3_solver.pop(1);
        }
    }
    for e in &inv {
        if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, e, bv, &sc) {
            ctx.z3_solver.assert(&z);
        }
    }
    Ok(inv)
}

/// Walk loop body and assert while-loop exit conditions in Z3.
/// This constrains loop variables (like j) to their post-loop values,
/// enabling the frame axiom to determine which indices were modified.
fn assert_while_exit_conditions(
    ctx: &mut crate::codegen::context::LoweringContext,
    stmts: &[crate::grammar::Stmt],
    bv: &HashMap<String, (crate::types::Type, crate::codegen::context::LocalKind)>,
) {
    assert_while_exit_depth(ctx, stmts, bv, 0);
}

fn assert_while_exit_depth(
    ctx: &mut crate::codegen::context::LoweringContext,
    stmts: &[crate::grammar::Stmt],
    bv: &HashMap<String, (crate::types::Type, crate::codegen::context::LocalKind)>,
    depth: usize,
) {
    if depth > 32 { return; }
    use crate::grammar::Stmt;
    for stmt in stmts {
        match stmt {
            Stmt::Syn(syn::Stmt::Expr(syn::Expr::While(w), _)) |
            Stmt::Expr(syn::Expr::While(w), _) => {
                let sc = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
                if let Ok(z) = crate::codegen::expr::translate_bool_to_z3(ctx, &w.cond, bv, &sc) {
                    ctx.z3_solver.assert(&z.not());
                }
            }
            Stmt::Unsafe(block) => assert_while_exit_depth(ctx, &block.stmts[..], bv, depth + 1),
            Stmt::While(w) => assert_while_exit_depth(ctx, &w.body.stmts[..], bv, depth + 1),
            Stmt::For(f) => assert_while_exit_depth(ctx, &f.body.stmts[..], bv, depth + 1),
            Stmt::If(salt_if) => {
                assert_while_exit_depth(ctx, &salt_if.then_branch.stmts[..], bv, depth + 1);
                if let Some(else_branch) = &salt_if.else_branch {
                    if let crate::grammar::SaltElse::Block(b) = else_branch.as_ref() {
                        assert_while_exit_depth(ctx, &b.stmts, bv, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
}
