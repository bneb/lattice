// Smoke tests for the module loader
use saltc::compile;

#[test]
fn test_compile_standalone() {
    let code = "package standalone; pub fn main() -> i32 { return 1; }";
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "standalone module should compile: {:?}", result.err());
}

#[test]
fn test_compile_lib_mode() {
    let code = "package libmode; pub fn add(a: i32, b: i32) -> i32 { return a + b; }";
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "lib-mode module should compile: {:?}", result.err());
}
