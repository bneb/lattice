//! Exhaustive Type Promotion Specification Tests
//!
//! These tests document and verify every valid and invalid type promotion
//! pair in the Salt compiler's `promote_numeric` function.
//!
//! Each test case covers one (from, to) pair permutation. The complete
//! matrix uses 20 core types × 20 = 400 pairs, organized by category.
//! Types covered: 9 integers, 2 floats, Bool, Unit, Pointer, Struct,
//! Enum, Tuple, Never, Reference, Owned, and Concrete (Result/Status).
//!
//! A "promotion" is an implicit conversion Salt applies when passing a value
//! of type A where type B is expected (e.g., function arguments).

mod tests {
    use crate::types::Type;
    use crate::codegen::type_bridge::promote_numeric;
    use crate::codegen::context::CodegenContext;
    use crate::grammar::SaltFile;

    /// All types used in the promotion matrix.
    /// Includes primitives, compound types, and linear types.
    fn all_types() -> Vec<(&'static str, Type)> {
        vec![
            // Primitive integers
            ("I8", Type::I8),
            ("I16", Type::I16),
            ("I32", Type::I32),
            ("I64", Type::I64),
            ("U8", Type::U8),
            ("U16", Type::U16),
            ("U32", Type::U32),
            ("U64", Type::U64),
            ("Usize", Type::Usize),
            // Floating point
            ("F32", Type::F32),
            ("F64", Type::F64),
            // Boolean and unit
            ("Bool", Type::Bool),
            ("Unit", Type::Unit),
            // Pointer
            ("Pointer(U8)", Type::Pointer {
                element: Box::new(Type::U8),
                provenance: crate::types::Provenance::Naked,
                is_mutable: true,
            }),
            // Compound types
            ("Struct(Foo)", Type::Struct("Foo".to_string())),
            ("Enum(Status)", Type::Enum("Status".to_string())),
            ("Tuple(I32,I32)", Type::Tuple(vec![Type::I32, Type::I32])),
            ("Never", Type::Never),
            ("Ref(I32)", Type::Reference(Box::new(Type::I32), false)),
            ("Owned(I32)", Type::Owned(Box::new(Type::I32))),
        ]
    }

    /// Try to promote a type and return Ok/Err.
    /// Uses a real CodegenContext to exercise the full promotion logic.
    fn try_promote(from: &Type, to: &Type) -> Result<String, String> {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        let mut out = String::new();
        ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%test_var", from, to))
    }

    // ========================================================================
    // Category 1: Identity Promotions (from == to)
    // All identity promotions MUST succeed as no-ops.
    // ========================================================================

    macro_rules! test_identity {
        ($name:ident, $ty:expr) => {
            #[test]
            fn $name() {
                let ty = $ty;
                let result = try_promote(&ty, &ty);
                assert!(result.is_ok(),
                    "Identity promotion {:?} -> {:?} must succeed, got: {:?}", ty, ty, result.err());
                assert_eq!(result.unwrap(), "%test_var",
                    "Identity promotion should return the original variable unchanged");
            }
        };
    }

    test_identity!(test_identity_i8,    Type::I8);
    test_identity!(test_identity_i16,   Type::I16);
    test_identity!(test_identity_i32,   Type::I32);
    test_identity!(test_identity_i64,   Type::I64);
    test_identity!(test_identity_u8,    Type::U8);
    test_identity!(test_identity_u16,   Type::U16);
    test_identity!(test_identity_u32,   Type::U32);
    test_identity!(test_identity_u64,   Type::U64);
    test_identity!(test_identity_usize, Type::Usize);
    test_identity!(test_identity_f32,   Type::F32);
    test_identity!(test_identity_f64,   Type::F64);
    test_identity!(test_identity_bool,  Type::Bool);

    // ========================================================================
    // Category 2: Signed ↔ Unsigned Same-Width Reinterpretation
    // These are bit-identical types and must succeed as no-ops.
    // ========================================================================

