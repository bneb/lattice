use salt_front::compile;

#[test]
fn test_use_after_free_fails_to_compile() {
    let src = r#"
        package kernel.test;
        extern fn malloc(size: i64) -> Ptr<u8>;
        extern fn free(p: Ptr<u8>);
        fn main() {
            let p = malloc(8);
            free(p);
            unsafe {
                let val = p[0]; // UAF!
            }
        }
    "#;
    
    let mlir_or_err = compile(src, false, None, true);
    
    assert!(mlir_or_err.is_err(), "UAF should fail to compile");
    let err = mlir_or_err.unwrap_err();
    let err_str = err.to_string();
    assert!(err_str.contains("UAF") || err_str.contains("freed") || err_str.contains("Freed"), "Error should mention UAF/Freed, got: {}", err_str);
}

#[test]
fn test_alias_invalidated_across_call() {
    let src = r#"
        package kernel.test;
        extern fn malloc(size: i64) -> Ptr<u8>;
        extern fn unknown(p: Ptr<u8>);
        fn main() {
            let p = malloc(8);
            unknown(p); // Conservative aliasing should mark `p` as Optional
            unsafe {
                let val = p[0]; // Should fail because it's Optional
            }
        }
    "#;
    
    let mlir_or_err = compile(src, false, None, true);
    
    assert!(mlir_or_err.is_err(), "Deref after opaque call should fail to compile");
    let err = mlir_or_err.unwrap_err();
    let err_str = err.to_string();
    assert!(err_str.contains("Optional") || err_str.contains("validity check"), "Error should mention Optional state or validity, got: {}", err_str);
}
#[test]
fn test_interprocedural_validity() {
    let src = r#"
        package kernel.test;
        extern fn malloc(size: i64) -> Ptr<u8>;
        extern fn free(p: Ptr<u8>);

        fn do_something(p: Ptr<u8>) requires valid(p); {
            unsafe {
                let val = p[0];
            }
        }

        fn main() {
            let p = malloc(8);
            do_something(p); // Should succeed
            free(p);
            do_something(p); // Should fail to compile
        }
    "#;
    
    let mlir_or_err = compile(src, false, None, true);
    assert!(mlir_or_err.is_err(), "Expected compilation to fail, but it succeeded with: \n{}", mlir_or_err.unwrap());
    let err_str = mlir_or_err.unwrap_err().to_string();
    assert!(err_str.contains("Precondition violated") || err_str.contains("valid"), "Expected precondition error, got: {}", err_str);
}

#[test]
fn test_dynamic_check_tier3() {
    let src = r#"
        package kernel.test;
        extern fn malloc(size: i64) -> Ptr<u8>;
        
        @dynamic_check
        @no_mangle
        fn do_something(p: Ptr<u8>) {
            unsafe {
                let val = p[0];
            }
        }
        
        fn main() -> i32 {
            let p = malloc(8);
            do_something(p);
            0
        }
    "#;
    
    // It should compile and emit salt_verify_epoch and mask out the tag.
    let mlir_or_err = compile(src, false, None, true);
    assert!(mlir_or_err.is_ok(), "Expected compilation to succeed: {:?}", mlir_or_err);
    
    let mlir = mlir_or_err.unwrap();
    assert!(mlir.contains("llvm.call @salt_verify_epoch"), "Expected salt_verify_epoch call in:\n{}", mlir);
    assert!(mlir.contains("llvm.mlir.constant(281474976710655 : i64)"), "Expected tag masking constant");
    assert!(mlir.contains("llvm.and"), "Expected tag masking operation");
}
