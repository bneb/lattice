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
}

fn record_store(name: &str, index_expr: Box<syn::Expr>, value_expr: Box<syn::Expr>) {
    STORE_RECORDS.with(|c| {
        c.borrow_mut().entry(name.to_string()).or_default()
            .push(StoreRecord { index_expr, value_expr });
    });
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
