use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

fn coerce_int_to_ptr(ctx: &mut LoweringContext, out: &mut String, raw_val: &str, ty: &Type) -> String {
    if matches!(ty, Type::I64 | Type::U64 | Type::Usize) {
        let ptr_cast = format!("%ptr_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", ptr_cast, raw_val));
        ptr_cast
    } else {
        raw_val.to_string()
    }
}

fn emit_i128_from_lo_hi(ctx: &mut LoweringContext, out: &mut String, lo: &str, hi: &str) -> String {
    let lo_128 = format!("%i128_lo_{}", ctx.next_id());
    let hi_128 = format!("%i128_hi_{}", ctx.next_id());
    let hi_shift = format!("%i128_shift_{}", ctx.next_id());
    let result = format!("%i128_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.extui {} : i64 to i128\n", lo_128, lo));
    out.push_str(&format!("    {} = arith.extui {} : i64 to i128\n", hi_128, hi));
    let shift_c = format!("%i128_c64_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.constant 64 : i128\n", shift_c));
    out.push_str(&format!("    {} = arith.shli {}, {} : i128\n", hi_shift, hi_128, shift_c));
    out.push_str(&format!("    {} = arith.ori {}, {} : i128\n", result, lo_128, hi_shift));
    result
}

fn emit_cmpxchg_tuple(
    ctx: &mut LoweringContext,
    out: &mut String,
    ptr: &str,
    cmp_val: &str,
    new_val: &str,
    cmp_ty: &Type,
    val_ty_str: &str,
) -> Result<(String, Type), String> {
    let res_struct_ty = format!("!llvm.struct<({}, i1)>", val_ty_str);
    let res_var = format!("%cmpxchg_res_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.cmpxchg {}, {}, {} acq_rel acquire : !llvm.ptr, {}\n",
        res_var, ptr, cmp_val, new_val, val_ty_str));
    let tuple_ty = Type::Tuple(vec![cmp_ty.clone(), Type::Bool]);
    let tuple_mlir_ty = tuple_ty.to_mlir_type(ctx)?;
    let val_ext = format!("%cx_val_{}", ctx.next_id());
    let succ_ext = format!("%cx_succ_{}", ctx.next_id());
    ctx.emit_extractvalue(out, &val_ext, &res_var, 0, &res_struct_ty);
    ctx.emit_extractvalue(out, &succ_ext, &res_var, 1, &res_struct_ty);
    let final_tup = format!("%cx_tuple_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", final_tup, tuple_mlir_ty));
    let t1 = format!("%cx_t1_{}", ctx.next_id());
    ctx.emit_insertvalue(out, &t1, &val_ext, &final_tup, 0, &tuple_mlir_ty);
    let t2 = format!("%cx_t2_{}", ctx.next_id());
    ctx.emit_insertvalue(out, &t2, &succ_ext, &t1, 1, &tuple_mlir_ty);
    Ok((t2, tuple_ty))
}

