#[cfg(test)]
mod tests {
    use crate::types::{Type, Provenance};
    use crate::codegen::types::numeric::*;

    #[test]
    fn test_get_numeric_idx_known_types() {
        assert_eq!(get_numeric_idx(&Type::I8), Some(0));
        assert_eq!(get_numeric_idx(&Type::I16), Some(1));
        assert_eq!(get_numeric_idx(&Type::I32), Some(2));
        assert_eq!(get_numeric_idx(&Type::I64), Some(3));
        assert_eq!(get_numeric_idx(&Type::U8), Some(4));
        assert_eq!(get_numeric_idx(&Type::U16), Some(5));
        assert_eq!(get_numeric_idx(&Type::U32), Some(6));
        assert_eq!(get_numeric_idx(&Type::U64), Some(7));
        assert_eq!(get_numeric_idx(&Type::Usize), Some(8));
        assert_eq!(get_numeric_idx(&Type::F32), Some(9));
        assert_eq!(get_numeric_idx(&Type::F64), Some(10));
        assert_eq!(get_numeric_idx(&Type::Bool), Some(11));
    }

    #[test]
    fn test_get_numeric_idx_unknown_types() {
        assert_eq!(get_numeric_idx(&Type::Unit), None);
        assert_eq!(get_numeric_idx(&Type::Struct("foo".into())), None);
    }

    #[test]
    fn test_get_bit_width_all_primitives() {
        assert_eq!(get_bit_width(&Type::I8), 8);
        assert_eq!(get_bit_width(&Type::U8), 8);
        assert_eq!(get_bit_width(&Type::Bool), 8);
        assert_eq!(get_bit_width(&Type::I16), 16);
        assert_eq!(get_bit_width(&Type::U16), 16);
        assert_eq!(get_bit_width(&Type::I32), 32);
        assert_eq!(get_bit_width(&Type::U32), 32);
        assert_eq!(get_bit_width(&Type::F32), 32);
        assert_eq!(get_bit_width(&Type::I64), 64);
        assert_eq!(get_bit_width(&Type::U64), 64);
        assert_eq!(get_bit_width(&Type::Usize), 64);
        assert_eq!(get_bit_width(&Type::F64), 64);
    }

    #[test]
    fn test_get_bit_width_unknown() {
        assert_eq!(get_bit_width(&Type::Unit), 0);
        assert_eq!(get_bit_width(&Type::Struct("foo".into())), 0);
    }

    #[test]
    fn test_get_arith_op_arith() {
        assert_eq!(get_arith_op(&syn::BinOp::Add(syn::token::Plus::default()), &Type::F32), "arith.addf");
        assert_eq!(get_arith_op(&syn::BinOp::Sub(syn::token::Minus::default()), &Type::F64), "arith.subf");
        assert_eq!(get_arith_op(&syn::BinOp::Mul(syn::token::Star::default()), &Type::F32), "arith.mulf");
        assert_eq!(get_arith_op(&syn::BinOp::Div(syn::token::Slash::default()), &Type::F32), "arith.divf");
        assert_eq!(get_arith_op(&syn::BinOp::Rem(syn::token::Percent::default()), &Type::F64), "arith.remf");
        assert_eq!(get_arith_op(&syn::BinOp::Add(syn::token::Plus::default()), &Type::I32), "arith.addi");
        assert_eq!(get_arith_op(&syn::BinOp::Sub(syn::token::Minus::default()), &Type::I64), "arith.subi");
        assert_eq!(get_arith_op(&syn::BinOp::Mul(syn::token::Star::default()), &Type::I8), "arith.muli");
        assert_eq!(get_arith_op(&syn::BinOp::Div(syn::token::Slash::default()), &Type::I32), "arith.divsi");
        assert_eq!(get_arith_op(&syn::BinOp::Rem(syn::token::Percent::default()), &Type::I64), "arith.remsi");
        assert_eq!(get_arith_op(&syn::BinOp::Div(syn::token::Slash::default()), &Type::U32), "arith.divui");
        assert_eq!(get_arith_op(&syn::BinOp::Rem(syn::token::Percent::default()), &Type::U64), "arith.remui");
    }

