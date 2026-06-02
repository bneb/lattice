//! Slice Verifier: Z3-based bounds check elision for SovereignBuffer slices
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

    // 5. THE PROOF: Can access at (start + offset) violate (< end)?
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

    // THE PROOF: Can (start + offset) >= end?
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> crate::z3_shim::Context {
        let cfg = crate::z3_shim::Config::new();
        crate::z3_shim::Context::new(&cfg)
    }

    // -------------------------------------------------------------------------
    // Slice Creation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_valid_slice_creation() {
        let ctx = make_ctx();
        // buf.slice(0, 100) on a 1024-byte buffer → trivially safe
        let result = verify_slice_creation(&ctx, 1024, 0, 100);
        assert_eq!(result, SliceProofResult::Proven);
    }

    #[test]
    fn test_full_buffer_slice() {
        let ctx = make_ctx();
        // buf.slice(0, 1024) on a 1024-byte buffer → exactly valid
        let result = verify_slice_creation(&ctx, 1024, 0, 1024);
        assert_eq!(result, SliceProofResult::Proven);
    }

    #[test]
    fn test_slice_exceeds_buffer() {
        let ctx = make_ctx();
        // buf.slice(0, 2048) on a 1024-byte buffer → overflow
        let result = verify_slice_creation(&ctx, 1024, 0, 2048);
        assert!(matches!(result, SliceProofResult::Unsafe(_)));
    }

    #[test]
    fn test_slice_inverted_bounds() {
        let ctx = make_ctx();
        // buf.slice(100, 50) → start > end
        let result = verify_slice_creation(&ctx, 1024, 100, 50);
        assert!(matches!(result, SliceProofResult::Unsafe(_)));
    }

    #[test]
    fn test_slice_negative_start() {
        let ctx = make_ctx();
        // buf.slice(-1, 100) → negative start
        let result = verify_slice_creation(&ctx, 1024, -1, 100);
        assert!(matches!(result, SliceProofResult::Unsafe(_)));
    }

    #[test]
    fn test_empty_slice() {
        let ctx = make_ctx();
        // buf.slice(50, 50) → zero-length slice, valid
        let result = verify_slice_creation(&ctx, 1024, 50, 50);
        assert_eq!(result, SliceProofResult::Proven);
    }

    // -------------------------------------------------------------------------
    // Slice Access Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_access_first_byte_of_slice() {
        let ctx = make_ctx();
        // buf.slice(0, 100)[0] → access at offset 0, end is 100
        let info = SliceInfo::new("test_handler")
            .with_buf_length(1024)
            .with_slice(0, 100);
        let result = verify_slice_access(&ctx, &info, 0);
        assert_eq!(result, SliceProofResult::Proven);
    }

    #[test]
    fn test_access_last_valid_byte() {
        let ctx = make_ctx();
        // buf.slice(0, 100)[99] → last valid index
        let info = SliceInfo::new("test_handler")
            .with_buf_length(1024)
            .with_slice(0, 100);
        let result = verify_slice_access(&ctx, &info, 99);
        assert_eq!(result, SliceProofResult::Proven);
    }

    #[test]
    fn test_access_beyond_slice_end() {
        let ctx = make_ctx();
        // buf.slice(0, 100)[100] → out of bounds (end is exclusive)
        let info = SliceInfo::new("test_handler")
            .with_buf_length(1024)
            .with_slice(0, 100);
        let result = verify_slice_access(&ctx, &info, 100);
        assert!(matches!(result, SliceProofResult::Unsafe(_)));
    }

    #[test]
    fn test_access_way_beyond_slice() {
        let ctx = make_ctx();
        // buf.slice(0, 100)[500] → way out of bounds
        let info = SliceInfo::new("echo_handler")
            .with_buf_length(1024)
            .with_slice(0, 100);
        let result = verify_slice_access(&ctx, &info, 500);
        assert!(matches!(result, SliceProofResult::Unsafe(_)));
    }

    // -------------------------------------------------------------------------
    // Discovery Integration (SIMD find_header_end)
    // -------------------------------------------------------------------------

    #[test]
    fn test_simd_discovery_enables_elision() {
        let ctx = make_ctx();
        // SIMD found \r\n\r\n at position 256 in a 4096-byte buffer
        // So we know: 0 <= view.end <= 256 <= buf.len
        // Access at offset 0 should be proven safe
        let info = SliceInfo::new("parse_http")
            .with_buf_length(4096)
            .with_slice(0, 256)
            .with_discovery_bound(256);
        let result = verify_slice_access(&ctx, &info, 0);
        assert_eq!(result, SliceProofResult::Proven);
    }

    #[test]
    fn test_simd_discovery_boundary_access() {
        let ctx = make_ctx();
        // Discovery bound at 256, slice is (0, 256), access at 255
        let info = SliceInfo::new("parse_http")
            .with_buf_length(4096)
            .with_slice(0, 256)
            .with_discovery_bound(256);
        let result = verify_slice_access(&ctx, &info, 255);
        assert_eq!(result, SliceProofResult::Proven);
    }

    #[test]
    fn test_simd_discovery_beyond_boundary() {
        let ctx = make_ctx();
        // Discovery bound at 256, trying to access at offset 256 → out of bounds
        let info = SliceInfo::new("parse_http")
            .with_buf_length(4096)
            .with_slice(0, 256)
            .with_discovery_bound(256);
        let result = verify_slice_access(&ctx, &info, 256);
        assert!(matches!(result, SliceProofResult::Unsafe(_)));
    }

    // -------------------------------------------------------------------------
    // HTTP Echo Handler Scenario (The multi_pulse_demo pattern)
    // -------------------------------------------------------------------------

    #[test]
    fn test_http_first_byte_check() {
        let ctx = make_ctx();
        // echo_handler: buf.slice(0, end) where end >= 4 (from \r\n\r\n)
        // Accessing view[0] to check for 'G' (GET request)
        // Z3 knows: end >= 4, so access at 0 is trivially safe
        let info = SliceInfo::new("echo_handler")
            .with_buf_length(4096)
            .with_slice(0, 4)
            .with_discovery_bound(4096);
        let result = verify_slice_access(&ctx, &info, 0);
        assert_eq!(
            result,
            SliceProofResult::Proven,
            "First byte of an HTTP request (checking for 'G') must be elided"
        );
    }

    #[test]
    fn test_http_method_slice_get() {
        let ctx = make_ctx();
        // "GET " is 4 bytes → slice(0, 4), access at index 3
        let info = SliceInfo::new("parse_http_request")
            .with_buf_length(4096)
            .with_slice(0, 4);
        let result = verify_slice_access(&ctx, &info, 3);
        assert_eq!(result, SliceProofResult::Proven);
    }

    // -------------------------------------------------------------------------
    // Edge Cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_zero_length_buffer_access() {
        let ctx = make_ctx();
        // Accessing any offset in a zero-length slice is always unsafe
        let info = SliceInfo::new("edge_case")
            .with_buf_length(1024)
            .with_slice(0, 0);
        let result = verify_slice_access(&ctx, &info, 0);
        assert!(matches!(result, SliceProofResult::Unsafe(_)));
    }

    #[test]
    fn test_mid_buffer_slice() {
        let ctx = make_ctx();
        // slice(500, 600) — accessing relative offset 50 (absolute 550)
        let info = SliceInfo::new("mid_handler")
            .with_buf_length(1024)
            .with_slice(500, 600);
        let result = verify_slice_access(&ctx, &info, 50);
        assert_eq!(result, SliceProofResult::Proven);
    }

    // =========================================================================
    // [CODE RED] Adversarial Diagnostic Tests
    // =========================================================================
    //
    // The Code Red suite verifies that the Z3 Formal Shadow is a RIGOROUS
    // ENFORCER, not a rubber stamp. Each test intentionally attempts to
    // trick the compiler into eliding a bounds check for invalid memory.
    //
    // Citadel Security Audit:
    //   Legal Access   (view[0])     → UNSAT → Proven → Elide Check
    //   Illegal Access (view[10000]) → SAT   → Unsafe → Halt Compilation
    //   Ambiguous      (view[i])     → SAT   → Unsafe → Force Runtime Check
    //

    /// CODE RED: The "Hijack" Scenario
    ///
    /// Attacker model: SIMD find_header_end returns end=128 on a 16384-byte buffer.
    /// Attacker tries `view[10000]` — reading 10000 bytes past the slice start.
    ///
    /// Z3 Logic:
    ///   Assert: 0 <= start=0, end<=128, limit=16384
    ///   Negate safety: start + 10000 >= end
    ///   Result: SAT (counterexample: end=128, 10000 >= 128)
    ///
    /// Expected: Unsafe → compiler HALTS compilation.
    #[test]
    fn test_code_red_hijack_view_10000() {
        let ctx = make_ctx();
        // Simulating out_of_bounds_hijack.salt:
        //   let end = find_header_end(buf); // returns dynamically, but <= 16384
        //   let view = buf.slice(0, end);
        //   let illegal = view[10000]; // CODE RED
        //
        // The slice end is dynamically bounded. We don't know the exact value,
        // but we know: 0 <= end <= 16384 (buf capacity).
        // Even with the maximum possible end (16384), we'd need to verify
        // that 10000 < end. But end could be as small as 4 (\r\n\r\n).
        //
        // Use a worst-case scenario: end is symbolic (unknown), only bounded
        // by the discovery invariant.
        let info = SliceInfo::new("dangerous_handler")
            .with_buf_length(16384);
        // Note: slice_end is NOT set — it's symbolic (unknown at compile time)
        // Only constraint: end <= buf_length (from DMA invariant)

        let result = verify_slice_access(&ctx, &info, 10000);

        assert!(
            matches!(result, SliceProofResult::Unsafe(_)),
            "[CODE RED] CRITICAL: The compiler MUST reject view[10000] on a \
             dynamically-bounded slice. Z3 should find a counterexample where \
             end <= 10000. Got: {:?}",
            result
        );
    }

    /// CODE RED: The same hijack with a discovery bound from SIMD.
    /// Even with discovery_bound=16384, end could be 128.
    /// view[10000] is STILL unsafe because end is only bounded above.
    #[test]
    fn test_code_red_hijack_with_discovery_bound() {
        let ctx = make_ctx();
        let info = SliceInfo::new("dangerous_handler")
            .with_buf_length(16384)
            .with_discovery_bound(16384);
        // end is symbolic, only know: 0 <= end <= 16384

        let result = verify_slice_access(&ctx, &info, 10000);

        assert!(
            matches!(result, SliceProofResult::Unsafe(_)),
            "[CODE RED] Even with discovery bound, view[10000] must be rejected \
             because end could be much smaller than 10000. Got: {:?}",
            result
        );
    }

    /// CODE RED: Verify that a small, provably-safe access PASSES
    /// on the same dynamically-bounded slice.
    ///
    /// If end is at least 4 (minimum HTTP header: "X\r\n\r\n"),
    /// then view[0] should be proven safe.
    #[test]
    fn test_code_red_legal_access_view_0() {
        let ctx = make_ctx();
        // Even with dynamic end, if we constrain end >= 4 (from finding \r\n\r\n),
        // then view[0] is trivially safe: 0 + 0 = 0 < 4.
        let info = SliceInfo::new("echo_handler")
            .with_buf_length(16384)
            .with_slice(0, 4);
        // end is concretely 4 (minimum from successful header parse)

        let result = verify_slice_access(&ctx, &info, 0);

        assert_eq!(
            result,
            SliceProofResult::Proven,
            "[CODE RED] Legal access view[0] with end=4 MUST be proven safe. \
             This is the 'elide check' fast path. Got: {:?}",
            result
        );
    }

    /// CODE RED: Ambiguous access — view[i] where i is symbolic.
    /// Without knowing i's range, the compiler MUST force a runtime check.
    #[test]
    fn test_code_red_ambiguous_symbolic_offset() {
        let ctx = make_ctx();
        // Scenario: for i in 0..N { view[i] }
        // Without loop invariant analysis proving i < end, this is ambiguous.
        let info = SliceInfo::new("loop_handler")
            .with_buf_length(4096)
            .with_slice(0, 256);

        // No upper bound on offset → Z3 can find offset = 256 (= end) → SAT
        let result = verify_dynamic_slice_access(&ctx, &info, None);

        assert!(
            matches!(result, SliceProofResult::Unsafe(_)),
            "[CODE RED] Symbolic offset view[i] without bound MUST force a \
             runtime check. The compiler cannot elide this. Got: {:?}",
            result
        );
    }

    /// CODE RED: Symbolic offset WITH a proven upper bound.
    /// If a loop invariant proves i < end, the access is safe.
    #[test]
    fn test_code_red_symbolic_offset_with_proven_bound() {
        let ctx = make_ctx();
        // Scenario: for i in 0..slice_end { view[i] }
        // Loop invariant: i < 256 (= slice_end)
        let info = SliceInfo::new("bounded_loop_handler")
            .with_buf_length(4096)
            .with_slice(0, 256);

        // Upper bound = 256 = slice_end → offset < 256, and end = 256
        // So offset < end is always true → UNSAT → Proven
        let result = verify_dynamic_slice_access(&ctx, &info, Some(256));

        assert_eq!(
            result,
            SliceProofResult::Proven,
            "[CODE RED] Symbolic offset with proven bound i < 256 on slice(0,256) \
             MUST be elided. Got: {:?}",
            result
        );
    }

    /// CODE RED: Symbolic offset with an INSUFFICIENT bound.
    /// The bound exceeds the slice end → unsafe.
    #[test]
    fn test_code_red_symbolic_offset_insufficient_bound() {
        let ctx = make_ctx();
        // Scenario: loop bound is 512 but slice is only 256 bytes
        let info = SliceInfo::new("overflow_loop_handler")
            .with_buf_length(4096)
            .with_slice(0, 256);

        // Upper bound = 512 > 256 (slice_end) → offset could be 256..511 → SAT
        let result = verify_dynamic_slice_access(&ctx, &info, Some(512));

        assert!(
            matches!(result, SliceProofResult::Unsafe(_)),
            "[CODE RED] Symbolic offset bounded by 512 on slice(0,256) MUST be \
             rejected. The loop could overflow the slice. Got: {:?}",
            result
        );
    }

    /// CODE RED: The exact "out_of_bounds_hijack.salt" scenario
    /// with realistic HTTP buffer parameters.
    #[test]
    fn test_code_red_exact_hijack_scenario() {
        let ctx = make_ctx();
        // Exact reproduction of out_of_bounds_hijack.salt:
        //   buf capacity = 16384 (4 pages)
        //   SIMD returns end = 128 (short HTTP GET)
        //   Attacker accesses view[10000]
        //
        // Z3 SMT-LIB equivalent:
        //   (assert (= start 0))
        //   (assert (= end 128))
        //   (assert (= limit 16384))
        //   (assert (>= (+ start 10000) end))  ; violation
        //   → SAT with model: start=0, end=128, 10000 >= 128
        let info = SliceInfo::new("dangerous_handler")
            .with_buf_length(16384)
            .with_slice(0, 128)
            .with_discovery_bound(16384);

        let result = verify_slice_access(&ctx, &info, 10000);

        assert!(
            matches!(result, SliceProofResult::Unsafe(_)),
            "[CODE RED EXACT] view[10000] on slice(0,128) MUST be rejected. \
             Z3 counterexample: 0 + 10000 = 10000 >= 128. Got: {:?}",
            result
        );

        // Also verify the error message mentions the function name
        if let SliceProofResult::Unsafe(msg) = &result {
            assert!(
                msg.contains("dangerous_handler"),
                "Error message should identify the function: {}",
                msg
            );
        }
    }
}

