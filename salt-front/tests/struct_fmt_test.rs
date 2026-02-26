// =============================================================================
// TDD Test Suite: Structural Formatting via fmt() method
//
// Tests the V5.0 structural formatting feature:
//   - Structs with fmt(&self, f: &mut Formatter) are formatted in f-strings
//   - The __fstring_append_expr! macro dispatches to correct append_* methods
//   - The Formatter chain is generated for struct types with fmt() 
//   - Primitive types still use direct append_* methods (no regression)
//
// All tests run through the full pipeline: preprocess → parse → emit_mlir
//
// NOTE: Tests use `package main;` for correct namespace resolution.
// F-strings require std.string (InterpolatedStringHandler) and its transitive
// deps (std.core.arena, std.core.ptr). Struct fmt tests also need std.core.fmt.
// =============================================================================

use salt_front::preprocess;
use salt_front::codegen::emit_mlir;
use salt_front::grammar::SaltFile;

/// Helper: preprocess Salt source, parse, and emit MLIR
fn compile_salt(src: &str) -> Result<String, String> {
    let preprocessed = preprocess(src);
    let file: SaltFile = syn::parse_str(&preprocessed)
        .map_err(|e| format!("Parse error: {} (preprocessed: {})", e, preprocessed))?;
    emit_mlir(&file, false, None, false, false, false, false, false, false, "")
}

/// Standard imports for f-string tests (InterpolatedStringHandler + deps)
const FSTRING_IMPORTS: &str = r#"
        use std.string;
        use std.core.arena;
        use std.core.ptr;
"#;

/// Standard imports for struct fmt tests (adds Formatter)
const STRUCT_FMT_IMPORTS: &str = r#"
        use std.string;
        use std.core.arena;
        use std.core.ptr;
        use std.core.fmt;
"#;

// =============================================================================
// Test 1: F-string with simple i32 variable — baseline regression test
// =============================================================================
#[test]
fn test_fstring_primitive_i32() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let x: i32 = 42;
            let s = f"Value: {{x}}";
        }}
    "#, FSTRING_IMPORTS);
    let res = compile_salt(&src);
    
    assert!(res.is_ok(), "f-string i32 failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    assert!(mlir.contains("@main"), "Should contain main function");
    
    // Should reference InterpolatedStringHandler (f-string pipeline)
    assert!(mlir.contains("InterpolatedStringHandler"),
        "Should use InterpolatedStringHandler. Got:\n{}", mlir);
}

// =============================================================================
// Test 2: F-string with i64 variable — should use append_i64
// =============================================================================
#[test]
fn test_fstring_primitive_i64() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let x: i64 = 10000;
            let s = f"Count: {{x}}";
        }}
    "#, FSTRING_IMPORTS);
    let res = compile_salt(&src);
    
    assert!(res.is_ok(), "f-string i64 failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    assert!(mlir.contains("append_i64") || mlir.contains("InterpolatedStringHandler"),
        "Should reference append_i64 or InterpolatedStringHandler. Got:\n{}", mlir);
}

// =============================================================================
// Test 3: F-string with f64 variable — should use append_f64
// =============================================================================
#[test]
fn test_fstring_primitive_f64() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let pi: f64 = 3.14;
            let s = f"Pi: {{pi}}";
        }}
    "#, FSTRING_IMPORTS);
    let res = compile_salt(&src);
    
    assert!(res.is_ok(), "f-string f64 failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    assert!(mlir.contains("append_f64") || mlir.contains("InterpolatedStringHandler"),
        "Should reference append_f64 or InterpolatedStringHandler. Got:\n{}", mlir);
}

// =============================================================================
// Test 4: F-string with bool variable — should use append_bool
// =============================================================================
#[test]
fn test_fstring_primitive_bool() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let flag: bool = true;
            let s = f"Active: {{flag}}";
        }}
    "#, FSTRING_IMPORTS);
    let res = compile_salt(&src);
    
    assert!(res.is_ok(), "f-string bool failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    assert!(mlir.contains("append_bool") || mlir.contains("InterpolatedStringHandler"),
        "Should reference append_bool or InterpolatedStringHandler. Got:\n{}", mlir);
}