    macro_rules! test_reinterpret {
        ($name:ident, $from:expr, $to:expr) => {
            #[test]
            fn $name() {
                let (from, to) = ($from, $to);
                let result = try_promote(&from, &to);
                assert!(result.is_ok(),
                    "Reinterpret {:?} -> {:?} must succeed, got: {:?}", from, to, result.err());
            }
        };
    }

    test_reinterpret!(test_reinterpret_i8_u8,   Type::I8,  Type::U8);
    test_reinterpret!(test_reinterpret_u8_i8,   Type::U8,  Type::I8);
    test_reinterpret!(test_reinterpret_i16_u16, Type::I16, Type::U16);
    test_reinterpret!(test_reinterpret_u16_i16, Type::U16, Type::I16);
    test_reinterpret!(test_reinterpret_i32_u32, Type::I32, Type::U32);
    test_reinterpret!(test_reinterpret_u32_i32, Type::U32, Type::I32);
    test_reinterpret!(test_reinterpret_i64_u64, Type::I64, Type::U64);
    test_reinterpret!(test_reinterpret_u64_i64, Type::U64, Type::I64);

    // ========================================================================
    // Category 3: Integer Widening (small → large)
    // Promotion from narrower to wider integer types.
    // ========================================================================

    macro_rules! test_widening {
        ($name:ident, $from:expr, $to:expr) => {
            #[test]
            fn $name() {
                let (from, to) = ($from, $to);
                let result = try_promote(&from, &to);
                assert!(result.is_ok(),
                    "Widening {:?} -> {:?} must succeed, got: {:?}", from, to, result.err());
                // Widening should emit a new SSA value (not the original)
                assert_ne!(result.unwrap(), "%test_var",
                    "Widening should emit a conversion instruction, not a no-op");
            }
        };
    }

    // Signed widening chain: I8 → I16 → I32 → I64
    test_widening!(test_widen_i8_i16,  Type::I8,  Type::I16);
    test_widening!(test_widen_i8_i32,  Type::I8,  Type::I32);
    test_widening!(test_widen_i8_i64,  Type::I8,  Type::I64);
    test_widening!(test_widen_i16_i32, Type::I16, Type::I32);
    test_widening!(test_widen_i16_i64, Type::I16, Type::I64);
    test_widening!(test_widen_i32_i64, Type::I32, Type::I64);

    // Unsigned widening chain: U8 → U16 → U32 → U64
    test_widening!(test_widen_u8_u16,  Type::U8,  Type::U16);
    test_widening!(test_widen_u8_u32,  Type::U8,  Type::U32);
    test_widening!(test_widen_u8_u64,  Type::U8,  Type::U64);
    test_widening!(test_widen_u16_u32, Type::U16, Type::U32);
    test_widening!(test_widen_u16_u64, Type::U16, Type::U64);
    test_widening!(test_widen_u32_u64, Type::U32, Type::U64);

    // Cross-sign widening (e.g., U8 → I32)
    test_widening!(test_widen_u8_i16,  Type::U8,  Type::I16);
    test_widening!(test_widen_u8_i32,  Type::U8,  Type::I32);
    test_widening!(test_widen_u8_i64,  Type::U8,  Type::I64);
    test_widening!(test_widen_u16_i32, Type::U16, Type::I32);
    test_widening!(test_widen_u16_i64, Type::U16, Type::I64);
    test_widening!(test_widen_u32_i64, Type::U32, Type::I64);

    // ========================================================================
    // Category 4: Integer Narrowing (large → small)
    // Truncation from wider to narrower integer types.
    // ========================================================================

    macro_rules! test_narrowing {
        ($name:ident, $from:expr, $to:expr) => {
            #[test]
            fn $name() {
                let (from, to) = ($from, $to);
                let result = try_promote(&from, &to);
                assert!(result.is_ok(),
                    "Narrowing {:?} -> {:?} must succeed (truncation), got: {:?}", from, to, result.err());
            }
        };
    }

