use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::codegen::type_casts::cast_numeric;

pub fn promote_numeric(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<String, String> {
    if from == to { return Ok(var.to_string()); }

    // Auto-deref: unwrap &T to T before promotion.
    // Loads the value through the reference so the inner type
    // can be promoted normally. Skipped when the target is also a
    // reference — ref-to-ref stays pointer-level (handled in the cast path).
    if let Type::Reference(inner, _) = from {
        if !matches!(to, Type::Reference(..)) && !to.k_is_ptr_type() {
            let loaded = format!("%deref_prom_{}", ctx.next_id());
            let mlir_ty = inner.to_mlir_type(ctx)?;
            ctx.emit_load(out, &loaded, var, &mlir_ty);
            return promote_numeric(ctx, out, &loaded, inner, to);
        }
    }

    if let Some(res) = promote_numeric_linear(ctx, out, var, from, to)? {
        return Ok(res);
    }

    if from.is_integer() && to.k_is_ptr_type() {
        return Err(format!(
            "KeuOS Type Error: Cannot promote integer {:?} to pointer {:?}. var={} - This indicates Context Contamination in the loop engine.", 
            from, to, var
        ));
    }

    promote_numeric_cast(ctx, out, var, from, to)
}

fn promote_numeric_linear(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<Option<String>, String> {
    if let Type::Owned(inner) = to {
        if **inner == *from { 
            let temp_ptr = format!("%auto_box_{}", ctx.next_id());
            let mlir_ty = inner.to_mlir_storage_type(ctx).map_err(|e| format!("Failed to get storage type for auto-box: {}", e))?;
            ctx.emit_alloca(out, &temp_ptr, &mlir_ty);
            ctx.emit_store(out, var, &temp_ptr, &mlir_ty);
            return Ok(Some(temp_ptr));
        }
    }
    if let Type::Reference(inner, _) = to {
         if inner.structural_eq(from) {
             let temp_ptr = format!("%auto_ref_{}", ctx.next_id());
             let mlir_ty = from.to_mlir_storage_type(ctx).map_err(|e| format!("Auto-ref storage type error: {}", e))?;
             ctx.emit_alloca(out, &temp_ptr, &mlir_ty);
             ctx.emit_store(out, var, &temp_ptr, &mlir_ty);
             return Ok(Some(temp_ptr));
         }
    }
    if let Type::Owned(inner) = from {
        if **inner == *to { 
            let val_res = format!("%auto_unbox_{}", ctx.next_id());
            let mlir_ty = to.to_mlir_storage_type(ctx).map_err(|e| format!("Failed to get storage type for auto-unbox: {}", e))?;
            ctx.emit_load(out, &val_res, var, &mlir_ty);
            return Ok(Some(val_res));
        }
    }
    if from.structural_eq(to) {
        return Ok(Some(var.to_string()));
    }
    
    match (from, to) {
        (Type::Struct(n1), Type::Concrete(n2, _)) | (Type::Concrete(n2, _), Type::Struct(n1)) => {
            if Type::base_names_equal(n1, n2) { return Ok(Some(var.to_string())); }
        },
        (Type::Concrete(n1, args1), Type::Concrete(n2, args2)) => {
            if Type::base_names_equal(n1, n2) && args1.len() == args2.len() { return Ok(Some(var.to_string())); }
        },
        _ => {}
    }

    if matches!(from, Type::Fn(_, _)) && matches!(to, Type::I64 | Type::U64) {
        let res = format!("%fn_to_int_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
        return Ok(Some(res));
    }

    let is_stringview_from = match from {
        Type::Struct(name) | Type::Concrete(name, _) => name.contains("StringView"),
        _ => false,
    };
    if is_stringview_from && (to.k_is_ptr_type() || matches!(to, Type::Reference(..))) {
        let res = format!("%sv_extract_ptr_{}", ctx.next_id());
        let sv_mlir = from.to_mlir_type(ctx).unwrap_or("!llvm.struct<(ptr, i64)>".to_string());
        out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", res, var, sv_mlir));
        return Ok(Some(res));
    }
    Ok(None)
}

fn promote_numeric_cast(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<String, String> {
    let res = format!("%prom_{}", ctx.next_id());
    let mut emit = |op: &str, src_ty: &str, dst_ty: &str| {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, var, src_ty, dst_ty));
    };

    match (from, to) {
        (Type::Never, _) => {
             let dst_ty_mlir = to.to_mlir_type(ctx)?;
             out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", res, dst_ty_mlir));
             return Ok(res);
        },
        (Type::I8, Type::U8) | (Type::U8, Type::I8) | (Type::I8, Type::I8) | (Type::U8, Type::U8) => return Ok(var.to_string()),
        (Type::I16, Type::U16) | (Type::U16, Type::I16) | (Type::I16, Type::I16) | (Type::U16, Type::U16) => return Ok(var.to_string()),
        (Type::I32, Type::U32) | (Type::U32, Type::I32) | (Type::I32, Type::I32) | (Type::U32, Type::U32) => return Ok(var.to_string()),
        (Type::I64, Type::U64) | (Type::U64, Type::I64) | (Type::I64, Type::I64) | (Type::U64, Type::U64) | (Type::Usize, Type::Usize) => return Ok(var.to_string()),
        
        (Type::Usize, Type::I64) | (Type::Usize, Type::U64) => {
            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", res, var));
            return Ok(res);
        },
        (Type::I64, Type::Usize) | (Type::U64, Type::Usize) => {
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, var));
            return Ok(res);
        },
        
        (Type::I32, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi {} : i32 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::U32, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i32 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::I16, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi {} : i16 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::U16, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i16 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::I8, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi {} : i8 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::U8, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i8 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        
        (Type::Array(from_inner, f_len, false), Type::Array(to_inner, t_len, true)) 
            if f_len == t_len && **from_inner == Type::Bool && **to_inner == Type::Bool => {
             return promote_array_packing(ctx, out, var, *f_len, to);
        },
        (from, to) if from.is_integer() && to.is_integer() => {
             if *from == Type::Usize {
                 let intermediate = format!("%idx_i64_{}", ctx.next_id());
                 out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", intermediate, var));
                 let dst_width = get_bit_width(to);
                 if dst_width < 64 {
                     out.push_str(&format!("    {} = arith.trunci {} : i64 to {}\n", res, intermediate, to.to_mlir_type(ctx)?));
                     return Ok(res);
                 } else {
                     return Ok(intermediate);
                 }
             }
             let src_width = get_bit_width(from);
             let dst_width = get_bit_width(to);
             if src_width == dst_width {
                 return Ok(var.to_string());
             } else if src_width > dst_width {
                 emit("arith.trunci", &from.to_mlir_type(ctx)?, &to.to_mlir_type(ctx)?);
                 return Ok(res);
             } else {
                 let op = if from.is_unsigned() { "arith.extui" } else { "arith.extsi" };
                 emit(op, &from.to_mlir_type(ctx)?, &to.to_mlir_type(ctx)?);
                 return Ok(res);
             }
        },
        (from, to) if from.is_integer() && to.is_float() => {
             let op = if from.is_unsigned() { "arith.uitofp" } else { "arith.sitofp" };
             let src_str = from.to_mlir_type(ctx)?;
             let dst_str = to.to_mlir_type(ctx)?;
             emit(op, &src_str, &dst_str);
             return Ok(res);
        },
        (Type::F32, Type::F64) => { emit("arith.extf", "f32", "f64"); return Ok(res); },
        (Type::F64, Type::F32) => { emit("arith.truncf", "f64", "f32"); return Ok(res); },
        
        (Type::Reference(_, _), Type::Reference(_, _)) => return Ok(var.to_string()),
        
        (Type::Reference(inner_from, _), to) if inner_from.as_ref() == to => {
            let mlir_to = to.to_mlir_type(ctx)?;
            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, var, mlir_to));
            return Ok(res);
        },
        
        (Type::F32, Type::Bool) => {
             out.push_str("    %cst_0_f32 = arith.constant 0.0 : f32\n");
             out.push_str(&format!("    {} = arith.cmpf \"une\", {}, %cst_0_f32 : f32\n", res, var));
             return Ok(res);
        }
        (Type::F64, Type::Bool) => {
             out.push_str("    %cst_0_f64 = arith.constant 0.0 : f64\n");
             out.push_str(&format!("    {} = arith.cmpf \"une\", {}, %cst_0_f64 : f64\n", res, var));
             return Ok(res);
        }
        (from, Type::Bool) if from.is_integer() => {
             let zero = format!("%c0_{}", ctx.next_id());
             let mlir_from = from.to_mlir_type(ctx)?;
             ctx.emit_const_int(out, &zero, 0, &mlir_from);
             out.push_str(&format!("    {} = arith.cmpi \"ne\", {}, {} : {}\n", res, var, zero, mlir_from));
             return Ok(res);
        },
        (Type::Bool, to) if to.is_integer() => {
             let dst_ty = to.to_mlir_type(ctx)?;
             emit("arith.extui", "i1", &dst_ty);
             return Ok(res);
        }
        (Type::Tuple(fs), Type::Tuple(ts)) if fs.len() == ts.len() => {
             return promote_tuple(ctx, out, var, from, to, &res);
        }
        _ => {}
    }

    promote_numeric_fallback(ctx, out, var, from, to, &res)
}

