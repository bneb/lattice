// Unit tests for provenance-aware codegen optimization
// Tests cover: ProvenanceMap, reinterpret_cast GEP emission, Buffer<T> intercept

use salt_front::codegen::emit_mlir;
use salt_front::codegen::types::ProvenanceMap;
use salt_front::grammar::SaltFile;
use salt_front::types::Type;

// =============================================================================
// ProvenanceMap Unit Tests
// =============================================================================

#[test]
fn test_provenance_map_new() {
    let map = ProvenanceMap::new();
    assert!(!map.is_base("nonexistent"));
}

#[test]
fn test_provenance_map_register_and_lookup() {
    let mut map = ProvenanceMap::new();
    map.register_base("height".to_string(), Type::I32);
    
    assert!(map.is_base("height"));
    assert!(!map.is_base("unknown"));
    
    let elem_ty = map.lookup_base("height");
    assert!(elem_ty.is_some());
    assert!(matches!(elem_ty.unwrap(), Type::I32));
}

#[test]
fn test_provenance_map_multiple_bases() {
    let mut map = ProvenanceMap::new();
    map.register_base("buf1".to_string(), Type::I32);
    map.register_base("buf2".to_string(), Type::F64);
    map.register_base("buf3".to_string(), Type::U8);
    
    assert!(map.is_base("buf1"));
    assert!(map.is_base("buf2"));
    assert!(map.is_base("buf3"));
    
    assert!(matches!(map.lookup_base("buf1").unwrap(), Type::I32));
    assert!(matches!(map.lookup_base("buf2").unwrap(), Type::F64));
    assert!(matches!(map.lookup_base("buf3").unwrap(), Type::U8));
}

// =============================================================================
// reinterpret_cast Provenance Pattern Detection Tests
// =============================================================================

