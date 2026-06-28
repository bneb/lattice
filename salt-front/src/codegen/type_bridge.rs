#[cfg(test)]
use crate::types::{Type, TypeKey};
pub use super::type_casts::cast_numeric;

pub use crate::codegen::types::numeric::get_numeric_idx;
pub use crate::codegen::types::numeric::PromotionTable;
pub use crate::codegen::types::numeric::PROMOTION_OPS;

pub use crate::codegen::types::numeric::get_arith_op;
pub use crate::codegen::types::numeric::get_comparison_pred;
pub use crate::codegen::types::numeric::promote_numeric;
pub(crate) use crate::codegen::types::numeric::get_bit_width;


// to_mlir_type impl moved to crate::codegen::types::mlir

// ============================================================================
// Pointer flattening and layout validation
// ============================================================================

/// Extracts the inner type from mangled pointer names.
pub use crate::codegen::types::layout::extract_ptr_inner;
pub use crate::codegen::types::layout::flatten_nested_ptr;
pub use crate::codegen::types::layout::prove_layout_compatibility;
pub use crate::codegen::types::layout::prove_layout_compatibility_ctx;

pub use crate::codegen::types::substitution::substitute_generics;
pub use crate::codegen::types::substitution::substitute_generics_ctx;
pub use crate::codegen::types::mlir::to_mlir_type;


