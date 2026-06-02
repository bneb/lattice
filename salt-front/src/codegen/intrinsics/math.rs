use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

pub fn emit_math_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    _expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    match name {
        "popcount" | "ctpop" => {
            if let Some(arg) = args.first() {
                let (v_var, v_ty) = emit_expr(ctx, out, arg, local_vars, None)?;
                let res_var = format!("%pop_{}", ctx.next_id());
                let mlir_ty = v_ty.to_mlir_type(ctx)?;
                out.push_str(&format!("    {} = math.ctpop {} : {}\n", res_var, v_var, mlir_ty));
                Ok(Some((res_var, v_ty)))
            } else {
                Err("Intrinsic 'popcount' expects 1 argument".to_string())
            }
        }
        "trailing_zeros" | "cttz" => {
            if let Some(arg) = args.first() {
                let (v_var, v_ty) = emit_expr(ctx, out, arg, local_vars, None)?;
                let res_var = format!("%tz_{}", ctx.next_id());
                let mlir_ty = v_ty.to_mlir_type(ctx)?;
                out.push_str(&format!("    {} = math.cttz {} : {}\n", res_var, v_var, mlir_ty));
                Ok(Some((res_var, v_ty)))
            } else {
                Err("Intrinsic 'trailing_zeros' expects 1 argument".to_string())
            }
        }
        "leading_zeros" | "ctlz" => {
            if let Some(arg) = args.first() {
                let (v_var, v_ty) = emit_expr(ctx, out, arg, local_vars, None)?;
                let res_var = format!("%lz_{}", ctx.next_id());
                let mlir_ty = v_ty.to_mlir_type(ctx)?;
                out.push_str(&format!("    {} = math.ctlz {} : {}\n", res_var, v_var, mlir_ty));
                Ok(Some((res_var, v_ty)))
            } else {
                Err("Intrinsic 'leading_zeros' expects 1 argument".to_string())
            }
        }
        "min" | "max" | "sqrt" | "pow" | "abs" | "ceil" | "floor" | "trunc" => {
            if args.is_empty() {
                return Err(format!("Intrinsic '{}' expects at least 1 argument", name));
            }
            let (v1, ty1) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let mlir_ty = ty1.to_mlir_type(ctx)?;
            let res = format!("%math_{}_{}", name, ctx.next_id());
            
            match name {
                "abs" => out.push_str(&format!("    {} = math.absf {} : {}\n", res, v1, mlir_ty)),
                "sqrt" => out.push_str(&format!("    {} = math.sqrt {} : {}\n", res, v1, mlir_ty)),
                "ceil" => out.push_str(&format!("    {} = math.ceil {} : {}\n", res, v1, mlir_ty)),
                "floor" => out.push_str(&format!("    {} = math.floor {} : {}\n", res, v1, mlir_ty)),
                "trunc" => out.push_str(&format!("    {} = math.trunc {} : {}\n", res, v1, mlir_ty)),
                "min" | "max" | "pow" => {
                    if args.len() < 2 {
                        return Err(format!("Intrinsic '{}' expects 2 arguments", name));
                    }
                    let (v2, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&ty1))?;
                    match name {
                        "min" => out.push_str(&format!("    {} = arith.minf {}, {} : {}\n", res, v1, v2, mlir_ty)),
                        "max" => out.push_str(&format!("    {} = arith.maxf {}, {} : {}\n", res, v1, v2, mlir_ty)),
                        "pow" => out.push_str(&format!("    {} = math.powf {}, {} : {}\n", res, v1, v2, mlir_ty)),
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
            Ok(Some((res, ty1)))
        }
        "std__math__ctz_u64" | "ctz_u64" => {
            if let Some(arg) = args.first() {
                let (v, _) = emit_expr(ctx, out, arg, local_vars, Some(&Type::U64))?;
                let res = format!("%ctz_u64_{}", ctx.next_id());
                out.push_str(&format!("    {} = math.cttz {} : i64\n", res, v));
                Ok(Some((res, Type::U64)))
            } else {
                Err("ctz_u64 expects 1 argument".to_string())
            }
        }
        "std__math__clz_u64" | "clz_u64" => {
            if let Some(arg) = args.first() {
                let (v, _) = emit_expr(ctx, out, arg, local_vars, Some(&Type::U64))?;
                let res = format!("%clz_u64_{}", ctx.next_id());
                out.push_str(&format!("    {} = math.ctlz {} : i64\n", res, v));
                Ok(Some((res, Type::U64)))
            } else {
                Err("clz_u64 expects 1 argument".to_string())
            }
        }
        "std__math__popcount_u64" | "popcount_u64" => {
            if let Some(arg) = args.first() {
                let (v, _) = emit_expr(ctx, out, arg, local_vars, Some(&Type::U64))?;
                let res = format!("%pop_u64_{}", ctx.next_id());
                out.push_str(&format!("    {} = math.ctpop {} : i64\n", res, v));
                Ok(Some((res, Type::U64)))
            } else {
                Err("popcount_u64 expects 1 argument".to_string())
            }
        }
        "std__math__expf" | "expf" => {
            if let Some(arg) = args.first() {
                let (v, _) = emit_expr(ctx, out, arg, local_vars, Some(&Type::F32))?;
                let res = format!("%expf_{}", ctx.next_id());
                out.push_str(&format!("    {} = math.exp {} : f32\n", res, v));
                Ok(Some((res, Type::F32)))
            } else {
                Err("expf expects 1 argument".to_string())
            }
        }
        "std__math__sqrtf" | "sqrtf" => {
            if args.len() != 1 { return Err("sqrtf expects 1 argument".to_string()); }
            let (val, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::F32))?;
            let res = format!("%math_sqrt_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.sqrt\"({}) : (f32) -> f32\n", res, val));
            return Ok(Some((res, Type::F32)));
        }
        "std__math__sinf" | "sinf" => {
            if args.len() != 1 { return Err("sinf expects 1 argument".to_string()); }
            let (val, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::F32))?;
            let res = format!("%math_sin_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.sin\"({}) : (f32) -> f32\n", res, val));
            return Ok(Some((res, Type::F32)));
        }
        "std__math__cosf" | "cosf" => {
            if args.len() != 1 { return Err("cosf expects 1 argument".to_string()); }
            let (val, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::F32))?;
            let res = format!("%math_cos_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.cos\"({}) : (f32) -> f32\n", res, val));
            return Ok(Some((res, Type::F32)));
        }
        "std__math__fabsf" | "fabsf" => {
            if args.len() != 1 { return Err("fabsf expects 1 argument".to_string()); }
            let (val, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::F32))?;
            let res = format!("%math_fabs_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.fabs\"({}) : (f32) -> f32\n", res, val));
            return Ok(Some((res, Type::F32)));
        }
        "std__math__floorf" | "floorf" => {
            if args.len() != 1 { return Err("floorf expects 1 argument".to_string()); }
            let (val, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::F32))?;
            let res = format!("%math_floor_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.floor\"({}) : (f32) -> f32\n", res, val));
            return Ok(Some((res, Type::F32)));
        }
        "std__math__ceilf" | "ceilf" => {
            if args.len() != 1 { return Err("ceilf expects 1 argument".to_string()); }
            let (val, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::F32))?;
            let res = format!("%math_ceil_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.ceil\"({}) : (f32) -> f32\n", res, val));
            return Ok(Some((res, Type::F32)));
        }
        "std__math__powf" | "powf" => {
            if args.len() != 2 { return Err("powf expects 2 arguments".to_string()); }
            let (v1, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::F32))?;
            let (v2, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::F32))?;
            let res = format!("%math_powf_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.pow\"({}, {}) : (f32, f32) -> f32\n", res, v1, v2));
            return Ok(Some((res, Type::F32)));
        }

        _ => Ok(None),
    }
}
