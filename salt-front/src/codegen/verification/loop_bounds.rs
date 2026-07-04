//! Thread-local trackers for verification context.
//!
//! Avoids plumbing new fields through CodegenContext→LoweringContext
//! and the resulting lifetime variance cascade.
//!
//! - LOOP_UB_STACK: stack of for-loop upper bound variable names.
//!   Pushed before loop body, popped after. Innermost loop is last.
//!   Used by memory.rs to find allocation bounds for nested-loop indices.
//! - REQUIRES_PARAMS: function parameters constrained by requires clauses.
//!   Set at function entry. Used for constant-index Ptr<T> bounds proofs.

use std::cell::RefCell;

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static LOOP_UB_STACK: RefCell<Vec<String>> = RefCell::new(Vec::new());
    #[allow(clippy::missing_const_for_thread_local)]
    static REQUIRES_PARAMS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    #[allow(clippy::missing_const_for_thread_local)]
    static CONCRETE_BOUND: RefCell<Option<i64>> = RefCell::new(None);
}

// --- Loop bound stack ---

pub(crate) fn push_loop_bound(name: String) {
    LOOP_UB_STACK.with(|c| c.borrow_mut().push(name));
}

pub(crate) fn pop_loop_bound() {
    LOOP_UB_STACK.with(|c| { c.borrow_mut().pop(); });
}

pub(crate) fn get_loop_bound_stack() -> Vec<String> {
    LOOP_UB_STACK.with(|c| c.borrow().clone())
}

// --- Requires-constrained parameters ---

pub(crate) fn set_requires_params(params: Vec<String>) {
    REQUIRES_PARAMS.with(|c| *c.borrow_mut() = params);
}

pub(crate) fn get_requires_params() -> Vec<String> {
    REQUIRES_PARAMS.with(|c| c.borrow().clone())
}

pub(crate) fn set_concrete_bound(bound: Option<i64>) {
    CONCRETE_BOUND.with(|c| *c.borrow_mut() = bound);
}

pub(crate) fn get_concrete_bound() -> Option<i64> {
    CONCRETE_BOUND.with(|c| *c.borrow())
}

