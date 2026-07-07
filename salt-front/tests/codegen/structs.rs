use saltc::compile;

#[test]
fn test_nested_struct() {
    let code = r#"
struct Inner { val: i32 }
struct Outer { inner: Inner }
fn test_nested() -> i32 {
    let o = Outer { inner: Inner { val: 42 } };
    return o.inner.val;
}
fn main() -> i32 { return test_nested(); }
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Nested struct failed: {:?}", result.err());
}

#[test]
fn test_array_in_struct() {
    let code = r#"
struct Data { arr: [i32; 3] }
fn test_arr_struct() -> i32 {
    let d = Data { arr: [1, 2, 3] };
    return d.arr[0] + d.arr[2];
}
fn main() -> i32 { return test_arr_struct(); }
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Array in struct failed: {:?}", result.err());
}

#[test]
fn test_generic_struct() {
    let code = r#"
struct Pair<T> { first: T, second: T }
fn test_pair() -> i32 {
    let p = Pair::<i32> { first: 10, second: 20 };
    return p.first + p.second;
}
fn main() -> i32 { return test_pair(); }
"#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Generic struct failed: {:?}", result.err());
}

#[test]
fn test_struct_field_access() {
    let code = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p: Point = Point { x: 10, y: 20 };
            return p.x + p.y;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Struct access failed: {:?}", result.err());
}

#[test]
fn test_struct_field_assign() {
    let code = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let mut p: Point = Point { x: 0, y: 0 };
            p.x = 10;
            p.y = 20;
            return p.x + p.y;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Struct assign failed: {:?}", result.err());
}

#[test]
fn test_struct_equality_check() {
    let code = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p1: Point = Point { x: 10, y: 20 };
            let p2: Point = Point { x: 10, y: 20 };
            if p1 == p2 {
                return 1;
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "struct equality failed: {:?}", result.err());
}

#[test]
fn test_struct_inequality() {
    let code = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p1: Point = Point { x: 10, y: 20 };
            let p2: Point = Point { x: 10, y: 30 };
            if p1 != p2 {
                return 1;
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "struct inequality failed: {:?}", result.err());
}
