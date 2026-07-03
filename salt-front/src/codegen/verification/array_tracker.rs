//! Z3 array state tracker — models array mutations as uninterpreted function versions.
//!
//! Arrays (Ptr<T>) are modeled as Z3 uninterpreted functions Int→Int.
//! Each indexed assignment `arr[i] = v` bumps the version counter so
//! subsequent translate_to_z3 calls for arr[j] use an updated function name.
//!
//! Thread-local version counter per array name. The for-loop emitter calls
//! process_array_stores_in_body after the body to bump versions for indexed
//! assignments.
//!
//! Full Z3 update axiom application (via apply_array_store_in_z3 in memory.rs)
//! creates `forall k != i: arr_new(k) = arr_old(k)` frame axioms — wired
//! when the for-loop inductive step is enabled.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static ARRAY_VERSIONS: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
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

#[allow(dead_code)]
pub(crate) fn reset() {
    ARRAY_VERSIONS.with(|c| c.borrow_mut().clear());
}

/// Scan loop body for indexed assignments and bump array versions.
/// This is the integration point for the Z3 array theory inductive step.
pub(crate) fn process_array_stores_in_body(stmts: &[crate::grammar::Stmt]) {
    use crate::grammar::Stmt;
    for stmt in stmts {
        if let Stmt::Syn(syn::Stmt::Expr(syn::Expr::Assign(assign), _)) = stmt {
            if let syn::Expr::Index(idx) = &*assign.left {
                if let syn::Expr::Path(p) = &*idx.expr {
                    if let Some(arr_name) = p.path.get_ident().map(|i| i.to_string()) {
                        bump_version(&arr_name);
                    }
                }
            }
        }
    }
}
