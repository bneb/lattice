//! Stack Array Tests: `let arr: [T; N]` using LLVM alloca
//!
//! Verifies that fixed-size arrays can be declared on the stack
//! and accessed via [] indexing, avoiding heap allocation entirely.

#[cfg(test)]
mod tests {
    /// Test 1: Basic stack array declaration and write should compile
    #[test]
    fn test_stack_array_basic_write() {
        let code = r#"
            package test::stack_array;
            fn main() -> i32 {
                let arr: [i32; 4];
                arr[0 as i64] = 42;
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Stack array declaration and write should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Test 2: Stack array read-back should produce correct type
    #[test]
    fn test_stack_array_read() {
        let code = r#"
            package test::stack_array_read;
            fn main() -> i32 {
                let arr: [i32; 4];
                arr[0 as i64] = 10;
                let x: i32 = arr[0 as i64];
                return x;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Stack array read should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Test 3: Stack array with loop initialization (trie pattern)
    #[test]
    fn test_stack_array_loop_init() {
        let code = r#"
            package test::stack_array_loop;
            fn main() -> i32 {
                let arr: [u64; 26];
                for i in 0..26 {
                    arr[i as i64] = 0;
                }
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Stack array loop init should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Test 4: Stack array should NOT trigger malloc leak tracking
    /// (no heap allocation involved)
    #[test]
    fn test_stack_array_no_leak_error() {
        let code = r#"
            package test::stack_array_no_leak;
            fn main() -> i32 {
                let arr: [i32; 100];
                arr[0 as i64] = 1;
                arr[99 as i64] = 2;
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Stack array should not trigger leak tracking, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Test 5: Stack array of u8 (sieve pattern)
    #[test]
    fn test_stack_array_u8() {
        let code = r#"
            package test::stack_array_u8;
            fn main() -> i32 {
                let flags: [u8; 256];
                flags[0 as i64] = 1;
                let v: u8 = flags[0 as i64];
                return v as i32;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Stack array of u8 should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }

    /// Test 6: Stack array passed to function (trie pattern)
    /// The array decays to a pointer when passed to a function expecting Ptr<T>
    #[test]
    fn test_stack_array_pass_to_function() {
        let code = r#"
            package test::stack_array_fn;
            use std.core.ptr.Ptr;
            fn fill(buf: Ptr<u8>, len: i64) {
                for i in 0..len {
                    buf[i] = 0;
                }
            }
            fn main() -> i32 {
                let word: [u8; 6];
                fill(word as Ptr<u8>, 6);
                return 0;
            }
        "#;
        let result = crate::compile(code, false, None, true);
        assert!(result.is_ok(),
            "Stack array passed to function should compile, got: {}",
            result.err().map(|e| format!("{}", e)).unwrap_or_default());
    }
}
