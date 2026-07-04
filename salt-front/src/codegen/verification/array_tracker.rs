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

#[allow(dead_code)]
pub(crate) fn get_store_names() -> Vec<String> {
    STORE_RECORDS.with(|c| c.borrow().keys().cloned().collect())
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
            // Recurse into the RHS for nested array accesses
            scan_expr_depth(&assign.right, depth + 1);
        }
        // Recurse into binary ops for patterns like: arr[i] + arr[j]
        syn::Expr::Binary(b) => {
            scan_expr_depth(&b.left, depth + 1);
            scan_expr_depth(&b.right, depth + 1);
        }
        // Recurse into call args for: f(arr[i])
        syn::Expr::Call(c) => {
            scan_expr_depth(&c.func, depth + 1);
            for arg in &c.args { scan_expr_depth(arg, depth + 1); }
        }
        // Recurse into method call args for: arr[i].method()
        syn::Expr::MethodCall(mc) => {
            scan_expr_depth(&mc.receiver, depth + 1);
            for arg in &mc.args { scan_expr_depth(arg, depth + 1); }
        }
        // Recurse into index expressions for nested: arr[arr[i]]
        syn::Expr::Index(idx) => {
            scan_expr_depth(&idx.expr, depth + 1);
            scan_expr_depth(&idx.index, depth + 1);
        }
        // Recurse into unary ops for: !cond, -arr[i]
        syn::Expr::Unary(u) => scan_expr_depth(&u.expr, depth + 1),
        syn::Expr::Paren(p) => scan_expr_depth(&p.expr, depth + 1),
        syn::Expr::Group(g) => scan_expr_depth(&g.expr, depth + 1),
        syn::Expr::Cast(c) => scan_expr_depth(&c.expr, depth + 1),
        syn::Expr::Field(f) => scan_expr_depth(&f.base, depth + 1),
        syn::Expr::Let(let_expr) => {
            scan_expr_depth(&let_expr.expr, depth + 1);
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
            // Case-split on while-loop variables for data-dependent branches
            let mut loop_vars: Vec<String> = Vec::new();
            extract_while_loop_vars(stmts, &mut loop_vars);
            // Check invariant at i+1 (inductive step) with case-splitting
            let next_val: syn::Expr = syn::parse_quote! { #var_ident + 1 };
            for e in &inv {
                let next_inv = crate::grammar::expr_utils::substitute_ident(e, &var_ident, &next_val);
                if let Ok(z3_next) = crate::codegen::expr::translate_bool_to_z3(ctx, &next_inv, bv, &sc) {
                    if loop_vars.is_empty() {
                        // No while-loop variables: check invariant directly
                        *ctx.total_checks += 1;
                        ctx.z3_solver.push(); ctx.z3_solver.assert(&z3_next.not());
                        if ctx.z3_solver.check() == crate::z3_shim::SatResult::Sat {
                            ctx.z3_solver.pop(1); ctx.z3_solver.pop(1);
                            return Err(format!("Z3: invariant not preserved at i={}", i_val + 1));
                        }
                        ctx.z3_solver.pop(1);
                        *ctx.elided_checks += 1;
                    } else {
                        // Case-split on each while-loop variable
                        for var_name in &loop_vars {
                            if let Some(z3_var) = resolve_var_z3(ctx, var_name, bv) {
                                let z3_zero = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, 0);
                                // Case A: var < 0 (loop exited via counter exhaustion)
                                *ctx.total_checks += 1;
                                ctx.z3_solver.push();
                                ctx.z3_solver.assert(&z3_var.lt(&z3_zero));
                                ctx.z3_solver.assert(&z3_next.not());
                                let case_a_proven = ctx.z3_solver.check() == crate::z3_shim::SatResult::Unsat;
                                ctx.z3_solver.pop(1);
                                // Case B: var >= 0 (loop exited via sorted element)
                                *ctx.total_checks += 1;
                                ctx.z3_solver.push();
                                ctx.z3_solver.assert(&z3_var.ge(&z3_zero));
                                ctx.z3_solver.assert(&z3_next.not());
                                let case_b_proven = ctx.z3_solver.check() == crate::z3_shim::SatResult::Unsat;
                                ctx.z3_solver.pop(1);
                                if !case_a_proven || !case_b_proven {
                                    ctx.z3_solver.pop(1);
                                    return Err(format!("Z3: invariant not preserved at i={} (case-split on {})", i_val + 1, var_name));
                                }
                                *ctx.elided_checks += 2;
                            }
                        }
                    }
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

/// Extract loop variable names from while-loop conditions in the body.
fn extract_while_loop_vars(stmts: &[crate::grammar::Stmt], out: &mut Vec<String>) {
    extract_vars_depth(stmts, out, 0);
}

fn extract_vars_depth(stmts: &[crate::grammar::Stmt], out: &mut Vec<String>, depth: usize) {
    if depth > 32 { return; }
    use crate::grammar::Stmt;
    for stmt in stmts {
        match stmt {
            Stmt::Syn(syn::Stmt::Expr(syn::Expr::While(w), _)) |
            Stmt::Expr(syn::Expr::While(w), _) => {
                collect_loop_var(&w.cond, out);
            }
            Stmt::While(w) => {
                collect_loop_var(&w.cond, out);
                extract_vars_depth(&w.body.stmts, out, depth + 1);
            }
            Stmt::Unsafe(block) => extract_vars_depth(&block.stmts, out, depth + 1),
            Stmt::For(f) => extract_vars_depth(&f.body.stmts, out, depth + 1),
            Stmt::If(salt_if) => {
                extract_vars_depth(&salt_if.then_branch.stmts, out, depth + 1);
                if let Some(else_branch) = &salt_if.else_branch {
                    if let crate::grammar::SaltElse::Block(b) = else_branch.as_ref() {
                        extract_vars_depth(&b.stmts, out, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Extract the primary loop variable from a while condition like `j >= 0 && arr[j] > key`.
fn collect_loop_var(cond: &syn::Expr, out: &mut Vec<String>) {
    if let syn::Expr::Binary(bin) = cond {
        if let syn::Expr::Binary(inner) = &*bin.left {
            if let syn::Expr::Path(p) = &*inner.left {
                if let Some(name) = p.path.get_ident().map(|i| i.to_string()) {
                    out.push(name);
                }
            }
        }
    }
}

/// Resolve a variable name through body_vars to get its Z3 Int.
fn resolve_var_z3<'a>(
    ctx: &crate::codegen::context::LoweringContext<'a, '_>,
    name: &str,
    bv: &HashMap<String, (crate::types::Type, crate::codegen::context::LocalKind)>,
) -> Option<crate::z3_shim::ast::Int<'a>> {
    if let Some((_, crate::codegen::context::LocalKind::SSA(ssa))) = bv.get(name) {
        ctx.symbolic_tracker.get(ssa).cloned()
    } else {
        ctx.symbolic_tracker.get(name).cloned()
    }
}
