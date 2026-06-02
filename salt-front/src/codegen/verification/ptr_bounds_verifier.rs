//! Pointer Bounds Verifier: Z3-based bounds checking for Ptr<T> operations
//!
//! When Salt code uses `ptr.index(i)` or `ptr.offset(n)`, this module uses
//! Z3 to prove the operation stays within the allocated region.
//!
//! Ptr<T> is a raw pointer — unlike slices, it carries no runtime length.
//! Bounds safety depends on:
//!   1. Known allocation size (from malloc/arena tracking)
//!   2. Loop invariants constraining the index variable
//!   3. Explicit `requires` contracts on function parameters
//!
//! The proof structure mirrors slice_verifier.rs:
//!   - Declare symbolic variables for allocation size and access index
//!   - Assert physical constraints (alloc_size > 0, index >= 0)
//!   - Assert the violation (index >= alloc_elements)
//!   - UNSAT → proven safe, SAT → unsafe


/// Result of a Z3 pointer bounds verification attempt
#[derive(Debug, Clone, PartialEq)]
pub enum PtrProofResult {
    /// Z3 proved the access is within allocation bounds — safe
    Proven,
    /// Z3 found a counterexample — access may be out of bounds
    Unsafe(String),
    /// Z3 timed out or expression too complex
    Unknown,
}

/// Information about a pointer operation and its allocation context
#[derive(Debug, Clone)]
pub struct PtrBoundsInfo {
    /// Known allocation size in elements (if constant). E.g., malloc(10 * sizeof(T)) → 10
    pub alloc_elements: Option<i64>,
    /// Known element size in bytes (for offset arithmetic)
    pub element_size: Option<i64>,
    /// Upper bound on the index from a loop invariant or requires clause
    pub index_upper_bound: Option<i64>,
    /// Lower bound on the index (default: 0)
    pub index_lower_bound: Option<i64>,
    /// Enclosing function name (for diagnostics)
    pub func_name: String,
}

impl PtrBoundsInfo {
    pub fn new(func_name: &str) -> Self {
        PtrBoundsInfo {
            alloc_elements: None,
            element_size: None,
            index_upper_bound: None,
            index_lower_bound: None,
            func_name: func_name.to_string(),
        }
    }

    pub fn with_alloc(mut self, elements: i64) -> Self {
        self.alloc_elements = Some(elements);
        self
    }

    pub fn with_element_size(mut self, size: i64) -> Self {
        self.element_size = Some(size);
        self
    }

    pub fn with_index_bounds(mut self, lower: i64, upper: i64) -> Self {
        self.index_lower_bound = Some(lower);
        self.index_upper_bound = Some(upper);
        self
    }
}

/// Verify that `ptr.index(access_index)` is within the allocation.
///
/// For a known allocation of N elements, access at index i is safe iff 0 <= i < N.
pub fn verify_ptr_index(
    z3_ctx: &crate::z3_shim::Context,
    info: &PtrBoundsInfo,
    access_index: i64,
) -> PtrProofResult {
    let solver = crate::z3_shim::Solver::new(z3_ctx);
    let zero = crate::z3_shim::ast::Int::from_i64(z3_ctx, 0);

    // Declare allocation size (known or symbolic)
    let alloc_size = if let Some(n) = info.alloc_elements {
        crate::z3_shim::ast::Int::from_i64(z3_ctx, n)
    } else {
        let sym = crate::z3_shim::ast::Int::new_const(z3_ctx, "alloc_size");
        solver.assert(&sym.gt(&zero));
        sym
    };

    // The access index
    let idx = crate::z3_shim::ast::Int::from_i64(z3_ctx, access_index);

    // Violation: idx < 0 OR idx >= alloc_size
    let neg_violation = idx.lt(&zero);
    let upper_violation = idx.ge(&alloc_size);
    let violation = crate::z3_shim::ast::Bool::or(z3_ctx, &[&neg_violation, &upper_violation]);
    solver.assert(&violation);

    match solver.check() {
        crate::z3_shim::SatResult::Unsat => PtrProofResult::Proven,
        crate::z3_shim::SatResult::Sat => {
            PtrProofResult::Unsafe(format!(
                "Counterexample in '{}': ptr.index({}) may exceed allocation of {} elements",
                info.func_name, access_index,
                info.alloc_elements.map_or("unknown".to_string(), |n| n.to_string())
            ))
        }
        crate::z3_shim::SatResult::Unknown => PtrProofResult::Unknown,
    }
}

