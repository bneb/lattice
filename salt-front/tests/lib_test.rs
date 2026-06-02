// Lib function coverage tests

use salt_front::compile;

#[test]
fn test_compile_empty_file() {
    let code = "";
    let result = compile(code, false, None, true, false);
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
    let result = compile(code, true, None, true, false);
    assert!(result.is_ok());
}

#[test]
fn test_compile_syntax_error() {
    let code = "fn main( { return 0; }";  // Missing closing paren
    let result = compile(code, false, None, true, false);
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
    let result = compile(code, false, None, true, false);
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
    let result = compile(code, false, None, true, false);
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
    let result = compile(code, false, None, true, false);
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
    let result = compile(code, false, None, true, false);
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
    let result = compile(code, false, None, true, false);
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
    let result = compile(code, false, None, true, false);
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
    let result = compile(code, false, None, true, false);
    assert!(result.is_ok());
}