    #[test]
    fn test_get_arith_op_assign() {
        assert_eq!(get_arith_op(&syn::BinOp::AddAssign(syn::token::PlusEq::default()), &Type::F32), "arith.addf");
        assert_eq!(get_arith_op(&syn::BinOp::SubAssign(syn::token::MinusEq::default()), &Type::I32), "arith.subi");
        assert_eq!(get_arith_op(&syn::BinOp::MulAssign(syn::token::StarEq::default()), &Type::I32), "arith.muli");
        assert_eq!(get_arith_op(&syn::BinOp::DivAssign(syn::token::SlashEq::default()), &Type::U32), "arith.divui");
        assert_eq!(get_arith_op(&syn::BinOp::RemAssign(syn::token::PercentEq::default()), &Type::I32), "arith.remsi");
    }

    #[test]
    fn test_get_arith_op_bitwise_and_shift() {
        assert_eq!(get_arith_op(&syn::BinOp::BitAnd(syn::token::And::default()), &Type::I32), "arith.andi");
        assert_eq!(get_arith_op(&syn::BinOp::BitOr(syn::token::Or::default()), &Type::I32), "arith.ori");
        assert_eq!(get_arith_op(&syn::BinOp::BitXor(syn::token::Caret::default()), &Type::I32), "arith.xori");
        assert_eq!(get_arith_op(&syn::BinOp::And(syn::token::AndAnd::default()), &Type::I32), "arith.andi");
        assert_eq!(get_arith_op(&syn::BinOp::Or(syn::token::OrOr::default()), &Type::I32), "arith.ori");
        assert_eq!(get_arith_op(&syn::BinOp::Shl(syn::token::Shl::default()), &Type::I32), "arith.shli");
        assert_eq!(get_arith_op(&syn::BinOp::Shl(syn::token::Shl::default()), &Type::U64), "arith.shli");
        assert_eq!(get_arith_op(&syn::BinOp::Shr(syn::token::Shr::default()), &Type::U32), "arith.shrui");
        assert_eq!(get_arith_op(&syn::BinOp::Shr(syn::token::Shr::default()), &Type::I32), "arith.shrsi");
    }

    #[test]
    fn test_get_arith_op_cmp() {
        assert_eq!(get_arith_op(&syn::BinOp::Eq(syn::token::EqEq::default()), &Type::F32), "arith.cmpf");
        assert_eq!(get_arith_op(&syn::BinOp::Lt(syn::token::Lt::default()), &Type::F64), "arith.cmpf");
        assert_eq!(get_arith_op(&syn::BinOp::Eq(syn::token::EqEq::default()), &Type::I32), "arith.cmpi");
        assert_eq!(get_arith_op(&syn::BinOp::Le(syn::token::Le::default()), &Type::U32), "arith.cmpi");
        assert_eq!(get_arith_op(&syn::BinOp::Eq(syn::token::EqEq::default()), &Type::Reference(Box::new(Type::I32), false)), "llvm.icmp");
        assert_eq!(get_arith_op(&syn::BinOp::Lt(syn::token::Lt::default()), &Type::Owned(Box::new(Type::I32))), "llvm.icmp");
        assert_eq!(get_arith_op(&syn::BinOp::Ge(syn::token::Ge::default()), &Type::Window(Box::new(Type::I32), "rw".into())), "llvm.icmp");
        assert_eq!(get_arith_op(&syn::BinOp::Ne(syn::token::Ne::default()), &Type::Pointer { element: Box::new(Type::I8), provenance: Provenance::Naked, is_mutable: true }), "llvm.icmp");
    }

    #[test]
    fn test_get_comparison_pred_float() {
        assert_eq!(get_comparison_pred(&syn::BinOp::Eq(syn::token::EqEq::default()), &Type::F32), "oeq");
        assert_eq!(get_comparison_pred(&syn::BinOp::Ne(syn::token::Ne::default()), &Type::F64), "une");
        assert_eq!(get_comparison_pred(&syn::BinOp::Lt(syn::token::Lt::default()), &Type::F32), "olt");
        assert_eq!(get_comparison_pred(&syn::BinOp::Le(syn::token::Le::default()), &Type::F64), "ole");
        assert_eq!(get_comparison_pred(&syn::BinOp::Gt(syn::token::Gt::default()), &Type::F32), "ogt");
        assert_eq!(get_comparison_pred(&syn::BinOp::Ge(syn::token::Ge::default()), &Type::F64), "oge");
    }

