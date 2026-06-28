//! Slice Verifier: Z3-based bounds check elision for KeuOSBuffer slices
//!
//! When Salt code creates a `buf.slice(start, end)` view, this pass uses
//! the Z3 solver to prove the slice bounds are valid at compile time.
//! If proven, the MLIR emitter skips the runtime bounds check, generating
//! a raw `llvm.load` instead of `cf.cond_br` + panic.
//!
//! The Formal Shadow Contract:
//!   ∀ slice ∈ RequestView:
//!     slice.start >= buf.base ∧ slice.end <= buf.base + buf.len
//!
//! Discovery Integration: When a SIMD intrinsic like `find_header_end`
//! returns a value, Z3 receives the invariant that the return value
//! is <= buf.length, enabling downstream slice elision.

use crate::z3_shim::ast::Ast;

/// Result of a Z3 slice verification attempt
#[derive(Debug, Clone, PartialEq)]
pub enum SliceProofResult {
    /// Z3 proved the access is always within bounds — elide the check
    Proven,
    /// Z3 found a counterexample — keep the runtime check
    Unsafe(String),
    /// Z3 timed out or the expression is too complex — keep the check
    Unknown,
}

/// Information about a slice operation extracted from the AST
#[derive(Debug, Clone)]
pub struct SliceInfo {
    pub buf_length: Option<i64>,     // Known buffer length (if constant)
    pub slice_start: Option<i64>,    // Slice start (if constant)
    pub slice_end: Option<i64>,      // Slice end (if constant)
    pub discovery_bound: Option<i64>, // Upper bound from SIMD discovery
    pub func_name: String,           // Enclosing function name
}

impl SliceInfo {
    pub fn new(func_name: &str) -> Self {
        SliceInfo {
            buf_length: None,
            slice_start: None,
            slice_end: None,
            discovery_bound: None,
            func_name: func_name.to_string(),
        }
    }

    pub fn with_buf_length(mut self, len: i64) -> Self {
        self.buf_length = Some(len);
        self
    }

    pub fn with_slice(mut self, start: i64, end: i64) -> Self {
        self.slice_start = Some(start);
        self.slice_end = Some(end);
        self
    }

    pub fn with_discovery_bound(mut self, bound: i64) -> Self {
        self.discovery_bound = Some(bound);
        self
    }
}

/// Verify that a slice access at `offset` within a `SliceInfo` is provably safe.
///
/// The Z3 solver attempts to find a counterexample where the access is
/// out of bounds. If no counterexample exists (UNSAT), the proof is solid.
pub fn verify_slice_access(
    z3_ctx: &crate::z3_shim::Context,
    slice: &SliceInfo,
    access_offset: i64,
) -> SliceProofResult {
    let solver = crate::z3_shim::Solver::new(z3_ctx);

    // 1. Declare symbolic variables
    let buf_len = crate::z3_shim::ast::Int::new_const(z3_ctx, "buf_len");
    let start = crate::z3_shim::ast::Int::new_const(z3_ctx, "start");
    let end = crate::z3_shim::ast::Int::new_const(z3_ctx, "end");
    let zero = crate::z3_shim::ast::Int::from_i64(z3_ctx, 0);

    // 2. Add physical reality constraints (DMA Arena invariants)
    // buf_len > 0
    solver.assert(&buf_len.gt(&zero));
    // 0 <= start <= end <= buf_len
    solver.assert(&start.ge(&zero));
    solver.assert(&start.le(&end));
    solver.assert(&end.le(&buf_len));

    // 3. Add concrete constraints from SliceInfo
    if let Some(len) = slice.buf_length {
        let len_const = crate::z3_shim::ast::Int::from_i64(z3_ctx, len);
        solver.assert(&buf_len._eq(&len_const));
    }
    if let Some(s) = slice.slice_start {
        let s_const = crate::z3_shim::ast::Int::from_i64(z3_ctx, s);
        solver.assert(&start._eq(&s_const));
    }
    if let Some(e) = slice.slice_end {
        let e_const = crate::z3_shim::ast::Int::from_i64(z3_ctx, e);
        solver.assert(&end._eq(&e_const));
    }

    // 4. Discovery integration: SIMD find_header_end provides an upper bound
    if let Some(bound) = slice.discovery_bound {
        let bound_const = crate::z3_shim::ast::Int::from_i64(z3_ctx, bound);
        solver.assert(&end.le(&bound_const));
        solver.assert(&bound_const.le(&buf_len));
    }

    // 5. Can access at (start + offset) violate (< end)?
    let offset_const = crate::z3_shim::ast::Int::from_i64(z3_ctx, access_offset);
    let access_pos = crate::z3_shim::ast::Int::add(z3_ctx, &[&start, &offset_const]);

    // Violation: access_pos >= end (out of bounds)
    let violation = access_pos.ge(&end);
    solver.assert(&violation);

    // If UNSAT: no counterexample exists → proof is solid
    match solver.check() {
        crate::z3_shim::SatResult::Unsat => {
            SliceProofResult::Proven
        }
        crate::z3_shim::SatResult::Sat => {
            let _model = solver.get_model().unwrap();
            let counter = format!(
                "Counterexample in '{}': access at offset {} may exceed slice bounds",
                slice.func_name, access_offset
            );
            SliceProofResult::Unsafe(counter)
        }
        crate::z3_shim::SatResult::Unknown => {
            SliceProofResult::Unknown
        }
    }
}

