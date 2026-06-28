#[cfg(test)]
mod tests {
    use crate::types::Type;
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
}