pub use crate::codegen::types::traits::{check_trait_constraint, validate_trait_constraints, has_unresolved_type_params};
pub use crate::codegen::types::resolution::{resolve_codegen_type, resolve_type, infer_expr_type, type_to_type_key};
pub use crate::codegen::types::zero_attr::zero_attr;
pub use crate::codegen::types::emit::{emit_const, emit_global_def};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::context::CodegenContext;
    use crate::registry::EnumInfo;
    use crate::grammar::SaltFile;

    #[test]
    fn test_enum_payload_packing() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let name = "PackingEnum".to_string();
        let variants = vec![
            ("A".to_string(), Some(Type::U8), 0),
            ("B".to_string(), Some(Type::Array(Box::new(Type::F64), 8, false)), 1),
        ];

        let info = EnumInfo {
            name: name.clone(),
            variants,
            max_payload_size: 64,
            template_name: None,
            specialization_args: vec![],
        };
        let key = TypeKey { path: vec![], name: name.clone(), specialization: None };
        ctx.enum_registry_mut().insert(key, info);

        let ty = Type::Enum(name);
        let mlir = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();
        // After enum type resolution fix: registered enums return their type alias
        // The inline struct definition with payload is emitted separately in type definitions
        assert_eq!(mlir, "!struct_PackingEnum", "Registered enum should use type alias");
    }

    // =========================================================================
    // TDD: Usize (MLIR index) ↔ I64 type conversion
    // =========================================================================
    // Bug context: The compiler generates MLIR `index` for `usize` params but
    // tracks them as `I64` in local_vars, causing `as i64` casts to be no-ops.
    // These tests ensure the conversion functions correctly emit arith.index_cast.

    #[test]
    fn test_usize_and_i64_are_distinct_types() {
        // CRITICAL: Type::Usize and Type::I64 must NOT be equal.
        // If they were, emit_cast's `if ty == target_ty` check would skip
        // the arith.index_cast, leaving index-typed values in i64 operations.
        assert_ne!(Type::Usize, Type::I64,
            "Type::Usize and Type::I64 must be distinct types");
        assert_ne!(Type::Usize, Type::U64,
            "Type::Usize and Type::U64 must be distinct types");
    }

    #[test]
    fn test_promote_numeric_usize_to_i64_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%arg_len", &Type::Usize, &Type::I64));

        assert!(result.is_ok(), "promote_numeric(Usize, I64) should succeed");
        assert!(out.contains("arith.index_cast"),
            "Usize→I64 must emit arith.index_cast, got: {}", out);
        assert!(out.contains("index to i64"),
            "Cast should be 'index to i64', got: {}", out);
    }

    #[test]
    fn test_promote_numeric_i64_to_usize_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%val", &Type::I64, &Type::Usize));

        assert!(result.is_ok(), "promote_numeric(I64, Usize) should succeed");
        assert!(out.contains("arith.index_cast"),
            "I64→Usize must emit arith.index_cast, got: {}", out);
        assert!(out.contains("i64 to index"),
            "Cast should be 'i64 to index', got: {}", out);
    }

    #[test]
    fn test_cast_numeric_usize_to_i64_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| cast_numeric(lctx, &mut out, "%arg_len", &Type::Usize, &Type::I64));

        assert!(result.is_ok(), "cast_numeric(Usize, I64) should succeed");
        assert!(out.contains("arith.index_cast"),
            "cast_numeric(Usize, I64) must emit arith.index_cast, got: {}", out);
    }

    #[test]
    fn test_usize_identity_does_not_emit_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%val", &Type::Usize, &Type::Usize));

        assert!(result.is_ok(), "promote_numeric(Usize, Usize) should succeed");
        assert!(out.is_empty(),
            "Usize→Usize should be identity (no MLIR emitted), got: {}", out);
    }

    // =========================================================================
    // TDD: Atomic<T> Type Emission — The Slab Memory Leak Root Cause
    // =========================================================================
    // Bug: Atomic<i32> globals emitted as `!llvm.ptr` with `null` init instead
    // of `i32` with `0 : i32` init. This causes LLVM Translation to reject the
    // MLIR with: "Global variable initializer type does not match global variable type!"
    //
    // Call graph layers to fix:
    //   Layer 0: to_mlir_type_simple(Atomic<T>) → T's MLIR type  [already works]
    //   Layer 1: zero_attr(Atomic<T>) → recurse to inner T
    //   Layer 2: to_mlir_storage_type_simple(Atomic<T>) → T's storage type
    //   Layer 3: emit_global_def sees Atomic<T> → unwraps to T for init_val

    // --- Layer 0: to_mlir_type_simple (already correct, assert for safety) ---
    #[test]
    fn test_atomic_i32_mlir_type_simple() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.to_mlir_type_simple(), "i32",
            "Atomic<i32> MLIR type should be 'i32', not '!llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_mlir_type_simple() {
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.to_mlir_type_simple(), "i64",
            "Atomic<u64> MLIR type should be 'i64'");
    }

    // --- Layer 1: zero_attr should recurse into inner type ---
    #[test]
    fn test_atomic_i32_zero_attr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ty = Type::Atomic(Box::new(Type::I32));
        let result = ctx.with_lowering_ctx(|lctx| zero_attr(lctx, &ty));
        assert!(result.is_ok(), "zero_attr(Atomic<i32>) should succeed");
        assert_eq!(result.unwrap(), "0 : i32",
            "zero_attr(Atomic<i32>) must be '0 : i32', not 'null : !llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_zero_attr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ty = Type::Atomic(Box::new(Type::U64));
        let result = ctx.with_lowering_ctx(|lctx| zero_attr(lctx, &ty));
        assert!(result.is_ok(), "zero_attr(Atomic<u64>) should succeed");
        assert_eq!(result.unwrap(), "0 : i64",
            "zero_attr(Atomic<u64>) must be '0 : i64', not 'null : !llvm.ptr'");
    }

    // --- Layer 2: to_mlir_storage_type_simple should unwrap to inner type ---
    #[test]
    fn test_atomic_i32_storage_type_simple() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.to_mlir_storage_type_simple(), "i32",
            "Atomic<i32> storage type should be 'i32', not '!llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_storage_type_simple() {
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.to_mlir_storage_type_simple(), "i64",
            "Atomic<u64> storage type should be 'i64'");
    }

    // --- Layer 3: k_is_ptr_type should NOT match Atomic ---
    #[test]
    fn test_atomic_is_not_ptr_type() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert!(!ty.k_is_ptr_type(),
            "Atomic<i32> is NOT a pointer type — it is a scalar wrapper");
    }

    // --- Layer 4: size_of should reflect inner type, not pointer ---
    #[test]
    fn test_atomic_i32_size_of() {
        let reg = std::collections::HashMap::new();
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.size_of(&reg), 4,
            "Atomic<i32> should be 4 bytes, not 8 (pointer size)");
    }

    #[test]
    fn test_atomic_u64_size_of() {
        let reg = std::collections::HashMap::new();
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.size_of(&reg), 8,
            "Atomic<u64> should be 8 bytes");
    }
}