    test_narrowing!(test_narrow_i64_i32, Type::I64, Type::I32);
    test_narrowing!(test_narrow_i64_i16, Type::I64, Type::I16);
    test_narrowing!(test_narrow_i64_i8,  Type::I64, Type::I8);
    test_narrowing!(test_narrow_i32_i16, Type::I32, Type::I16);
    test_narrowing!(test_narrow_i32_i8,  Type::I32, Type::I8);
    test_narrowing!(test_narrow_i16_i8,  Type::I16, Type::I8);

    test_narrowing!(test_narrow_u64_u32, Type::U64, Type::U32);
    test_narrowing!(test_narrow_u64_u16, Type::U64, Type::U16);
    test_narrowing!(test_narrow_u64_u8,  Type::U64, Type::U8);
    test_narrowing!(test_narrow_u32_u16, Type::U32, Type::U16);
    test_narrowing!(test_narrow_u32_u8,  Type::U32, Type::U8);
    test_narrowing!(test_narrow_u16_u8,  Type::U16, Type::U8);

    // ========================================================================
    // Category 5: Usize ↔ Integer Conversions (index_cast)
    // Usize is MLIR's `index` type — requires arith.index_cast.
    // ========================================================================

    macro_rules! test_usize_conv {
        ($name:ident, $from:expr, $to:expr) => {
            #[test]
            fn $name() {
                let (from, to) = ($from, $to);
                let result = try_promote(&from, &to);
                assert!(result.is_ok(),
                    "Usize conversion {:?} -> {:?} must succeed, got: {:?}", from, to, result.err());
            }
        };
    }

    // Usize → Integer
    test_usize_conv!(test_usize_to_i64, Type::Usize, Type::I64);
    test_usize_conv!(test_usize_to_u64, Type::Usize, Type::U64);
    test_usize_conv!(test_usize_to_i32, Type::Usize, Type::I32);
    test_usize_conv!(test_usize_to_u32, Type::Usize, Type::U32);
    test_usize_conv!(test_usize_to_i16, Type::Usize, Type::I16);
    test_usize_conv!(test_usize_to_u16, Type::Usize, Type::U16);
    test_usize_conv!(test_usize_to_i8,  Type::Usize, Type::I8);
    test_usize_conv!(test_usize_to_u8,  Type::Usize, Type::U8);

    // Integer → Usize
    test_usize_conv!(test_i64_to_usize, Type::I64, Type::Usize);
    test_usize_conv!(test_u64_to_usize, Type::U64, Type::Usize);
    test_usize_conv!(test_i32_to_usize, Type::I32, Type::Usize);
    test_usize_conv!(test_u32_to_usize, Type::U32, Type::Usize);
    test_usize_conv!(test_i16_to_usize, Type::I16, Type::Usize);
    test_usize_conv!(test_u16_to_usize, Type::U16, Type::Usize);
    test_usize_conv!(test_i8_to_usize,  Type::I8,  Type::Usize);
    test_usize_conv!(test_u8_to_usize,  Type::U8,  Type::Usize);

    // ========================================================================
    // Category 6: Integer → Float Promotion
    // ========================================================================

    macro_rules! test_int_to_float {
        ($name:ident, $from:expr, $to:expr) => {
            #[test]
            fn $name() {
                let (from, to) = ($from, $to);
                let result = try_promote(&from, &to);
                assert!(result.is_ok(),
                    "Integer→Float {:?} -> {:?} must succeed, got: {:?}", from, to, result.err());
            }
        };
    }

