use syn::Expr;
use salt_front::grammar::*;
use salt_front::types::Type;
use salt_front::codegen::context::{CodegenContext, LocalKind};
use salt_front::codegen::expr::emit_expr;
use std::collections::{BTreeMap, HashMap};
use salt_front::registry::EnumInfo;
use salt_front::types::TypeKey;

macro_rules! with_ctx {
    ($ctx:ident, $block:block) => {
        let code = "fn main() {}";
        let file: SaltFile = syn::parse_str(code).unwrap();
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut $ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        $block
    };
}

#[test]
fn test_shift_edge_cases() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = HashMap::new();
        
        // 1. Shift by zero
        let expr: Expr = syn::parse_str("x << 0").unwrap();
        locals.insert("x".to_string(), (Type::I32, LocalKind::SSA("%x".to_string())));
        // We need to provide a value for x in the context or just check IR
        // Actually emit_expr reads from localvars.
        
        let _ = ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &expr, &mut locals, None));
        assert!(out.contains("arith.shli"));
        
        // 2. Shift by large value
        out.clear();
        let expr: Expr = syn::parse_str("x >> 33").unwrap();
        let _ = ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &expr, &mut locals, None));
        assert!(out.contains("arith.shrsi") || out.contains("arith.shrui"));
    });
}

#[test]
fn test_logical_short_circuit_complex() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = HashMap::new();
        locals.insert("a".to_string(), (Type::Bool, LocalKind::SSA("%a".to_string())));
        locals.insert("b".to_string(), (Type::Bool, LocalKind::SSA("%b".to_string())));
        locals.insert("c".to_string(), (Type::Bool, LocalKind::SSA("%c".to_string())));
        
        // Permutations of short-circuiting
        let exprs = vec![
            "a && b && c",
            "a || b || c",
            "(a && b) || c",
            "a && (b || c)",
            "true && a",
            "false || b",
        ];

        for s in exprs {
            out.clear();
            let expr: Expr = syn::parse_str(s).unwrap();
            let _ = ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &expr, &mut locals, None));
            assert!(out.contains("cf.cond_br"));
        }
    });
}

#[test]
fn test_error_paths_expr() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = HashMap::new();
        
        // 1. Undefined variable
        let expr: Expr = syn::parse_str("undefined_var").unwrap();
        assert!(ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &expr, &mut locals, None)).is_err());
        
        // 2. Undefined function call
        let expr: Expr = syn::parse_str("undef_fn()").unwrap();
        assert!(ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &expr, &mut locals, None)).is_err());

        // 3. Logic on non-bools
        let expr: Expr = syn::parse_str("1 && 2").unwrap();
        assert!(ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &expr, &mut locals, None)).is_err());

        // 4. Bitwise on non-integers (floats)
        let expr: Expr = syn::parse_str("1.0 & 2.0").unwrap();
        assert!(ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &expr, &mut locals, None)).is_err());

        // 5. Arithmetic on non-numerics (bools)
        let expr: Expr = syn::parse_str("true + false").unwrap();
        assert!(ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &expr, &mut locals, None)).is_err());
    });
}

#[test]
fn test_expr_edge_cases_and_interner() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = HashMap::new();
        
        // 1. Boolean arrays (hits zext logic)
        let code = "[true, false]";
        let expr: syn::Expr = syn::parse_str(code).unwrap();
        ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &expr, &mut locals, None)).unwrap();
        assert!(out.contains("arith.extui"));

        // 2. String literal reuse (hits interner cache)
        let s_code = "\"reuse\"";
        let s_expr: syn::Expr = syn::parse_str(s_code).unwrap();
        ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &s_expr, &mut locals, None)).unwrap();
        ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &s_expr, &mut locals, None)).unwrap();
        // The second call hits the cache
    });
}

#[test]
fn test_expr_error_paths_expanded() {
    with_ctx!(ctx, {
        let mut out = String::new();
        let mut locals = HashMap::new();
        
        // 1. Use of moved variable
        locals.insert("y".to_string(), (Type::I32, LocalKind::Ptr("%y_ptr".to_string())));
        ctx.consumed_vars_mut().insert("y".to_string());
        let move_code = "y";
        let move_expr: syn::Expr = syn::parse_str(move_code).unwrap();
        let res = ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &move_expr, &mut locals, None));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("moved value"));

        // 2. ? operator on non-Status enum
        let try_code = "my_enum?";
        locals.insert("my_enum".to_string(), (Type::Enum("OtherEnum".to_string()), LocalKind::SSA("%e".to_string())));
        let key = TypeKey { path: vec![], name: "OtherEnum".to_string(), specialization: None };
        ctx.enum_registry_mut().insert(key, EnumInfo {
            name: "OtherEnum".to_string(),
            variants: vec![("V1".to_string(), None, 0)],
            max_payload_size: 0,
            template_name: None,
            specialization_args: vec![],
        });
        let try_expr: syn::Expr = syn::parse_str(try_code).unwrap();
        let res_try = ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &try_expr, &mut locals, None));
        assert!(res_try.is_err());
        assert!(res_try.unwrap_err().contains("requires Result<T> type"));

        // 3. Array type mismatch
        let arr_code = "[1, true]";
        let arr_expr: syn::Expr = syn::parse_str(arr_code).unwrap();
        let res_arr = ctx.with_lowering_ctx(|lctx| emit_expr(lctx, &mut out, &arr_expr, &mut locals, None));
        assert!(res_arr.is_err());
    });
}
