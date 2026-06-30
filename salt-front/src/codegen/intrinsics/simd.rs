use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

fn emit_neon_cmpeq(
    ctx: &mut LoweringContext,
    out: &mut String,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<Option<(String, Type)>, String> {
    if args.len() != 2 {
        return Err("Intrinsic 'm4_neon_cmpeq_i8' expects 2 arguments (vec, char)".to_string());
    }
    let (vec_var, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
    let (char_var, _) = emit_expr(ctx, out, &args[1], local_vars, None)?;
    let trunc = format!("%ceq_byte_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.trunci {} : i64 to i8\n", trunc, char_var));
    let splat = format!("%ceq_splat_{}", ctx.next_id());
    out.push_str(&format!("    {} = \"llvm.mlir.undef\"() : () -> vector<16xi8>\n", splat));
    let splat_full = format!("%ceq_splat_full_{}", ctx.next_id());
    out.push_str(&format!("    {} = \"llvm.intr.aarch64.neon.dup.lane.v16i8\"({}, {}) : (vector<16xi8>, i32) -> vector<16xi8>\n", splat_full, splat, "0"));
    let res = format!("%ceq_res_{}", ctx.next_id());
    out.push_str(&format!("    {} = \"llvm.intr.aarch64.neon.cmeq.v16i8\"({}, {}) : (vector<16xi8>, vector<16xi8>) -> vector<16xi8>\n", res, vec_var, splat_full));
    let reduced = format!("%ceq_max_{}", ctx.next_id());
    out.push_str(&format!("    {} = \"llvm.intr.aarch64.neon.umaxv.i8.v16i8\"({}) : (vector<16xi8>) -> i8\n", reduced, res));
    let ext = format!("%ceq_ext_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.extui {} : i8 to i64\n", ext, reduced));
    Ok(Some((ext, Type::I64)))
}

