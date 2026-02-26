use salt_front::codegen::context::CodegenContext;
use salt_front::types::{Type, TypeKey};
use salt_front::codegen::context::LocalKind;
use salt_front::codegen::expr::{emit_expr, emit_path};
use salt_front::codegen::stmt::{emit_stmt, emit_block};
use salt_front::codegen::type_bridge::{resolve_type, resolve_codegen_type};
use std::collections::{BTreeMap, HashMap};
use salt_front::registry::StructInfo;
use salt_front::grammar::SaltFile;
use std::cell::RefCell;
use std::rc::Rc;

macro_rules! with_ctx {
    ($ctx:ident, $code:expr) => {
        let file = salt_front::grammar::SaltFile {
            package: None,
            imports: vec![],
            items: vec![],
        };
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let $ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        $code
    };
}

#[test]
fn test_saturation_attack_salt_file() {
    let torture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/torture/coverage_torture.salt");
    let code = std::fs::read_to_string(torture_path).expect("Failed to read coverage_torture.salt");
    
    let result = salt_front::compile(&code, false, None, true, false);
    match result {
        Ok(mlir) => {
            // Salt-opt verification if available
            let root_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            let salt_opt = root_dir.join("salt/build/salt-opt");
            if salt_opt.exists() {
                 use std::process::{Command, Stdio};
                 use std::io::Write;
                 let mut child = Command::new(&salt_opt)
                     .stdin(Stdio::piped())
                     .stdout(Stdio::piped())
                     .stderr(Stdio::piped())
                     .spawn().unwrap();
                 child.stdin.as_mut().unwrap().write_all(mlir.as_bytes()).unwrap();
                 let output = child.wait_with_output().unwrap();
                 assert!(output.status.success(), "salt-opt failed for Saturation Attack:\n{}", String::from_utf8_lossy(&output.stderr));
            }
        }
        Err(e) => panic!("Saturation Attack compilation failed: {:?}", e),
    }
}