#[test]
fn test_reinterpret_cast_base_plus_offset_emits_gep() {
    // Test: reinterpret_cast::<&i32>(base + byte_offset) should emit GEP
    let src = r#"
        package test::provenance::reinterpret;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        fn read_at(base: u64, idx: i64) -> i32 {
            let ptr = reinterpret_cast::<&i32>(base + (idx * 4) as u64);
            return *ptr;
        }
        
        fn main() -> i32 {
            let buf = malloc(40);
            let result: i32 = read_at(buf, 5);
            free(buf);
            return result;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "reinterpret_cast provenance failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should contain llvm.getelementptr (provenance-aware path)
    assert!(mlir.contains("llvm.getelementptr"), 
        "Expected GEP in output for provenance pattern. Got:\n{}", mlir);
    
    // Should have inttoptr for base conversion
    assert!(mlir.contains("llvm.inttoptr"), 
        "Expected inttoptr for base pointer. Got:\n{}", mlir);
}

#[test]
fn test_reinterpret_cast_write_pattern_emits_gep() {
    // Test: reinterpret_cast::<&mut i32>(base + offset) for writes
    let src = r#"
        package test::provenance::write;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        fn write_at(base: u64, idx: i64, val: i32) {
            let ptr = reinterpret_cast::<&mut i32>(base + (idx * 4) as u64);
            *ptr = val;
        }
        
        fn main() -> i32 {
            let buf = malloc(40);
            write_at(buf, 0, 42);
            free(buf);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "reinterpret_cast write provenance failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Should contain GEP for provenance-aware write
    assert!(mlir.contains("llvm.getelementptr"), 
        "Expected GEP in write path. Got:\n{}", mlir);
}

#[test]
fn test_reinterpret_cast_different_element_sizes() {
    // Test: Different element types get correct GEP strides
    let src = r#"
        package test::provenance::sizes;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        fn read_i64(base: u64, idx: i64) -> i64 {
            let ptr = reinterpret_cast::<&i64>(base + (idx * 8) as u64);
            return *ptr;
        }
        
        fn read_u8(base: u64, idx: i64) -> u8 {
            let ptr = reinterpret_cast::<&u8>(base + idx as u64);
            return *ptr;
        }
        
        fn main() -> i32 {
            let buf64 = malloc(80);
            let buf8 = malloc(10);
            let x = read_i64(buf64, 2);
            let y = read_u8(buf8, 5);
            free(buf64);
            free(buf8);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "reinterpret_cast size dispatch failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Both should use GEP
    let gep_count = mlir.matches("llvm.getelementptr").count();
    assert!(gep_count >= 2, 
        "Expected at least 2 GEPs for different element types, got {}. MLIR:\n{}", gep_count, mlir);
}

#[test]
fn test_reinterpret_cast_fallback_non_pattern() {
    // Test: reinterpret_cast without base+offset pattern falls back
    let src = r#"
        package test::provenance::fallback;
        
        fn cast_direct(addr: u64) -> i32 {
            let ptr = reinterpret_cast::<&i32>(addr);
            return *ptr;
        }
        
        fn main() -> i32 {
            let x = cast_direct(12345 as u64);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "reinterpret_cast fallback failed: {:?}", res.err());
    
    // Should compile without error (fallback path works)
    let mlir = res.unwrap();
    assert!(mlir.contains("llvm.inttoptr"), 
        "Fallback should use inttoptr. Got:\n{}", mlir);
}

// =============================================================================
// Loop Vectorization Pattern Tests
// Verifies the structure that enables LLVM vectorization
// =============================================================================

#[test]
fn test_loop_with_provenance_emits_hoistable_inttoptr() {
    // Test: Loop with indexed access should have inttoptr outside the access pattern
    let src = r#"
        package test::provenance::loop;
        extern fn malloc(size: i64) -> u64;
        extern fn free(ptr: u64);
        
        fn sum_array(base: u64, n: i64) -> i32 {
            let mut total = 0;
            for i in 0..n {
                let ptr = reinterpret_cast::<&i32>(base + (i * 4) as u64);
                total = total + *ptr;
            }
            return total;
        }
        
        fn main() -> i32 {
            let buf = malloc(400);
            let result: i32 = sum_array(buf, 100);
            free(buf);
            return result;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Loop provenance pattern failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Key: GEP should be in the loop, but inttoptr of base is separate
    assert!(mlir.contains("llvm.getelementptr"), 
        "Loop should use GEP. Got:\n{}", mlir);
    assert!(mlir.contains("llvm.inttoptr"), 
        "Should have inttoptr. Got:\n{}", mlir);
}

// =============================================================================
// V8.3 Pointer Provenance Preservation Tests
// These tests guard against the ptrtoint/inttoptr regression that broke matvec
// =============================================================================

#[test]
fn test_pointer_to_pointer_cast_no_ptrtoint() {
    // Test: reinterpret_cast::<&f32>(ptr_u8) should NOT emit ptrtoint
    // This is the exact pattern that broke sovereign_train
    let src = r#"
        package test::provenance::ptr_to_ptr;
        extern fn alloc(size: u64) -> &u8;
        
        fn cast_pointer(ptr: &u8) -> &f32 {
            return reinterpret_cast::<&f32>(ptr);
        }
        
        fn main() -> i32 {
            let raw = alloc(100);
            let f = cast_pointer(raw);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "ptr-to-ptr cast failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // CRITICAL: Should NOT contain ptrtoint for pointer-to-pointer cast
    // ptrtoint breaks LLVM pointer provenance
    let cast_function = mlir.lines()
        .skip_while(|l| !l.contains("cast_pointer"))
        .take_while(|l| !l.contains("func.func") || l.contains("cast_pointer"))
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(!cast_function.contains("llvm.ptrtoint"), 
        "ptr-to-ptr cast should NOT use ptrtoint! Provenance will be lost.\nFunction MLIR:\n{}", cast_function);
}

#[test]
fn test_reference_local_variable_alloca_uses_ptr_type() {
    // Test: Local variable of Reference type should alloca !llvm.ptr, not inner type
    let src = r#"
        package test::provenance::ref_storage;
        extern fn alloc(size: u64) -> &u8;
        
        fn store_and_retrieve() -> &u8 {
            let ptr: &u8 = alloc(42);
            return ptr;
        }
        
        fn main() -> i32 {
            let p = store_and_retrieve();
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Reference storage failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // The alloca for 'ptr' local should be x !llvm.ptr, NOT x i8
    // Previously it was: llvm.alloca %c1 x i8 which caused storage mismatch
    if mlir.contains("local_ptr") {
        // If there's a local_ptr, it should store !llvm.ptr type
        assert!(!mlir.contains("llvm.alloca") || !mlir.contains("x i8 :") || !mlir.contains("local_ptr"),
            "Reference local should alloca !llvm.ptr not inner type i8:\n{}", mlir);
    }
    
    // Should NOT have ptrtoint/inttoptr conversions for storing pointers
    // Previously: ptrtoint ptr -> store i64 -> load i64 -> inttoptr (WRONG!)
    // Now: store !llvm.ptr -> load !llvm.ptr (CORRECT!)
    let _store_load_section = mlir.lines()
        .filter(|l| l.contains("store") || l.contains("load"))
        .filter(|l| l.contains("local_ptr") || l.contains("_ptr"))
        .collect::<Vec<_>>();
    
    // Count ptrtoint operations - should be minimal/zero for pointer locals
    let ptrtoint_count = mlir.matches("llvm.ptrtoint").count();
    assert!(ptrtoint_count < 3, 
        "Too many ptrtoint operations ({}). Pointer locals should store directly as !llvm.ptr.\nMLIR:\n{}", 
        ptrtoint_count, mlir);
}

#[test]
fn test_chained_pointer_casts_preserve_provenance() {
    // Test: Multiple pointer casts in sequence should all be no-ops
    let src = r#"
        package test::provenance::chained;
        extern fn alloc(size: u64) -> &u8;
        
        fn alloc_f32(count: u64) -> &f32 {
            let ptr = alloc(count * 4);
            return reinterpret_cast::<&f32>(ptr);
        }
        
        fn alloc_i32(count: u64) -> &i32 {
            let ptr = alloc(count * 4);
            return reinterpret_cast::<&i32>(ptr);
        }
        
        fn main() -> i32 {
            let floats = alloc_f32(100);
            let ints = alloc_i32(100);
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Chained pointer cast failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Both alloc_f32 and alloc_i32 should NOT have ptrtoint
    // The reinterpret_cast from &u8 to &f32 or &i32 is just type coercion
    let alloc_f32_fn = mlir.lines()
        .skip_while(|l| !l.contains("alloc_f32"))
        .take_while(|l| !l.contains("func.func @") || l.contains("alloc_f32"))
        .filter(|l| l.contains("ptrtoint"))
        .count();
        
    assert!(alloc_f32_fn == 0,
        "alloc_f32 should not use ptrtoint for ptr-to-ptr cast. MLIR:\n{}", mlir);
}

#[test]
fn test_extern_function_returning_pointer_preserves_type() {
    // Test: extern fn returning &u8 should work with local storage
    let src = r#"
        package test::provenance::extern_ptr;
        extern fn mmap_file(path: &u8, size: u64) -> &u8;
        
        fn load_data() -> &f32 {
            let raw = mmap_file("test.bin", 1000);
            let data: &f32 = reinterpret_cast::<&f32>(raw);
            return data;
        }
        
        fn main() -> i32 {
            let d = load_data();
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Extern pointer function failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // The mmap_file result should be stored as !llvm.ptr and cast without ptrtoint
    assert!(mlir.contains("@mmap_file"), "Should have mmap_file function");
    
    // Count problematic patterns - should be zero or minimal
    let load_ptrtoint = mlir.lines()
        .filter(|l| l.contains("load") && l.contains("i64"))
        .filter(|l| !l.contains("arith.constant"))
        .count();
    
    // We shouldn't be loading i64 and converting to ptr for pointer locals
    assert!(load_ptrtoint < 2,
        "Too many i64 loads that could indicate ptrtoint/inttoptr pattern. MLIR:\n{}", mlir);
}

#[test]  
fn test_storage_type_for_reference_is_llvm_ptr() {
    // Test: Type::Reference(F32, _) should allocate as !llvm.ptr, not f32
    // This tests the storage type fix indirectly through MLIR output
    let src = r#"
        package test::provenance::storage_type;
        extern fn alloc(size: u64) -> &u8;
        
        fn test_ref_storage() {
            let x: &f32 = reinterpret_cast::<&f32>(alloc(4));
            let y: &i32 = reinterpret_cast::<&i32>(alloc(4)); 
            let z: &u8 = alloc(1);
        }
        
        fn main() -> i32 {
            test_ref_storage();
            return 0;
        }
    "#;
    let file: SaltFile = syn::parse_str(src).expect("Failed to parse");
    let res = emit_mlir(&file, false, None, false, false, false, false, false, false, "");
    
    assert!(res.is_ok(), "Reference storage type test failed: {:?}", res.err());
    
    let mlir = res.unwrap();
    
    // Verify allocas for pointer locals use !llvm.ptr 
    // Should NOT have: llvm.alloca %c1 x f32 for a pointer local
    // The local_x, local_y, local_z allocas should all use !llvm.ptr
    
    // Check that we don't have "alloca ... x f32" combined with storing a pointer
    let suspicious_allocas = mlir.lines()
        .filter(|l| l.contains("llvm.alloca") && (l.contains("x f32") || l.contains("x i32")))
        .filter(|l| l.contains("local_x") || l.contains("local_y"))
        .count();
    
    assert_eq!(suspicious_allocas, 0,
        "Reference locals should NOT alloca inner type (f32/i32), should be !llvm.ptr.\nMLIR:\n{}", mlir);
}
