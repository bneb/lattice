use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

/// Helper for generic-type bit-count operations (popcount, trailing_zeros, leading_zeros).
fn emit_generic_bit_op(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<Option<(String, Type)>, String> {
    let (mlir_op, prefix) = match name {
        "popcount" | "ctpop" => ("math.ctpop", "pop"),
        "trailing_zeros" | "cttz" => ("math.cttz", "tz"),
        "leading_zeros" | "ctlz" => ("math.ctlz", "lz"),
        _ => return Ok(None),
    };
    let arg = args.first()
        .ok_or_else(|| format!("Intrinsic '{}' expects 1 argument", name))?;
    let (v_var, v_ty) = emit_expr(ctx, out, arg, local_vars, None)?;
    let res_var = format!("%{}_{}", prefix, ctx.next_id());
    let mlir_ty = v_ty.to_mlir_type(ctx)?;
    out.push_str(&format!(
        "    {} = {} {} : {}\n", res_var, mlir_op, v_var, mlir_ty
    ));
    Ok(Some((res_var, v_ty)))
}

/// Helper for fixed-u64 bit-count operations (ctz_u64, clz_u64, popcount_u64).
fn emit_u64_bit_op(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<Option<(String, Type)>, String> {
    let (mlir_op, prefix) = match name {
        "std__math__ctz_u64" | "ctz_u64" => ("math.cttz", "ctz_u64"),
        "std__math__clz_u64" | "clz_u64" => ("math.ctlz", "clz_u64"),
        "std__math__popcount_u64" | "popcount_u64" => ("math.ctpop", "pop_u64"),
        _ => return Ok(None),
    };
    let arg = args.first()
        .ok_or_else(|| format!("{} expects 1 argument", name))?;
    let (v, _) = emit_expr(ctx, out, arg, local_vars, Some(&Type::U64))?;
    let res = format!("%{}_{}", prefix, ctx.next_id());
    out.push_str(&format!("    {} = {} {} : i64\n", res, mlir_op, v));
    Ok(Some((res, Type::U64)))
}

/// Helper for math.float operations (abs, sqrt, ceil, floor, trunc, min, max, pow).
fn emit_math_float_op(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<Option<(String, Type)>, String> {
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

/// Helper for F32 intrinsics (expf, sqrtf, sinf, cosf, fabsf, floorf, ceilf, powf).
fn emit_f32_op(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<Option<(String, Type)>, String> {
    let (intr_op, var_prefix, num_args, is_llvm) = match name {
        "std__math__expf" | "expf" => ("math.exp", "expf", 1, false),
        "std__math__sqrtf" | "sqrtf" => ("llvm.intr.sqrt", "math_sqrt", 1, true),
        "std__math__sinf" | "sinf" => ("llvm.intr.sin", "math_sin", 1, true),
        "std__math__cosf" | "cosf" => ("llvm.intr.cos", "math_cos", 1, true),
        "std__math__fabsf" | "fabsf" => ("llvm.intr.fabs", "math_fabs", 1, true),
        "std__math__floorf" | "floorf" => ("llvm.intr.floor", "math_floor", 1, true),
        "std__math__ceilf" | "ceilf" => ("llvm.intr.ceil", "math_ceil", 1, true),
        "std__math__powf" | "powf" => ("llvm.intr.pow", "math_powf", 2, true),
        _ => return Ok(None),
    };

    if args.len() != num_args as usize {
        let plural = if num_args == 1 { "" } else { "s" };
        return Err(format!("{} expects {} argument{}", name, num_args, plural));
    }

    let (v1, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::F32))?;
    let res = format!("%{}_{}", var_prefix, ctx.next_id());

    if num_args == 1 {
        if is_llvm {
            out.push_str(&format!(
                "    {} = \"{}\"({}) : (f32) -> f32\n", res, intr_op, v1
            ));
        } else {
            out.push_str(&format!("    {} = {} {} : f32\n", res, intr_op, v1));
        }
    } else {
        let (v2, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::F32))?;
        out.push_str(&format!(
            "    {} = \"{}\"({}, {}) : (f32, f32) -> f32\n", res, intr_op, v1, v2
        ));
    }
    Ok(Some((res, Type::F32)))
}

pub fn emit_math_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    _expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    match name {
        // Generic bit-count operations
        "popcount" | "ctpop" | "trailing_zeros" | "cttz" | "leading_zeros" | "ctlz" => {
            emit_generic_bit_op(ctx, out, name, args, local_vars)
        }
        // Fixed-u64 bit-count operations
        "std__math__ctz_u64" | "ctz_u64"
        | "std__math__clz_u64" | "clz_u64"
        | "std__math__popcount_u64" | "popcount_u64" => {
            emit_u64_bit_op(ctx, out, name, args, local_vars)
        }
        // Math float unary/binary operations
        "min" | "max" | "sqrt" | "pow" | "abs" | "ceil" | "floor" | "trunc" => {
            emit_math_float_op(ctx, out, name, args, local_vars)
        }
        // F32-specific math operations
        "std__math__expf" | "expf"
        | "std__math__sqrtf" | "sqrtf"
        | "std__math__sinf" | "sinf"
        | "std__math__cosf" | "cosf"
        | "std__math__fabsf" | "fabsf"
        | "std__math__floorf" | "floorf"
        | "std__math__ceilf" | "ceilf"
        | "std__math__powf" | "powf" => {
            emit_f32_op(ctx, out, name, args, local_vars)
        }
        _ => Ok(None),
    }
}