#[test]
fn test_stmt_error_paths_saturation() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = BTreeMap::new();

        // 1. Non-bool while condition
        let code_while = "while 123 { }";
        let stmt_while: salt_front::grammar::Stmt = syn::parse_str(code_while).unwrap();
        let res_while = emit_stmt(&ctx, &mut out, &stmt_while, &mut locals);
        assert!(res_while.is_err());
        assert!(res_while.unwrap_err().contains("While condition must be boolean"));

        // 2. Non-bool if condition
        let code_if = "if 123 { }";
        let stmt_if: salt_front::grammar::Stmt = syn::parse_str(code_if).unwrap();
        let res_if = emit_stmt(&ctx, &mut out, &stmt_if, &mut locals);
        assert!(res_if.is_err());
        assert!(res_if.unwrap_err().contains("If condition must be boolean"));

        // 3. Continue outside loop
        let code_cont = "continue;";
        let stmt_cont: salt_front::grammar::Stmt = syn::parse_str(code_cont).unwrap();
        let res_cont = emit_stmt(&ctx, &mut out, &stmt_cont, &mut locals);
        assert!(res_cont.is_err());
        assert!(res_cont.unwrap_err().contains("Continue outside of loop"));

        // 4. Break outside loop
        let code_break = "break;";
        let stmt_break: salt_front::grammar::Stmt = syn::parse_str(code_break).unwrap();
        let res_break = emit_stmt(&ctx, &mut out, &stmt_break, &mut locals);
        assert!(res_break.is_err());
        assert!(res_break.unwrap_err().contains("Break outside of loop"));

        // 5. Tuple pattern length mismatch
        let code_tuple = "let (a, b) = (1, 2, 3);";
        let stmt_tuple: salt_front::grammar::Stmt = syn::parse_str(code_tuple).unwrap();
        let res_tuple = emit_stmt(&ctx, &mut out, &stmt_tuple, &mut locals);
        assert!(res_tuple.is_err());
        assert!(res_tuple.unwrap_err().contains("Tuple pattern length mismatch"));

        // 6. Unknown struct in pattern
        let code_struct = "let Unknown { x } = some_val;";
        locals.insert("some_val".to_string(), (Type::Struct("Unknown".to_string()), LocalKind::SSA("%v".to_string())));
        let stmt_struct: salt_front::grammar::Stmt = syn::parse_str(code_struct).unwrap();
        let res_struct = emit_stmt(&ctx, &mut out, &stmt_struct, &mut locals);
        assert!(res_struct.is_err());
        assert!(res_struct.unwrap_err().contains("Unknown struct Unknown"));

        // 7. SSA to Ptr re-assignment (promotion to storage)
        let mut locals_ptr = BTreeMap::new();
        locals_ptr.insert("x".to_string(), (Type::I32, LocalKind::Ptr("%x_ptr".to_string())));
        let code_ptr = "let x = 10;";
        let stmt_ptr: salt_front::grammar::Stmt = syn::parse_str(code_ptr).unwrap();
        let res_ptr = emit_stmt(&ctx, &mut out, &stmt_ptr, &mut locals_ptr);
        assert!(res_ptr.is_ok());
        assert!(out.contains("llvm.store"));

        // 8. hoist_allocas fallback (implicit I32)
        // This is hard to trigger via parse_str because syn::Local usually has Ty or Init.
        // We can manually construct a Stmt.
        let manual_local = syn::Local {
            attrs: vec![],
            let_token: Default::default(),
            pat: syn::Pat::Ident(syn::PatIdent {
                attrs: vec![],
                by_ref: None,
                mutability: None,
                ident: syn::Ident::new("hoist_fallback", proc_macro2::Span::call_site()),
                subpat: None,
            }),
            init: None,
            semi_token: Default::default(),
        };
        let stmt_hoist = salt_front::grammar::Stmt::Syn(syn::Stmt::Local(manual_local));
        let mut locals_hoist = BTreeMap::new();
        let res_hoist = emit_block(&ctx, &mut out, &[stmt_hoist], &mut locals_hoist);
        assert!(res_hoist.is_ok());
        assert!(locals_hoist.contains_key("hoist_fallback"));
        assert_eq!(locals_hoist.get("hoist_fallback").unwrap().0, Type::I32);
    });
}

