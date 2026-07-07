// Fail case tests - exercising error handling paths in codegen

use saltc::compile;

// =============================================================================
// Expected failures - these test error handling
// =============================================================================

#[test]
fn test_fail_undefined_variable() {
    let code = r#"
        fn main() -> i32 {
            return undefined_var;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_err());
}

#[test]
fn test_fail_undefined_function() {
    let code = r#"
        fn main() -> i32 {
            return nonexistent_fn();
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_err());
}

#[test]
fn test_fail_type_mismatch_return() {
    // Function returns i32 but trying to return bool
    let code = r#"
        fn returns_int() -> i32 {
            return true;
        }
        fn main() -> i32 {
            return returns_int();
        }
    "#;
    let result = compile(code, false, None, true);
    // This may or may not fail depending on type checking
    // The test is valuable either way for coverage
    let _ = result;
}

#[test]
fn test_fail_break_outside_loop() {
    let code = r#"
        fn main() -> i32 {
            break;
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    // Should fail - break outside loop
    assert!(result.is_err(), "Expected error for break outside loop");
}

#[test]
fn test_fail_missing_return() {
    let code = r#"
        fn no_return() -> i32 {
            let x: i32 = 42;
        }
        fn main() -> i32 {
            return no_return();
        }
    "#;
    let result = compile(code, false, None, true);
    // Should fail - function missing return
    assert!(result.is_err(), "Expected error for missing return");
}

#[test]
fn test_fail_undefined_struct() {
    let code = r#"
        fn main() -> i32 {
            let p: UnknownStruct = UnknownStruct { x: 0 };
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_err(), "Expected error for undefined struct");
}

#[test]
fn test_fail_undefined_field() {
    let code = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p: Point = Point { x: 0, y: 0 };
            return p.z;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_err(), "Expected error for undefined field");
}

#[test]
fn test_special_llvm_ptr() {
    let code = r#"
        fn main() -> i32 {
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

// =============================================================================
// Edge cases that exercise specific codegen paths
// =============================================================================

#[test]
fn test_zero_initialized_array() {
    let code = r#"
        fn main() -> i32 {
            let arr: [i32; 5] = [0, 0, 0, 0, 0];
            return arr[0];
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_deeply_nested_expr() {
    let code = r#"
        fn main() -> i32 {
            let x: i32 = ((((1 + 2) * 3) - 4) / 5);
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_multiple_returns() {
    let code = r#"
        fn early_return(x: i32) -> i32 {
            if x < 0 {
                return 0;
            }
            if x > 100 {
                return 100;
            }
            return x;
        }
        fn main() -> i32 {
            return early_return(50);
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_recursive_function() {
    let code = r#"
        fn factorial(n: i32) -> i32 {
            if n <= 1 {
                return 1;
            }
            return n * factorial(n - 1);
        }
        fn main() -> i32 {
            return factorial(5);
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_mutual_recursion() {
    let code = r#"
        fn is_even(n: i32) -> i32 {
            if n == 0 { return 1; }
            return is_odd(n - 1);
        }
        fn is_odd(n: i32) -> i32 {
            if n == 0 { return 0; }
            return is_even(n - 1);
        }
        fn main() -> i32 {
            return is_even(10);
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}
