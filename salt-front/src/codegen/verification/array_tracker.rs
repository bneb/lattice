//! Z3 array state tracker — models array mutations as uninterpreted function versions.
//!
//! Arrays (Ptr<T>) are modeled as Z3 uninterpreted functions Int→Int.
//! Each indexed assignment `arr[i] = v` records the store and bumps the version.
//! Frame axioms (forall k != i: arr_new(k) = arr_old(k)) are emitted lazily
//! from translate_to_z3 when it detects a version change.

use std::cell::RefCell;
use std::collections::HashMap;

/// Record of an array store: (index_var_name, value_var_name)
/// Names refer to entries in the LoweringContext's symbolic_tracker.
#[derive(Clone, Debug)]
pub(crate) struct StoreRecord {
    pub index_name: String,
    pub value_name: String,
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

/// Bump version counter (used by apply_array_store_in_z3 for full Z3 integration).
pub(crate) fn bump_version(name: &str) -> usize {
    ARRAY_VERSIONS.with(|c| {
        let map = &mut *c.borrow_mut();
        let v = map.get(name).copied().unwrap_or(0) + 1;
        map.insert(name.to_string(), v);
        v
    })
}

// Records an indexed store for later frame axiom emission
pub(crate) fn record_store(name: &str, index_name: &str, value_name: &str) {
    ARRAY_VERSIONS.with(|c| {
        let map = &mut *c.borrow_mut();
        let v = map.get(name).copied().unwrap_or(0) + 1;
        map.insert(name.to_string(), v);
    });
    STORE_RECORDS.with(|c| {
        c.borrow_mut()
            .entry(name.to_string())
            .or_default()
            .push(StoreRecord {
                index_name: index_name.to_string(),
                value_name: value_name.to_string(),
            });
    });
}

// Get store records for an array version range [from_ver, to_ver)
pub(crate) fn get_stores(name: &str, from_ver: usize) -> Vec<StoreRecord> {
    STORE_RECORDS.with(|c| {
        c.borrow().get(name)
            .map(|v| v[from_ver..].to_vec())
            .unwrap_or_default()
    })
}

// Check if frame axioms for version `ver` of `name` have been emitted
pub(crate) fn frame_emitted(name: &str, ver: usize) -> bool {
    EMITTED_FRAMES.with(|c| {
        c.borrow().get(name).copied().unwrap_or(0) >= ver
    })
}

// Mark frame axioms as emitted up to version `ver`
pub(crate) fn mark_frame_emitted(name: &str, ver: usize) {
    EMITTED_FRAMES.with(|c| {
        c.borrow_mut().insert(name.to_string(), ver);
    });
}

#[allow(dead_code)]
pub(crate) fn reset() {
    ARRAY_VERSIONS.with(|c| c.borrow_mut().clear());
    STORE_RECORDS.with(|c| c.borrow_mut().clear());
    EMITTED_FRAMES.with(|c| c.borrow_mut().clear());
}

/// Scan loop body for indexed assignments and record stores.
pub(crate) fn process_array_stores_in_body(stmts: &[crate::grammar::Stmt]) {
    use crate::grammar::Stmt;
    for stmt in stmts {
        if let Stmt::Syn(syn::Stmt::Expr(syn::Expr::Assign(assign), _)) = stmt {
            if let syn::Expr::Index(idx) = &*assign.left {
                if let syn::Expr::Path(p) = &*idx.expr {
                    if let Some(arr_name) = p.path.get_ident().map(|i| i.to_string()) {
                        let idx_name = idx_name(&idx.index);
                        let val_name = expr_name(&assign.right);
                        record_store(&arr_name, &idx_name, &val_name);
                    }
                }
            }
        }
    }
}

// Extract a best-effort string name from an expression for store tracking
fn idx_name(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Path(p) => p.path.get_ident().map(|i| i.to_string()).unwrap_or_default(),
        syn::Expr::Binary(_) => "computed_idx".to_string(),
        syn::Expr::Lit(lit) => format!("{:?}", lit.lit),
        _ => "unknown_idx".to_string(),
    }
}

fn expr_name(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Path(p) => p.path.get_ident().map(|i| i.to_string()).unwrap_or_default(),
        syn::Expr::Lit(lit) => format!("{:?}", lit.lit),
        _ => "computed_val".to_string(),
    }
}