fn emit_atomic_cas_128(
    ctx: &mut LoweringContext,
    out: &mut String,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<Option<(String, Type)>, String> {
    let (raw_addr_val, addr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
    let addr_val = coerce_int_to_ptr(ctx, out, &raw_addr_val, &addr_ty);
    let (exp_lo, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
    let (exp_hi, _) = emit_expr(ctx, out, &args[2], local_vars, Some(&Type::I64))?;
    let (des_lo, _) = emit_expr(ctx, out, &args[3], local_vars, Some(&Type::I64))?;
    let (des_hi, _) = emit_expr(ctx, out, &args[4], local_vars, Some(&Type::I64))?;
    let exp_128 = emit_i128_from_lo_hi(ctx, out, &exp_lo, &exp_hi);
    let des_128 = emit_i128_from_lo_hi(ctx, out, &des_lo, &des_hi);
    let cas_res = format!("%cas128_res_{}", ctx.next_id());
    out.push_str(&format!(
        "    {} = llvm.cmpxchg {}, {}, {} acq_rel acquire : !llvm.ptr, i128\n",
        cas_res, addr_val, exp_128, des_128
    ));
    let cas_val_128 = format!("%cas128_val_{}", ctx.next_id());
    let cas_success = format!("%cas128_succ_{}", ctx.next_id());
    let res_struct_ty = "!llvm.struct<(i128, i1)>";
    out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", cas_val_128, cas_res, res_struct_ty));
    out.push_str(&format!("    {} = llvm.extractvalue {}[1] : {}\n", cas_success, cas_res, res_struct_ty));
    let cas_lo = format!("%cas128_lo_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.trunci {} : i128 to i64\n", cas_lo, cas_val_128));
    let shift_c64 = format!("%cas128_shr64_{}", ctx.next_id());
    let cas_hi_128 = format!("%cas128_hi128_{}", ctx.next_id());
    let cas_hi = format!("%cas128_hi_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.constant 64 : i128\n", shift_c64));
    out.push_str(&format!("    {} = arith.shrui {}, {} : i128\n", cas_hi_128, cas_val_128, shift_c64));
    out.push_str(&format!("    {} = arith.trunci {} : i128 to i64\n", cas_hi, cas_hi_128));
    let tuple_ty = Type::Tuple(vec![Type::U64, Type::U64, Type::Bool]);
    let tuple_mlir_ty = tuple_ty.to_mlir_type(ctx)?;
    let tuple_undef = format!("%cas128_tup_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", tuple_undef, tuple_mlir_ty));
    let tuple_s1 = format!("%cas128_t1_{}", ctx.next_id());
    ctx.emit_insertvalue(out, &tuple_s1, &cas_lo, &tuple_undef, 0, &tuple_mlir_ty);
    let tuple_s2 = format!("%cas128_t2_{}", ctx.next_id());
    ctx.emit_insertvalue(out, &tuple_s2, &cas_hi, &tuple_s1, 1, &tuple_mlir_ty);
    let tuple_s3 = format!("%cas128_t3_{}", ctx.next_id());
    ctx.emit_insertvalue(out, &tuple_s3, &cas_success, &tuple_s2, 2, &tuple_mlir_ty);
    Ok(Some((tuple_s3, tuple_ty)))
}