    test_int_to_float!(test_i8_to_f32,  Type::I8,  Type::F32);
    test_int_to_float!(test_i16_to_f32, Type::I16, Type::F32);
    test_int_to_float!(test_i32_to_f32, Type::I32, Type::F32);
    test_int_to_float!(test_i64_to_f32, Type::I64, Type::F32);
    test_int_to_float!(test_u8_to_f32,  Type::U8,  Type::F32);
    test_int_to_float!(test_u16_to_f32, Type::U16, Type::F32);
    test_int_to_float!(test_u32_to_f32, Type::U32, Type::F32);
    test_int_to_float!(test_u64_to_f32, Type::U64, Type::F32);
    test_int_to_float!(test_i8_to_f64,  Type::I8,  Type::F64);
    test_int_to_float!(test_i16_to_f64, Type::I16, Type::F64);
    test_int_to_float!(test_i32_to_f64, Type::I32, Type::F64);
    test_int_to_float!(test_i64_to_f64, Type::I64, Type::F64);
    test_int_to_float!(test_u8_to_f64,  Type::U8,  Type::F64);
    test_int_to_float!(test_u16_to_f64, Type::U16, Type::F64);
    test_int_to_float!(test_u32_to_f64, Type::U32, Type::F64);
    test_int_to_float!(test_u64_to_f64, Type::U64, Type::F64);

    // ========================================================================
    // Category 7: Bool ↔ Integer/Float Conversions
    // ========================================================================

    macro_rules! test_bool_conv {
        ($name:ident, $from:expr, $to:expr) => {
            #[test]
            fn $name() {
                let (from, to) = ($from, $to);
                let result = try_promote(&from, &to);
                assert!(result.is_ok(),
                    "Bool conversion {:?} -> {:?} must succeed, got: {:?}", from, to, result.err());
            }
        };
    }

    // Bool → Integer (zero extension: false=0, true=1)
    test_bool_conv!(test_bool_to_i8,    Type::Bool, Type::I8);
    test_bool_conv!(test_bool_to_i16,   Type::Bool, Type::I16);
    test_bool_conv!(test_bool_to_i32,   Type::Bool, Type::I32);
    test_bool_conv!(test_bool_to_i64,   Type::Bool, Type::I64);
    test_bool_conv!(test_bool_to_u8,    Type::Bool, Type::U8);
    test_bool_conv!(test_bool_to_u16,   Type::Bool, Type::U16);
    test_bool_conv!(test_bool_to_u32,   Type::Bool, Type::U32);
    test_bool_conv!(test_bool_to_u64,   Type::Bool, Type::U64);

    // Integer → Bool (comparison: x != 0)
    test_bool_conv!(test_i8_to_bool,    Type::I8,   Type::Bool);
    test_bool_conv!(test_i16_to_bool,   Type::I16,  Type::Bool);
    test_bool_conv!(test_i32_to_bool,   Type::I32,  Type::Bool);
    test_bool_conv!(test_i64_to_bool,   Type::I64,  Type::Bool);
    test_bool_conv!(test_u8_to_bool,    Type::U8,   Type::Bool);
    test_bool_conv!(test_u16_to_bool,   Type::U16,  Type::Bool);
    test_bool_conv!(test_u32_to_bool,   Type::U32,  Type::Bool);
    test_bool_conv!(test_u64_to_bool,   Type::U64,  Type::Bool);

    // Float → Bool (comparison: x != 0.0)
    test_bool_conv!(test_f32_to_bool, Type::F32, Type::Bool);
    test_bool_conv!(test_f64_to_bool, Type::F64, Type::Bool);

    // ========================================================================
    // Category 8: Float ↔ Float Promotion
    // ========================================================================

    #[test]
    fn test_f32_to_f64() {
        let result = try_promote(&Type::F32, &Type::F64);
        assert!(result.is_ok(),
            "F32 -> F64 widening must succeed, got: {:?}", result.err());
    }

    #[test]
    fn test_f64_to_f32() {
        let result = try_promote(&Type::F64, &Type::F32);
        assert!(result.is_ok(),
            "F64 -> F32 narrowing must succeed, got: {:?}", result.err());
    }