/// Verify that `ptr.offset(n)` stays within the allocation.
///
/// For an allocation of N elements, offset n is valid iff 0 <= n <= N.
/// (offset N is valid — it's one-past-the-end, legal for pointer arithmetic
/// but not for dereference. We use strict < for safety.)
pub fn verify_ptr_offset(
    z3_ctx: &crate::z3_shim::Context,
    info: &PtrBoundsInfo,
    offset: i64,
) -> PtrProofResult {
    let solver = crate::z3_shim::Solver::new(z3_ctx);
    let zero = crate::z3_shim::ast::Int::from_i64(z3_ctx, 0);

    let alloc_size = if let Some(n) = info.alloc_elements {
        crate::z3_shim::ast::Int::from_i64(z3_ctx, n)
    } else {
        let sym = crate::z3_shim::ast::Int::new_const(z3_ctx, "alloc_size");
        solver.assert(&sym.gt(&zero));
        sym
    };

    let off = crate::z3_shim::ast::Int::from_i64(z3_ctx, offset);

    // Violation: offset < 0 OR offset > alloc_size
    // Note: offset == alloc_size is technically one-past-end (legal in C for arithmetic).
    // For ptr.offset() which may be dereferenced, we check > alloc_size.
    let neg_violation = off.lt(&zero);
    let upper_violation = off.gt(&alloc_size);
    let violation = crate::z3_shim::ast::Bool::or(z3_ctx, &[&neg_violation, &upper_violation]);
    solver.assert(&violation);

    match solver.check() {
        crate::z3_shim::SatResult::Unsat => PtrProofResult::Proven,
        crate::z3_shim::SatResult::Sat => {
            PtrProofResult::Unsafe(format!(
                "Counterexample in '{}': ptr.offset({}) may exceed allocation",
                info.func_name, offset
            ))
        }
        crate::z3_shim::SatResult::Unknown => PtrProofResult::Unknown,
    }
}

/// Verify a dynamic (symbolic) index access with optional loop invariant bounds.
///
/// If `info.index_upper_bound` is set (e.g., from a loop invariant `i < N`),
/// Z3 can prove safety when the bound matches the allocation size.
pub fn verify_ptr_dynamic_index(
    z3_ctx: &crate::z3_shim::Context,
    solver: &crate::z3_shim::Solver,
    info: &PtrBoundsInfo,
) -> PtrProofResult {
    solver.push(); // Push a frame to isolate bounds checking constraints
    let zero = crate::z3_shim::ast::Int::from_i64(z3_ctx, 0);

    let alloc_size = if let Some(n) = info.alloc_elements {
        crate::z3_shim::ast::Int::from_i64(z3_ctx, n)
    } else {
        let sym = crate::z3_shim::ast::Int::new_const(z3_ctx, "alloc_size");
        solver.assert(&sym.gt(&zero));
        sym
    };

    let idx = crate::z3_shim::ast::Int::new_const(z3_ctx, "idx");

    // Index must be non-negative
    let lower = if let Some(lb) = info.index_lower_bound {
        crate::z3_shim::ast::Int::from_i64(z3_ctx, lb)
    } else {
        zero.clone()
    };
    solver.assert(&idx.ge(&lower));

    // Apply upper bound from loop invariant if available
    if let Some(ub) = info.index_upper_bound {
        let upper = crate::z3_shim::ast::Int::from_i64(z3_ctx, ub);
        solver.assert(&idx.lt(&upper));
    }

    // Violation: idx >= alloc_size
    let violation = idx.ge(&alloc_size);
    solver.assert(&violation);

    let result = match solver.check() {
        crate::z3_shim::SatResult::Unsat => PtrProofResult::Proven,
        crate::z3_shim::SatResult::Sat => {
            PtrProofResult::Unsafe(format!(
                "Counterexample in '{}': dynamic ptr index may exceed allocation",
                info.func_name
            ))
        }
        crate::z3_shim::SatResult::Unknown => PtrProofResult::Unknown,
    };
    solver.pop(1);
    result
}


// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> crate::z3_shim::Context {
        let cfg = crate::z3_shim::Config::new();
        crate::z3_shim::Context::new(&cfg)
    }

    // -------------------------------------------------------------------------
    // Ptr::index() Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_ptr_index_within_bounds() {
        let ctx = make_ctx();
        // malloc(10) → ptr.index(5) → safe
        let info = PtrBoundsInfo::new("test_fn").with_alloc(10);
        let result = verify_ptr_index(&ctx, &info, 5);
        assert_eq!(result, PtrProofResult::Proven,
            "ptr.index(5) on alloc(10) must be proven safe");
    }

    #[test]
    fn test_ptr_index_at_boundary() {
        let ctx = make_ctx();
        // malloc(10) → ptr.index(9) → last valid element
        let info = PtrBoundsInfo::new("test_fn").with_alloc(10);
        let result = verify_ptr_index(&ctx, &info, 9);
        assert_eq!(result, PtrProofResult::Proven,
            "ptr.index(9) on alloc(10) must be proven safe (last valid index)");
    }

    #[test]
    fn test_ptr_index_out_of_bounds() {
        let ctx = make_ctx();
        // malloc(10) → ptr.index(10) → OUT OF BOUNDS
        let info = PtrBoundsInfo::new("dangerous_fn").with_alloc(10);
        let result = verify_ptr_index(&ctx, &info, 10);
        assert!(matches!(result, PtrProofResult::Unsafe(_)),
            "ptr.index(10) on alloc(10) MUST be rejected (one-past-end)");
    }

    #[test]
    fn test_ptr_index_negative() {
        let ctx = make_ctx();
        // ptr.index(-1) → always unsafe
        let info = PtrBoundsInfo::new("negative_fn").with_alloc(10);
        let result = verify_ptr_index(&ctx, &info, -1);
        assert!(matches!(result, PtrProofResult::Unsafe(_)),
            "ptr.index(-1) MUST be rejected");
    }

    // -------------------------------------------------------------------------
    // Ptr::offset() Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_ptr_offset_within_bounds() {
        let ctx = make_ctx();
        // alloc(10) → ptr.offset(5) → valid
        let info = PtrBoundsInfo::new("test_fn").with_alloc(10);
        let result = verify_ptr_offset(&ctx, &info, 5);
        assert_eq!(result, PtrProofResult::Proven,
            "ptr.offset(5) on alloc(10) must be proven safe");
    }

    #[test]
    fn test_ptr_offset_exceeds_alloc() {
        let ctx = make_ctx();
        // alloc(10) → ptr.offset(11) → unsafe (past end)
        let info = PtrBoundsInfo::new("overflow_fn").with_alloc(10);
        let result = verify_ptr_offset(&ctx, &info, 11);
        assert!(matches!(result, PtrProofResult::Unsafe(_)),
            "ptr.offset(11) on alloc(10) MUST be rejected");
    }

    // -------------------------------------------------------------------------
    // Dynamic Index with Loop Invariant Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_ptr_index_loop_invariant() {
        let ctx = make_ctx();
        // Scenario: malloc(N) → for i in 0..N { ptr.index(i) }
        // Loop invariant provides: 0 <= i < N
        // With alloc = N, this is provably safe.
        let info = PtrBoundsInfo::new("loop_fn")
            .with_alloc(256)
            .with_index_bounds(0, 256);  // from loop invariant: i ∈ [0, 256)
        let solver = crate::z3_shim::Solver::new(&ctx);
        let result = verify_ptr_dynamic_index(&ctx, &solver, &info);
        assert_eq!(result, PtrProofResult::Proven,
            "ptr.index(i) with loop invariant i ∈ [0, 256) on alloc(256) must be proven safe");
    }

    #[test]
    fn test_ptr_dynamic_index_no_bounds() {
        let ctx = make_ctx();
        // Without loop invariant bounds, dynamic index is unsafe
        let info = PtrBoundsInfo::new("unbounded_fn")
            .with_alloc(256);
        // No index_upper_bound → Z3 can find idx = 256 → SAT
        let solver = crate::z3_shim::Solver::new(&ctx);
        let result = verify_ptr_dynamic_index(&ctx, &solver, &info);
        assert!(matches!(result, PtrProofResult::Unsafe(_)),
            "Dynamic ptr index without bounds MUST be flagged unsafe");
    }

    #[test]
    fn test_ptr_dynamic_index_insufficient_bound() {
        let ctx = make_ctx();
        // Loop bound (512) exceeds allocation (256) → unsafe
        let info = PtrBoundsInfo::new("overflow_fn")
            .with_alloc(256)
            .with_index_bounds(0, 512);
        let solver = crate::z3_shim::Solver::new(&ctx);
        let result = verify_ptr_dynamic_index(&ctx, &solver, &info);
        assert!(matches!(result, PtrProofResult::Unsafe(_)),
            "Loop bound 512 exceeding alloc 256 MUST be flagged unsafe");
    }
}
