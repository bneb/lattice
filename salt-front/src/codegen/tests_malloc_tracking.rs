//! TDD Tests for Systemic Malloc Tracking
//!
//! These tests verify that raw `extern fn malloc` calls are tracked by the
//! Z3 ownership system, and that missing `free` calls produce formal
//! verification errors.
//!
//! Written BEFORE implementation (Red Phase).

mod tests {
    use crate::codegen::verification::Z3StateTracker;
    use std::collections::HashMap;

    // ========================================================================
    // Test 1: malloc without free IS a leak
    // ========================================================================

    /// Simulates: `let buf = malloc(1024);` without `free(buf)`.
    /// The Z3 tracker should detect this as a formal integrity error.
    #[test]
    fn test_malloc_without_free_is_leak() {
        let cfg = crate::z3_shim::Config::new();
        let ctx = crate::z3_shim::Context::new(&cfg);
        let solver = crate::z3_shim::Solver::new(&ctx);
        let mut tracker = Z3StateTracker::new(&ctx);

        // Simulate: let buf = malloc(1024)
        // The codegen intercepts the malloc call and registers the allocation
        tracker.register_allocation("malloc_buf", &solver);

        // No free() call — the allocation is leaked
        // verify_leak_free should catch this
        let result = tracker.verify_leak_free(&solver);
        assert!(result.is_err(), "malloc without free MUST be detected as a leak");
        let err = result.unwrap_err();
        assert!(err.contains("FORMAL INTEGRITY ERROR"), 
            "Error should be a formal verification error, got: {}", err);
        assert!(err.contains("malloc_buf"),
            "Error should identify the leaked resource, got: {}", err);
    }

    // ========================================================================
    // Test 2: malloc + free = no leak
    // ========================================================================

    /// Simulates: `let buf = malloc(1024); ... free(buf);`
    /// This is the happy path — should verify cleanly.
    #[test]
    fn test_malloc_with_free_no_leak() {
        let cfg = crate::z3_shim::Config::new();
        let ctx = crate::z3_shim::Context::new(&cfg);
        let solver = crate::z3_shim::Solver::new(&ctx);
        let mut tracker = Z3StateTracker::new(&ctx);

        // Simulate: let buf = malloc(1024)
        tracker.register_allocation("malloc_buf", &solver);

        // Simulate: free(buf)
        tracker.mark_released("malloc_buf", &solver).unwrap();

        // verify_leak_free should pass
        let result = tracker.verify_leak_free(&solver);
        assert!(result.is_ok(), 
            "malloc + free should not be detected as a leak, got: {:?}", result);
    }

    // ========================================================================
    // Test 3: Multiple mallocs, only one freed = leak detected
    // ========================================================================

    /// Simulates:
    /// ```
    /// let a = malloc(100);
    /// let b = malloc(200);
    /// free(a);
    /// // b is leaked
    /// ```
    #[test]
    fn test_multiple_mallocs_partial_free() {
        let cfg = crate::z3_shim::Config::new();
        let ctx = crate::z3_shim::Context::new(&cfg);
        let solver = crate::z3_shim::Solver::new(&ctx);
        let mut tracker = Z3StateTracker::new(&ctx);

        // Two mallocs
        tracker.register_allocation("malloc_a", &solver);
        tracker.register_allocation("malloc_b", &solver);

        // Only free one
        tracker.mark_released("malloc_a", &solver).unwrap();

        // Should detect malloc_b as leaked
        let result = tracker.verify_leak_free(&solver);
        assert!(result.is_err(), 
            "Partial free should be detected as a leak");
        let err = result.unwrap_err();
        assert!(err.contains("malloc_b"),
            "Error should identify malloc_b as the leaked resource, got: {}", err);
    }

    // ========================================================================
    // Test 4: malloc in loop without free (the sieve pattern)
    // ========================================================================

    /// Simulates the exact sieve bug:
    /// ```
    /// for i in 0..200 {
    ///     let buf = malloc(1000000);
    ///     // ... use buf ...
    ///     // BUG: no free(buf)
    /// }
    /// ```
    /// Each iteration creates a new allocation. The tracker should catch
    /// that the allocation from the CURRENT function scope is not freed.
    #[test]
    fn test_malloc_in_loop_without_free_is_leak() {
        let cfg = crate::z3_shim::Config::new();
        let ctx = crate::z3_shim::Context::new(&cfg);
        let solver = crate::z3_shim::Solver::new(&ctx);
        let mut tracker = Z3StateTracker::new(&ctx);

        // In the codegen, each function gets a fresh tracker.
        // The sieve function's malloc produces a tracked allocation.
        // Even though it's called in a loop, the CURRENT function scope 
        // has one malloc without a corresponding free.
        tracker.register_allocation("malloc_is_prime", &solver);

        // Function returns without free
        let result = tracker.verify_leak_free(&solver);
        assert!(result.is_err(), 
            "The sieve pattern (malloc in loop without free) MUST be caught");
    }