pub fn emit_atomic_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    _expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    match name {
        "cycle_counter" | "keuos__cycle_counter" => {
            if !args.is_empty() {
                return Err("cycle_counter() takes no arguments".to_string());
            }
            let res = format!("%cycles_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.readcyclecounter\"() : () -> i64\n", res));
            Ok(Some((res, Type::I64)))
        }
        "atomic_cas_ptr" | "keuos__atomic_cas_ptr" => {
            if args.len() != 3 {
                return Err("atomic_cas_ptr expects 3 arguments: (addr, old, new)".to_string());
            }
            let (addr_val, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let (old_val, _) = emit_expr(ctx, out, &args[1], local_vars, None)?;
            let (new_val, _) = emit_expr(ctx, out, &args[2], local_vars, None)?;
            let cas_res = format!("%cas_res_{}", ctx.next_id());
            let cas_val = format!("%cas_val_{}", ctx.next_id());
            out.push_str(&format!(
                "    {} = \"llvm.cmpxchg\"({}, {}, {}) {{success_ordering = 5 : i64, failure_ordering = 2 : i64}} : (!llvm.ptr, !llvm.ptr, !llvm.ptr) -> !llvm.struct<(!llvm.ptr, i1)>\n",
                cas_res, addr_val, old_val, new_val
            ));
            out.push_str(&format!(
                "    {} = llvm.extractvalue {}[0] : !llvm.struct<(!llvm.ptr, i1)>\n",
                cas_val, cas_res
            ));
            Ok(Some((cas_val, Type::Pointer {
                element: Box::new(Type::I8),
                provenance: crate::types::Provenance::Naked,
                is_mutable: true,
            })))
        }
        "atomic_add_i64" | "keuos__atomic_add_i64" => {
            if args.len() != 2 {
                return Err("atomic_add_i64 expects 2 arguments: (addr, delta)".to_string());
            }
            let (raw_addr_val, addr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let addr_val = coerce_int_to_ptr(ctx, out, &raw_addr_val, &addr_ty);
            let (delta_val, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
            let res = format!("%atomic_add_{}", ctx.next_id());
            out.push_str(&format!(
                "    {} = \"llvm.atomicrmw\"({}, {}) {{bin_op = 1 : i64, ordering = 5 : i64}} : (!llvm.ptr, i64) -> i64\n",
                res, addr_val, delta_val
            ));
            Ok(Some((res, Type::I64)))
        }
        "salt_atomic_cas_i64" | "atomic_cas_i64" | "keuos__atomic_cas_i64" => {
            if args.len() != 3 {
                return Err("atomic_cas_i64 expects 3 arguments: (addr, expected, desired)".to_string());
            }
            let (raw_addr_val, addr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let addr_val = coerce_int_to_ptr(ctx, out, &raw_addr_val, &addr_ty);
            let (old_val, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
            let (new_val, _) = emit_expr(ctx, out, &args[2], local_vars, Some(&Type::I64))?;
            let cas_res = format!("%cas_res_{}", ctx.next_id());
            let cas_val = format!("%cas_val_{}", ctx.next_id());
            out.push_str(&format!(
                "    {} = \"llvm.cmpxchg\"({}, {}, {}) {{success_ordering = 5 : i64, failure_ordering = 2 : i64}} : (!llvm.ptr, i64, i64) -> !llvm.struct<(i64, i1)>\n",
                cas_res, addr_val, old_val, new_val
            ));
            out.push_str(&format!(
                "    {} = llvm.extractvalue {}[0] : !llvm.struct<(i64, i1)>\n",
                cas_val, cas_res
            ));
            Ok(Some((cas_val, Type::I64)))
        }
        "atomic_load_i64" | "keuos__atomic_load_i64" => {
            if args.len() != 1 {
                return Err("Intrinsic 'atomic_load_i64' expects 1 argument (ptr)".to_string());
            }
            let (raw_ptr_var, ptr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let ptr_var = coerce_int_to_ptr(ctx, out, &raw_ptr_var, &ptr_ty);
            let res = format!("%atomic_load_{}", ctx.next_id());
            out.push_str(&format!(
                "    {} = \"llvm.load\"({}) {{alignment = 8 : i64, ordering = 4 : i64}} : (!llvm.ptr) -> i64\n",
                res, ptr_var
            ));
            Ok(Some((res, Type::I64)))
        }
        "atomic_store_i64" | "keuos__atomic_store_i64" => {
            if args.len() != 2 {
                return Err("Intrinsic 'atomic_store_i64' expects 2 arguments (ptr, val)".to_string());
            }
            let (raw_ptr_var, ptr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let ptr_var = coerce_int_to_ptr(ctx, out, &raw_ptr_var, &ptr_ty);
            let (val_var, _) = emit_expr(ctx, out, &args[1], local_vars, None)?;
            out.push_str(&format!(
                "    \"llvm.store\"({}, {}) {{alignment = 8 : i64, ordering = 5 : i64}} : (i64, !llvm.ptr) -> ()\n",
                val_var, ptr_var
            ));
            Ok(Some(("".to_string(), Type::Unit)))
        }
        "atomic_cas_128" | "keuos__atomic_cas_128" => {
            if args.len() != 5 {
                return Err("atomic_cas_128 expects 5 arguments: (addr, exp_lo, exp_hi, des_lo, des_hi)".to_string());
            }
            emit_atomic_cas_128(ctx, out, args, local_vars)
        }
        "cmpxchg" => {
            if args.len() != 3 {
                return Err("Intrinsic 'cmpxchg' expects 3 arguments: (ptr, cmp, new)".to_string());
            }
            let (ptr, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let (cmp_val, cmp_ty) = emit_expr(ctx, out, &args[1], local_vars, None)?;
            let (new_val, _) = emit_expr(ctx, out, &args[2], local_vars, Some(&cmp_ty))?;
            let val_ty_str = cmp_ty.to_mlir_type(ctx)?;
            Ok(Some(emit_cmpxchg_tuple(ctx, out, &ptr, &cmp_val, &new_val, &cmp_ty, &val_ty_str)?))
        }
        _ => Ok(None),
    }
}
