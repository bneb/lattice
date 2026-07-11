use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::codegen::type_bridge;

fn cast_pointer_and_references(
    ctx: &mut LoweringContext,
    out: &mut String,
    var: &str,
    from: &Type,
    to: &Type,
    res: &str,
) -> Result<Option<String>, String> {
    match (from, to) {
        (Type::Reference(_, _), Type::Reference(_, _)) => Ok(Some(var.to_string())),
        (Type::Reference(_, _), Type::Pointer { .. }) => Ok(Some(var.to_string())),
        (Type::U64 | Type::Usize | Type::I64, Type::Pointer { .. }) => {
            let src_ty = from.to_mlir_type(ctx)?;
            let int_val = if src_ty != "i64" {
                let temp = format!("%inttoptr_prep_{}", ctx.next_id());
                if matches!(from, Type::I64) {
                    out.push_str(&format!("    {} = arith.extsi {} : {} to i64\n", temp, var, src_ty));
                } else {
                    out.push_str(&format!("    {} = arith.extui {} : {} to i64\n", temp, var, src_ty));
                }
                temp
            } else {
                var.to_string()
            };
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", res, int_val));
            Ok(Some(res.to_string()))
        }
        (Type::Pointer { .. }, Type::U64 | Type::Usize | Type::I64) => {
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
            Ok(Some(res.to_string()))
        }
        (Type::Array(ref _inner, _, _), Type::Pointer { .. }) => {
            Ok(Some(var.to_string()))
        }
        (Type::Fn(_, _), Type::I64 | Type::U64) => {
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
            Ok(Some(res.to_string()))
        }
        (Type::Fn(_, _), Type::Pointer { .. }) => {
            Ok(Some(var.to_string()))
        }
        (Type::U64 | Type::Usize | Type::I64, Type::Reference(_, _)) => {
            int_to_ptr(ctx, out, var, from, res)
        }
        (Type::Reference(_, _), Type::U64 | Type::Usize | Type::I64) => {
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
            Ok(Some(res.to_string()))
        }
        _ => Ok(None),
    }
}

fn int_to_ptr(
    ctx: &mut LoweringContext,
    out: &mut String,
    var: &str,
    from: &Type,
    res: &str,
) -> Result<Option<String>, String> {
    let src_ty = from.to_mlir_type(ctx)?;
    let int_val = if src_ty != "i64" {
        let temp = format!("%inttoptr_ref_{}", ctx.next_id());
        if matches!(from, Type::Usize) {
            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", temp, var));
        } else {
            out.push_str(&format!("    {} = arith.extsi {} : {} to i64\n", temp, var, src_ty));
        }
        temp
    } else {
        var.to_string()
    };
    out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", res, int_val));
    Ok(Some(res.to_string()))
}