    // ========================================================================
    // Category 9: Pointer ↔ Pointer (same MLIR type)
    // All Ptr<T> lower to !llvm.ptr, so promotions between them should be no-ops.
    // ========================================================================

    #[test]
    fn test_ptr_u8_to_ptr_u8() {
        let ptr = Type::Pointer {
            element: Box::new(Type::U8),
            provenance: crate::types::Provenance::Naked,
            is_mutable: true,
        };
        let result = try_promote(&ptr, &ptr);
        assert!(result.is_ok(),
            "Ptr<u8> -> Ptr<u8> identity must succeed, got: {:?}", result.err());
    }

    #[test]
    fn test_ptr_u8_to_ptr_f32() {
        let ptr_u8 = Type::Pointer {
            element: Box::new(Type::U8),
            provenance: crate::types::Provenance::Naked,
            is_mutable: true,
        };
        let ptr_f32 = Type::Pointer {
            element: Box::new(Type::F32),
            provenance: crate::types::Provenance::Naked,
            is_mutable: true,
        };
        let result = try_promote(&ptr_u8, &ptr_f32);
        // Both lower to !llvm.ptr, so MLIR identity bypass should make this succeed.
        assert!(result.is_ok(),
            "Ptr<u8> -> Ptr<f32> (both !llvm.ptr) must succeed, got: {:?}", result.err());
    }

    // ========================================================================
    // Category 10: Invalid Promotions (MUST fail)
    // These are intentionally unsupported type promotions.
    // ========================================================================

    macro_rules! test_invalid_promotion {
        ($name:ident, $from:expr, $to:expr) => {
            #[test]
            fn $name() {
                let (from, to) = ($from, $to);
                let result = try_promote(&from, &to);
                assert!(result.is_err(),
                    "Invalid promotion {:?} -> {:?} MUST fail, but succeeded", from, to);
            }
        };
    }

    // Integer → Pointer (pointer provenance guard — prevents context contamination)
    test_invalid_promotion!(test_invalid_i32_to_ptr, Type::I32, Type::Pointer {
        element: Box::new(Type::U8),
        provenance: crate::types::Provenance::Naked,
        is_mutable: true,
    });
    test_invalid_promotion!(test_invalid_i64_to_ptr, Type::I64, Type::Pointer {
        element: Box::new(Type::U8),
        provenance: crate::types::Provenance::Naked,
        is_mutable: true,
    });

    // Float → Integer (not valid as implicit promotion — requires explicit cast)
    test_invalid_promotion!(test_invalid_f32_to_i32, Type::F32, Type::I32);
    test_invalid_promotion!(test_invalid_f64_to_i64, Type::F64, Type::I64);
    test_invalid_promotion!(test_invalid_f32_to_u32, Type::F32, Type::U32);
    test_invalid_promotion!(test_invalid_f64_to_u64, Type::F64, Type::U64);

    // Pointer → Integer (not valid as implicit promotion)
    test_invalid_promotion!(test_invalid_ptr_to_i32, Type::Pointer {
        element: Box::new(Type::U8),
        provenance: crate::types::Provenance::Naked,
        is_mutable: true,
    }, Type::I32);
    test_invalid_promotion!(test_invalid_ptr_to_i64, Type::Pointer {
        element: Box::new(Type::U8),
        provenance: crate::types::Provenance::Naked,
        is_mutable: true,
    }, Type::I64);

    // Unit → anything (Unit is a zero-sized type, not promotable)
    test_invalid_promotion!(test_invalid_unit_to_i32, Type::Unit, Type::I32);
    test_invalid_promotion!(test_invalid_unit_to_bool, Type::Unit, Type::Bool);

    // Anything → Unit
    test_invalid_promotion!(test_invalid_i32_to_unit, Type::I32, Type::Unit);
    test_invalid_promotion!(test_invalid_bool_to_unit, Type::Bool, Type::Unit);

