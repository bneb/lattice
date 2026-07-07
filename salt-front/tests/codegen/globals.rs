use saltc::compile;

#[test]
fn test_global_read_write() {
    let code = r#"
        global COUNTER: i32 = 0;
        fn main() -> i32 {
            COUNTER = COUNTER + 1;
            return COUNTER;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "global rw failed: {:?}", result.err());
}

#[test]
fn test_const_in_expression() {
    let code = r#"
        const MAX: i32 = 100;
        fn main() -> i32 {
            let x: i32 = MAX / 2;
            return x;
        }
    "#;
    let result = compile(code, false, None, true);
    assert!(result.is_ok(), "const expr failed: {:?}", result.err());
}