    #[test]
    fn test_get_comparison_pred_signed() {
        assert_eq!(get_comparison_pred(&syn::BinOp::Eq(syn::token::EqEq::default()), &Type::I32), "eq");
        assert_eq!(get_comparison_pred(&syn::BinOp::Ne(syn::token::Ne::default()), &Type::I64), "ne");
        assert_eq!(get_comparison_pred(&syn::BinOp::Lt(syn::token::Lt::default()), &Type::I8), "slt");
        assert_eq!(get_comparison_pred(&syn::BinOp::Le(syn::token::Le::default()), &Type::I16), "sle");
        assert_eq!(get_comparison_pred(&syn::BinOp::Gt(syn::token::Gt::default()), &Type::I32), "sgt");
        assert_eq!(get_comparison_pred(&syn::BinOp::Ge(syn::token::Ge::default()), &Type::I64), "sge");
    }

    #[test]
    fn test_get_comparison_pred_unsigned() {
        assert_eq!(get_comparison_pred(&syn::BinOp::Lt(syn::token::Lt::default()), &Type::U32), "ult");
        assert_eq!(get_comparison_pred(&syn::BinOp::Le(syn::token::Le::default()), &Type::U64), "ule");
        assert_eq!(get_comparison_pred(&syn::BinOp::Gt(syn::token::Gt::default()), &Type::U16), "ugt");
        assert_eq!(get_comparison_pred(&syn::BinOp::Ge(syn::token::Ge::default()), &Type::U8), "uge");
        assert_eq!(get_comparison_pred(&syn::BinOp::Eq(syn::token::EqEq::default()), &Type::U32), "eq");
        assert_eq!(get_comparison_pred(&syn::BinOp::Ne(syn::token::Ne::default()), &Type::Bool), "ne");
    }

    #[test]
    fn test_get_comparison_pred_pointer() {
        let ptr = Type::Pointer { element: Box::new(Type::I32), provenance: Provenance::Naked, is_mutable: true };
        assert_eq!(get_comparison_pred(&syn::BinOp::Lt(syn::token::Lt::default()), &ptr), "ult");
        assert_eq!(get_comparison_pred(&syn::BinOp::Eq(syn::token::EqEq::default()), &ptr), "eq");
        assert_eq!(get_comparison_pred(&syn::BinOp::Gt(syn::token::Gt::default()), &ptr), "ugt");
    }

    #[test]
    fn test_get_comparison_pred_default() {
        assert_eq!(get_comparison_pred(&syn::BinOp::Add(syn::token::Plus::default()), &Type::I32), "eq");
    }

    #[test]
    fn test_get_arith_op_assign_bitwise() {
        assert_eq!(get_arith_op(&syn::BinOp::BitAndAssign(syn::token::AndEq::default()), &Type::I32), "arith.andi");
        assert_eq!(get_arith_op(&syn::BinOp::BitOrAssign(syn::token::OrEq::default()), &Type::U64), "arith.ori");
        assert_eq!(get_arith_op(&syn::BinOp::BitXorAssign(syn::token::CaretEq::default()), &Type::I8), "arith.xori");
        assert_eq!(get_arith_op(&syn::BinOp::ShlAssign(syn::token::ShlEq::default()), &Type::I16), "arith.shli");
        assert_eq!(get_arith_op(&syn::BinOp::ShrAssign(syn::token::ShrEq::default()), &Type::I8), "arith.shrsi");
    }

    #[test]
    fn test_get_arith_op_shr_assign() {
        assert_eq!(get_arith_op(&syn::BinOp::ShrAssign(syn::token::ShrEq::default()), &Type::U32), "arith.shrui");
        assert_eq!(get_arith_op(&syn::BinOp::ShrAssign(syn::token::ShrEq::default()), &Type::I32), "arith.shrsi");
        assert_eq!(get_arith_op(&syn::BinOp::ShrAssign(syn::token::ShrEq::default()), &Type::U64), "arith.shrui");
        assert_eq!(get_arith_op(&syn::BinOp::ShrAssign(syn::token::ShrEq::default()), &Type::I64), "arith.shrsi");
    }

    #[test]
    fn test_get_comparison_pred_edge_cases() {
        let ref_i32 = Type::Reference(Box::new(Type::I32), false);
        let owned_i32 = Type::Owned(Box::new(Type::I32));
        assert_eq!(get_comparison_pred(&syn::BinOp::Gt(syn::token::Gt::default()), &ref_i32), "sgt");
        assert_eq!(get_comparison_pred(&syn::BinOp::Ge(syn::token::Ge::default()), &ref_i32), "sge");
        assert_eq!(get_comparison_pred(&syn::BinOp::Ne(syn::token::Ne::default()), &owned_i32), "ne");
        assert_eq!(get_comparison_pred(&syn::BinOp::Lt(syn::token::Lt::default()), &owned_i32), "slt");
        assert_eq!(get_comparison_pred(&syn::BinOp::Eq(syn::token::EqEq::default()), &ref_i32), "eq");
    }