#[test]
fn test_expr_error_paths_saturation() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = BTreeMap::new();

        // 1. Package used as value
        ctx.imports.borrow_mut().push(salt_front::grammar::ImportDecl {
            name: {
                let mut p = syn::punctuated::Punctuated::new();
                p.push(syn::Ident::new("std", proc_macro2::Span::call_site()));
                p
            },
            alias: None,
            group: None,
        });
        let code_pkg = "std";
        let expr_pkg: syn::Expr = syn::parse_str(code_pkg).unwrap();
        let res_pkg = emit_expr(&ctx, &mut out, &expr_pkg, &mut locals, None);
        assert!(res_pkg.is_err());
        assert!(res_pkg.unwrap_err().contains("Package or module 'std' used as value"));

        // 2. ? operator non-Status
        let code_try = "123?";
        let expr_try: syn::Expr = syn::parse_str(code_try).unwrap();
        let res_try = emit_expr(&ctx, &mut out, &expr_try, &mut locals, None);
        assert!(res_try.is_err());
        assert!(res_try.unwrap_err().contains("requires Result<T> type"));

        // 3. Array element type mismatch
        let code_arr = "[1, true]";
        let expr_arr: syn::Expr = syn::parse_str(code_arr).unwrap();
        let res_arr = emit_expr(&ctx, &mut out, &expr_arr, &mut locals, None);
        assert!(res_arr.is_err());
        assert!(res_arr.unwrap_err().contains("Array element type mismatch"));

        // 4. Match on non-enum
        let code_match = "match 123 { }";
        let expr_match: syn::Expr = syn::parse_str(code_match).unwrap();
        let res_match = emit_expr(&ctx, &mut out, &expr_match, &mut locals, None);
        assert!(res_match.is_err());
        assert!(res_match.unwrap_err().contains("Match only supported on Enums"));

        // 5. Unknown enum in match
        locals.insert("e".to_string(), (Type::Enum("UnknownEnum".to_string()), LocalKind::SSA("%e".to_string())));
        let code_match2 = "match e { }";
        let expr_match2: syn::Expr = syn::parse_str(code_match2).unwrap();
        let res_match2 = emit_expr(&ctx, &mut out, &expr_match2, &mut locals, None);
        assert!(res_match2.is_err());
        assert!(res_match2.unwrap_err().contains("Unknown enum UnknownEnum"));

        // 6. Unknown variant in match
        let key = TypeKey { path: vec![], name: "MyEnum".to_string(), specialization: None };
        ctx.enum_registry_mut().insert(key, salt_front::registry::EnumInfo {
            name: "MyEnum".to_string(),
            variants: vec![("V1".to_string(), None, 0)],
            max_payload_size: 0,
            template_name: None,
            specialization_args: vec![],
        });
        locals.insert("e2".to_string(), (Type::Enum("MyEnum".to_string()), LocalKind::SSA("%e2".to_string())));
        let code_match3 = "match e2 { MyEnum::V2 => 0 }";
        let expr_match3: syn::Expr = syn::parse_str(code_match3).unwrap();
        let res_match3 = emit_expr(&ctx, &mut out, &expr_match3, &mut locals, None);
        assert!(res_match3.is_err());
        assert!(res_match3.unwrap_err().contains("Unknown variant V2"));

        // 7. Catch-all pattern in match
        let code_match4 = "match e2 { v => 0 }";
        let expr_match4: syn::Expr = syn::parse_str(code_match4).unwrap();
        let res_match4 = emit_expr(&ctx, &mut out, &expr_match4, &mut locals, None);
        assert!(res_match4.is_err());
        assert!(res_match4.unwrap_err().contains("Catch-all patterns not supported"));

        // 8. Large array repeat
        let code_rep = "[0; 1000]";
        let expr_rep: syn::Expr = syn::parse_str(code_rep).unwrap();
        let res_rep = emit_expr(&ctx, &mut out, &expr_rep, &mut locals, None);
        assert!(res_rep.is_err());
        assert!(res_rep.unwrap_err().contains("too large for unrolled initialization"));

        // 9. Invalid literals (too large for u64)
        let expr_large: syn::Expr = syn::parse_str("18446744073709551616").unwrap();
        let res_large = emit_expr(&ctx, &mut out, &expr_large, &mut locals, None);
        assert!(res_large.is_err());

        // 11. Stmt::Return
        let code_ret = "return 123;";
        let stmt_ret: salt_front::grammar::Stmt = syn::parse_str(code_ret).unwrap();
        ctx.current_ret_ty.borrow_mut().replace(Type::I32);
        let res_ret = emit_stmt(&ctx, &mut out, &stmt_ret, &mut locals);
        assert!(res_ret.is_ok());
        assert!(out.contains("func.return"));

        // 12. Stmt::Unsafe
        let code_unsafe = "unsafe { let x = 1; }";
        let stmt_unsafe: salt_front::grammar::Stmt = syn::parse_str(code_unsafe).unwrap();
        let res_unsafe = emit_stmt(&ctx, &mut out, &stmt_unsafe, &mut locals);
        assert!(res_unsafe.is_ok());

        // 13. Stmt::For with no start/end
        let code_for_no_start = "for i in ..10 { }";
        let stmt_for: salt_front::grammar::Stmt = syn::parse_str(code_for_no_start).unwrap();
        let res_for = emit_stmt(&ctx, &mut out, &stmt_for, &mut locals);
        assert!(res_for.is_ok());

        // 14. Nested SaltElse::If
        let code_else_if = "if true { } else if false { } else { }";
        let stmt_else_if: salt_front::grammar::Stmt = syn::parse_str(code_else_if).unwrap();
        let res_else_if = emit_stmt(&ctx, &mut out, &stmt_else_if, &mut locals);
        assert!(res_else_if.is_ok());

        // 15. emit_cleanup_for_return with Type::Owned
        let mut locals_owned = BTreeMap::new();
        locals_owned.insert("pkg_ptr".to_string(), (Type::Owned(Box::new(Type::I32)), LocalKind::Ptr("%ptr".to_string())));
        let res_cleanup = salt_front::codegen::stmt::emit_cleanup_for_return(&ctx, &mut out, &locals_owned);
        assert!(res_cleanup.is_ok());
        assert!(out.contains("salt.drop"));
        // 16. Stmt::For with open-ended ranges (Error case)
        let code_for_no_end = "for i in 0.. { }";
        let stmt_for_no_end: salt_front::grammar::Stmt = syn::parse_str(code_for_no_end).unwrap();
        let res_for_no_end = emit_stmt(&ctx, &mut out, &stmt_for_no_end, &mut locals);
        assert!(res_for_no_end.is_err());
        assert!(res_for_no_end.unwrap_err().contains("Infinite for-loops not supported yet"));

        // 17. Constant lookup in emit_expr (with package prefix)
        ctx.imports.borrow_mut().push(salt_front::grammar::ImportDecl {
            name: {
                let mut p = syn::punctuated::Punctuated::new();
                p.push(syn::Ident::new("std", proc_macro2::Span::call_site()));
                p
            },
            alias: None,
            group: None,
        });
        ctx.evaluator.borrow_mut().constant_table.insert("std__MY_CONST".to_string(), salt_front::evaluator::ConstValue::Integer(42));
        let expr_const: syn::Expr = syn::parse_str("std.MY_CONST").unwrap();
        let res_const = emit_expr(&ctx, &mut out, &expr_const, &mut locals, None);
        assert!(res_const.is_ok());

        // 18. Function pointer decay in emit_path
        ctx.globals.borrow_mut().insert("my_fn".to_string(), Type::Fn(vec![], Box::new(Type::Unit)));
        let _res_fn = emit_path(&ctx, &mut out, &syn::parse_str("my_fn").unwrap(), &mut locals, None);
        assert!(_res_fn.is_ok());

        // 20. Stmt::For with pulse(off)
        let code_for_off = "@pulse(off) for i in 0..10 { }";
        let stmt_for_off: salt_front::grammar::Stmt = syn::parse_str(code_for_off).unwrap();
        let res_for_off = emit_stmt(&ctx, &mut out, &stmt_for_off, &mut locals);
        assert!(res_for_off.is_ok());

        // 21. Successful Try operator on Result enum
        let key = TypeKey { path: vec![], name: "Result_i32".to_string(), specialization: None };
        ctx.enum_registry_mut().insert(key, salt_front::registry::EnumInfo {
            name: "Result_i32".to_string(),
            variants: vec![
                ("Ok".to_string(), Some(Type::I32), 0),
                ("Err".to_string(), Some(Type::Struct("Status".to_string())), 1),
            ],
            max_payload_size: 8,
            template_name: Some("Result".to_string()),
            specialization_args: vec![Type::I32],
        });
        locals.insert("r".to_string(), (Type::Concrete("Result".to_string(), vec![Type::I32]), LocalKind::SSA("%r".to_string())));
        let code_try_ok = "r?";
        let expr_try_ok: syn::Expr = syn::parse_str(code_try_ok).unwrap();
        let res_try_ok = emit_expr(&ctx, &mut out, &expr_try_ok, &mut locals, None);
        assert!(res_try_ok.is_ok());

        // 22. Struct name canonicalization in to_mlir_type
        let key = TypeKey { path: vec!["std".to_string()], name: "MyStruct".to_string(), specialization: None };
        ctx.struct_registry_mut().insert(key, salt_front::registry::StructInfo {
            name: "std.MyStruct".to_string(),
            fields: HashMap::new(),
            field_order: vec![],
            template_name: None,
            specialization_args: vec![],
        });
        let ty_struct = Type::Struct("MyStruct".to_string());
        let mlir_struct = ty_struct.to_mlir_type(&ctx);
        assert!(mlir_struct.is_ok());
        assert!(mlir_struct.unwrap().contains("std.MyStruct"));

        // 23. Specialization cache hit
        let _s1 = ctx.request_specialization("my_fn", vec![Type::I32], None);
        let _s2 = ctx.request_specialization("my_fn", vec![Type::I32], None);
        assert_eq!(_s1, _s2);

        // 24. Array type resolution with length expression
        let code_arr = "[i32; 10 + 10]";
        let syn_arr: syn::Type = syn::parse_str(code_arr).unwrap();
        let salt_arr = resolve_type(&ctx, &syn_arr);
        assert!(matches!(salt_arr, Type::Array(_, 20)));

        // 25. Tuple storage type
        let ty_tuple = Type::Tuple(vec![Type::I32, Type::I64]);
        let storage_tuple = ty_tuple.to_mlir_storage_type(&ctx);
        assert!(storage_tuple.is_ok());
        assert!(storage_tuple.unwrap().contains("!llvm.struct<(i32, i64)>"));

        // 26. resolve_codegen_type recursion (Array/Tuple)
        let ty_arr_rec = Type::Array(Box::new(Type::I32), 10);
        let res_arr_rec = resolve_codegen_type(&ctx, &ty_arr_rec);
        assert!(matches!(res_arr_rec, Type::Array(_, 10)));

        let ty_tup_rec = Type::Tuple(vec![Type::I32]);
        let res_tup_rec = resolve_codegen_type(&ctx, &ty_tup_rec);
        assert!(matches!(res_tup_rec, Type::Tuple(_)));

        // 27. Stmt::Return(Some) with promotion
        *ctx.current_ret_ty.borrow_mut() = Some(Type::I64);
        let code_ret = "salt_return 1;";
        let stmt_ret: salt_front::grammar::Stmt = syn::parse_str(code_ret).unwrap();
        let res_ret = emit_stmt(&ctx, &mut out, &stmt_ret, &mut locals);
        assert!(res_ret.is_ok());
        assert!(out.contains("func.return"));
        assert!(out.contains("i64"));

        // 28. Stmt::Return(None)
        *ctx.current_ret_ty.borrow_mut() = None;
        let code_ret_none = "salt_return;";
        let stmt_ret_none: salt_front::grammar::Stmt = syn::parse_str(code_ret_none).unwrap();
        let res_ret_none = emit_stmt(&ctx, &mut out, &stmt_ret_none, &mut locals);
        assert!(res_ret_none.is_ok());

        // 29. Stmt::Invariant
        locals.insert("i".to_string(), (Type::I32, LocalKind::SSA("%i".to_string())));
        let code_inv = "invariant i < 10;";
        let stmt_inv: salt_front::grammar::Stmt = syn::parse_str(code_inv).unwrap();
        let res_inv = emit_stmt(&ctx, &mut out, &stmt_inv, &mut locals);
        assert!(res_inv.is_ok());
        assert!(out.contains("salt.verify"));

        // 30. Stmt::Break / Stmt::Continue
        ctx.break_labels.borrow_mut().push("break_target".to_string());
        ctx.continue_labels.borrow_mut().push("continue_target".to_string());
        let res_break = emit_stmt(&ctx, &mut out, &salt_front::grammar::Stmt::Break, &mut locals);
        let res_continue = emit_stmt(&ctx, &mut out, &salt_front::grammar::Stmt::Continue, &mut locals);
        assert!(res_break.is_ok());
        assert!(res_continue.is_ok());
        ctx.break_labels.borrow_mut().pop();
        ctx.continue_labels.borrow_mut().pop();

        // 31. Stmt::Unsafe
        let code_unsafe = "unsafe { let x = 1; }";
        let stmt_unsafe: salt_front::grammar::Stmt = syn::parse_str(code_unsafe).unwrap();
        let res_unsafe = emit_stmt(&ctx, &mut out, &stmt_unsafe, &mut locals);
        assert!(res_unsafe.is_ok());

        // 32. Use-before-def in scan_local_definitions (via library call)
         // 32. Use-before-def in scan_local_definitions (via library call)
         let file = SaltFile::empty(); // Dummy file
         ctx.scan_defs_from_file(&file);
    });
}