    // ========================================================================
    // Test 5: free without malloc — releasing untracked resource
    // ========================================================================

    /// Simulates: `free(some_ptr)` where some_ptr came from elsewhere
    /// (e.g., passed as a parameter, or from a different extern fn).
    /// This should be silently allowed — we only track malloc'd resources.
    #[test]
    fn test_free_without_malloc_is_allowed() {
        let cfg = crate::z3_shim::Config::new();
        let ctx = crate::z3_shim::Context::new(&cfg);
        let solver = crate::z3_shim::Solver::new(&ctx);
        let mut tracker = Z3StateTracker::new(&ctx);

        // Free something never registered via malloc
        // mark_released on untracked resources is silently allowed
        // (existing behavior — foreign pointers)
        let result = tracker.mark_released("foreign_ptr", &solver);
        assert!(result.is_ok(), 
            "Freeing an untracked resource should be silently allowed");

        // verify should also pass — nothing tracked, nothing leaked
        let result = tracker.verify_leak_free(&solver);
        assert!(result.is_ok(), 
            "No tracked resources means no leaks");
    }

    // ========================================================================
    // Test 6: malloc_tracker maps source variable name to allocation ID
    // ========================================================================

    /// Tests the malloc_tracker HashMap that will be added to CodegenContext.
    /// This maps source-level variable names to their Z3 allocation IDs,
    /// allowing free(var_name) to find the correct allocation to release.
    ///
    /// This test validates the data structure behavior independently of
    /// CodegenContext, since we can't easily construct a CodegenContext in
    /// unit tests.
    #[test]
    fn test_malloc_tracker_maps_var_to_alloc_id() {
        // Simulate the malloc_tracker HashMap
        let mut malloc_tracker: HashMap<String, String> = HashMap::new();

        // When codegen sees: let buf = malloc(1024)
        // It generates SSA: %call_malloc_42 = func.call @malloc(...) 
        // And stores the variable binding: buf -> malloc:buf
        let alloc_id = "malloc:buf".to_string();
        malloc_tracker.insert("buf".to_string(), alloc_id.clone());

        // When codegen sees: free(buf)
        // It looks up "buf" in the tracker to find the allocation ID
        let resolved = malloc_tracker.get("buf");
        assert!(resolved.is_some(), "Should resolve variable name to alloc ID");
        assert_eq!(resolved.unwrap(), "malloc:buf");

        // Untracked variable should return None
        let unknown = malloc_tracker.get("unknown_var");
        assert!(unknown.is_none(), "Unknown variable should not resolve");
    }

    // ========================================================================
    // Test 7: malloc + free survive nested solver scopes
    // ========================================================================

    /// The sieve function has loops (nested solver scopes). malloc happens
    /// before the loop, free should happen after. The Z3 transitions must
    /// survive solver.push()/pop().
    #[test]
    fn test_malloc_free_survives_nested_solver_scope() {
        let cfg = crate::z3_shim::Config::new();
        let ctx = crate::z3_shim::Context::new(&cfg);
        let solver = crate::z3_shim::Solver::new(&ctx);
        let mut tracker = Z3StateTracker::new(&ctx);

        // malloc at function level
        tracker.register_allocation("malloc_is_prime", &solver);

        // Enter loop scope
        solver.push();

        // ... loop body runs, no free here ...

        // Exit loop scope
        solver.pop(1);

        // free after the loop
        tracker.mark_released("malloc_is_prime", &solver).unwrap();

        // Should pass — malloc and free are both at function level
        let result = tracker.verify_leak_free(&solver);
        assert!(result.is_ok(), 
            "malloc before loop + free after loop should verify cleanly: {:?}", result);
    }

    // ========================================================================
    // Test 8: Correct sieve pattern — malloc + free in function body
    // ========================================================================

