use salt_front::codegen::compile;

#[test]
fn test_interprocedural_validity() {
    let src = r#"
        package kernel.test;
        extern fn malloc(size: i64) -> Ptr<u8>;
        extern fn free(p: Ptr<u8>);

        @requires valid(p)
        fn do_something(p: Ptr<u8>) {
            let val = p[0];
        }

        fn main() {
            let p = malloc(8);
            do_something(p); // Should succeed
            free(p);
            do_something(p); // Should fail to compile
        }
    "#;
    
    let mlir_or_err = compile(src, false, None, true, false);
    assert!(mlir_or_err.is_err());
    let err_str = mlir_or_err.unwrap_err().to_string();
    assert!(err_str.contains("Precondition violated") || err_str.contains("valid"), "Expected precondition error, got: {}", err_str);
}