pub fn cast_numeric(
    ctx: &mut LoweringContext,
    out: &mut String,
    var: &str,
    from: &Type,
    to: &Type,
) -> Result<String, String> {
    if from.structural_eq(to) {
        return Ok(var.to_string());
    }

    let w_from = type_bridge::get_bit_width(from);
    let w_to = type_bridge::get_bit_width(to);

    let involves_usize = *from == Type::Usize || *to == Type::Usize;
    if w_from != 0 && w_from == w_to && from.is_integer() && to.is_integer() && !involves_usize {
        return Ok(var.to_string());
    }

    if let Ok(res) = type_bridge::promote_numeric(ctx, out, var, from, to) {
        return Ok(res);
    }

    let res = format!("%cast_{}", ctx.next_id());
    let mut emit = |op: &str, src_ty: &str, dst_ty: &str| {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, var, src_ty, dst_ty));
    };

    match (from, to) {
        (Type::Usize, Type::I32 | Type::U32) => {
            emit_usize_to_int(ctx, out, var, &res, "i32")
        }
        (Type::Usize, Type::I16 | Type::U16) => {
            emit_usize_to_int(ctx, out, var, &res, "i16")
        }
        (Type::Usize, Type::I8 | Type::U8) => {
            emit_usize_to_int(ctx, out, var, &res, "i8")
        }
        (from, to) if from.is_integer() && to.is_integer() && w_from > w_to => {
            let src_str = from.to_mlir_type(ctx)?;
            let dst_str = to.to_mlir_type(ctx)?;
            emit("arith.trunci", &src_str, &dst_str);
            Ok(res)
        }
        (from, to) if from.is_integer() && to.is_integer() && w_from < w_to => {
            let op = if from.is_unsigned() { "arith.extui" } else { "arith.extsi" };
            let src_str = from.to_mlir_type(ctx)?;
            let dst_str = to.to_mlir_type(ctx)?;
            emit(op, &src_str, &dst_str);
            Ok(res)
        }
        (Type::F64, Type::F32) => {
            emit("arith.truncf", "f64", "f32");
            Ok(res)
        }
        (Type::F32 | Type::F64, Type::I8 | Type::I32 | Type::I64) => {
            let src_str = from.to_mlir_type(ctx)?;
            let dst_str = to.to_mlir_type(ctx)?;
            emit("arith.fptosi", &src_str, &dst_str);
            Ok(res)
        }
        (Type::F32 | Type::F64, Type::U8 | Type::U32 | Type::U64 | Type::Usize) => {
            let src_str = from.to_mlir_type(ctx)?;
            let dst_str = to.to_mlir_type(ctx)?;
            emit("arith.fptoui", &src_str, &dst_str);
            Ok(res)
        }
        (Type::I8 | Type::I32 | Type::I64, Type::F32 | Type::F64) => {
            let src_str = from.to_mlir_type(ctx)?;
            let dst_str = to.to_mlir_type(ctx)?;
            emit("arith.sitofp", &src_str, &dst_str);
            Ok(res)
        }
        (Type::U8 | Type::U32 | Type::U64 | Type::Usize, Type::F32 | Type::F64) => {
            let src_str = from.to_mlir_type(ctx)?;
            let dst_str = to.to_mlir_type(ctx)?;
            emit("arith.uitofp", &src_str, &dst_str);
            Ok(res)
        }
        (Type::Struct(_) | Type::Concrete(..), Type::Struct(_) | Type::Concrete(..)) => {
            cast_struct_to_struct(ctx, out, var, from, to, &res)
        }
        _ => {
            if let Some(r) = cast_pointer_and_references(ctx, out, var, from, to, &res)? {
                return Ok(r);
            }
            Err(format!(
                "Unsupported explicit cast {} -> {}",
                from.mangle_suffix(),
                to.mangle_suffix()
            ))
        }
    }
}

fn emit_usize_to_int(
    ctx: &mut LoweringContext,
    out: &mut String,
    var: &str,
    res: &str,
    target: &str,
) -> Result<String, String> {
    let intermediate = format!("%idx_i64_{}", ctx.next_id());
    out.push_str(&format!(
        "    {} = arith.index_cast {} : index to i64\n",
        intermediate, var
    ));
    out.push_str(&format!(
        "    {} = arith.trunci {} : i64 to {}\n",
        res, intermediate, target
    ));
    Ok(res.to_string())
}

fn cast_struct_to_struct(
    ctx: &mut LoweringContext,
    out: &mut String,
    var: &str,
    from: &Type,
    to: &Type,
    res: &str,
) -> Result<String, String> {
    if !type_bridge::prove_layout_compatibility_ctx(ctx, from, to) {
        let struct_registry = ctx.struct_registry();
        let size_from = from.size_of(struct_registry);
        let size_to = to.size_of(struct_registry);
        let align_from = from.align_of(struct_registry);
        let align_to = to.align_of(struct_registry);
        let _ = struct_registry;
        return Err(format!(
            "FORMAL INTEGRITY ERROR: Unsound cast from {} to {}. \
             Layout compatibility could not be proven. \
             Source: size={}, align={}. Target: size={}, align={}.",
            from.mangle_suffix(),
            to.mangle_suffix(),
            size_from,
            align_from,
            size_to,
            align_to
        ));
    }
    let src_ty_mlir = from.to_mlir_storage_type(ctx)?;
    let dst_ty_mlir = to.to_mlir_storage_type(ctx)?;
    out.push_str(&format!(
        "    {} = llvm.bitcast {} : {} to {}\n",
        res, var, src_ty_mlir, dst_ty_mlir
    ));
    Ok(res.to_string())
}
