// Smoke tests for the entity registry
use saltc::compile;

#[test]
fn test_compile_with_struct() {
    let code = "package test; struct Point { x: i32, y: i32 } pub fn main() -> i32 { let p = Point{x:1,y:2}; return p.x + p.y; }";
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "struct should compile: {:?}", result.err());
}

#[test]
fn test_compile_with_import() {
    let code = "package test; use std.core.str.StringView; pub fn main() -> i32 { return 0; }";
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "import should compile: {:?}", result.err());
}
