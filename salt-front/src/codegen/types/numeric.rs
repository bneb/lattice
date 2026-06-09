use crate::types::Type;
use crate::codegen::context::LoweringContext;

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

pub const PROMOTION_OPS: [[Option<(&str, &str, &str)>; 12]; 12] = {
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
    table[9][10] = Some(("arith.extf", "f32", "f64"));   // F32 -> F64

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
        syn::BinOp::And(_) => "arith.andi".to_string(), // Logical and for i1
        syn::BinOp::Or(_) => "arith.ori".to_string(),   // Logical or for i1
        syn::BinOp::Eq(_) | syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_) | syn::BinOp::Ne(_) => {
            if is_float { "arith.cmpf".to_string() } 
            else if matches!(ty, Type::Reference(..) | Type::Owned(..) | Type::Window(..) | Type::Pointer { .. }) { "llvm.icmp".to_string() }
            else { "arith.cmpi".to_string() }
        }
        _ => panic!("Unhandled binary op: {:?}", op),
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

pub fn promote_numeric(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<String, String> {    
    if from == to { return Ok(var.to_string()); }
    println!("IN_CAST: from={:?} to={:?} is_ptr={} is_int={}", from, to, from.k_is_ptr_type(), to.is_integer());
    
    if let Type::Owned(inner) = to {
        if **inner == *from { 
            let temp_ptr = format!("%auto_box_{}", ctx.next_id());
            let mlir_ty = inner.to_mlir_storage_type(ctx).map_err(|e| format!("Failed to get storage type for auto-box: {}", e))?;
            ctx.emit_alloca(out, &temp_ptr, &mlir_ty);
            ctx.emit_store(out, var, &temp_ptr, &mlir_ty);
            return Ok(temp_ptr);
        }
    }
    if let Type::Reference(inner, _) = to {
         if inner.structural_eq(from) {
             let temp_ptr = format!("%auto_ref_{}", ctx.next_id());
             let mlir_ty = from.to_mlir_storage_type(ctx).map_err(|e| format!("Auto-ref storage type error: {}", e))?;
             ctx.emit_alloca(out, &temp_ptr, &mlir_ty);
             ctx.emit_store(out, var, &temp_ptr, &mlir_ty);
             return Ok(temp_ptr);
         }
    }
    if let Type::Owned(inner) = from {
        if **inner == *to { 
            let val_res = format!("%auto_unbox_{}", ctx.next_id());
            let mlir_ty = to.to_mlir_storage_type(ctx).map_err(|e| format!("Failed to get storage type for auto-unbox: {}", e))?;
            ctx.emit_load(out, &val_res, var, &mlir_ty);
            return Ok(val_res);
        }
    }
    if from.structural_eq(to) {
        return Ok(var.to_string());
    }
    
    match (from, to) {
        (Type::Struct(n1), Type::Concrete(n2, _)) |
        (Type::Concrete(n2, _), Type::Struct(n1)) => {
            if Type::base_names_equal(n1, n2) {
                return Ok(var.to_string());
            }
        },
        (Type::Concrete(n1, args1), Type::Concrete(n2, args2)) => {
            if Type::base_names_equal(n1, n2) && args1.len() == args2.len() {
                return Ok(var.to_string());
            }
        },
        _ => {}
    }

    if matches!(from, Type::Fn(_, _)) && matches!(to, Type::I64 | Type::U64) {
        let res = format!("%fn_to_int_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
        return Ok(res);
    }

    {
        let is_stringview_from = match from {
            Type::Struct(name) | Type::Concrete(name, _) => name.contains("StringView"),
            _ => false,
        };
        if is_stringview_from && (to.k_is_ptr_type() || matches!(to, Type::Reference(..))) {
            let res = format!("%sv_extract_ptr_{}", ctx.next_id());
            let sv_mlir = from.to_mlir_type(ctx).unwrap_or("!llvm.struct<(ptr, i64)>".to_string());
            out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", res, var, sv_mlir));
            return Ok(res);
        }
    }

    if from.is_integer() && to.k_is_ptr_type() {
        return Err(format!(
            "KeuOS Type Error: Cannot promote integer {:?} to pointer {:?}. \
             var={} - This indicates Context Contamination in the loop engine.", 
            from, to, var
        ));
    }

    let res = format!("%prom_{}", ctx.next_id());
    let mut emit = |op: &str, src_ty: &str, dst_ty: &str| {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, var, src_ty, dst_ty));
    };

    match (from, to) {
        (Type::Never, _) => {
             let dst_ty_mlir = to.to_mlir_type(ctx).map_err(|e| e)?;
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
        (Type::I32, Type::Usize) | (Type::U32, Type::Usize) | (Type::I16, Type::Usize) | (Type::U16, Type::Usize) | (Type::I8, Type::Usize) | (Type::U8, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            let op = if from.is_unsigned() { "arith.extui" } else { "arith.extsi" };
            let src_mlir = from.to_mlir_type(ctx)?;
            out.push_str(&format!("    {} = {} {} : {} to i64\n", intermediate, op, var, src_mlir));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
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
        (Type::F32, Type::F64) => {
             emit("arith.extf", "f32", "f64");
             return Ok(res);
        },
        (Type::F64, Type::F32) => {
             emit("arith.truncf", "f64", "f32");
             return Ok(res);
        },
        (Type::Reference(inner_from, _), to) if inner_from.as_ref() == to => {
            let mlir_to = to.to_mlir_type(ctx)?;
            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, var, mlir_to));
            return Ok(res);
        },
        (from, Type::Bool) if from.is_integer() || from.is_float() => {
             let mlir_from = from.to_mlir_type(ctx)?;
             if from.is_float() {
                out.push_str(&format!("    %cst_0_{} = arith.constant 0.0 : {}\n", mlir_from, mlir_from));
                out.push_str(&format!("    {} = arith.cmpf \"une\", {}, %cst_0_{} : {}\n", res, var, mlir_from, mlir_from));
             } else {
                let zero = format!("%c0_{}", ctx.next_id());
                ctx.emit_const_int(out, &zero, 0, &mlir_from);
                out.push_str(&format!("    {} = arith.cmpi \"ne\", {}, {} : {}\n", res, var, zero, mlir_from));
             }
             return Ok(res);
        },
        (Type::Bool, to) if to.is_integer() => {
             let dst_ty = to.to_mlir_type(ctx)?;
             emit("arith.extui", "i1", &dst_ty);
             return Ok(res);
        }
        _ => {}
    }

    if let (Some(f_idx), Some(t_idx)) = (get_numeric_idx(from), get_numeric_idx(to)) {
        if let Some((op, src_ty, dst_ty)) = PROMOTION_OPS[f_idx][t_idx] {
            emit(op, src_ty, dst_ty);
            return Ok(res);
        }
    }

    if from.canonical_eq(to) { return Ok(var.to_string()); }

    if let (Ok(mlir_from), Ok(mlir_to)) = (from.to_mlir_type(ctx), to.to_mlir_type(ctx)) {
        if mlir_from == mlir_to {
             let registry = ctx.struct_registry();
             if from.size_of(&registry) == to.size_of(&registry) {
                 return Ok(var.to_string());
             }
        }
    }

    if from.k_is_ptr_type() && to.k_is_ptr_type() { return Ok(var.to_string()); }
    if let Type::Pointer { ref element, .. } = from {
        if element.as_ref() == to { return Ok(var.to_string()); }
    }

    Err(format!("Numeric promotion not supported from {:?} to {:?} (var: {})", from, to, var))
}