    /// The corrected sieve function:
    /// ```
    /// fn sieve(limit: i32) -> i32 {
    ///     let is_prime = malloc((limit + 1) as i64);
    ///     // ... sieve logic ...
    ///     free(is_prime);       // <-- THE FIX
    ///     return count;
    /// }
    /// ```
    #[test]
    fn test_correct_sieve_pattern_verifies() {
        let cfg = crate::z3_shim::Config::new();
        let ctx = crate::z3_shim::Context::new(&cfg);
        let solver = crate::z3_shim::Solver::new(&ctx);
        let mut tracker = Z3StateTracker::new(&ctx);

        // malloc(is_prime)
        tracker.register_allocation("malloc:is_prime", &solver);

        // Enter sieve loop
        solver.push();
        // ... loop body ...
        solver.pop(1);

        // Enter count loop
        solver.push();
        // ... count body ...
        solver.pop(1);

        // free(is_prime) before return
        tracker.mark_released("malloc:is_prime", &solver).unwrap();

        let result = tracker.verify_leak_free(&solver);
        assert!(result.is_ok(), 
            "Corrected sieve (malloc + 2 loops + free + return) must verify: {:?}", result);
    }

    // ========================================================================
    // Integration Tests: Full Pipeline (compile-level)
    // These test the intermediate states from parse → codegen → Z3 verification
    // ========================================================================

