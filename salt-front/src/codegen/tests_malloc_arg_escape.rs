//! TDD Tests: Malloc Tracker — Function Argument Escape
//!
//! When a malloc'd pointer is passed as an argument to another function,
//! the tracker should NOT flag it as a leak. The pointer's ownership is
//! "shared" or "transferred" to the callee.
//!
//! Bug discovered during Basalt WASM TDD:
//!   let tokens = malloc(n * 8);
//!   basalt_engine_ingest_prompt(es, tokens, n);
//!   // Z3 flags tokens as leaked — WRONG
//!
//! Written BEFORE implementation (Red Phase).

#[cfg(test)]
mod tests {

    fn compile_program(code: &str) -> Result<String, String> {
        crate::compile(code, false, None, true)
            .map_err(|e| format!("{}", e))
    }

    // =========================================================================
    // RED Test 1: malloc'd pointer passed to function, not freed → SHOULD PASS
    // =========================================================================

    /// The exact Basalt pattern:
    ///   let tokens = malloc(24);
    ///   consume_tokens(tokens, 3);
    /// The tracker must realize tokens escaped via function argument.
    #[test]
    fn test_malloc_passed_as_arg_not_leaked() {
        let code = r#"
            package test::arg_escape;
            extern fn malloc(size: i64) -> Ptr<u8>;
            fn consume(p: Ptr<u8>, n: i64) -> i64 {
                return n;
            }
            fn main() -> i32 {
                let buf = malloc(24);
                let r = consume(buf, 3);
                return r as i32;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "malloc'd pointer passed as function argument should not be flagged as leak, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 2: malloc'd pointer passed to extern fn → SHOULD PASS
    // =========================================================================

    /// Passing a malloc'd pointer to an extern C function (the WASM bridge pattern).
    #[test]
    fn test_malloc_passed_to_extern_not_leaked() {
        let code = r#"
            package test::extern_arg_escape;
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn process_buffer(buf: Ptr<u8>, len: i64);
            fn main() -> i32 {
                let buf = malloc(64);
                process_buffer(buf, 64);
                return 0;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "malloc'd pointer passed to extern fn should not be flagged as leak, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 3: malloc'd pointer passed AND freed → SHOULD PASS (no double-count)
    // =========================================================================

    /// Even if the pointer is marked escaped via argument AND later freed,
    /// the tracker should not error.
    #[test]
    fn test_malloc_passed_and_freed_not_leaked() {
        let code = r#"
            package test::arg_and_free;
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);
            fn use_buffer(p: Ptr<u8>) -> i32 {
                return 42;
            }
            fn main() -> i32 {
                let buf = malloc(64);
                let r = use_buffer(buf);
                free(buf);
                return r;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "malloc'd pointer passed to fn AND freed should compile cleanly, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 4: Multiple malloc'd pointers, one passed, one not → partial leak
    // =========================================================================

    /// Regression: we still want ACTUAL leaks detected. Only argument-escape
    /// should be forgiven.
    #[test]
    fn test_actual_leak_still_detected_when_arg_escape_works() {
        let code = r#"
            package test::mixed_escape;
            extern fn malloc(size: i64) -> Ptr<u8>;
            fn consume(p: Ptr<u8>) -> i32 {
                return 1;
            }
            fn main() -> i32 {
                let a = malloc(100);
                let b = malloc(200);
                consume(a);
                return 0;
            }
        "#;
        let result = compile_program(code);
        // 'b' is a real leak — never passed to anything, never freed, never returned
        assert!(result.is_err(),
            "Actual leak (b never used/freed/passed) should still be detected");
        let err = result.unwrap_err();
        assert!(err.contains("malloc:b"),
            "Error should identify 'malloc:b' as leaked, got: {}", err);
    }
}
