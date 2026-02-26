// =============================================================================
// TDD Test Suite: String write operations — reinterpret_cast → Ptr<T> indexing
//
// Verifies all migrated byte-write patterns compile correctly:
//   - write_i32_unchecked (zero, positive, negative)
//   - write_i64_unchecked (zero, positive, negative)
//   - write_str_unchecked (memcpy path)
//   - Writer::write_i32 / write_i64 (with reserve)
//   - write_str (bulk copy with reserve)
//   - append_literal_unchecked (f-string path)
//   - append_i32_unchecked (f-string int formatting)
//   - data[index] = val (direct Ptr L-value)
//
// All tests run through: preprocess → parse → emit_mlir
// =============================================================================

use salt_front::preprocess;
use salt_front::codegen::emit_mlir;
use salt_front::grammar::SaltFile;

fn compile_salt(src: &str) -> Result<String, String> {
    let preprocessed = preprocess(src);
    let file: SaltFile = syn::parse_str(&preprocessed)
        .map_err(|e| format!("Parse error: {} (preprocessed: {})", e, preprocessed))?;
    emit_mlir(&file, false, None, false, false, false, false, false, false, "")
}

const STRING_IMPORTS: &str = r#"
        use std.core.ptr;
        use std.string;
"#;

// =============================================================================
// Test 1: String::with_capacity + push_byte — baseline functionality
// =============================================================================
#[test]
fn test_string_push_byte() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let mut s = String::with_capacity(64);
            s.push_byte(72);  // 'H'
            s.push_byte(105); // 'i'
        }}
    "#, STRING_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "String push_byte failed: {:?}", res.err());
}

// =============================================================================
// Test 2: write_i32_unchecked — exercises self.data[self.len] = val
// =============================================================================
#[test]
fn test_string_write_i32_unchecked() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let mut s = String::with_capacity(64);
            // Tests zero path: self.data[self.len] = 48
            s.write_i32_unchecked(0);
            // Tests positive path: write_ptr[offset] = digit
            s.write_i32_unchecked(42);
            // Tests negative path: write_ptr[offset] = 45 (minus sign)
            s.write_i32_unchecked(-7);
        }}
    "#, STRING_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "write_i32_unchecked failed: {:?}", res.err());
}

// =============================================================================
// Test 3: write_i64_unchecked — exercises self.data[self.len] = val
// =============================================================================
#[test]
fn test_string_write_i64_unchecked() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let mut s = String::with_capacity(64);
            s.write_i64_unchecked(0);
            s.write_i64_unchecked(123456789);
            s.write_i64_unchecked(-42);
        }}
    "#, STRING_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "write_i64_unchecked failed: {:?}", res.err());
}

// =============================================================================
// Test 4: write_str_unchecked — exercises Ptr.offset() + memcpy
// =============================================================================
#[test]
fn test_string_write_str_unchecked() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let mut s = String::with_capacity(64);
            s.write_str_unchecked("hello", 5);
        }}
    "#, STRING_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "write_str_unchecked failed: {:?}", res.err());
}

// =============================================================================
// Test 5: Writer trait write_i32 — exercises reserve + write_ptr[offset]
// =============================================================================
#[test]
fn test_string_writer_write_i32() {
    let src = format!(r#"
        package main;
        {}
        use std.io.writer;
        fn main() {{
            let mut s = String::with_capacity(64);
            s.write_i32(0);
            s.write_i32(999);
            s.write_i32(-123);
        }}
    "#, STRING_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Writer::write_i32 failed: {:?}", res.err());
}

// =============================================================================
// Test 6: Writer trait write_i64 — exercises reserve + write_ptr[offset]
// =============================================================================
#[test]
fn test_string_writer_write_i64() {
    let src = format!(r#"
        package main;
        {}
        use std.io.writer;
        fn main() {{
            let mut s = String::with_capacity(64);
            s.write_i64(0);
            s.write_i64(9999999999);
            s.write_i64(-1);
        }}
    "#, STRING_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Writer::write_i64 failed: {:?}", res.err());
}

// =============================================================================
// Test 7: write_str (bulk copy with reserve)
// =============================================================================
#[test]
fn test_string_write_str() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let mut s = String::with_capacity(16);
            s.write_str("hello world", 11);
        }}
    "#, STRING_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "write_str failed: {:?}", res.err());
}

// =============================================================================
// Test 8: f-string operations (append_literal_unchecked + append_i32_unchecked)
// =============================================================================
#[test]
fn test_string_fstring_append_ops() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let mut s = String::with_capacity(64);
            // Exercises both write_str_unchecked + write_i32_unchecked paths
            s.write_str_unchecked("Item ", 5);
            s.write_i32_unchecked(42);
            s.write_i64_unchecked(-99);
        }}
    "#, STRING_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "f-string append ops failed: {:?}", res.err());
}

// =============================================================================
// Test 9: Direct Ptr<T> L-value indexing (the core pattern we're verifying)
// =============================================================================
#[test]
fn test_ptr_lvalue_indexing() {
    let src = format!(r#"
        package main;
        {}
        use std.core.arena;
        fn main() {{
            let arena = Arena::new(4096);
            let p = arena.alloc_array::<u8>(32);
            // Direct L-value write via ptr[i] = val
            p[0] = 72;    // 'H'
            p[1] = 101;   // 'e'
            p[2] = 108;   // 'l'
            // Read back via ptr[i]
            let h = p[0];
        }}
    "#, STRING_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Ptr L-value indexing failed: {:?}", res.err());
}
