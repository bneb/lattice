//! Thread-local trackers for verification context.
//!
//! Avoids plumbing new fields through CodegenContext→LoweringContext
//! and the resulting lifetime variance cascade.
//!
//! - LOOP_UB_NAME: current for-loop's upper bound variable name.
//!   Set before loop body, cleared after. Used by memory.rs for Ptr<T> bounds.
//! - REQUIRES_PARAMS: function parameters constrained by requires clauses.
//!   Set at function entry. Used for constant-index Ptr<T> bounds proofs.

use std::cell::RefCell;

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static LOOP_UB_NAME: RefCell<Option<String>> = RefCell::new(None);
    #[allow(clippy::missing_const_for_thread_local)]
    static REQUIRES_PARAMS: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

// --- Loop bound ---

pub(crate) fn set_loop_bound_name(name: Option<String>) {
    LOOP_UB_NAME.with(|c| *c.borrow_mut() = name);
}

pub(crate) fn get_loop_bound_name() -> Option<String> {
    LOOP_UB_NAME.with(|c| c.borrow().clone())
}

// --- Requires-constrained parameters ---

pub(crate) fn set_requires_params(params: Vec<String>) {
    REQUIRES_PARAMS.with(|c| *c.borrow_mut() = params);
}

pub(crate) fn get_requires_params() -> Vec<String> {
    REQUIRES_PARAMS.with(|c| c.borrow().clone())
}
