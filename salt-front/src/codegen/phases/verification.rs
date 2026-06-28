//! Phase 4: Verification State
//! Contains Z3 context, solver, and symbolic tracking for formal verification.

use std::collections::HashMap;

/// Phase 4: Z3 verification state (isolated for solver queries)
pub struct VerificationState<'a> {
    // --- Z3 verification core ---
    /// Z3 context reference
    pub z3_ctx: &'a crate::z3_shim::Context,
    /// Z3 solver instance
    pub z3_solver: crate::z3_shim::Solver<'a>,
    /// Symbolic variable tracker: var_name -> Z3 integer
    pub symbolic_tracker: HashMap<String, crate::z3_shim::ast::Int<'a>>,
    /// Z3 ownership state tracker for RAII verification
    pub ownership_tracker: crate::codegen::verification::Z3StateTracker<'a>,
    /// Number of bounds checks elided by Z3 proofs
    pub elided_checks: usize,
    /// Total number of bounds checks encountered
    pub total_checks: usize,
    /// Number of loop invariants auto-injected by the loop_invariant pass
    pub loop_invariants_injected: usize,
    // --- Absorbed from CodegenContext façade ---
    /// Standalone malloc tracker with dependency graph
    pub malloc_tracker: crate::codegen::verification::MallocTracker,
    /// Pending malloc result: set by expr, consumed by stmt
    pub pending_malloc_result: Option<String>,
    /// Flow-sensitive pointer state tracker (Valid / Empty / Optional)
    pub pointer_tracker: crate::codegen::verification::PointerStateTracker,
    /// Pending pointer state: set by emit_call, consumed by stmt
    pub pending_pointer_state: Option<crate::codegen::verification::PointerState>,
    /// Arena escape tracker (scope-depth taint analysis)
    pub arena_escape_tracker: crate::codegen::verification::ArenaEscapeTracker,
    /// Pending arena provenance from Arena::alloc
    pub pending_arena_provenance: Option<String>,
}

impl<'a> VerificationState<'a> {
    pub fn new(z3_ctx: &'a crate::z3_shim::Context) -> Self {
        Self {
            z3_solver: crate::z3_shim::Solver::new(z3_ctx),
            symbolic_tracker: HashMap::new(),
            ownership_tracker: crate::codegen::verification::Z3StateTracker::new(z3_ctx),
            elided_checks: 0,
            total_checks: 0,
            loop_invariants_injected: 0,
            z3_ctx,
            malloc_tracker: crate::codegen::verification::MallocTracker::new(),
            pending_malloc_result: None,
            pointer_tracker: crate::codegen::verification::PointerStateTracker::new(),
            pending_pointer_state: None,
            arena_escape_tracker: crate::codegen::verification::ArenaEscapeTracker::new(),
            pending_arena_provenance: None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_verification_state_new() {
        let cfg = crate::z3_shim::Config::new();
        let ctx = crate::z3_shim::Context::new(&cfg);
        let state = super::VerificationState::new(&ctx);
        assert_eq!(state.elided_checks, 0);
        assert_eq!(state.total_checks, 0);
        assert!(state.pending_malloc_result.is_none());
    }
}
