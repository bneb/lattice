// Comptime pass coverage tests

use saltc::compile;

#[test]
fn test_comptime_basic() {
    let code = r#"
        fn main() -> i32 {
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_comptime_const() {
    let code = r#"
        const X: i32 = 10;
        fn main() -> i32 {
            return X;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_comptime_global() {
    let code = r#"
        global G: i32 = 42;
        fn main() -> i32 {
            return G;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_comptime_const_expr() {
    let code = r#"
        const A: i32 = 5 + 3;
        const B: i32 = A * 2;
        fn main() -> i32 {
            return B;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_comptime_hex_const() {
    let code = r#"
        const MASK: i32 = 0xFF00;
        fn main() -> i32 {
            return MASK;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_comptime_binary_const() {
    let code = r#"
        const FLAGS: i32 = 0b1010;
        fn main() -> i32 {
            return FLAGS;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_comptime_bool_const() {
    let code = r#"
        const ENABLED: bool = true;
        fn main() -> i32 {
            if ENABLED {
                return 1;
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_comptime_const_in_array() {
    let code = r#"
        const SIZE: i32 = 10;
        fn main() -> i32 {
            let arr: [i32; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
            return arr[0];
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_complex_arithmetic() {
    let code = r#"
        const A: i32 = 10;
        const B: i32 = (A + 5) * 2 - 3;
        const C: i32 = B / 2;
        fn main() -> i32 {
            return C;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}
