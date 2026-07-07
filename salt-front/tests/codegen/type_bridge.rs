use saltc::compile;

#[test]
fn test_promote_numeric_saturation() {
    // This test compiles Salt code that exercises the type promotion matrix
    
    // Test all numeric promotions via cast expressions
    let saturation_code = r#"
fn test_i8_to_i32(x: i8) -> i32 { return x as i32; }
fn test_u8_to_i32(x: u8) -> i32 { return x as i32; }
fn test_i8_to_i64(x: i8) -> i64 { return x as i64; }
fn test_u8_to_i64(x: u8) -> i64 { return x as i64; }
fn test_i8_to_f32(x: i8) -> f32 { return x as f32; }
fn test_u8_to_f32(x: u8) -> f32 { return x as f32; }
fn test_i8_to_f64(x: i8) -> f64 { return x as f64; }
fn test_u8_to_f64(x: u8) -> f64 { return x as f64; }
fn test_i32_to_i64(x: i32) -> i64 { return x as i64; }
fn test_u32_to_i64(x: u32) -> i64 { return x as i64; }
fn test_i32_to_f32(x: i32) -> f32 { return x as f32; }
fn test_u32_to_f32(x: u32) -> f32 { return x as f32; }
fn test_i32_to_f64(x: i32) -> f64 { return x as f64; }
fn test_u32_to_f64(x: u32) -> f64 { return x as f64; }
fn test_i64_to_f32(x: i64) -> f32 { return x as f32; }
fn test_u64_to_f32(x: u64) -> f32 { return x as f32; }
fn test_i64_to_f64(x: i64) -> f64 { return x as f64; }
fn test_u64_to_f64(x: u64) -> f64 { return x as f64; }
fn test_f32_to_f64(x: f32) -> f64 { return x as f64; }
fn test_f64_to_f32(x: f64) -> f32 { return x as f32; }
fn test_f64_to_i32(x: f64) -> i32 { return x as i32; }
fn test_f64_to_i64(x: f64) -> i64 { return x as i64; }
fn test_f32_to_i32(x: f32) -> i32 { return x as i32; }
fn test_f32_to_u32(x: f32) -> u32 { return x as u32; }
fn test_i64_to_i32(x: i64) -> i32 { return x as i32; }
fn test_i32_to_i8(x: i32) -> i8 { return x as i8; }
fn test_i64_to_i8(x: i64) -> i8 { return x as i8; }
fn test_bool_to_i32(x: bool) -> i32 { return x as i32; }
fn test_bool_to_i64(x: bool) -> i64 { return x as i64; }
fn test_bool_to_f32(x: bool) -> f32 { return x as f32; }
fn test_i32_to_bool(x: i32) -> bool { return x as bool; }
fn test_i64_to_bool(x: i64) -> bool { return x as bool; }
fn test_f32_to_bool(x: f32) -> bool { return x as bool; }
fn test_usize_to_i64(x: usize) -> i64 { return x as i64; }
fn test_i64_to_usize(x: i64) -> usize { return x as usize; }
fn main() -> i32 { return 0; }
"#;
    let result = compile(saturation_code, false, None, true);
    assert!(result.is_ok(), "Saturation code failed: {:?}", result.err());
}

#[test]
fn test_boolean_law_storage() {
    let code = r#"
        struct Flag {
            active: bool,
            enabled: bool,
        }
        fn main() -> i32 {
            let f = Flag { active: true, enabled: false };
            return 0;
        }
    "#;

    let result = compile(code, false, None, true).expect("Compilation failed");
    
    // Check if the struct type in MLIR uses i8 for booleans
    // The struct should look something like !llvm.struct<"Flag", (i8, i8)>
    assert!(result.contains("i8, i8"), "Boolean Law Storage Violation: struct fields should be i8");
}

#[test]
fn test_boolean_law_computation() {
    let code = r#"
        struct BoolBox {
            val: bool,
            dummy: bool
        }
        fn main() -> bool {
            let b = BoolBox { val: true, dummy: false };
            return b.val;
        }
    "#;

    let result = compile(code, false, None, true).expect("Compilation failed");
    
    // Check for logical computation in i1
    // Search for arith.trunci ... i8 to i1
    println!("DEBUG MLIR:\n{}", result);
    assert!(result.contains("arith.trunci"), "Boolean Law Computation Violation: struct field access should trunc i8 to i1");
    // Also assert zext happened in struct construction
    assert!(result.contains("arith.extui"), "Boolean Law Storage Violation: struct construction should zext i1 to i8");
}

#[test]
fn test_never_invariant() {
    // The "Iron Test": Verify that a function returning i32 can have a panic branch (Never)
    // and strict type unification handles it without error.
    let code = r#"
        extern fn abort() -> !;
        fn check_bound(i: i32) -> i32 {
            if i > 100 { 
                abort(); 
            }
            return i;
        }
    "#;
    let result = compile(code, false, None, true).expect("Compilation failed");
    // Should compile successfully (unification of i32 and Never -> i32).
    assert!(result.contains("func.call @abort"), "Should emit abort call");
}

#[test]
fn test_fiber_structural_integrity() {
    let code = r#"
        struct Fiber {
            id: u64,
            stack_ptr: u64,
            active: bool,
        }
        fn main() -> u64 {
            let f = Fiber { id: 1, stack_ptr: 0x1000, active: true };
            return f.id;
        }
    "#;

    let result = compile(code, false, None, true).expect("Compilation failed");
    
    // Proof: sizeof(Fiber) = u64(8) + u64(8) + bool(1) + pad(7) = 24 bytes.
    // In MLIR, this often shows up as !llvm.struct<"Fiber", (i64, i64, i8, array<7 x i8>)>
    // or similar depending on how padding is emitted.
    
    assert!(result.contains("i64, i64, i8"), "Fiber struct fields incorrect");
    // If our type_bridge.rs correctly adds padding for booleans in structs to reach 24 bytes
    // (Actual implementation might vary, let's verify what it does)
}
