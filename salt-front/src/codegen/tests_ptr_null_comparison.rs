//! TDD Tests: Ptr<T> to i64 Promotion in Comparisons
//!
//! Salt should support `(ptr as i64) == 0` for null checks.
//! Currently, `promote_numeric` in type_bridge.rs falls through to error
//! for Pointer → I64, even though `cast_numeric` handles it correctly.
//!
//! Written BEFORE implementation (Red Phase).

#[cfg(test)]
mod tests {

    fn compile_program(code: &str) -> Result<String, String> {
        crate::compile(code, false, None, true)
            .map_err(|e| format!("{}", e))
    }

    // =========================================================================
    // RED Test 1: Ptr<u8> as i64 == 0  (null check pattern)
    // =========================================================================

    /// The exact pattern from Basalt's ingest_prompt:
    ///   if (tokens_ptr as i64) == 0 { return; }
    /// This currently fails with "Numeric promotion not supported".
    #[test]
    fn test_ptr_as_i64_eq_zero_compiles() {
        let code = r#"
            package test::ptr_cmp;
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);
            fn check_null(p: Ptr<u8>) -> i32 {
                if (p as i64) == 0 {
                    return -1;
                }
                return 0;
            }
            fn main() -> i32 {
                let buf = malloc(8);
                let r = check_null(buf);
                free(buf);
                return r;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Ptr<u8> as i64 == 0 should compile (null check pattern), got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 2: Ptr<i64> as i64 == 0  (typed pointer null check)
    // =========================================================================

    #[test]
    fn test_typed_ptr_as_i64_eq_zero_compiles() {
        let code = r#"
            package test::typed_ptr_cmp;
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);
            fn is_null(p: Ptr<u8>) -> i64 {
                if (p as i64) != 0 {
                    return 1;
                }
                return 0;
            }
            fn main() -> i32 {
                let buf = malloc(16);
                let r = is_null(buf);
                free(buf);
                return r as i32;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Ptr<u8> as i64 != 0 should compile, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 3: MLIR output contains ptrtoint for ptr-to-i64 cast
    // =========================================================================

    #[test]
    fn test_ptr_cast_emits_ptrtoint() {
        let code = r#"
            package test::ptrtoint;
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);
            fn addr(p: Ptr<u8>) -> i64 {
                return p as i64;
            }
            fn main() -> i32 {
                let buf = malloc(8);
                let a = addr(buf);
                free(buf);
                return 0;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Ptr<u8> as i64 return should compile, got: {}",
            result.err().unwrap_or_default());
        let mlir = result.unwrap();
        assert!(mlir.contains("llvm.ptrtoint"),
            "MLIR should contain llvm.ptrtoint for Ptr→i64 cast");
    }
}
