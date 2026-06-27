use crate::types::Type;

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
