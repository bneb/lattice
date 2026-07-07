use saltc::compile;

#[test]
fn test_cmpxchg_codegen() {
    let code = r#"
    fn test_cas(ptr: &mut i64, expected: i64, new_val: i64) -> i64 {
        let res_tuple: (i64, bool) = cmpxchg(ptr, expected, new_val);
        // We return the old value
        return res_tuple.0;
    }
    
    fn main() -> i32 {
        let mut val: i64 = 100;
        let old: i64 = test_cas(&mut val, 100, 200);
        return 0;
    }
    "#;
    
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "cmpxchg compilation failed: {:?}", result.err());
    
    let mlir = result.unwrap();
    println!("Generated MLIR:\n{}", mlir);
    
    // key instruction: llvm.cmpxchg ... acq_rel acquire
    assert!(mlir.contains("llvm.cmpxchg"), "Missing llvm.cmpxchg instruction");
    assert!(mlir.contains("acq_rel acquire"), "Missing memory ordering acq_rel acquire");
    
    // Check for success flag extraction (tuple packing logic creates insertvalue)
    // We expect the result of cmpxchg to be a struct {i64, i1}
    // And then converted to tuple {i64, i8} (or i1 depending on implementation)
    
    // My implementation:
    // let tuple_mlir_ty = tuple_ty.to_mlir_type(self)?; // !llvm.struct<(i64, i8)> likely
    // self.emit_cast(out, &success_i8, "arith.extui", &success_extracted, "i1", "i8");
    
    assert!(mlir.contains("arith.extui"), "Missing bool cast (i1 -> i8) for tuple");
}
