//! TDD Tests: Nested Struct Access through Ptr<T>
//!
//! When Ptr<Outer> points to a struct where a field is itself a struct (not a Ptr),
//! accessing the inner struct's fields requires a chain of GEPs:
//!   ptr.config.dim  →  GEP(ptr, config_offset) → GEP(config_addr, dim_offset) → load
//!
//! The Basalt workaround was using serialize/deserialize helpers.
//!
//! Written BEFORE implementation (Red Phase).

#[cfg(test)]
mod tests {

    fn compile_program(code: &str) -> Result<String, String> {
        crate::compile(code, false, None, true)
            .map_err(|e| format!("{}", e))
    }

    // =========================================================================
    // RED Test 1: Read field of inline struct through Ptr
    // =========================================================================

    /// ptr.config.dim where config is an inline Config struct (not Ptr<Config>)
    #[test]
    fn test_ptr_nested_struct_field_read() {
        let code = r#"
            package test::nested_read;
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);
            struct Config {
                dim: i32,
                n_layers: i32
            }
            struct Engine {
                config: Config,
                pos: i32
            }
            fn main() -> i32 {
                let e: Ptr<Engine> = malloc(12) as Ptr<Engine>;
                e.pos = 0;
                e.config.dim = 288;
                let r = e.config.dim;
                free(e as Ptr<u8>);
                return r;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Ptr<Engine>.config.dim (nested struct read) should compile, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 2: Write field of inline struct through Ptr
    // =========================================================================

    #[test]
    fn test_ptr_nested_struct_field_write() {
        let code = r#"
            package test::nested_write;
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);
            struct Inner {
                x: i32,
                y: i32
            }
            struct Outer {
                inner: Inner,
                z: i32
            }
            fn main() -> i32 {
                let p: Ptr<Outer> = malloc(12) as Ptr<Outer>;
                p.inner.x = 10;
                p.inner.y = 20;
                p.z = 30;
                let r = p.inner.x;
                free(p as Ptr<u8>);
                return r;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Ptr<Outer>.inner.x = 10 (nested struct write) should compile, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 3: Three levels deep — ptr.a.b.c
    // =========================================================================

    #[test]
    fn test_ptr_triple_nested_struct_field() {
        let code = r#"
            package test::triple_nested;
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);
            struct Position { x: i32 }
            struct Transform { pos: Position }
            struct Entity { transform: Transform, id: i32 }
            fn main() -> i32 {
                let e: Ptr<Entity> = malloc(12) as Ptr<Entity>;
                e.transform.pos.x = 42;
                let r = e.transform.pos.x;
                free(e as Ptr<u8>);
                return r;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Ptr<Entity>.transform.pos.x (3-level nested) should compile, got: {}",
            result.err().unwrap_or_default());
    }

    // =========================================================================
    // RED Test 4: Mixed Ptr and inline struct nesting
    // =========================================================================

    /// ptr.config.dim where ptr->Ptr<Engine>, Engine.config is inline Config
    /// but Engine.state is Ptr<RunState>
    #[test]
    fn test_mixed_ptr_and_inline_nesting() {
        let code = r#"
            package test::mixed;
            extern fn malloc(size: i64) -> Ptr<u8>;
            extern fn free(ptr: Ptr<u8>);
            struct Config { dim: i32 }
            struct RunState { pos: i32 }
            struct Engine {
                config: Config,
                state: Ptr<RunState>
            }
            fn main() -> i32 {
                let e: Ptr<Engine> = malloc(24) as Ptr<Engine>;
                e.config.dim = 576;
                let s: Ptr<RunState> = malloc(4) as Ptr<RunState>;
                e.state = s;
                e.state.pos = 0;
                let r = e.config.dim;
                free(s as Ptr<u8>);
                free(e as Ptr<u8>);
                return r;
            }
        "#;
        let result = compile_program(code);
        assert!(result.is_ok(),
            "Mixed Ptr and inline struct nesting should compile, got: {}",
            result.err().unwrap_or_default());
    }
}
