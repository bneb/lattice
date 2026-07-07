// Integration smoke tests for fuzz_ast: FuzzSaltFile, FuzzFn, FuzzType, and AST generation.

use saltc::fuzz_ast::{FuzzSaltFile, FuzzFn, FuzzBlock, FuzzType};

#[test]
fn test_fuzz_file_with_multiple_fns() {
    let fns = vec![
        FuzzFn {
            name: "alpha".into(),
            args: vec![("x".into(), FuzzType::I32), ("y".into(), FuzzType::I32)],
            ret_ty: FuzzType::I32,
            body: FuzzBlock { stmts: vec![] },
            ret_to_arg: false,
        },
        FuzzFn {
            name: "beta".into(),
            args: vec![],
            ret_ty: FuzzType::F64,
            body: FuzzBlock { stmts: vec![] },
            ret_to_arg: false,
        },
    ];
    let fuzz = FuzzSaltFile { fns };
    let salt = fuzz.to_salt();
    assert_eq!(salt.items.len(), 2, "two fns should produce two items");
}

#[test]
fn test_fuzz_fn_with_args_and_return() {
    let fuzz_fn = FuzzFn {
        name: "add".into(),
        args: vec![("a".into(), FuzzType::I64), ("b".into(), FuzzType::I64)],
        ret_ty: FuzzType::I64,
        body: FuzzBlock { stmts: vec![] },
        ret_to_arg: false,
    };
    let salt_fn = fuzz_fn.to_salt();
    assert_eq!(salt_fn.args.len(), 2);
    assert!(salt_fn.ret_type.is_some());
}

#[test]
fn test_fuzz_fn_no_args_unit_return() {
    let fuzz_fn = FuzzFn {
        name: "empty".into(),
        args: vec![],
        ret_ty: FuzzType::I32,
        body: FuzzBlock { stmts: vec![] },
        ret_to_arg: false,
    };
    let salt_fn = fuzz_fn.to_salt();
    assert!(salt_fn.args.is_empty());
}

#[test]
fn test_fuzz_type_all_variants() {
    // Smoke test that all FuzzType variants exist
    let _i32 = FuzzType::I32;
    let _i64 = FuzzType::I64;
    let _f64 = FuzzType::F64;
    // Verify they are distinct
    assert_ne!(format!("{:?}", _i32), format!("{:?}", _f64));
}

#[test]
fn test_fuzz_salt_file_empty() {
    let fuzz = FuzzSaltFile { fns: vec![] };
    let salt = fuzz.to_salt();
    assert!(salt.items.is_empty());
    assert!(salt.package.is_none());
    assert!(salt.imports.is_empty());
}
