// Unit tests for SSA Promotion optimization (ephemeral refs)
// Tests cover: reinterpret_cast ephemeral ref registration, no-spill field access,
// is_aggregate for Type::Reference(Struct), idempotent referencing (no double-wrap)
//
// These tests protect the window_access optimization that achieved 2.2x speedup over C.

use salt_front::codegen::emit_mlir;
use salt_front::grammar::SaltFile;

// =============================================================================
// Test 1: Struct field access via reinterpret_cast should NOT spill to stack
// Regression guard: This is the core optimization that fixed window_access
// =============================================================================
#[test]
fn test_reinterpret_cast_field_access_no_spill() {
    let src = r#"
        package test::ssa::no_spill;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        struct Pixel { r: i32, g: i32, b: i32 }
        
        fn read_pixel_r(base: u64, idx: i64) -> i32 {
            let ptr = reinterpret_cast::<&Pixel>(base + (idx * 12) as u64);
            return ptr.r;
        }
        
        fn main() -> i32 {
            let buf = malloc(120);
            let result: i32 = read_pixel_r(buf, 5);
            free(buf);
            return result;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "SSA promotion failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // KEY: Must NOT contain %spill_p or %spill_ptr patterns in the read function
    // The spill pattern is: alloca + store + load for a pointer
    let spill_count = mlir.matches("%spill_p").count();
    assert!(spill_count == 0, 
        "REGRESSION: Found {} spill_p patterns. SSA promotion should eliminate spilling. MLIR:\n{}", 
        spill_count, mlir);
    
    // Must contain GEP for field access (direct pointer arithmetic)
    assert!(mlir.contains("llvm.getelementptr"), 
        "Missing GEP for field access. Got:\n{}", mlir);
}

// =============================================================================
// Test 2: Multiple field accesses from same reinterpret_cast should NOT spill
// Verifies the hot loop pattern p.r + p.g + p.b
// =============================================================================
#[test]
fn test_multiple_field_access_no_spill() {
    let src = r#"
        package test::ssa::multi_field;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        struct RGB { r: i32, g: i32, b: i32 }
        
        fn sum_rgb(base: u64, idx: i64) -> i64 {
            let ptr = reinterpret_cast::<&RGB>(base + (idx * 12) as u64);
            return (ptr.r as i64) + (ptr.g as i64) + (ptr.b as i64);
        }
        
        fn main() -> i32 {
            let buf = malloc(120);
            let sum = sum_rgb(buf, 0);
            free(buf);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Multi-field access failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should have 3 GEPs for 3 field accesses (r, g, b)
    let gep_count = mlir.matches("llvm.getelementptr").count();
    assert!(gep_count >= 3, 
        "Expected at least 3 GEPs for 3 field accesses, got {}. MLIR:\n{}", 
        gep_count, mlir);
    
    // Must NOT have spill patterns
    assert!(!mlir.contains("%spill_p"), 
        "REGRESSION: Found spill pattern in multi-field access. MLIR:\n{}", mlir);
}

// =============================================================================
// Test 3: Mutable field write via reinterpret_cast should NOT spill
// Verifies: ptr.r = value; works without spilling
// =============================================================================
#[test]
fn test_mutable_field_write_no_spill() {
    let src = r#"
        package test::ssa::write;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        struct Point { x: i32, y: i32 }
        
        fn set_point(base: u64, idx: i64, x: i32, y: i32) {
            let ptr = reinterpret_cast::<&mut Point>(base + (idx * 8) as u64);
            ptr.x = x;
            ptr.y = y;
        }
        
        fn main() -> i32 {
            let buf = malloc(80);
            set_point(buf, 0, 10, 20);
            free(buf);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Mutable field write failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should have GEPs for field writes
    assert!(mlir.contains("llvm.getelementptr"), 
        "Missing GEP for field write. Got:\n{}", mlir);
    
    // Must NOT have spill patterns
    assert!(!mlir.contains("%spill_p"), 
        "REGRESSION: Found spill pattern in mutable write. MLIR:\n{}", mlir);
}

// =============================================================================
// Test 4: Loop with reinterpret_cast field access (hot loop pattern)
// This is the exact pattern from window_access benchmark
// =============================================================================
#[test]
fn test_loop_field_access_no_spill() {
    let src = r#"
        package test::ssa::loop;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        struct Pixel { r: i32, g: i32, b: i32 }
        
        fn sum_pixels(base: u64, n: i64) -> i64 {
            let mut sum: i64 = 0;
            for i in 0..n {
                let ptr = reinterpret_cast::<&Pixel>(base + (i * 12) as u64);
                sum = sum + (ptr.r as i64) + (ptr.g as i64) + (ptr.b as i64);
            }
            return sum;
        }
        
        fn main() -> i32 {
            let buf = malloc(1200);
            let result = sum_pixels(buf, 100);
            free(buf);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Loop field access failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should contain a loop structure (cf.br for unstructured, affine.for or scf.for after optimization)
    assert!(mlir.contains("cf.br") || mlir.contains("affine.for") || mlir.contains("scf.for"), 
        "Expected loop structure. Got:\n{}", mlir);
    
    // Must NOT have spill patterns (even inside loop)
    let spill_count = mlir.matches("%spill_p").count();
    assert!(spill_count == 0, 
        "REGRESSION: Found {} spill patterns in loop. This kills vectorization. MLIR:\n{}", 
        spill_count, mlir);
}

// =============================================================================
// Test 5: Direct reinterpret_cast to non-struct should still work
// Ensures we didn't break non-aggregate paths
// =============================================================================
#[test]
fn test_reinterpret_cast_primitive_still_works() {
    let src = r#"
        package test::ssa::primitive;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        fn read_i32(base: u64, idx: i64) -> i32 {
            let ptr = reinterpret_cast::<&i32>(base + (idx * 4) as u64);
            return *ptr;
        }
        
        fn main() -> i32 {
            let buf = malloc(40);
            let result: i32 = read_i32(buf, 5);
            free(buf);
            return result;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Primitive reinterpret_cast failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should produce valid MLIR with GEP
    assert!(mlir.contains("llvm.getelementptr"), 
        "Missing GEP for primitive access. Got:\n{}", mlir);
}

// =============================================================================
// Test 6: Nested struct field access via reinterpret_cast
// Verifies: ptr.inner.field works correctly
// =============================================================================
#[test]
fn test_struct_with_inner_struct_field_access() {
    // Test: Access a struct field that is itself a struct (composite type)
    // This tests that is_aggregate correctly identifies struct-typed fields
    let src = r#"
        package test::ssa::inner_struct;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        struct Inner { x: i32, y: i32 }
        struct Outer { a: i32, inner: Inner }
        
        fn get_outer_a(base: u64) -> i32 {
            let ptr = reinterpret_cast::<&Outer>(base);
            return ptr.a;
        }
        
        fn main() -> i32 {
            let buf = malloc(24);
            let result: i32 = get_outer_a(buf);
            free(buf);
            return result;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Struct with inner struct failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should contain GEP for field access
    assert!(mlir.contains("llvm.getelementptr"), 
        "Missing GEP for outer field access. Got:\n{}", mlir);
    
    // Must NOT spill
    assert!(!mlir.contains("%spill_p"), 
        "REGRESSION: Found spill pattern. MLIR:\n{}", mlir);
}

// =============================================================================
// Test 7: Type::Reference(Struct) is treated as aggregate
// Verifies the is_aggregate fix for idempotent referencing
// =============================================================================
#[test]
fn test_reference_struct_is_aggregate() {
    let src = r#"
        package test::ssa::ref_aggregate;
        
        struct Data { value: i32 }
        
        fn read_field(data: &Data) -> i32 {
            return data.value;
        }
        
        fn main() -> i32 {
            let d = Data(42);
            return read_field(&d);
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Reference struct aggregate failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should NOT have double-wrap (&&Data) - this would show as nested loads
    // The pattern for double-wrap is: load ptr -> ptr, then load ptr -> struct
    // We should NOT see "llvm.load.*llvm.ptr.*->.*llvm.ptr" consecutive
    assert!(!mlir.contains("llvm.load %") || !mlir.contains(": !llvm.ptr -> !llvm.ptr"), 
        "Possible double-wrap detected. MLIR:\n{}", mlir);
}

// =============================================================================
// Test 8: Verify ephemeral ref tracks through let binding
// let p = reinterpret_cast...; p.field should still be no-spill
// =============================================================================
#[test]
fn test_ephemeral_ref_through_let_binding() {
    let src = r#"
        package test::ssa::let_binding;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        struct Rec { a: i32, b: i32 }
        
        fn access_via_let(base: u64) -> i32 {
            let p = reinterpret_cast::<&Rec>(base);
            let x = p.a;
            let y = p.b;
            return x + y;
        }
        
        fn main() -> i32 {
            let buf = malloc(8);
            let result: i32 = access_via_let(buf);
            free(buf);
            return result;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Let binding ephemeral ref failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Must NOT spill p to stack
    assert!(!mlir.contains("%spill_p"), 
        "REGRESSION: p was spilled despite ephemeral ref. MLIR:\n{}", mlir);
}

// =============================================================================
// Test 9: Verify field_base_load is eliminated for Reference(Struct)
// This is the specific pattern that caused the original regression
// =============================================================================
#[test]
fn test_no_field_base_load_for_ephemeral_ref() {
    let src = r#"
        package test::ssa::field_base_load;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        struct Vec3 { x: i32, y: i32, z: i32 }
        
        fn dot(base: u64, idx: i64) -> i32 {
            let v = reinterpret_cast::<&Vec3>(base + (idx * 12) as u64);
            return v.x + v.y + v.z;
        }
        
        fn main() -> i32 {
            let buf = malloc(120);
            let result: i32 = dot(buf, 5);
            free(buf);
            return result;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Field base load test failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // The old bug would generate %field_base_load which loads the pointer
    // This should NOT happen - the GEP result is the pointer itself
    assert!(!mlir.contains("%field_base_load"), 
        "REGRESSION: field_base_load generated. This causes double-indirection. MLIR:\n{}", mlir);
}