pub fn cast_numeric(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<String, String> {
    if from == to { return Ok(var.to_string()); }
    println!("IN_CAST: from={:?} to={:?} is_ptr={} is_int={}", from, to, from.k_is_ptr_type(), to.is_integer());
    
    // Pointer -> Integer cast (ptrtoint)
    println!("CAST: from={:?} to={:?} k_is_ptr={}, to_is_int={}", from, to, from.k_is_ptr_type(), to.is_integer());
    if from.k_is_ptr_type() && to.is_integer() {
        let res = format!("%ptr_to_int_{}", ctx.next_id());
        let dst_mlir = to.to_mlir_type(ctx)?;
        out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to {}\n", res, var, dst_mlir));
        return Ok(res);
    }
    
    // Integer -> Pointer cast (inttoptr)
    if from.is_integer() && to.k_is_ptr_type() {
        let res = format!("%int_to_ptr_{}", ctx.next_id());
        let src_mlir = from.to_mlir_type(ctx)?;
        out.push_str(&format!("    {} = llvm.inttoptr {} : {} to !llvm.ptr\n", res, var, src_mlir));
        return Ok(res);
    }

    // Standard numeric promotion
    promote_numeric(ctx, out, var, from, to)
}

fn get_bit_width(ty: &Type) -> u32 {
    match ty {
        Type::I8 | Type::U8 | Type::Bool => 8,
        Type::I16 | Type::U16 => 16,
        Type::I32 | Type::U32 | Type::F32 => 32,
        Type::I64 | Type::U64 | Type::F64 | Type::Usize | Type::Pointer { .. } | Type::Reference(..) => 64,
        _ => 64,
    }
}