    // ========================================================================
    // Category 11: Compound Type Identity Promotions
    // Struct, Enum, Tuple, Never, Reference, Owned — identity must succeed.
    // ========================================================================

    test_identity!(test_identity_struct, Type::Struct("Foo".to_string()));
    test_identity!(test_identity_enum,   Type::Enum("Status".to_string()));
    test_identity!(test_identity_tuple,  Type::Tuple(vec![Type::I32, Type::I32]));
    test_identity!(test_identity_never,  Type::Never);

    #[test]
    fn test_identity_reference() {
        let ty = Type::Reference(Box::new(Type::I32), false);
        let result = try_promote(&ty, &ty);
        assert!(result.is_ok(),
            "Identity Reference -> Reference must succeed, got: {:?}", result.err());
    }

    #[test]
    fn test_identity_owned() {
        let ty = Type::Owned(Box::new(Type::I32));
        let result = try_promote(&ty, &ty);
        assert!(result.is_ok(),
            "Identity Owned -> Owned must succeed, got: {:?}", result.err());
    }

    // ========================================================================
    // Category 12: Owned/Reference Linear Promotions
    // Value T -> Owned<T> (auto-box) and Owned<T> -> T (auto-unbox)
    // Value T -> Reference<T> (auto-ref)
    // ========================================================================

    #[test]
    fn test_value_to_owned_auto_box() {
        let result = try_promote(&Type::I32, &Type::Owned(Box::new(Type::I32)));
        assert!(result.is_ok(),
            "I32 -> Owned<I32> (auto-box) must succeed, got: {:?}", result.err());
    }

    #[test]
    fn test_owned_to_value_auto_unbox() {
        let result = try_promote(&Type::Owned(Box::new(Type::I32)), &Type::I32);
        assert!(result.is_ok(),
            "Owned<I32> -> I32 (auto-unbox) must succeed, got: {:?}", result.err());
    }

    #[test]
    fn test_value_to_reference_auto_ref() {
        let result = try_promote(&Type::I32, &Type::Reference(Box::new(Type::I32), false));
        assert!(result.is_ok(),
            "I32 -> &I32 (auto-ref) must succeed, got: {:?}", result.err());
    }

    #[test]
    fn test_ref_to_ref_promotion() {
        let from = Type::Reference(Box::new(Type::I32), false);
        let to = Type::Reference(Box::new(Type::I64), false);
        let result = try_promote(&from, &to);
        assert!(result.is_ok(),
            "&I32 -> &I64 (reference-to-reference) must succeed, got: {:?}", result.err());
    }

    // ========================================================================
    // Category 13: Never Type Promotions
    // Never (bottom type) promotes to anything — used for unreachable code.
    // ========================================================================

    macro_rules! test_never_to {
        ($name:ident, $to:expr) => {
            #[test]
            fn $name() {
                let to = $to;
                let result = try_promote(&Type::Never, &to);
                assert!(result.is_ok(),
                    "Never -> {:?} must succeed (bottom type), got: {:?}", to, result.err());
            }
        };
    }

    test_never_to!(test_never_to_i32,  Type::I32);
    test_never_to!(test_never_to_i64,  Type::I64);
    test_never_to!(test_never_to_f64,  Type::F64);
    test_never_to!(test_never_to_bool, Type::Bool);
    test_never_to!(test_never_to_ptr,  Type::Pointer {
        element: Box::new(Type::U8),
        provenance: crate::types::Provenance::Naked,
        is_mutable: true,
    });

    // ========================================================================
    // Category 14: Cross-Category Invalid Promotions (compound ↔ primitive)
    // These verify that unrelated type categories cannot be implicitly promoted.
    // ========================================================================

