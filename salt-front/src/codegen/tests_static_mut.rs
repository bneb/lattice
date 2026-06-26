//! TDD Tests: `static mut` Support (Mutable Global Variables)
//!
//! Salt currently has `global` and `var` keywords for globals, but they are
//! immutable. Basalt needs `static mut` for engine state that persists
//! across WASM function calls without using a flat Ptr<i64> buffer.
//!
//! Written BEFORE implementation (Red Phase).

#[cfg(test)]
mod tests {

    fn compile_program(code: &str) -> Result<String, String> {
        crate::compile(code, false, None, true)
            .map_err(|e| format!("{}", e))
    }

    // =========================================================================
    // RED Test 1: Global variable read/write (simplest case)
    // =========================================================================

    /// A global integer that can be written to and read from.
    #[test]
    fn test_global_var_read_write() {
        let code = r#"
            package test::static_rw;
            var COUNTER: i64 = 0;
            fn increment() -> i64 {
                COUNTER = COUNTER + 1;
                return COUNTER;
            }
            fn main() -> i32 {
                let a = increment();
                let b = increment();
                return (a + b) as i32;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Global var read/write should compile, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 2: Global variable as a Ptr<T> (engine state pattern)
    // =========================================================================

    /// The Basalt pattern: a global Ptr that holds engine state.
    #[test]
    fn test_global_ptr_state() {
        let code = r#"
            package test::static_ptr;
            extern fn malloc(size: i64) -> Ptr<u8>;
            var ENGINE_STATE: Ptr<u8> = 0 as Ptr<u8>;
            fn init() -> i32 {
                ENGINE_STATE = malloc(1024);
                return 0;
            }
            fn main() -> i32 {
                return init();
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Global Ptr<u8> should compile and be assignable, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 3: Global struct (config pattern)
    // =========================================================================

    /// A global struct variable — the ideal replacement for flat buffer state.
    #[test]
    fn test_global_struct_variable() {
        let code = r#"
            package test::static_struct;
            struct Config {
                dim: i32,
                n_layers: i32
            }
            var CONFIG: Config;
            fn init() -> i32 {
                CONFIG.dim = 288;
                CONFIG.n_layers = 6;
                return 0;
            }
            fn main() -> i32 {
                init();
                return CONFIG.dim;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Global struct with field assignment should compile, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 4: Multiple globals in different functions
    // =========================================================================

    #[test]
    fn test_multiple_globals_cross_function() {
        let code = r#"
            package test::multi_global;
            var POS: i64 = 0;
            var TOKEN: i64 = 0;
            fn advance(tok: i64) -> i64 {
                TOKEN = tok;
                POS = POS + 1;
                return POS;
            }
            fn main() -> i32 {
                let p = advance(42);
                return p as i32;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Multiple globals written from functions should compile, got: {}",
            result.err().unwrap_or_default());
    }
}
