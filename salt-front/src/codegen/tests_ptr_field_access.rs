//! Tests: Ptr<T> L-Value Field Access
//!
//! Tests that `ptr.field = val` works correctly through Ptr<T> auto-dereference.
//! The emit_lvalue path must handle Type::Pointer the same way it handles
//! Type::Reference: load the pointer value, then GEP into the struct field.

#[cfg(test)]
mod tests {
    /// Helper: Compile a Salt program through the full pipeline.
    /// Returns Ok(mlir_string) or Err(error_message).
    fn compile_salt_program(source: &str) -> Result<String, String> {
        let processed = crate::preprocess(source);
        let mut file: crate::grammar::SaltFile = syn::parse_str(&processed)
            .map_err(|e| format!("Parse error: {}", e))?;

        // Build registry with std imports
        let mut registry = crate::registry::Registry::new();
        registry.register(crate::registry::ModuleInfo::new("main"));
        crate::cli::load_imports(&file, &mut registry);

        crate::compile_ast(&mut file, true, Some(&registry), false, false, false, false, false, false, false, "<test>")
            .map_err(|e| format!("{}", e))
    }

    // =========================================================================
    // Test 1: Basic Ptr<T> field WRITE (L-value)
    // =========================================================================

    #[test]
    fn test_ptr_field_write_basic() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            extern fn malloc(size: i64) -> u64;

            struct Point { x: i32, y: i32 }

            fn main() -> u64 {
                let p: Ptr<Point> = malloc(8) as Ptr<Point>;
                p.x = 10;
                p.y = 20;
                return p.addr();
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Ptr<Point>.x = 10 should compile (L-value field write through Ptr<T>). Error: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 2: Ptr<T> field READ (R-value)
    // =========================================================================

    #[test]
    fn test_ptr_field_read_basic() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            extern fn malloc(size: i64) -> u64;

            struct Point { x: i32, y: i32 }

            fn main() -> u64 {
                let p: Ptr<Point> = malloc(8) as Ptr<Point>;
                let _val = p.x;
                return p.addr();
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Ptr<Point>.x read should compile. Error: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 3: Chained Ptr<T> field access (ptr.next.val = 42)
    // =========================================================================

    #[test]
    fn test_ptr_field_chained_write() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            extern fn malloc(size: i64) -> u64;

            struct Node {
                val: i32,
                next: Ptr<Node>
            }

            fn make_node() -> Ptr<Node> {
                let n: Ptr<Node> = malloc(16) as Ptr<Node>;
                return n;
            }

            fn main() -> i32 {
                let a = make_node();
                let b = make_node();
                a.next = b;
                a.next.val = 42;
                return 0;
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Chained Ptr<Node> field write (a.next.val = 42) should compile. Error: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 4: Ptr<T> with compound assignment (ptr.count += 1)
    // =========================================================================

    #[test]
    fn test_ptr_field_compound_assign() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            extern fn malloc(size: i64) -> u64;

            struct Counter { count: i32 }

            fn main() -> u64 {
                let c: Ptr<Counter> = malloc(4) as Ptr<Counter>;
                c.count = 0;
                c.count += 1;
                c.count += 1;
                return c.addr();
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Ptr<Counter>.count += 1 should compile. Error: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 5: Doubly-linked list pattern (node.prev.next = node.next)
    // =========================================================================

    #[test]
    fn test_ptr_field_linked_list_pattern() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            extern fn malloc(size: i64) -> u64;

            struct LRUNode {
                key: i32,
                prev: Ptr<LRUNode>,
                next: Ptr<LRUNode>
            }

            fn remove_node(node: Ptr<LRUNode>) {
                node.prev.next = node.next;
                node.next.prev = node.prev;
            }

            fn main() -> i32 {
                return 0;
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "node.prev.next = node.next should compile (linked list pattern). Error: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 6: Pointer truthiness with Ptr<T>
    // =========================================================================

    #[test]
    fn test_ptr_truthiness_in_condition() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            extern fn malloc(size: i64) -> u64;


            struct Node { val: i32, next: Ptr<Node> }

            fn main() -> u64 {
                let n: Ptr<Node> = malloc(16) as Ptr<Node>;
                n.next = Ptr::<Node>::empty();
                if n {
                    return n.addr();
                }
                return n.addr();
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Pointer truthiness (if n {{ ... }}) should compile. Error: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 7: Ptr-to-Ptr cast (as Ptr<T>)
    // =========================================================================

    #[test]
    fn test_ptr_to_ptr_cast() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            extern fn malloc(size: i64) -> u64;

            struct Point { x: i32, y: i32 }
            struct Vec2 { a: i32, b: i32 }

            fn main() -> u64 {
                let p: Ptr<Point> = malloc(8) as Ptr<Point>;
                p.x = 42;
                let v: Ptr<Vec2> = p as Ptr<Vec2>;
                return v.addr();
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Ptr<A> as Ptr<B> should compile (opaque pointer no-op). Error: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 8: &ptr.field (field borrowing → Ptr<FieldType>)
    // =========================================================================

    #[test]
    fn test_addr_of_ptr_field() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            extern fn malloc(size: i64) -> u64;

            struct Point { x: i32, y: i32 }

            fn main() -> u64 {
                let p: Ptr<Point> = malloc(8) as Ptr<Point>;
                p.x = 99;
                let xp: Ptr<i32> = &p.x;
                return p.addr();
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "&ptr.field should compile and return Ptr<FieldType>. Error: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 9: Ptr<T> index compound assignment (ptr[i] += expr)
    // =========================================================================

    #[test]
    fn test_ptr_index_compound_assign() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            extern fn malloc(size: i64) -> u64;

            fn main() -> u64 {
                let p: Ptr<f32> = malloc(40) as Ptr<f32>;
                p[0] = 1.0f32;
                p[0] += 2.0f32;
                p[3] -= 0.5f32;
                p[7] *= 3.0f32;
                p[1] /= 2.0f32;
                return p.addr();
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Ptr<f32>[i] += expr should compile (compound assign on indexed ptr). Error: {:?}",
            result.err());
    }

    // =========================================================================
    // Test 10: Slice<T> index compound assignment (slice[i] -= expr)
    // =========================================================================

    #[test]
    fn test_slice_index_compound_assign() {
        let source = r#"
            package main
            use std.core.ptr.Ptr
            use std.core.slice.Slice
            extern fn malloc(size: i64) -> u64;

            fn main() -> u64 {
                let p: Ptr<f32> = malloc(40) as Ptr<f32>;
                let s = Slice::<f32>::new(p, 10);
                s[0] = 1.0f32;
                s[0] += 2.0f32;
                s[3] -= 0.5f32;
                return p.addr();
            }
        "#;

        let result = compile_salt_program(source);
        assert!(result.is_ok(),
            "Slice<f32>[i] += expr should compile (compound assign on indexed slice). Error: {:?}",
            result.err());
    }
}