// =============================================================================
// Test 5: CORE TEST — Struct with fmt() in f-string produces Formatter chain
//         This is the PRIMARY test for structural formatting.
// =============================================================================
#[test]
fn test_fstring_struct_with_fmt() {
    let src = format!(r#"
        package main;
        
        {}
        
        struct Point {{
            x: i64,
            y: i64,
        }}
        
        impl Point {{
            pub fn new(x: i64, y: i64) -> Point {{
                return Point {{ x: x, y: y }};
            }}
            
            pub fn fmt(&self, f: &mut Formatter) {{
                f.write_i64(self.x);
            }}
        }}
        
        fn main() {{
            let p = Point::new(10, 20);
            let s = f"Location: {{p}}";
        }}
    "#, STRUCT_FMT_IMPORTS);
    let res = compile_salt(&src);
    
    assert!(res.is_ok(), "f-string struct with fmt failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    assert!(mlir.contains("@main"), "Should contain main function");
    
    // CORE ASSERTION: Formatter chain should be used for struct with fmt()
    assert!(mlir.contains("Formatter") || mlir.contains("fmt"),
        "Should reference Formatter or fmt for struct with fmt() method. Got:\n{}", mlir);
    
    // append_fmt_result bridges Formatter output to InterpolatedStringHandler
    assert!(mlir.contains("append_fmt_result"),
        "Should call append_fmt_result to bridge Formatter output to Handler. Got:\n{}", mlir);
}

// =============================================================================
// Test 6: Struct WITHOUT fmt() in f-string — should produce a compile error
//         Structs must implement fmt() to be used in f-string interpolation.
// =============================================================================
#[test]
fn test_fstring_struct_without_fmt() {
    let src = format!(r#"
        package main;
        {}
        
        struct Color {{
            r: i32,
            g: i32,
            b: i32,
        }}
        
        fn main() {{
            let c = Color {{ r: 255, g: 128, b: 0 }};
            let s = f"Color: {{c}}";
        }}
    "#, FSTRING_IMPORTS);
    let res = compile_salt(&src);
    
    // Structs without fmt() should produce a compile error in f-strings
    assert!(res.is_err(), "f-string with struct without fmt should produce an error");
    
    let err = res.unwrap_err();
    assert!(err.contains("Color") || err.contains("cast"),
        "Error should reference the struct type. Got: {}", err);
}

// =============================================================================
// Test 7: Mixed f-string with struct (fmt) and primitives
// =============================================================================
#[test]
fn test_fstring_mixed_struct_and_primitives() {
    let src = format!(r#"
        package main;
        
        {}
        
        struct Vec2 {{
            x: i64,
            y: i64,
        }}
        
        impl Vec2 {{
            pub fn fmt(&self, f: &mut Formatter) {{
                f.write_i64(self.x);
            }}
        }}
        
        fn main() {{
            let v = Vec2 {{ x: 5, y: 10 }};
            let score: i64 = 100;
            let s = f"Score: {{score}} at {{v}}";
        }}
    "#, STRUCT_FMT_IMPORTS);
    let res = compile_salt(&src);
    
    assert!(res.is_ok(), "f-string mixed struct/primitive failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should have both append_i64 (for score) and append_fmt_result (for v)
    assert!(mlir.contains("append_i64"),
        "Should use append_i64 for i64 primitive 'score'. Got:\n{}", mlir);
    assert!(mlir.contains("append_fmt_result"),
        "Should use append_fmt_result for struct 'v'. Got:\n{}", mlir);
}

// =============================================================================
// Test 8: Multiple structs with fmt() in same f-string
// =============================================================================
#[test]
fn test_fstring_multiple_structs() {
    let src = format!(r#"
        package main;
        
        {}
        
        struct Pos {{
            x: i64,
            y: i64,
        }}
        
        impl Pos {{
            pub fn fmt(&self, f: &mut Formatter) {{
                f.write_i64(self.x);
            }}
        }}
        
        fn main() {{
            let a = Pos {{ x: 1, y: 2 }};
            let b = Pos {{ x: 3, y: 4 }};
            let s = f"From {{a}} to {{b}}";
        }}
    "#, STRUCT_FMT_IMPORTS);
    let res = compile_salt(&src);
    
    assert!(res.is_ok(), "f-string multiple structs failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should have TWO formatter chains (one per struct)
    let fmt_result_count = mlir.matches("append_fmt_result").count();
    assert!(fmt_result_count >= 2,
        "Expected at least 2 append_fmt_result calls (one per struct), got {}. MLIR:\n{}", 
        fmt_result_count, mlir);
}

// =============================================================================
// Test 9: println with f-string containing struct with fmt()
// =============================================================================
#[test]
fn test_println_fstring_struct_fmt() {
    let src = format!(r#"
        package main;
        
        {}
        
        struct Coord {{
            x: i64,
            y: i64,
        }}
        
        impl Coord {{
            pub fn fmt(&self, f: &mut Formatter) {{
                f.write_i64(self.x);
            }}
        }}
        
        fn main() {{
            let c = Coord {{ x: 10, y: 20 }};
            println(f"Position: {{c}}");
        }}
    "#, STRUCT_FMT_IMPORTS);
    let res = compile_salt(&src);
    
    assert!(res.is_ok(), "println f-string struct fmt failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    assert!(mlir.contains("@main"), "Should contain main function");
    
    // println with f-string uses the streaming path
    assert!(mlir.contains("@__salt_print_literal") || mlir.contains("@__salt_print_i64"),
        "println path should emit print hooks. Got:\n{}", mlir);
}

// =============================================================================
// Test 10: F-string literal-only (no interpolation) — regression guard
// =============================================================================
#[test]
fn test_fstring_literal_only() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let s = f"Hello, World!";
        }}
    "#, FSTRING_IMPORTS);
    let res = compile_salt(&src);
    
    assert!(res.is_ok(), "f-string literal-only failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    assert!(mlir.contains("Hello, World!"),
        "Should contain literal text. Got:\n{}", mlir);
}