fn promote_numeric_fallback(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type, res: &str) -> Result<String, String> {
    let mut emit = |op: &str, src_ty: &str, dst_ty: &str| {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, var, src_ty, dst_ty));
    };

    if let (Some(f_idx), Some(t_idx)) = (get_numeric_idx(from), get_numeric_idx(to)) {
        if let Some((op, src_ty, dst_ty)) = PROMOTION_OPS[f_idx][t_idx] {
            emit(op, src_ty, dst_ty);
            return Ok(res.to_string());
        }
    }

    if from.canonical_eq(to) {
        return Ok(var.to_string());
    }

    if let (Ok(mlir_from), Ok(mlir_to)) = (from.to_mlir_type(ctx), to.to_mlir_type(ctx)) {
        if mlir_from == mlir_to {
             let registry = ctx.struct_registry();
             if from.size_of(registry) == to.size_of(registry) {
                 return Ok(var.to_string());
             }
        }
    }

    match (from, to) {
        (Type::Struct(n), Type::Concrete(..)) | (Type::Concrete(..), Type::Struct(n)) => {
            let other = if matches!(from, Type::Struct(_)) { to } else { from };
            fn normalize_fqn(s: &str) -> String {
                let protected = s.replace("__", "\x01");
                let parts: Vec<&str> = protected.split('_').collect();
                let normalized: Vec<String> = parts.iter().map(|part| {
                    let restored = part.replace('\x01', "__");
                    if restored.contains("__") {
                        restored.rsplit("__").next().unwrap_or(&restored).to_string()
                    } else {
                        restored
                    }
                }).collect();
                normalized.join("_")
            }
            let n_norm = normalize_fqn(n);
            let other_norm = normalize_fqn(&other.mangle_suffix());
            if n_norm == other_norm {
                return Ok(var.to_string());
            }
        }
        _ => {}
    }

    if from.k_is_ptr_type() && to.k_is_ptr_type() {
        return Ok(var.to_string());
    }

    Err(format!("Numeric promotion not supported from {:?} to {:?} (var: {}) in function '{}'", from, to, var, ctx.current_fn_name()))
}

