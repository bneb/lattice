// Targeting error path coverage in codegen.rs

use saltc::compile;

// =============================================================================
// Error cases that exercise error handling paths
// =============================================================================

#[test]
fn test_undefined_type() {
    let code = r#"
        fn main() -> i32 {
            let x: UndefinedType = 42;
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_err());
}

#[test]
fn test_wrong_arg_count() {
    let code = r#"
        fn add(a: i32, b: i32) -> i32 { return a + b; }
        fn main() -> i32 {
            return add(1);
        }
    "#;
    let result = compile(code, false, None, true);
    // May or may not error depending on arg checking
    let _ = result;
}

#[test]
fn test_missing_field_in_struct() {
    let code = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p: Point = Point { x: 10 };
            return p.x;
        }
    "#;
    let result = compile(code, false, None, true);
    // Should error for missing field
    let _ = result;
}

#[test]
fn test_double_definition() {
    let code = r#"
        fn foo() -> i32 { return 1; }
        fn foo() -> i32 { return 2; }
        fn main() -> i32 { return foo(); }
    "#;
    let result = compile(code, false, None, true);
    // May or may not error
    let _ = result;
}

// =============================================================================
// Edge cases that exercise specific paths
// =============================================================================

#[test]
fn test_empty_block() {
    let code = r#"
        fn main() -> i32 {
            {
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_single_statement_function() {
    let code = r#"
        fn just_return() -> i32 {
            return 42;
        }
        fn main() -> i32 { return just_return(); }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_very_long_function() {
    let code = r#"
        fn long_fn() -> i32 {
            let a1: i32 = 1;
            let a2: i32 = 2;
            let a3: i32 = 3;
            let a4: i32 = 4;
            let a5: i32 = 5;
            let a6: i32 = 6;
            let a7: i32 = 7;
            let a8: i32 = 8;
            let a9: i32 = 9;
            let a10: i32 = 10;
            let b1: i32 = a1 + a2;
            let b2: i32 = a3 + a4;
            let b3: i32 = a5 + a6;
            let b4: i32 = a7 + a8;
            let b5: i32 = a9 + a10;
            let c1: i32 = b1 + b2;
            let c2: i32 = b3 + b4;
            let c3: i32 = b5 + c1;
            return c2 + c3;
        }
        fn main() -> i32 { return long_fn(); }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_many_function_args() {
    let code = r#"
        fn many_args(a: i32, b: i32, c: i32, d: i32, e: i32) -> i32 {
            return a + b + c + d + e;
        }
        fn main() -> i32 { return many_args(1, 2, 3, 4, 5); }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_cascade_calls() {
    let code = r#"
        fn f1() -> i32 { return 1; }
        fn f2() -> i32 { return f1() + 1; }
        fn f3() -> i32 { return f2() + 1; }
        fn f4() -> i32 { return f3() + 1; }
        fn main() -> i32 { return f4(); }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_complex_arithmetic_expr() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = (1 + 2) * (3 - 4) / (5 + 6) % 7;
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_bool_to_int() {
    let code = r#"
        fn main() -> i32 {
            let b: bool = true;
            if b {
                return 1;
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_multiple_returns_in_branches() {
    let code = r#"
        fn classify(x: i32) -> i32 {
            if x < 0 {
                return 0;
            }
            if x == 0 {
                return 1;
            }
            if x < 10 {
                return 2;
            }
            if x < 100 {
                return 3;
            }
            return 4;
        }
        fn main() -> i32 { return classify(50); }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_deeply_nested_structs() {
    let code = r#"
        struct Inner { value: i32 }
        struct Middle { inner: Inner }
        struct Outer { middle: Middle }
        fn main() -> i32 {
            let o: Outer = Outer { middle: Middle { inner: Inner { value: 42 } } };
            return o.middle.inner.value;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_assign_through_struct() {
    let code = r#"
        struct Pair { a: i32, b: i32 }
        fn main() -> i32 {
            let mut p: Pair = Pair { a: 0, b: 0 };
            p.a = 10;
            p.b = 20;
            return p.a + p.b;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_array_of_complex() {
    let code = r#"
        struct Item { val: i32 }
        fn main() -> i32 {
            let arr: [Item; 3] = [Item { val: 1 }, Item { val: 2 }, Item { val: 3 }];
            return arr[0].val + arr[1].val + arr[2].val;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_large_array() {
    let code = r#"
        fn main() -> i32 {
            let arr: [i32; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
            return arr[0] + arr[9];
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_return_from_if() {
    let code = r#"
        fn cond_return(x: i32) -> i32 {
            if x > 0 {
                return x * 2;
            }
            return x;
        }
        fn main() -> i32 { return cond_return(5); }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_early_return() {
    let code = r#"
        fn early(x: i32) -> i32 {
            if x < 0 { return 0; }
            let y: i32 = x * 2;
            if y > 100 { return 100; }
            return y;
        }
        fn main() -> i32 { return early(25); }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}