#[test]
fn test_expr_extreme_paths_saturation() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = BTreeMap::new();

        // 1. Try (?) operator on Result
        let key = TypeKey { path: vec![], name: "Result_i32".to_string(), specialization: None };
        ctx.enum_registry_mut().insert(key, salt_front::registry::EnumInfo {
            name: "Result_i32".to_string(),
            variants: vec![
                ("Ok".to_string(), Some(Type::I32), 0),
                ("Err".to_string(), Some(Type::Struct("Status".to_string())), 1),
            ],
            max_payload_size: 8,
            template_name: Some("Result".to_string()),
            specialization_args: vec![Type::I32],
        });
        locals.insert("s".to_string(), (Type::Concrete("Result".to_string(), vec![Type::I32]), LocalKind::SSA("%s".to_string())));
        let code_try = "s?";
        let expr_try: syn::Expr = syn::parse_str(code_try).unwrap();
        let res_try = emit_expr(&ctx, &mut out, &expr_try, &mut locals, None);
        assert!(res_try.is_ok());

        // 2. syn::Expr::Return in emit_expr
        let code_ret_expr = "return 42";
        let expr_ret: syn::Expr = syn::parse_str(code_ret_expr).unwrap();
        ctx.current_ret_ty.borrow_mut().replace(Type::I32);
        let res_ret = emit_expr(&ctx, &mut out, &expr_ret, &mut locals, None);
        assert!(res_ret.is_ok());
        assert_eq!(res_ret.unwrap().1, Type::Never);

        // 3. Array repeat with non-constant len (triggered via evaluator Err)
        let code_rep = "[0; unknown_len]";
        let expr_rep: syn::Expr = syn::parse_str(code_rep).unwrap();
        let res_rep = emit_expr(&ctx, &mut out, &expr_rep, &mut locals, None);
        assert!(res_rep.is_err());
        assert!(res_rep.unwrap_err().contains("must be a constant integer"));

        // 4. emit_lit hex literal success
        let expr_hex: syn::Expr = syn::parse_str("0xFF").unwrap();
        let res_hex = emit_expr(&ctx, &mut out, &expr_hex, &mut locals, None);
        assert!(res_hex.is_ok());
        assert_eq!(res_hex.unwrap().1, Type::I32);

        // 6. Bool and Float constants in emit_expr
        ctx.evaluator.borrow_mut().constant_table.insert("MY_BOOL".to_string(), salt_front::evaluator::ConstValue::Bool(true));
        ctx.evaluator.borrow_mut().constant_table.insert("MY_FLOAT".to_string(), salt_front::evaluator::ConstValue::Float(3.14));
        let _res_bool = emit_expr(&ctx, &mut out, &syn::parse_str("MY_BOOL").unwrap(), &mut locals, None);
        let _res_float = emit_expr(&ctx, &mut out, &syn::parse_str("MY_FLOAT").unwrap(), &mut locals, None);
        assert!(_res_bool.is_ok());
        assert!(_res_float.is_ok());

        // 7. Window indexing in emit_index
        locals.insert("win".to_string(), (Type::Window(Box::new(Type::I32), "r1".to_string()), LocalKind::SSA("%win_ptr".to_string())));
        let code_win_idx = "win[0]";
        let expr_win_idx: syn::Expr = syn::parse_str(code_win_idx).unwrap();
        let res_win_idx = emit_expr(&ctx, &mut out, &expr_win_idx, &mut locals, None);
        assert!(res_win_idx.is_ok());

        // 8. Reference indexing in emit_index
        locals.insert("ref_val".to_string(), (Type::Reference(Box::new(Type::I32), false), LocalKind::SSA("%ref_ptr".to_string())));
        let code_ref_idx = "ref_val[0]";
        let expr_ref_idx: syn::Expr = syn::parse_str(code_ref_idx).unwrap();
        let res_ref_idx = emit_expr(&ctx, &mut out, &expr_ref_idx, &mut locals, None);
        assert!(res_ref_idx.is_ok());

        // 10. Match with TupleStruct pattern (payload)
        let key = TypeKey { path: vec![], name: "PayloadEnum".to_string(), specialization: None };
        ctx.enum_registry_mut().insert(key, salt_front::registry::EnumInfo {
            name: "PayloadEnum".to_string(),
            variants: vec![("V1".to_string(), Some(Type::I32), 0)],
            max_payload_size: 4,
            template_name: None,
            specialization_args: vec![],
        });
        locals.insert("pe".to_string(), (Type::Enum("PayloadEnum".to_string()), LocalKind::SSA("%pe_val".to_string())));
        let code_match_payload = "match pe { PayloadEnum::V1(x) => x }";
        let expr_match_payload: syn::Expr = syn::parse_str(code_match_payload).unwrap();
        let res_match_payload = emit_expr(&ctx, &mut out, &expr_match_payload, &mut locals, Some(&Type::I32));
        assert!(res_match_payload.is_ok());

        // 11. Type::Concrete in to_mlir_type
        let ty_concrete = Type::Concrete("MyTemplate".to_string(), vec![Type::I32]);
        let mlir_concrete = ty_concrete.to_mlir_type(&ctx);
        assert!(mlir_concrete.is_ok());

        // 12. Reference to Struct field
        let key = TypeKey { path: vec![], name: "Point".to_string(), specialization: None };
        ctx.struct_registry_mut().insert(key, salt_front::registry::StructInfo {
            name: "Point".to_string(),
            fields: vec![("x".to_string(), (0, Type::I32))].into_iter().collect(),
            field_order: vec![Type::I32],
            template_name: None,
            specialization_args: vec![],
        });
        locals.insert("pt".to_string(), (Type::Struct("Point".to_string()), LocalKind::Ptr("%pt_ptr".to_string())));
        let code_ref_field = "&pt.x";
        let expr_ref_field: syn::Expr = syn::parse_str(code_ref_field).unwrap();
        let res_ref_field = emit_expr(&ctx, &mut out, &expr_ref_field, &mut locals, None);
        assert!(res_ref_field.is_ok());

        // 13. Field access on Reference(Tuple)
        locals.insert("ref_tuple".to_string(), (Type::Reference(Box::new(Type::Tuple(vec![Type::I32])), false), LocalKind::SSA("%rt_ptr".to_string())));
        let code_ref_tuple = "ref_tuple.0";
        let expr_ref_tuple: syn::Expr = syn::parse_str(code_ref_tuple).unwrap();
        let res_ref_tuple = emit_expr(&ctx, &mut out, &expr_ref_tuple, &mut locals, None);
        assert!(res_ref_tuple.is_ok());

        // 14. Field access on Owned(Struct)
        locals.insert("owned_pt".to_string(), (Type::Owned(Box::new(Type::Struct("Point".to_string()))), LocalKind::SSA("%opt_ptr".to_string())));
        let code_owned_field = "owned_pt.x";
        let expr_owned_field: syn::Expr = syn::parse_str(code_owned_field).unwrap();
        let res_owned_field = emit_expr(&ctx, &mut out, &expr_owned_field, &mut locals, None);
        assert!(res_owned_field.is_ok());

        // 15. Struct instantiation with fields
        let code_struct_init = "Point { x: 10 }";
        let expr_struct_init: syn::Expr = syn::parse_str(code_struct_init).unwrap();
        let res_struct_init = emit_expr(&ctx, &mut out, &expr_struct_init, &mut locals, None);
        assert!(res_struct_init.is_ok());

        // 16. Float Assignment Operators
        locals.insert("f".to_string(), (Type::F64, LocalKind::Ptr("%f_ptr".to_string())));
        let code_f_add = "f += 1.0;";
        let stmt_f_add: salt_front::grammar::Stmt = syn::parse_str(code_f_add).unwrap();
        let res_f_add = emit_stmt(&ctx, &mut out, &stmt_f_add, &mut locals);
        assert!(res_f_add.is_ok());

        // 17. reinterpret_cast (int to ptr) - using turbofish for syn
        let code_cast_ptr = "reinterpret_cast::<i64>(0i64)";
        let expr_cast_ptr: syn::Expr = syn::parse_str(code_cast_ptr).unwrap();
        let res_cast_ptr = emit_expr(&ctx, &mut out, &expr_cast_ptr, &mut locals, None);
        assert!(res_cast_ptr.is_ok());

        // 18. reinterpret_cast (ptr to int)
        locals.insert("p".to_string(), (Type::Reference(Box::new(Type::I32), false), LocalKind::SSA("%p_ptr".to_string())));
        let code_cast_int = "reinterpret_cast::<i64>(p)";
        let expr_cast_int: syn::Expr = syn::parse_str(code_cast_int).unwrap();
        let res_cast_int = emit_expr(&ctx, &mut out, &expr_cast_int, &mut locals, None);
        assert!(res_cast_int.is_ok());

        // 19. reinterpret_cast (float bitcast)
        let code_cast_f64 = "reinterpret_cast::<f64>(0i64)";
        let expr_cast_f64: syn::Expr = syn::parse_str(code_cast_f64).unwrap();
        let res_cast_f64 = emit_expr(&ctx, &mut out, &expr_cast_f64, &mut locals, None);
        assert!(res_cast_f64.is_ok());

        // 21. Complex Logical Ops (short-circuiting)
        let code_logic = "(true && false) || (true && true)";
        let expr_logic: syn::Expr = syn::parse_str(code_logic).unwrap();
        let res_logic = emit_expr(&ctx, &mut out, &expr_logic, &mut locals, None);
        assert!(res_logic.is_ok());

        // 22. Owned spill in emit_call
        ctx.globals.borrow_mut().insert("take_owned".to_string(), Type::Fn(vec![Type::Owned(Box::new(Type::I32))], Box::new(Type::Unit)));
        let code_call_owned = "take_owned(123)";
        let expr_call_owned: syn::Expr = syn::parse_str(code_call_owned).unwrap();
        let res_call_owned = emit_expr(&ctx, &mut out, &expr_call_owned, &mut locals, None);
        assert!(res_call_owned.is_ok());

        // 23. resolve_codegen_type self-type
        *ctx.current_self_ty.borrow_mut() = Some(Type::I32);
        let res_self = salt_front::codegen::type_bridge::resolve_codegen_type(&ctx, &Type::SelfType);
        assert_eq!(res_self, Type::I32);

        // 24. reinterpret_cast (aggregate spill)
        let code_cast_agg = "reinterpret_cast::<Point>(0i64)";
        let expr_cast_agg: syn::Expr = syn::parse_str(code_cast_agg).unwrap();
        let res_cast_agg = emit_expr(&ctx, &mut out, &expr_cast_agg, &mut locals, None);
        assert!(res_cast_agg.is_ok());

        // 25. unify_types in emit_if_expr
        let code_if_unify = "if true { 1i32 } else { 2i64 }";
        let expr_if_unify: syn::Expr = syn::parse_str(code_if_unify).unwrap();
        let res_if_unify = emit_expr(&ctx, &mut out, &expr_if_unify, &mut locals, None);
        assert!(res_if_unify.is_ok());
    });
}