    // Struct → primitive (invalid)
    test_invalid_promotion!(test_invalid_struct_to_i32,  Type::Struct("Foo".to_string()), Type::I32);
    test_invalid_promotion!(test_invalid_struct_to_bool, Type::Struct("Foo".to_string()), Type::Bool);
    test_invalid_promotion!(test_invalid_struct_to_ptr,  Type::Struct("Foo".to_string()), Type::Pointer {
        element: Box::new(Type::U8),
        provenance: crate::types::Provenance::Naked,
        is_mutable: true,
    });

    // Primitive → Struct (invalid)
    test_invalid_promotion!(test_invalid_i32_to_struct,  Type::I32, Type::Struct("Foo".to_string()));
    test_invalid_promotion!(test_invalid_bool_to_struct, Type::Bool, Type::Struct("Foo".to_string()));

    // Enum → primitive (invalid)
    test_invalid_promotion!(test_invalid_enum_to_i32,  Type::Enum("Status".to_string()), Type::I32);
    test_invalid_promotion!(test_invalid_enum_to_bool, Type::Enum("Status".to_string()), Type::Bool);

    // Primitive → Enum (invalid)
    test_invalid_promotion!(test_invalid_i32_to_enum,  Type::I32, Type::Enum("Status".to_string()));
    test_invalid_promotion!(test_invalid_bool_to_enum, Type::Bool, Type::Enum("Status".to_string()));

    // Tuple → primitive (invalid)
    test_invalid_promotion!(test_invalid_tuple_to_i32, Type::Tuple(vec![Type::I32, Type::I32]), Type::I32);

    // Pointer → Struct (invalid)
    test_invalid_promotion!(test_invalid_ptr_to_struct, Type::Pointer {
        element: Box::new(Type::U8),
        provenance: crate::types::Provenance::Naked,
        is_mutable: true,
    }, Type::Struct("Foo".to_string()));

    // Struct → Enum (invalid — different compound categories)
    test_invalid_promotion!(test_invalid_struct_to_enum, Type::Struct("Foo".to_string()), Type::Enum("Status".to_string()));

    // ========================================================================
    // Category 15: Full Matrix Report (documentation test)
    // Runs all 400 pairs and prints the promotion matrix.
    // This test always passes — it's for documentation and gap-finding.
    // ========================================================================

    #[test]
    fn test_promotion_matrix_report() {
        let types = all_types();
        let mut supported = 0;
        let mut unsupported = 0;
        let mut failures: Vec<String> = Vec::new();

        for (from_name, from_ty) in &types {
            for (to_name, to_ty) in &types {
                let result = try_promote(from_ty, to_ty);
                if result.is_ok() {
                    supported += 1;
                } else {
                    unsupported += 1;
                    if from_name != to_name {
                        failures.push(format!("{:>16} -> {:<16}: {}", 
                            from_name, to_name, result.unwrap_err()));
                    }
                }
            }
        }

        eprintln!("\n=== Type Promotion Matrix Report ===");
        eprintln!("Total pairs: {}", types.len() * types.len());
        eprintln!("Supported:   {}", supported);
        eprintln!("Unsupported: {}", unsupported);
        if !failures.is_empty() {
            eprintln!("\nUnsupported promotions:");
            for f in &failures {
                eprintln!("  {}", f);
            }
        }
        eprintln!("====================================\n");
    }

    // ========================================================================
    // Category 16: Struct ↔ Concrete Invalid Promotions
    // This tests the exact pattern that caused the merge_sorted_lists bug:
    // Struct("ListNode") → Concrete("Box", [Struct("ListNode")]) must FAIL.
    // promote_numeric must not silently accept mismatched compound types.
    // ========================================================================

    // Struct → Concrete(Box, [Struct]) — the specific Box::new bug pattern
    test_invalid_promotion!(test_invalid_struct_to_boxed_struct,
        Type::Struct("ListNode".to_string()),
        Type::Concrete("std__core__boxed__Box".to_string(), vec![Type::Struct("ListNode".to_string())])
    );

