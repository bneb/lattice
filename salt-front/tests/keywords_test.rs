// Smoke tests for Salt custom keywords: fn, struct, package, requires, ensures.

use saltc::compile;

#[test]
fn test_package_and_fn_keyword() {
    let code = r#"
        package test::basic;
        fn main() -> i32 { return 42; }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "package + fn keywords should parse: {:?}", result.err());
}

#[test]
fn test_struct_keyword() {
    let code = r#"
        package test::structs;
        struct Point { x: i32, y: i32 }
        pub fn main() -> i32 {
            let p = Point { x: 1, y: 2 };
            return p.x + p.y;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "struct keyword should parse: {:?}", result.err());
}

#[test]
fn test_requires_keyword() {
    let code = r#"
        package test::requires_demo;
        fn div(a: i32, b: i32) -> i32
            requires { b != 0 }
        {
            return a / b;
        }
        fn main() -> i32 { return div(10, 2); }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "requires keyword should parse: {:?}", result.err());
}


#[test]
fn test_use_keyword() {
    let code = r#"
        package test::imports;
        use std::core::ptr::*;
        fn main() -> i32 { return 0; }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "use keyword should parse: {:?}", result.err());
}
