// Canonical Type Resolution Tests
// Tests that struct literal tracing resolves FQNs via the Symbol Table,
// not via string-matching heuristics.

use saltc::compile;

/// TDD Test: Box::new(StructLiteral) must resolve to main__StructName
/// without explicit type annotations.
/// This covers the class of issues where Node != main__Node.
#[test]
fn test_canonical_struct_resolution() {
    // This is the content of test_box_infer.salt — 3 struct types with Box::new
    let code = r#"
        package main

        use std.core.ptr.Ptr
        use std.core.boxed.Box

        extern fn printf_shim(fmt: &u8, val: i64);

        struct Simple { val: i32 }

        struct Node {
            val: i32,
            left: Ptr<Node>,
            right: Ptr<Node>
        }

        struct ListNode {
            data: i64,
            next: Ptr<ListNode>
        }

        fn main() -> i32 {
            // Case 1: Simple struct literal
            let b1 = Box::new(Simple { val: 10 });
            let s1 = b1.read();
            printf_shim("simple: %lld\n", s1.val as i64);
            b1.drop();

            // Case 2: Struct with Ptr<T> fields (recursive)
            let b2 = Box::new(Node {
                val: 42,
                left: Ptr<Node>::empty(),
                right: Ptr<Node>::empty()
            });
            let n = b2.read();
            printf_shim("node: %lld\n", n.val as i64);
            b2.drop();

            // Case 3: Different struct type in same function
            let b3 = Box::new(ListNode {
                data: 99,
                next: Ptr<ListNode>::empty()
            });
            let ln = b3.read();
            printf_shim("list: %lld\n", ln.data);
            b3.drop();

            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Canonical struct resolution failed: {:?}", result.err());
}

/// Test: Struct literal in a non-main package must also canonicalize correctly.
#[test]
fn test_canonical_struct_resolution_custom_package() {
    let code = r#"
        package mylib

        struct Point {
            x: i32,
            y: i32
        }

        fn make_point() -> Point {
            return Point { x: 10, y: 20 };
        }

        fn main() -> i32 {
            let p = make_point();
            return p.x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Custom package struct resolution failed: {:?}", result.err());
}