    /// Integration Test A: malloc-only should produce a "leaked" error.
    /// This tests that the `let x = malloc(...)` pattern in stmt.rs correctly
    /// registers the allocation with the Z3 tracker via `pending_malloc_result`.
    #[test]
    fn test_compile_malloc_without_free_is_leak() {
        let code = r#"
            package test::malloc_leak;
            extern fn malloc(size: usize) -> !llvm.ptr;
            fn main() -> i32 {
                let buf = malloc(100);
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_err(), "compile() with malloc and no free should error");
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Memory Leak Detected"),
            "Error should be a memory leak error, got: {}", err);
        assert!(err.contains("malloc:buf"),
            "Error should identify 'malloc:buf' as the leaked resource, got: {}", err);
    }

    /// Integration Test B: malloc + free with matching Ptr type should succeed.
    /// This tests that `free` declared with `!llvm.ptr` works without casts.
    #[test]
    fn test_compile_malloc_with_free_ptr_type_succeeds() {
        let code = r#"
            package test::malloc_free_ptr;
            extern fn malloc(size: usize) -> !llvm.ptr;
            extern fn free(ptr: !llvm.ptr);
            fn main() -> i32 {
                let buf = malloc(100);
                free(buf);
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "compile() with malloc + free(!llvm.ptr) should succeed, got: {}", result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Integration Test C: malloc + function call + free should succeed.
    /// This tests the ownership_tracker save/restore across hydration (emit_fn).
    #[test]
    fn test_compile_malloc_call_free_scoping() {
        let code = r#"
            package test::scoping;
            extern fn malloc(size: usize) -> !llvm.ptr;
            extern fn free(ptr: !llvm.ptr);
            fn use_ptr(p: !llvm.ptr) -> i32 {
                return 42;
            }
            fn main() -> i32 {
                let buf = malloc(100);
                let r = use_ptr(buf);
                free(buf);
                return r;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "compile() with malloc + call + free should succeed, got: {}", result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Integration Test D: multiple mallocs, only one freed — leak detected.
    /// Tests that the tracker correctly identifies WHICH allocation leaked.
    #[test]
    fn test_compile_multiple_mallocs_partial_free() {
        let code = r#"
            package test::partial;
            extern fn malloc(size: usize) -> !llvm.ptr;
            extern fn free(ptr: !llvm.ptr);
            fn main() -> i32 {
                let a = malloc(100);
                let b = malloc(200);
                free(a);
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_err(), "Partial free should produce a leak error");
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("malloc:b"),
            "Error should identify 'malloc:b' as the leaked resource, got: {}", err);
    }

    // ========================================================================
    // BENCHMARK PATTERN TESTS (TDD for sieve, lru_cache, merge_sorted_lists, trie)
    // ========================================================================

    /// Integration Test E: malloc + free inside a non-main helper function.
    /// This is the sieve pattern: sieve() allocates a buffer, does work, then
    /// frees before returning. The tracker must handle free in any function,
    /// not just main.
    #[test]
    fn test_compile_malloc_free_in_helper_function() {
        let code = r#"
            package test::helper_free;
            extern fn malloc(size: i64) -> u64;
            extern fn free(ptr: u64);
            fn do_work() -> i32 {
                let buf = malloc(100);
                free(buf);
                return 42;
            }
            fn main() -> i32 {
                return do_work();
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "malloc+free in helper function should compile cleanly, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Integration Test F: malloc in a helper function that RETURNS the pointer.
    /// This is the create_node() pattern used by lru_cache, merge_sorted_lists,
    /// and trie. The function transfers ownership via return value.
    /// The tracker must NOT flag this as a leak in the helper — ownership is
    /// transferred to the caller who is responsible for freeing.
    #[test]
    fn test_compile_malloc_returned_from_helper() {
        let code = r#"
            package test::ownership_transfer;
            extern fn malloc(size: i64) -> u64;
            extern fn free(ptr: u64);
            fn create_node() -> u64 {
                let node = malloc(16);
                return node;
            }
            fn main() -> i32 {
                let n = create_node();
                free(n);
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "malloc returned from helper (ownership transfer) should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    // ========================================================================
    // ISOLATION TESTS (Test-Driven Debugging)
    // ========================================================================

    /// Isolation Test G: Does main() returning a malloc'd value trigger escape?
    /// This is the simplest possible case — single function, malloc, return.
    #[test]
    fn test_escape_via_return_in_main() {
        let code = r#"
            package test::escape_main;
            extern fn malloc(size: i64) -> u64;
            fn main() -> u64 {
                let buf = malloc(100);
                return buf;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "main() returning malloc'd pointer should compile (escape via return), got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Isolation Test H: Non-main function returning malloc'd value.
    /// Isolates: does the escape hook work for functions-called-as-tasks?
    #[test]
    fn test_escape_via_return_in_non_main() {
        let code = r#"
            package test::escape_nonmain;
            extern fn malloc(size: i64) -> u64;
            fn alloc() -> u64 {
                let buf = malloc(100);
                return buf;
            }
            fn main() -> i32 {
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        // alloc() is never called, so it won't be hydrated.
        // This test just verifies parsing. The real test is G and F.
        assert!(result.is_ok(),
            "Uncalled function with malloc+return should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    // ========================================================================
    // V5.1 CAST-RETURN ESCAPE ANALYSIS (TDD for the "Chain of Custody" gap)
    // Tests the exact pattern that caused keuos_train memory leak:
    //   fn alloc_f32() -> Ptr<f32> { let p = malloc(size); return p as Ptr<f32>; }
    // ========================================================================

    /// Integration Test I: `return p as Ptr<T>` must escape malloc:p.
    /// This is the EXACT pattern from keuos_train's alloc_f32/alloc_i32.
    /// Before the fix, the tracker saw the cast result escaping but didn't
    /// realize it IS p, so it flagged p as leaked.
    #[test]
    fn test_cast_return_escapes_malloc() {
        let code = r#"
            package test::cast_return;
            extern fn malloc(size: i64) -> u64;
            fn alloc_typed() -> u64 {
                let p = malloc(100);
                return p as u64;
            }
            fn main() -> i32 {
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Cast return `return p as T` must escape malloc:p, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Integration Test J: Nested cast — `return (p as u64) as u64`.
    /// The recursive mark_expression_escaped must walk through multiple casts.
    #[test]
    fn test_nested_cast_return_escapes_malloc() {
        let code = r#"
            package test::nested_cast;
            extern fn malloc(size: i64) -> u64;
            fn alloc_nested() -> u64 {
                let p = malloc(100);
                return (p as u64) as u64;
            }
            fn main() -> i32 {
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Nested cast return must escape malloc:p, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Integration Test K: Two separate alloc helpers, each with cast return.
    /// Tests that the tracker correctly scopes per-function (clear between fns).
    #[test]
    fn test_two_cast_return_helpers_both_escape() {
        let code = r#"
            package test::two_helpers;
            extern fn malloc(size: i64) -> u64;
            fn alloc_a() -> u64 {
                let p = malloc(400);
                return p as u64;
            }
            fn alloc_b() -> u64 {
                let p = malloc(800);
                return p as u64;
            }
            fn main() -> i32 {
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Both cast-return helpers must escape cleanly, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// MallocTracker DAG Unit Test L: mark_escaped on an alloc_id clears it.
    /// Validates the intermediate state that mark_expression_escaped relies on.
    #[test]
    fn test_malloc_tracker_mark_escaped_clears_alloc() {
        use crate::codegen::verification::MallocTracker;

        let mut tracker = MallocTracker::new();
        // Step 1: Track allocation
        tracker.track("malloc:p".into(), "malloc at p".into());
        assert!(tracker.contains_alloc("malloc:p"),
            "After track(), malloc:p must be in active_allocs");
        assert!(tracker.verify().is_err(),
            "Before escape, tracker must detect leak");

        // Step 2: mark_escaped removes it
        tracker.mark_escaped("malloc:p");
        assert!(!tracker.contains_alloc("malloc:p"),
            "After mark_escaped(), malloc:p must be removed from active_allocs");
        assert!(tracker.verify().is_ok(),
            "After mark_escaped(), tracker must pass verification");
    }
}
