// =============================================================================
// TDD Test Suite: Slice<T> — Safe Fat Pointer View
//
// Tests the Slice<T> type compiles correctly through the pipeline:
//   - Construction via Slice::new(ptr, len)
//   - Read via .at(index) with requires clause
//   - Write via .set(index, val) with requires clause
//   - Sub-slicing via .sub(start, end)
//   - Interop via .as_ptr(), .len()
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

const SLICE_IMPORTS: &str = r#"
        use std.core.ptr;
        use std.core.slice;
        use std.core.arena;
"#;

// =============================================================================
// Test 1: Basic Slice construction and .len()
// =============================================================================
#[test]
fn test_slice_construction() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let arena = Arena::new(4096);
            let ptr = arena.alloc_array::<i64>(10);
            let s = Slice::<i64>::new(ptr, 10);
            let n = s.len();
        }}
    "#, SLICE_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Slice construction failed: {:?}", res.err());
}

// =============================================================================
// Test 2: Slice.at() — verified read
// =============================================================================
#[test]
fn test_slice_at() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let arena = Arena::new(4096);
            let ptr = arena.alloc_array::<i64>(10);
            let s = Slice::<i64>::new(ptr, 10);
            let val = s.at(5);
        }}
    "#, SLICE_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Slice.at() failed: {:?}", res.err());
}

// =============================================================================
// Test 3: Slice.set() — verified write
// =============================================================================
#[test]
fn test_slice_set() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let arena = Arena::new(4096);
            let ptr = arena.alloc_array::<i64>(10);
            let s = Slice::<i64>::new(ptr, 10);
            s.set(3, 42);
        }}
    "#, SLICE_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Slice.set() failed: {:?}", res.err());
}

// =============================================================================
// Test 4: Slice.sub() — sub-slicing
// =============================================================================
#[test]
fn test_slice_sub() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let arena = Arena::new(4096);
            let ptr = arena.alloc_array::<i64>(100);
            let s = Slice::<i64>::new(ptr, 100);
            let window = s.sub(10, 50);
            let n = window.len();
        }}
    "#, SLICE_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Slice.sub() failed: {:?}", res.err());
}

// =============================================================================
// Test 5: Slice with f32 (MNIST-like usage)
// =============================================================================
#[test]
fn test_slice_f32_mnist_pattern() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let arena = Arena::new(65536);
            let ptr = arena.alloc_array::<f32>(784);
            let weights = Slice::<f32>::new(ptr, 784);
            weights.set(0, 0.5f32);
            let v = weights.at(0);
        }}
    "#, SLICE_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Slice f32 MNIST pattern failed: {:?}", res.err());
}

// =============================================================================
// Test 6: Slice in function parameters
// =============================================================================
#[test]
fn test_slice_as_param() {
    let src = format!(r#"
        package main;
        {}

        fn sum_slice(s: Slice<i64>, n: i64) -> i64 {{
            let mut total: i64 = 0;
            for i in 0..n {{
                total = total + s.at(i);
            }}
            return total;
        }}

        fn main() {{
            let arena = Arena::new(4096);
            let ptr = arena.alloc_array::<i64>(5);
            let s = Slice::<i64>::new(ptr, 5);
            let result = sum_slice(s, 5);
        }}
    "#, SLICE_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Slice as function param failed: {:?}", res.err());
}

// =============================================================================
// Test 7: Slice.offset() — pointer arithmetic with bounds
// =============================================================================
#[test]
fn test_slice_offset() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let arena = Arena::new(4096);
            let ptr = arena.alloc_array::<f32>(100);
            let s = Slice::<f32>::new(ptr, 100);
            let rest = s.offset(10);
            let n = rest.len();
        }}
    "#, SLICE_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Slice.offset() failed: {:?}", res.err());
}

// =============================================================================
// Test 8: Slice.as_ptr() — escape hatch for interop
// =============================================================================
#[test]
fn test_slice_as_ptr() {
    let src = format!(r#"
        package main;
        {}
        fn main() {{
            let arena = Arena::new(4096);
            let ptr = arena.alloc_array::<i64>(10);
            let s = Slice::<i64>::new(ptr, 10);
            let raw = s.as_ptr();
        }}
    "#, SLICE_IMPORTS);
    let res = compile_salt(&src);
    assert!(res.is_ok(), "Slice.as_ptr() failed: {:?}", res.err());
}
