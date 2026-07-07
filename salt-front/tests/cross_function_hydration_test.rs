// Tests for cross-function struct hydration
// Issue: Struct literals inside non-main functions failed to compile because
// the struct wasn't in the registry when the function was hydrated via monomorphization task.

use saltc::compile;

/// Test that struct literals work correctly inside non-main functions
#[test]
fn test_struct_literal_in_helper_function() {
    let code = r#"
        package test::cross_fn;
        struct Point { x: i32, y: i32 }
        fn make_point() -> Point {
            return Point { x: 10, y: 20 };
        }
        fn main() -> i32 {
            let p: Point = make_point();
            return p.x + p.y;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Cross-function struct literal failed: {:?}", result.err());
}

/// Test nested helper function calls with struct literals
#[test]
fn test_nested_struct_literal_helper_functions() {
    let code = r#"
        package test::nested;
        struct Vec3 { x: f64, y: f64, z: f64 }
        fn make_origin() -> Vec3 {
            return Vec3 { x: 0.0, y: 0.0, z: 0.0 };
        }
        fn make_unit_x() -> Vec3 {
            return Vec3 { x: 1.0, y: 0.0, z: 0.0 };
        }
        fn sum_vectors(a: Vec3, b: Vec3) -> Vec3 {
            return Vec3 { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z };
        }
        fn main() -> i32 {
            let origin: Vec3 = make_origin();
            let unit: Vec3 = make_unit_x();
            let sum: Vec3 = sum_vectors(origin, unit);
            return sum.x as i32;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Nested struct helper functions failed: {:?}", result.err());
}

/// Test struct literal with llvm.ptr field (as in window_access benchmark)
#[test]
fn test_struct_literal_with_llvm_ptr() {
    let code = r#"
        package test::ptr_struct;
        extern fn malloc(size: usize) -> !llvm.ptr;
        extern fn free(ptr: Ptr<u8>);
        struct InnerWindow { ptr: !llvm.ptr, len: i64 }
        fn wrap_ptr(p: Ptr<u8>, size: i64) -> InnerWindow {
            return InnerWindow { ptr: p, len: size };
        }
        fn main() -> i32 {
            let p = malloc(8);
            let w: InnerWindow = wrap_ptr(p, 16);
            let result: i32 = w.len as i32;
            free(p);
            return result;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Struct with llvm.ptr field failed: {:?}", result.err());
}

/// Test reinterpret_cast intrinsic works in non-main functions
#[test]
fn test_reinterpret_cast_in_helper_function() {
    let code = r#"
        package test::cast;
        struct Wrapper { value: i64 }
        fn cast_to_i64(w: Wrapper) -> i64 {
            return reinterpret_cast<i64>(w);
        }
        fn main() -> i32 {
            let w: Wrapper = Wrapper { value: 42 };
            let v: i64 = cast_to_i64(w);
            return v as i32;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "reinterpret_cast in helper function failed: {:?}", result.err());
}

/// Test multiple struct operations in the same helper function  
#[test]
fn test_multiple_struct_types_in_helper() {
    let code = r#"
        package test::multi;
        struct Data { x: i32, y: i64, z: f64 }
        fn make_data() -> Data {
            return Data { x: 42, y: 100, z: 1.5 };
        }
        fn main() -> i32 {
            let d: Data = make_data();
            return d.x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Multiple struct fields in helper failed: {:?}", result.err());
}

/// Test struct literal in deeply nested function call chain
#[test]
fn test_deep_call_chain_struct_literal() {
    let code = r#"
        package test::deep;
        struct Data { value: i32 }
        fn level3() -> Data { return Data { value: 3 }; }
        fn level2() -> Data { return level3(); }
        fn level1() -> Data { return level2(); }
        fn main() -> i32 {
            let d: Data = level1();
            return d.value;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Deep call chain struct literal failed: {:?}", result.err());
}
