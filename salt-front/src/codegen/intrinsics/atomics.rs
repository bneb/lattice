use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

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
                "    {} = \"llvm.cmpxchg\"({}, {}, {}) {{\
                    success_ordering = 5 : i64, \
                    failure_ordering = 2 : i64\
                }} : (!llvm.ptr, !llvm.ptr, !llvm.ptr) -> !llvm.struct<(!llvm.ptr, i1)>\n",
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
            
            let addr_val = if matches!(addr_ty, Type::I64 | Type::U64 | Type::Usize) {
                let ptr_cast = format!("%add_ptr_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", ptr_cast, raw_addr_val));
                ptr_cast
            } else {
                raw_addr_val
            };
            
            let (delta_val, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;

            let res = format!("%atomic_add_{}", ctx.next_id());
            out.push_str(&format!(
                "    {} = \"llvm.atomicrmw\"({}, {}) {{\
                    bin_op = 1 : i64, \
                    ordering = 5 : i64\
                }} : (!llvm.ptr, i64) -> i64\n",
                res, addr_val, delta_val
            ));
            Ok(Some((res, Type::I64)))
        }
        "salt_atomic_cas_i64" | "atomic_cas_i64" | "keuos__atomic_cas_i64" => {
            if args.len() != 3 {
                return Err("atomic_cas_i64 expects 3 arguments: (addr, expected, desired)".to_string());
            }
            let (raw_addr_val, addr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            
            let addr_val = if matches!(addr_ty, Type::I64 | Type::U64 | Type::Usize) {
                let ptr_cast = format!("%cas_ptr_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", ptr_cast, raw_addr_val));
                ptr_cast
            } else {
                raw_addr_val
            };
            
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
            let ptr_var = if matches!(ptr_ty, Type::I64 | Type::U64 | Type::Usize) {
                let ptr_cast = format!("%load_ptr_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", ptr_cast, raw_ptr_var));
                ptr_cast
            } else {
                raw_ptr_var
            };
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
            let ptr_var = if matches!(ptr_ty, Type::I64 | Type::U64 | Type::Usize) {
                let ptr_cast = format!("%store_ptr_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", ptr_cast, raw_ptr_var));
                ptr_cast
            } else {
                raw_ptr_var
            };
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
            let (raw_addr_val, addr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            
            let addr_val = if matches!(addr_ty, Type::I64 | Type::U64 | Type::Usize) {
                let ptr_cast = format!("%cas128_ptr_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", ptr_cast, raw_addr_val));
                ptr_cast
            } else {
                raw_addr_val
            };

            let (exp_lo, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
            let (exp_hi, _) = emit_expr(ctx, out, &args[2], local_vars, Some(&Type::I64))?;
            let (des_lo, _) = emit_expr(ctx, out, &args[3], local_vars, Some(&Type::I64))?;
            let (des_hi, _) = emit_expr(ctx, out, &args[4], local_vars, Some(&Type::I64))?;

            let exp_lo_128 = format!("%cas128_exp_lo_{}", ctx.next_id());
            let exp_hi_128 = format!("%cas128_exp_hi_{}", ctx.next_id());
            let exp_hi_shift = format!("%cas128_exp_shift_{}", ctx.next_id());
            let exp_128 = format!("%cas128_exp_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i64 to i128\n", exp_lo_128, exp_lo));
            out.push_str(&format!("    {} = arith.extui {} : i64 to i128\n", exp_hi_128, exp_hi));
            let shift_const = format!("%cas128_c64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 64 : i128\n", shift_const));
            out.push_str(&format!("    {} = arith.shli {}, {} : i128\n", exp_hi_shift, exp_hi_128, shift_const));
            out.push_str(&format!("    {} = arith.ori {}, {} : i128\n", exp_128, exp_lo_128, exp_hi_shift));

            let des_lo_128 = format!("%cas128_des_lo_{}", ctx.next_id());
            let des_hi_128 = format!("%cas128_des_hi_{}", ctx.next_id());
            let des_hi_shift = format!("%cas128_des_shift_{}", ctx.next_id());
            let des_128 = format!("%cas128_des_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i64 to i128\n", des_lo_128, des_lo));
            out.push_str(&format!("    {} = arith.extui {} : i64 to i128\n", des_hi_128, des_hi));
            let shift_const2 = format!("%cas128_c64b_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 64 : i128\n", shift_const2));
            out.push_str(&format!("    {} = arith.shli {}, {} : i128\n", des_hi_shift, des_hi_128, shift_const2));
            out.push_str(&format!("    {} = arith.ori {}, {} : i128\n", des_128, des_lo_128, des_hi_shift));

            let cas_res = format!("%cas128_res_{}", ctx.next_id());
            let res_struct_ty = "!llvm.struct<(i128, i1)>";
            out.push_str(&format!(
                "    {} = \"llvm.cmpxchg\"({}, {}, {}) {{success_ordering = 5 : i64, failure_ordering = 2 : i64}} : (!llvm.ptr, i128, i128) -> {}\n",
                cas_res, addr_val, exp_128, des_128, res_struct_ty
            ));

            let cas_val_128 = format!("%cas128_val_{}", ctx.next_id());
            let cas_success = format!("%cas128_succ_{}", ctx.next_id());
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
        "cmpxchg" => {
            if args.len() != 3 {
                return Err("Intrinsic 'cmpxchg' expects 3 arguments: (ptr, cmp, new)".to_string());
            }
            let (ptr, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let (cmp_val, cmp_ty) = emit_expr(ctx, out, &args[1], local_vars, None)?;
            let (new_val, _) = emit_expr(ctx, out, &args[2], local_vars, Some(&cmp_ty))?;
            
            let val_ty = cmp_ty.to_mlir_type(ctx)?;
            let res_struct_ty = format!("!llvm.struct<({}, i1)>", val_ty);
            let res_var = format!("%cmpxchg_res_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.cmpxchg {}, {}, {} acq_rel acquire : !llvm.ptr, {}\n", 
                res_var, ptr, cmp_val, new_val, val_ty));
            
            let tuple_ty = Type::Tuple(vec![cmp_ty.clone(), Type::Bool]);
            let tuple_mlir_ty = tuple_ty.to_mlir_type(ctx)?; 
            let val_extracted = format!("%cx_val_{}", ctx.next_id());
            let success_extracted = format!("%cx_succ_{}", ctx.next_id());
            ctx.emit_extractvalue(out, &val_extracted, &res_var, 0, &res_struct_ty);
            ctx.emit_extractvalue(out, &success_extracted, &res_var, 1, &res_struct_ty);
            
            let final_tuple = format!("%cx_tuple_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", final_tuple, tuple_mlir_ty));
            let tuple_step1 = format!("%cx_t1_{}", ctx.next_id());
            ctx.emit_insertvalue(out, &tuple_step1, &val_extracted, &final_tuple, 0, &tuple_mlir_ty);
            let tuple_step2 = format!("%cx_t2_{}", ctx.next_id());
            ctx.emit_insertvalue(out, &tuple_step2, &success_extracted, &tuple_step1, 1, &tuple_mlir_ty);
            Ok(Some((tuple_step2, tuple_ty)))
        }
        _ => Ok(None),
    }
}
