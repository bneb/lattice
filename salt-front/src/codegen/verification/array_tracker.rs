//! Z3 array state tracker — models array mutations as uninterpreted function versions.
//!
//! Arrays (Ptr<T>) are modeled as Z3 uninterpreted functions Int→Int.
//! Each indexed assignment `arr[i] = v` records the store expressions and bumps
//! the version. Frame axioms are emitted lazily from translate_to_z3.

use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone)]
#[allow(dead_code)] // Fields used when lazy emission is enabled
pub(crate) struct StoreRecord {
    pub index_expr: Box<syn::Expr>,
    pub value_expr: Box<syn::Expr>,
}

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static ARRAY_VERSIONS: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
    #[allow(clippy::missing_const_for_thread_local)]
    static STORE_RECORDS: RefCell<HashMap<String, Vec<StoreRecord>>> = RefCell::new(HashMap::new());
    #[allow(clippy::missing_const_for_thread_local)]
    static EMITTED_FRAMES: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
}

pub(crate) fn get_version(name: &str) -> usize {
    ARRAY_VERSIONS.with(|c| c.borrow().get(name).copied().unwrap_or(0))
}

pub(crate) fn bump_version(name: &str) -> usize {
    ARRAY_VERSIONS.with(|c| {
        let map = &mut *c.borrow_mut();
        let v = map.get(name).copied().unwrap_or(0) + 1;
        map.insert(name.to_string(), v);
        v
    })
}

fn record_store(name: &str, index_expr: Box<syn::Expr>, value_expr: Box<syn::Expr>) {
    ARRAY_VERSIONS.with(|c| {
        let map = &mut *c.borrow_mut();
        let v = map.get(name).copied().unwrap_or(0) + 1;
        map.insert(name.to_string(), v);
    });
    STORE_RECORDS.with(|c| {
        c.borrow_mut().entry(name.to_string()).or_default()
            .push(StoreRecord { index_expr, value_expr });
    });
}

#[allow(dead_code)] // Used when lazy emission is enabled
pub(crate) fn get_stores(name: &str, from_ver: usize) -> Vec<StoreRecord> {
    STORE_RECORDS.with(|c| {
        c.borrow().get(name).map(|v| v[from_ver..].to_vec()).unwrap_or_default()
    })
}

#[allow(dead_code)] // Used when lazy emission is enabled
pub(crate) fn frame_emitted(name: &str, ver: usize) -> bool {
    EMITTED_FRAMES.with(|c| c.borrow().get(name).copied().unwrap_or(0) >= ver)
}

#[allow(dead_code)] // Used when lazy emission is enabled
pub(crate) fn mark_frame_emitted(name: &str, ver: usize) {
    EMITTED_FRAMES.with(|c| { c.borrow_mut().insert(name.to_string(), ver); });
}

/// Scan loop body recursively for indexed assignments.
pub(crate) fn process_array_stores_in_body(stmts: &[crate::grammar::Stmt]) {
    process_array_stores_in_body_depth(stmts, 0)
}

fn process_array_stores_in_body_depth(stmts: &[crate::grammar::Stmt], depth: usize) {
    if depth > 32 { return; } // safety limit
    use crate::grammar::Stmt;
    for stmt in stmts {
        match stmt {
            Stmt::Syn(s) => scan_syn_stmt_depth(s, depth + 1),
            Stmt::Expr(e, _) => scan_syn_expr_depth(e, depth + 1),
            Stmt::Unsafe(block) => process_array_stores_in_body_depth(&block.stmts, depth + 1),
            Stmt::While(w) => process_array_stores_in_body_depth(&w.body.stmts, depth + 1),
            Stmt::For(f) => process_array_stores_in_body_depth(&f.body.stmts, depth + 1),
            Stmt::If(salt_if) => {
                process_array_stores_in_body_depth(&salt_if.then_branch.stmts, depth + 1);
                if let Some(else_branch) = &salt_if.else_branch {
                    if let crate::grammar::SaltElse::Block(b) = else_branch.as_ref() {
                        process_array_stores_in_body_depth(&b.stmts, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
}

#[allow(dead_code)] // Entry points, may be called directly
fn scan_syn_stmt(stmt: &syn::Stmt) { scan_syn_stmt_depth(stmt, 0); }
#[allow(dead_code)]
fn scan_syn_expr(expr: &syn::Expr) { scan_syn_expr_depth(expr, 0); }

fn scan_syn_stmt_depth(stmt: &syn::Stmt, depth: usize) {
    if depth > 32 { return; }
    if let syn::Stmt::Expr(expr, _) = stmt { scan_syn_expr_depth(expr, depth + 1); }
}

fn scan_syn_expr_depth(expr: &syn::Expr, depth: usize) {
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
            scan_syn_expr_depth(&assign.right, depth + 1);
        }
        syn::Expr::Unsafe(u) => { for s in &u.block.stmts { scan_syn_stmt_depth(s, depth + 1); } }
        syn::Expr::While(w) => { for s in &w.body.stmts { scan_syn_stmt_depth(s, depth + 1); } }
        syn::Expr::Block(b) => { for s in &b.block.stmts { scan_syn_stmt_depth(s, depth + 1); } }
        syn::Expr::If(if_expr) => {
            for s in &if_expr.then_branch.stmts { scan_syn_stmt_depth(s, depth + 1); }
            if let Some((_, else_expr)) = &if_expr.else_branch { scan_syn_expr_depth(else_expr, depth + 1); }
        }
        syn::Expr::ForLoop(f) => { for s in &f.body.stmts { scan_syn_stmt_depth(s, depth + 1); } }
        syn::Expr::Loop(l) => { for s in &l.body.stmts { scan_syn_stmt_depth(s, depth + 1); } }
        _ => {}
    }
}