    #[test]
    fn test_promotion_ops_table() {
        assert_eq!(PROMOTION_OPS[get_numeric_idx(&Type::I32).unwrap()][get_numeric_idx(&Type::I64).unwrap()], Some(("arith.extsi", "i32", "i64")));
        assert_eq!(PROMOTION_OPS[get_numeric_idx(&Type::I16).unwrap()][get_numeric_idx(&Type::I32).unwrap()], Some(("arith.extsi", "i16", "i32")));
        assert_eq!(PROMOTION_OPS[get_numeric_idx(&Type::F32).unwrap()][get_numeric_idx(&Type::F64).unwrap()], Some(("arith.extf", "f32", "f64")));
        assert_eq!(PROMOTION_OPS[get_numeric_idx(&Type::U32).unwrap()][get_numeric_idx(&Type::I64).unwrap()], Some(("arith.extui", "i32", "i64")));
        assert_eq!(PROMOTION_OPS[get_numeric_idx(&Type::I8).unwrap()][get_numeric_idx(&Type::I32).unwrap()], Some(("arith.extsi", "i8", "i32")));
        assert_eq!(PROMOTION_OPS[get_numeric_idx(&Type::U16).unwrap()][get_numeric_idx(&Type::I32).unwrap()], Some(("arith.extui", "i16", "i32")));
        assert_eq!(PROMOTION_OPS[get_numeric_idx(&Type::I64).unwrap()][get_numeric_idx(&Type::I8).unwrap()], None);
    }

    #[test]
    fn test_promotion_ops_table_some() {
        let entries = [
            ((0,1,"arith.extsi","i8","i16")),((0,5,"arith.extsi","i8","i16")),
            ((0,2,"arith.extsi","i8","i32")),((0,6,"arith.extsi","i8","i32")),
            ((0,3,"arith.extsi","i8","i64")),((0,7,"arith.extsi","i8","i64")),((0,8,"arith.extsi","i8","i64")),
            ((1,2,"arith.extsi","i16","i32")),((1,6,"arith.extsi","i16","i32")),
            ((1,3,"arith.extsi","i16","i64")),((1,7,"arith.extsi","i16","i64")),((1,8,"arith.extsi","i16","i64")),
            ((2,3,"arith.extsi","i32","i64")),((2,7,"arith.extsi","i32","i64")),((2,8,"arith.extsi","i32","i64")),
            ((4,1,"arith.extui","i8","i16")),((4,5,"arith.extui","i8","i16")),
            ((4,2,"arith.extui","i8","i32")),((4,6,"arith.extui","i8","i32")),
            ((4,3,"arith.extui","i8","i64")),((4,7,"arith.extui","i8","i64")),((4,8,"arith.extui","i8","i64")),
            ((5,2,"arith.extui","i16","i32")),((5,6,"arith.extui","i16","i32")),
            ((5,3,"arith.extui","i16","i64")),((5,7,"arith.extui","i16","i64")),((5,8,"arith.extui","i16","i64")),
            ((6,3,"arith.extui","i32","i64")),((6,7,"arith.extui","i32","i64")),((6,8,"arith.extui","i32","i64")),
            ((9,10,"arith.extf","f32","f64")),
        ];
        for &(from, to, op, src, dst) in &entries {
            assert_eq!(PROMOTION_OPS[from][to], Some((op, src, dst)),
                "PROMOTION_OPS[{}][{}] mismatch", from, to);
        }
    }

    #[test]
    fn test_promotion_ops_table_none() {
        let some = vec![
            (0,1),(0,5),(0,2),(0,6),(0,3),(0,7),(0,8),
            (1,2),(1,6),(1,3),(1,7),(1,8),
            (2,3),(2,7),(2,8),
            (4,1),(4,5),(4,2),(4,6),(4,3),(4,7),(4,8),
            (5,2),(5,6),(5,3),(5,7),(5,8),
            (6,3),(6,7),(6,8),
            (9,10),
        ];
        for n in 0i32..144 {
            let (i, j) = ((n / 12) as usize, (n % 12) as usize);
            if !some.contains(&(i, j)) {
                assert_eq!(PROMOTION_OPS[i][j], None,
                    "Expected None at PROMOTION_OPS[{}][{}], got {:?}",
                    i, j, PROMOTION_OPS[i][j]);
            }
        }
    }
}
