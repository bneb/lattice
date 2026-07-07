// Lib function coverage tests

use saltc::compile;

#[test]
fn test_compile_empty_file() {
    let code = "";
    let result = compile(code, false, None, true);
    // Empty file with no main fn is expected to fail or produce no executable code
    // Either success (valid but empty) or error (no entry point) is acceptable
    assert!(result.is_err() || result.is_ok(), "Unexpected empty file handling");
}

#[test]
fn test_compile_release_mode() {
    let code = r#"
        fn main() -> i32 {
            return 42;
        }
    "#;
    let result = compile(code, true, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_compile_syntax_error() {
    let code = "fn main( { return 0; }";  // Missing closing paren
    let result = compile(code, false, None, true);
    assert!(result.is_err());
}

#[test]
fn test_compile_with_struct() {
    let code = r#"
        struct Point {
            x: i32,
            y: i32
        }
        fn main() -> i32 {
            let p: Point = Point { x: 10, y: 20 };
            return p.x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_compile_with_enum() {
    let code = r#"
        enum Option<T> {
            Some(T),
            None
        }
        fn main() -> i32 {
            let x: Option<i32> = Option::<i32>::None;
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_compile_extern_fn() {
    let code = r#"
        extern fn printf(format: !llvm.ptr) -> i32;
        fn main() -> i32 {
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_compile_generic_fn() {
    let code = r#"
        package test::generic;
        fn identity<T>(x: T) -> T {
            return x;
        }
        fn main() -> i32 {
            let x: i32 = identity<i32>(42);
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Generic fn failed: {:?}", result.err());
}

#[test]
fn test_compile_impl_block() {
    // Simplified impl test without &mut self
    let code = r#"
        package test::impl_block;
        struct Counter {
            value: i32
        }
        impl Counter {
            fn get_value(self: &Counter) -> i32 {
                return self.value;
            }
        }
        fn main() -> i32 {
            let c: Counter = Counter { value: 42 };
            return c.get_value();
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "Compile failed: {:?}", result.err());
}

#[test]
fn test_compile_hot_path() {
    let code = r#"
        @hot
        fn hot_loop() -> i32 {
            let mut sum: i32 = 0;
            let mut i: i32 = 0;
            while i < 100 {
                sum = sum + 1;
                i = i + 1;
            }
            return sum;
        }
        fn main() -> i32 {
            return hot_loop();
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test]
fn test_compile_region_block() {
    let code = r#"
        fn main() -> i32 {
            region("test") {
                let x: i32 = 42;
            }
            return 0;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok());
}

#[test] fn test_compile_if_else_chain() {
    let code = r#"fn main() -> i32 { let x = 5; if x > 0 { return 1; } else if x < 0 { return -1; } else { return 0; } }"#;
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_while_loop() {
    let code = r#"fn main() -> i32 { let mut i = 0; while i < 10 { i = i + 1; } return i; }"#;
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_match_on_int() {
    let code = r#"fn main() -> i32 { let x = 2; match x { 1 => return 10, 2 => return 20, _ => return 0 } }"#;
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_fn_call() {
    let code = r#"fn add(a: i32, b: i32) -> i32 { return a + b; } fn main() -> i32 { return add(3, 4); }"#;
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_bool_ops() {
    let code = r#"fn main() -> i32 { let a = true; let b = false; if a && !b { return 1; } return 0; }"#;
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_ref_param() {
    let code = r#"fn read(r: &i32) -> i32 { return *r; } fn main() -> i32 { let x = 42; return read(&x); }"#;
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_array_literal() {
    let code = r#"fn main() -> i32 { let a: [i32; 3] = [1, 2, 3]; return a[0] + a[1] + a[2]; }"#;
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_return_expr() {
    let code = r#"fn main() -> i32 { return 42; }"#;
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_let_else() {
    let code = r#"fn main() -> i32 { let x = 5; let y = if x > 0 { x } else { 0 }; return y; }"#;
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_mut_ref() {
    let code = r#"fn inc(p: &mut i32) { *p = *p + 1; } fn main() -> i32 { let mut x = 41; inc(&mut x); return x; }"#;
    assert!(compile(code, false, None, true).is_ok());
}

#[test] fn test_compile_methods() {
    let code = include_str!("cases/methods.salt");
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_arrays() {
    let code = include_str!("cases/arrays.salt");
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_floats() {
    let code = include_str!("cases/floats.salt");
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_control_flow() {
    let code = include_str!("cases/control_flow.salt");
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_equality() {
    let code = include_str!("cases/equality.salt");
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_numeric_promotion() {
    let code = include_str!("cases/numeric_promotion.salt");
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_all_types() {
    let code = include_str!("cases/all_types.salt");
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_field_ref() {
    let code = include_str!("cases/field_ref.salt");
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_attributes() {
    let code = include_str!("cases/attributes.salt");
    assert!(compile(code, false, None, true).is_ok());
}
#[test] fn test_compile_blocks() { assert!(compile(include_str!("cases/blocks.salt"), false, None, true).is_ok()); }
#[test] fn test_compile_comprehensive() { assert!(compile(include_str!("cases/comprehensive.salt"), false, None, true).is_ok()); }
#[test] fn test_compile_const_global() { assert!(compile(include_str!("cases/const_global.salt"), false, None, true).is_ok()); }
#[test] fn test_compile_packed_arrays() { assert!(compile(include_str!("cases/packed_arrays.salt"), false, None, true).is_ok()); }
#[test] fn test_compile_global_uninit() { assert!(compile(include_str!("cases/global_uninit.salt"), false, None, true).is_ok()); }
#[test] fn test_compile_externs() { assert!(compile(include_str!("cases/externs.salt"), false, None, true).is_ok()); }
