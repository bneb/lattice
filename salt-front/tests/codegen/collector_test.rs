// Smoke tests for the monomorphization collector
use saltc::compile;

#[test]
fn test_compile_simple_function() {
    let code = "package test; pub fn main() -> i32 { return 42; }";
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "simple function should compile: {:?}", result.err());
}

#[test]
fn test_compile_with_generic() {
    let code = "package test; fn id<T>(x: T) -> T { return x; } pub fn main() -> i32 { return id(42); }";
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "generic function should compile: {:?}", result.err());
}