fn promote_array_packing(ctx: &mut LoweringContext, out: &mut String, var: &str, f_len: usize, to: &Type) -> Result<String, String> {
     let packed_storage_ty = to.to_mlir_storage_type(ctx)?;
     let mut current_packed = format!("%packed_prom_{}", ctx.next_id());
     out.push_str(&format!("    {} = llvm.mlir.zero : {}\n", current_packed, packed_storage_ty));
     
     let unpacked_storage_ty_str = format!("!llvm.array<{} x i1>", f_len);
     
     let mut current_word_ssa = String::new();
     for i in 0..f_len {
         let bit_idx = i % 64;
         if bit_idx == 0 {
             let zero = format!("%zero_w_{}", ctx.next_id());
             ctx.emit_const_int(out, &zero, 0, "i64");
             current_word_ssa = zero;
         }
         
         let elem = format!("%elem_{}_{}", i, ctx.next_id());
         out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", elem, var, i, unpacked_storage_ty_str));
         
         let elem_ext = format!("%elem_ext_{}", ctx.next_id());
         ctx.emit_cast(out, &elem_ext, "arith.extui", &elem, "i8", "i64");
         
         let shifted = format!("%shifted_{}", ctx.next_id());
         let shift_amt = format!("%sh_amt_{}", ctx.next_id());
         ctx.emit_const_int(out, &shift_amt, bit_idx as i64, "i64");
         ctx.emit_binop(out, &shifted, "arith.shli", &elem_ext, &shift_amt, "i64");
         
         let new_word = format!("%accum_w_{}_{}", i, ctx.next_id());
         ctx.emit_binop(out, &new_word, "arith.ori", &current_word_ssa, &shifted, "i64");
         current_word_ssa = new_word;
         
         if bit_idx == 63 || i == f_len - 1 {
             let word_idx = i / 64;
             let inserted = format!("%packed_insert_{}", ctx.next_id());
             out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", inserted, current_word_ssa, current_packed, word_idx, packed_storage_ty));
             current_packed = inserted;
         }
     }
     Ok(current_packed)
}
fn promote_tuple(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type, res: &str) -> Result<String, String> {
     let (fs, ts) = match (from, to) { (Type::Tuple(f), Type::Tuple(t)) => (f, t), _ => return Err("promote_tuple requires tuple types".to_string()) };
     let target_mlir = to.to_mlir_storage_type(ctx)?;
     let src_mlir = from.to_mlir_storage_type(ctx)?;
     
     let first_init = format!("{}_init", res.replace("%", ""));
     out.push_str(&format!("    %{} = llvm.mlir.undef : {}\n", first_init, target_mlir));
     
     let mut current_struct_ssa = format!("%{}", first_init);
     
     for (i, (f_ty, t_ty)) in fs.iter().zip(ts.iter()).enumerate() {
        let elem_val = format!("%{}_elem_{}", res.replace("%", ""), i);
         ctx.emit_extractvalue(out, &elem_val, var, i, &src_mlir);
         
         let prom_elem = match promote_numeric(ctx, out, &elem_val, f_ty, t_ty) {
             Ok(r) => r,
             Err(_) => cast_numeric(ctx, out, &elem_val, f_ty, t_ty)?
         };
         
         let target_name = if i == fs.len() - 1 {
             res.to_string()
         } else {
             format!("{}_chain_{}", res, i)
         };
         
         out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", 
             target_name, prom_elem, current_struct_ssa, i, target_mlir));
         
         current_struct_ssa = target_name;
     }
     Ok(res.to_string())
}

