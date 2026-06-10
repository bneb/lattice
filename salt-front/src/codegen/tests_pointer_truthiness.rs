//! TDD Tests for Pointer Truthiness
//!
//! These tests define the desired behavior for `if ptr { ... }` sugar.
//! RED PHASE: All tests should FAIL with "must be boolean" before implementation.
//! GREEN PHASE: All tests should PASS after implementation.

#[cfg(test)]
mod tests {
    use crate::types::{Type, Provenance};
    

    // ============================
    // 1. Smallest-scope unit tests
    // ============================

    /// Pointer types should be recognized as truthy-capable
    #[test]
    fn test_pointer_is_truthy_type() {
        let ptr_ty = Type::Pointer {
            element: Box::new(Type::I32),
            provenance: Provenance::Naked,
            is_mutable: true,
        };
        assert!(ptr_ty.k_is_ptr_type(), "Pointer type should be recognized as ptr type");
    }

    /// Non-pointer types should NOT be truthy
    #[test]
    fn test_i32_is_not_truthy() {
        assert!(!Type::I32.k_is_ptr_type());
    }

    #[test]
    fn test_bool_is_not_ptr_type() {
        assert!(!Type::Bool.k_is_ptr_type());
    }

    #[test]
    fn test_struct_is_not_ptr_type() {
        assert!(!Type::Struct("File".to_string()).k_is_ptr_type());
    }

    /// Ptr<f32> should be truthy-capable
    #[test]
    fn test_ptr_f32_is_truthy() {
        let ptr_ty = Type::Pointer {
            element: Box::new(Type::F32),
            provenance: Provenance::Naked,
            is_mutable: true,
        };
        assert!(ptr_ty.k_is_ptr_type());
    }

    /// Ptr<Ptr<i32>> should be truthy-capable (nested pointer)
    #[test]
    fn test_nested_ptr_is_truthy() {
        let inner = Type::Pointer {
            element: Box::new(Type::I32),
            provenance: Provenance::Naked,
            is_mutable: true,
        };
        let outer = Type::Pointer {
            element: Box::new(inner),
            provenance: Provenance::Naked,
            is_mutable: true,
        };
        assert!(outer.k_is_ptr_type());
    }
}
