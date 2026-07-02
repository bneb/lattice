//! Thread-local tracker for the current for-loop's upper bound variable name.
//!
//! Set by for_loop_emit before entering a loop body, cleared after.
//! Read by memory.rs during pointer bounds checks to use the loop bound
//! as the effective allocation size for Ptr<T> indexing.
//!
//! This avoids plumbing a new field through CodegenContext→LoweringContext
//! and the resulting lifetime variance cascade.

use std::cell::RefCell;

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static LOOP_UB_NAME: RefCell<Option<String>> = RefCell::new(None);
}

pub(crate) fn set_loop_bound_name(name: Option<String>) {
    LOOP_UB_NAME.with(|c| *c.borrow_mut() = name);
}

pub(crate) fn get_loop_bound_name() -> Option<String> {
    LOOP_UB_NAME.with(|c| c.borrow().clone())
}