fn emit_vector_relu(
    ctx: &mut LoweringContext,
    out: &mut String,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<Option<(String, Type)>, String> {
    if args.len() != 1 {
        return Err("v_relu expects 1 argument".to_string());
    }
    let (a, ty_a) = emit_expr(ctx, out, &args[0], local_vars, None)?;
    let res = format!("%vrelu_{}", ctx.next_id());
    let mlir_ty = ty_a.to_mlir_type(ctx)?;
    let (_shape, inner_ty) = if let Type::Concrete(_, args) = &ty_a {
        if args.len() >= 2 { (0, &args[0]) } else { (0, &Type::F32) }
    } else { (0, &Type::F32) };
    let zero_const = format!("%cst_zero_{}", ctx.next_id());
    if inner_ty.is_float() {
        out.push_str(&format!("    {} = arith.constant dense<0.0> : {}\n", zero_const, mlir_ty));
    } else {
        out.push_str(&format!("    {} = arith.constant dense<0> : {}\n", zero_const, mlir_ty));
    }
    let op = if inner_ty.is_float() { "arith.maxnumf" } else { "arith.maxsi" };
    out.push_str(&format!("    {} = {} {}, {} : {}\n", res, op, a, zero_const, mlir_ty));
    Ok(Some((res, ty_a)))
}

pub fn emit_simd_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    _expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    match name {
        "m4_neon_load128" | "keuos__neon_load128" => {
            if args.len() != 1 {
                return Err("Intrinsic 'm4_neon_load128' expects 1 argument (ptr)".to_string());
            }
            let (ptr_var, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let coerced = format!("%neon_ptr_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.inttoptr\"({}) : (i64) -> !llvm.ptr\n", coerced, ptr_var));
            let res = format!("%neon_ld_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.aarch64.neon.ld1\"({}) : (!llvm.ptr) -> vector<16xi8>\n", res, coerced));
            let cast = format!("%neon_ld_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.bitcast\"({}) : (vector<16xi8>) -> !llvm.array<2 x i64>\n", cast, res));
            let lo = format!("%neon_lo_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.extractvalue {}[0] : !llvm.array<2 x i64>\n", lo, cast));
            Ok(Some((lo, Type::I64)))
        }
        "m4_neon_cmpeq_i8" | "keuos__neon_cmpeq" => emit_neon_cmpeq(ctx, out, args, local_vars),
        "m4_neon_movemask" | "keuos__neon_movemask" => {
            if args.len() != 1 {
                return Err("Intrinsic 'm4_neon_movemask' expects 1 argument (vec)".to_string());
            }
            let (vec_var, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let shr = format!("%mmask_shr_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.aarch64.neon.ushr.v16i8\"({}) {{amount = 7 : i32}} : (vector<16xi8>) -> vector<16xi8>\n", shr, vec_var));
            let sum = format!("%mmask_sum_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.aarch64.neon.addv.i8.v16i8\"({}) : (vector<16xi8>) -> i8\n", sum, shr));
            let ext = format!("%mmask_ext_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i8 to i64\n", ext, sum));
            Ok(Some((ext, Type::I64)))
        }
        "v_load" => {
             if args.len() != 2 { return Err("v_load expects 2 arguments: (ptr, offset)".to_string()); }
             let (ptr_val, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
             let (offset_val, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
             let gep = format!("%vload_gep_{}", ctx.next_id());
             out.push_str(&format!("    {} = llvm.getelementptr inbounds {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", gep, ptr_val, offset_val));
             let res = format!("%vload_{}", ctx.next_id());
             out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> vector<4xf32>\n", res, gep));
             Ok(Some((res, Type::Concrete("Vector4f32".to_string(), vec![]))))
        }
        "v_store" => {
             if args.len() != 3 { return Err("v_store expects 3 arguments: (ptr, offset, vec)".to_string()); }
             let (ptr_val, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
             let (offset_val, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
             let (vec_val, _) = emit_expr(ctx, out, &args[2], local_vars, None)?;
             let gep = format!("%vstore_gep_{}", ctx.next_id());
             out.push_str(&format!("    {} = llvm.getelementptr inbounds {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", gep, ptr_val, offset_val));
             out.push_str(&format!("    llvm.store {}, {} : vector<4xf32>, !llvm.ptr\n", vec_val, gep));
             Ok(Some(("".to_string(), Type::Unit)))
        }
        "v_mul" | "v_add" | "v_max" => {
             if args.len() != 2 { return Err(format!("{} expects 2 arguments", name)); }
             let (a, ty_a) = emit_expr(ctx, out, &args[0], local_vars, None)?;
             let (b, ty_b) = emit_expr(ctx, out, &args[1], local_vars, Some(&ty_a))?;
             if ty_a != ty_b { return Err(format!("SIMD Error: {} requires identical types", name)); }
             let res = format!("%v{}_{}", &name[2..], ctx.next_id());
             let mlir_ty = ty_a.to_mlir_type(ctx)?;
             let is_float = if let Type::Concrete(_, args) = &ty_a { !args.is_empty() && args[0].is_float() } else { true };
             let op = match name {
                 "v_mul" => if is_float { "arith.mulf" } else { "arith.muli" },
                 "v_add" => if is_float { "arith.addf" } else { "arith.addi" },
                 "v_max" => if is_float { "arith.maxnumf" } else { "arith.maxsi" },
                 _ => unreachable!(),
             };
             out.push_str(&format!("    {} = {} {}, {} : {}\n", res, op, a, b, mlir_ty));
             Ok(Some((res, ty_a)))
        }
        "v_fma" => {
             if args.len() != 3 { return Err("v_fma expects 3 arguments".to_string()); }
             let (acc, ty_acc) = emit_expr(ctx, out, &args[0], local_vars, None)?;
             let (a, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&ty_acc))?;
             let (b, _) = emit_expr(ctx, out, &args[2], local_vars, Some(&ty_acc))?;
             let res = format!("%vfma_{}", ctx.next_id());
             let mlir_ty = ty_acc.to_mlir_type(ctx)?;
             out.push_str(&format!("    {} = vector.fma {}, {}, {} : {}\n", res, a, b, acc, mlir_ty));
             Ok(Some((res, ty_acc)))
        }
        "v_relu" => emit_vector_relu(ctx, out, args, local_vars),
        "v_hsum" => {
             if args.len() != 1 { return Err("v_hsum expects 1 argument".to_string()); }
             let (v, ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
             let mlir_ty = ty.to_mlir_type(ctx)?;
             let inner_ty = if let Type::Concrete(_, args) = &ty {
                 if !args.is_empty() { args[0].clone() } else { Type::F32 }
             } else { Type::F32 };
             let scalar_mlir = inner_ty.to_mlir_type(ctx)?;

             let res = format!("%vhsum_{}", ctx.next_id());
             out.push_str(&format!("    {} = vector.reduction <add>, {} : {} into {}\n", res, v, mlir_ty, scalar_mlir));
             Ok(Some((res, inner_ty)))
        }
        "v_broadcast" => {
             if args.len() != 1 { return Err("v_broadcast expects 1 argument".to_string()); }
             let (s, s_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
             let res_ty = if let Some(target) = _expected_ty {
                  target.clone()
             } else {
                  return Err("v_broadcast requires expected type context for lane count".to_string());
             };
             let mlir_ty = res_ty.to_mlir_type(ctx)?;
             let res = format!("%vbc_{}", ctx.next_id());
             out.push_str(&format!("    {} = vector.broadcast {} : {} to {}\n", res, s, s_ty.to_mlir_type(ctx)?, mlir_ty));
             Ok(Some((res, res_ty)))
        }

        _ => Ok(None),
    }
}