pub(crate) fn get_bit_width(ty: &Type) -> u32 {
    match ty {
        Type::Bool | Type::I8 | Type::U8 => 8,
        Type::I16 | Type::U16 => 16,
        Type::I32 | Type::U32 | Type::F32 => 32,
        Type::I64 | Type::U64 | Type::Usize | Type::F64 => 64,
        _ => 0
    }
}

pub fn get_numeric_idx(ty: &Type) -> Option<usize> {
    match ty {
        Type::I8 => Some(0),
        Type::I16 => Some(1),
        Type::I32 => Some(2),
        Type::I64 => Some(3),
        Type::U8 => Some(4),
        Type::U16 => Some(5),
        Type::U32 => Some(6),
        Type::U64 => Some(7),
        Type::Usize => Some(8),
        Type::F32 => Some(9),
        Type::F64 => Some(10),
        Type::Bool => Some(11),
        _ => None
    }
}

pub type PromotionTable = [[Option<(&'static str, &'static str, &'static str)>; 12]; 12];
pub const PROMOTION_OPS: PromotionTable = {
    let mut table = [[None; 12]; 12];

    // I32 -> I64/U64/Usize
    table[2][3] = Some(("arith.extsi", "i32", "i64"));
    table[2][7] = Some(("arith.extsi", "i32", "i64"));
    table[2][8] = Some(("arith.extsi", "i32", "i64"));

    // I16 -> I32/U32
    table[1][2] = Some(("arith.extsi", "i16", "i32"));
    table[1][6] = Some(("arith.extsi", "i16", "i32"));
    // I16 -> I64/U64/Usize
    table[1][3] = Some(("arith.extsi", "i16", "i64"));
    table[1][7] = Some(("arith.extsi", "i16", "i64"));
    table[1][8] = Some(("arith.extsi", "i16", "i64"));

    // I8 -> I16/U16
    table[0][1] = Some(("arith.extsi", "i8", "i16"));
    table[0][5] = Some(("arith.extsi", "i8", "i16"));
    // I8 -> I32/U32
    table[0][2] = Some(("arith.extsi", "i8", "i32"));
    table[0][6] = Some(("arith.extsi", "i8", "i32"));
    // I8 -> I64/U64/Usize
    table[0][3] = Some(("arith.extsi", "i8", "i64"));
    table[0][7] = Some(("arith.extsi", "i8", "i64"));
    table[0][8] = Some(("arith.extsi", "i8", "i64"));

    // U32 -> I64/U64/Usize
    table[6][3] = Some(("arith.extui", "i32", "i64"));
    table[6][7] = Some(("arith.extui", "i32", "i64"));
    table[6][8] = Some(("arith.extui", "i32", "i64"));

    // U16 -> I32/U32
    table[5][2] = Some(("arith.extui", "i16", "i32"));
    table[5][6] = Some(("arith.extui", "i16", "i32"));
    // U16 -> I64/U64/Usize
    table[5][3] = Some(("arith.extui", "i16", "i64"));
    table[5][7] = Some(("arith.extui", "i16", "i64"));
    table[5][8] = Some(("arith.extui", "i16", "i64"));

    // U8 -> I16/U16
    table[4][1] = Some(("arith.extui", "i8", "i16"));
    table[4][5] = Some(("arith.extui", "i8", "i16"));
    // U8 -> I32/U32
    table[4][2] = Some(("arith.extui", "i8", "i32"));
    table[4][6] = Some(("arith.extui", "i8", "i32"));
    // U8 -> I64/U64/Usize
    table[4][3] = Some(("arith.extui", "i8", "i64"));
    table[4][7] = Some(("arith.extui", "i8", "i64"));
    table[4][8] = Some(("arith.extui", "i8", "i64"));

    // Float promotions
    table[9][10] = Some(("arith.extf", "f32", "f64"));

    table
};

pub fn get_arith_op(op: &syn::BinOp, ty: &Type) -> String {
    let is_float = matches!(ty, Type::F32 | Type::F64);
    let is_unsigned = ty.is_unsigned();
    match op {
        syn::BinOp::Add(_) | syn::BinOp::AddAssign(_) => if is_float { "arith.addf" } else { "arith.addi" }.to_string(),
        syn::BinOp::Sub(_) | syn::BinOp::SubAssign(_) => if is_float { "arith.subf" } else { "arith.subi" }.to_string(),
        syn::BinOp::Mul(_) | syn::BinOp::MulAssign(_) => if is_float { "arith.mulf" } else { "arith.muli" }.to_string(),
        syn::BinOp::Div(_) | syn::BinOp::DivAssign(_) => if is_float { "arith.divf" } else if is_unsigned { "arith.divui" } else { "arith.divsi" }.to_string(),
        syn::BinOp::Rem(_) | syn::BinOp::RemAssign(_) => if is_float { "arith.remf" } else if is_unsigned { "arith.remui" } else { "arith.remsi" }.to_string(),
        syn::BinOp::BitAnd(_) | syn::BinOp::BitAndAssign(_) => "arith.andi".to_string(),
        syn::BinOp::BitOr(_) | syn::BinOp::BitOrAssign(_) => "arith.ori".to_string(),
        syn::BinOp::BitXor(_) | syn::BinOp::BitXorAssign(_) => "arith.xori".to_string(),
        syn::BinOp::Shl(_) | syn::BinOp::ShlAssign(_) => "arith.shli".to_string(),
        syn::BinOp::Shr(_) | syn::BinOp::ShrAssign(_) => if is_unsigned { "arith.shrui" } else { "arith.shrsi" }.to_string(),
        syn::BinOp::And(_) => "arith.andi".to_string(),
        syn::BinOp::Or(_) => "arith.ori".to_string(),
        syn::BinOp::Eq(_) | syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_) | syn::BinOp::Ne(_) => {
            if is_float { "arith.cmpf".to_string() }
            else if matches!(ty, Type::Reference(..) | Type::Owned(..) | Type::Window(..) | Type::Pointer { .. }) { "llvm.icmp".to_string() }
            else { "arith.cmpi".to_string() }
        }
        _ => crate::ice!("Unhandled binary op: {:?}", op),
    }
}

pub fn get_comparison_pred(op: &syn::BinOp, ty: &Type) -> String {
    let is_float = matches!(ty, Type::F32 | Type::F64);
    let is_unsigned = ty.is_unsigned() || matches!(ty, Type::Pointer { .. });
    match op {
        syn::BinOp::Eq(_) => if is_float { "oeq".to_string() } else { "eq".to_string() },
        syn::BinOp::Ne(_) => if is_float { "une".to_string() } else { "ne".to_string() },
        syn::BinOp::Lt(_) => if is_float { "olt" } else if is_unsigned { "ult" } else { "slt" }.to_string(),
        syn::BinOp::Le(_) => if is_float { "ole" } else if is_unsigned { "ule" } else { "sle" }.to_string(),
        syn::BinOp::Gt(_) => if is_float { "ogt" } else if is_unsigned { "ugt" } else { "sgt" }.to_string(),
        syn::BinOp::Ge(_) => if is_float { "oge" } else if is_unsigned { "uge" } else { "sge" }.to_string(),
        _ => "eq".to_string(),
    }
}
