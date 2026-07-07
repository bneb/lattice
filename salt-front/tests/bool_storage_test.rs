// Tests for bool storage in struct fields
// Issue: Bool fields in structs were emitted as i1 instead of i8, causing
// MLIR type mismatches when storing bool values to struct fields.

use saltc::compile;

/// Test that bool fields in structs are correctly stored as i8
#[test]
fn test_bool_field_assignment() {
    let code = r#"
        package test::bool_field;
        extern fn malloc(size: usize) -> !llvm.ptr;
        extern fn free(ptr: Ptr<u8>);
        struct Node { active: bool }
        fn main() -> i32 {
            let ptr = malloc(8);
            let node = ptr as &mut Node;
            node.active = true;
            if node.active { free(ptr); return 1; }
            free(ptr);
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Bool field assignment failed: {:?}", result.err());
}

/// Test bool field in struct with other fields
#[test]
fn test_bool_with_other_fields() {
    let code = r#"
        package test::mixed;
        extern fn malloc(size: usize) -> !llvm.ptr;
        extern fn free(ptr: Ptr<u8>);
        struct Entry { value: i64, is_valid: bool, count: i32 }
        fn main() -> i32 {
            let ptr = malloc(24);
            let e = ptr as &mut Entry;
            e.value = 42;
            e.is_valid = true;
            e.count = 10;
            if e.is_valid { free(ptr); return e.count; }
            free(ptr);
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Mixed struct with bool failed: {:?}", result.err());
}

/// Test bool field in helper function (cross-hydration)
#[test]
fn test_bool_field_in_helper() {
    let code = r#"
        package test::helper;
        extern fn malloc(size: usize) -> !llvm.ptr;
        extern fn free(ptr: Ptr<u8>);
        struct Flag { enabled: bool }
        fn set_flag(f: &mut Flag, val: bool) {
            f.enabled = val;
        }
        fn main() -> i32 {
            let ptr = malloc(8);
            let f = ptr as &mut Flag;
            set_flag(f, true);
            if f.enabled { free(ptr); return 1; }
            free(ptr);
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Bool field in helper function failed: {:?}", result.err());
}

/// Test array field and bool field in struct (simplified trie-like pattern)
#[test]
fn test_struct_with_ptr_array_and_bool() {
    let code = r#"
        package test::trie_like;
        extern fn malloc(size: usize) -> !llvm.ptr;
        extern fn free(ptr: Ptr<u8>);
        struct TrieNode {
            depth: i32,
            is_word: bool
        }
        fn main() -> i32 {
            let ptr = malloc(16);
            let node = ptr as &mut TrieNode;
            node.depth = 3;
            node.is_word = false;
            node.is_word = true;
            if node.is_word { free(ptr); return node.depth; }
            free(ptr);
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Trie-like struct with bool failed: {:?}", result.err());
}

/// Test native malloc integration
#[test]
fn test_native_malloc() {
    let code = r#"
        package test::malloc;
        extern fn malloc(size: usize) -> !llvm.ptr;
        extern fn free(ptr: Ptr<u8>);
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let ptr = malloc(8);
            let p = ptr as &mut Point;
            p.x = 10;
            p.y = 20;
            free(ptr);
            return p.x + p.y;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Native malloc failed: {:?}", result.err());
}
