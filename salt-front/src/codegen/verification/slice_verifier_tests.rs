#[cfg(test)]
mod tests {
    use crate::codegen::verification::slice_verifier::*;

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
    // Adversarial Diagnostic Tests
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