    // Struct → Concrete(Vec, [Struct])
    test_invalid_promotion!(test_invalid_struct_to_vec_struct,
        Type::Struct("Foo".to_string()),
        Type::Concrete("Vec".to_string(), vec![Type::Struct("Foo".to_string())])
    );

    // Concrete(Box, [Struct]) → Struct — reverse direction
    test_invalid_promotion!(test_invalid_boxed_struct_to_struct,
        Type::Concrete("std__core__boxed__Box".to_string(), vec![Type::Struct("ListNode".to_string())]),
        Type::Struct("ListNode".to_string())
    );

    // Struct("A") → Struct("B") — different structs
    test_invalid_promotion!(test_invalid_struct_a_to_struct_b,
        Type::Struct("Foo".to_string()),
        Type::Struct("Bar".to_string())
    );

    // Concrete(Box, [A]) → Concrete(Box, [B]) — same wrapper, different inner type
    // promote_numeric accepts this via base_names_equal (both lowered to same MLIR struct)
    #[test]
    fn test_valid_same_wrapper_different_inner() {
        let from = Type::Concrete("Box".to_string(), vec![Type::Struct("Foo".to_string())]);
        let to = Type::Concrete("Box".to_string(), vec![Type::Struct("Bar".to_string())]);
        let result = try_promote(&from, &to);
        assert!(result.is_ok(),
            "Same-wrapper Concrete (Box<Foo> → Box<Bar>) accepted via base_names_equal, got: {:?}", result.err());
    }

    // Concrete(Box, [A]) → Concrete(Vec, [A]) — different wrapper, same inner type
    test_invalid_promotion!(test_invalid_box_to_vec_same_inner,
        Type::Concrete("Box".to_string(), vec![Type::Struct("Foo".to_string())]),
        Type::Concrete("Vec".to_string(), vec![Type::Struct("Foo".to_string())])
    );

    // Result, Option, HashMap — other common Concrete types

    // Struct → Concrete(Result, [Struct, Struct])
    test_invalid_promotion!(test_invalid_struct_to_result,
        Type::Struct("MyError".to_string()),
        Type::Concrete("Result".to_string(), vec![
            Type::Struct("Data".to_string()),
            Type::Struct("MyError".to_string()),
        ])
    );

    // Struct → Concrete(Option, [Struct])
    test_invalid_promotion!(test_invalid_struct_to_option,
        Type::Struct("Node".to_string()),
        Type::Concrete("Option".to_string(), vec![Type::Struct("Node".to_string())])
    );

    // Concrete(Option, [Struct]) → Struct
    test_invalid_promotion!(test_invalid_option_to_struct,
        Type::Concrete("Option".to_string(), vec![Type::Struct("Node".to_string())]),
        Type::Struct("Node".to_string())
    );

    // Concrete(Result, [A, B]) → Concrete(Option, [A]) — different wrappers
    test_invalid_promotion!(test_invalid_result_to_option,
        Type::Concrete("Result".to_string(), vec![Type::I32, Type::Struct("Err".to_string())]),
        Type::Concrete("Option".to_string(), vec![Type::I32])
    );

    // Concrete(HashMap, [K, V]) → Struct
    test_invalid_promotion!(test_invalid_hashmap_to_struct,
        Type::Concrete("HashMap".to_string(), vec![Type::I64, Type::I64]),
        Type::Struct("Foo".to_string())
    );

    // Enum → Concrete(Box, [Enum])
    test_invalid_promotion!(test_invalid_enum_to_boxed_enum,
        Type::Enum("Status".to_string()),
        Type::Concrete("Box".to_string(), vec![Type::Enum("Status".to_string())])
    );

    // Primitive → Concrete(Option, [Primitive])
    test_invalid_promotion!(test_invalid_i64_to_option_i64,
        Type::I64,
        Type::Concrete("Option".to_string(), vec![Type::I64])
    );
}