/// Verify a complete slice creation: buf.slice(start, end) where buf.length is known
pub fn verify_slice_creation(
    z3_ctx: &crate::z3_shim::Context,
    buf_length: i64,
    slice_start: i64,
    slice_end: i64,
) -> SliceProofResult {
    let solver = crate::z3_shim::Solver::new(z3_ctx);
    let zero = crate::z3_shim::ast::Int::from_i64(z3_ctx, 0);
    let len = crate::z3_shim::ast::Int::from_i64(z3_ctx, buf_length);
    let start = crate::z3_shim::ast::Int::from_i64(z3_ctx, slice_start);
    let end = crate::z3_shim::ast::Int::from_i64(z3_ctx, slice_end);

    // The violation: start > end OR end > buf_length OR start < 0
    let v1 = start.gt(&end);
    let v2 = end.gt(&len);
    let v3 = start.lt(&zero);
    let any_violation = crate::z3_shim::ast::Bool::or(z3_ctx, &[&v1, &v2, &v3]);

    solver.assert(&any_violation);

    match solver.check() {
        crate::z3_shim::SatResult::Unsat => SliceProofResult::Proven,
        crate::z3_shim::SatResult::Sat => {
            SliceProofResult::Unsafe("Slice bounds may exceed buffer".to_string())
        }
        crate::z3_shim::SatResult::Unknown => SliceProofResult::Unknown,
    }
}

/// Verify a slice access where the offset is symbolic (unknown at compile time).
///
/// This is the "ambiguous" case: `view[i]` where `i` is a runtime variable.
/// Without additional constraints on `i`, Z3 cannot prove safety, so the
/// compiler must force a runtime bounds check.
///
/// If `offset_upper_bound` is provided, Z3 has a chance to prove safety
/// if the bound is within the slice range.
pub fn verify_dynamic_slice_access(
    z3_ctx: &crate::z3_shim::Context,
    slice: &SliceInfo,
    offset_upper_bound: Option<i64>,
) -> SliceProofResult {
    let solver = crate::z3_shim::Solver::new(z3_ctx);

    // Symbolic variables
    let buf_len = crate::z3_shim::ast::Int::new_const(z3_ctx, "buf_len");
    let start = crate::z3_shim::ast::Int::new_const(z3_ctx, "start");
    let end = crate::z3_shim::ast::Int::new_const(z3_ctx, "end");
    let offset = crate::z3_shim::ast::Int::new_const(z3_ctx, "offset"); // symbolic!
    let zero = crate::z3_shim::ast::Int::from_i64(z3_ctx, 0);

    // Physical invariants
    solver.assert(&buf_len.gt(&zero));
    solver.assert(&start.ge(&zero));
    solver.assert(&start.le(&end));
    solver.assert(&end.le(&buf_len));

    // Offset must be non-negative
    solver.assert(&offset.ge(&zero));

    // Concrete constraints from SliceInfo
    if let Some(len) = slice.buf_length {
        solver.assert(&buf_len._eq(&crate::z3_shim::ast::Int::from_i64(z3_ctx, len)));
    }
    if let Some(s) = slice.slice_start {
        solver.assert(&start._eq(&crate::z3_shim::ast::Int::from_i64(z3_ctx, s)));
    }
    if let Some(e) = slice.slice_end {
        solver.assert(&end._eq(&crate::z3_shim::ast::Int::from_i64(z3_ctx, e)));
    }

    // If we have an upper bound on the offset (e.g., from a loop invariant)
    if let Some(bound) = offset_upper_bound {
        let bound_const = crate::z3_shim::ast::Int::from_i64(z3_ctx, bound);
        solver.assert(&offset.lt(&bound_const));
    }

    // Discovery integration
    if let Some(bound) = slice.discovery_bound {
        let bound_const = crate::z3_shim::ast::Int::from_i64(z3_ctx, bound);
        solver.assert(&end.le(&bound_const));
        solver.assert(&bound_const.le(&buf_len));
    }

    // Can (start + offset) >= end?
    let access_pos = crate::z3_shim::ast::Int::add(z3_ctx, &[&start, &offset]);
    let violation = access_pos.ge(&end);
    solver.assert(&violation);

    match solver.check() {
        crate::z3_shim::SatResult::Unsat => SliceProofResult::Proven,
        crate::z3_shim::SatResult::Sat => {
            let counter = format!(
                "Counterexample in '{}': symbolic offset may exceed slice bounds",
                slice.func_name
            );
            SliceProofResult::Unsafe(counter)
        }
        crate::z3_shim::SatResult::Unknown => SliceProofResult::Unknown,
    }
}

// =============================================================================
// Tests
// =============================================================================
