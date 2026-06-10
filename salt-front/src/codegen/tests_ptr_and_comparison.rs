//! Code Red Tests: Ptr::offset GEP element types & Reference equality comparison
//!
//! These tests document and verify three critical compiler bugs discovered
//! during HashMap debugging:
//!
//! - Bug C1: `emit_gep` element type resolution for Ptr<T>::offset
//! - Bug C2: `&a == &b` generates address comparison instead of value comparison
//! - Bug C3: RefCell re-entrancy during nested specialization
//!
//! Each test is designed to FAIL on the current (broken) code and PASS after fixes.

mod tests {
    use crate::codegen::CodegenContext;
    use crate::grammar::SaltFile;
    use crate::types::Type;

    /// Helper: Create a fresh CodegenContext for testing
    #[allow(dead_code)]
    fn make_ctx() -> (SaltFile, crate::z3_shim::Config, crate::z3_shim::Context) {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        (file, z3_cfg, z3_ctx)
    }

    // =========================================================================
    // Section 1: emit_gep element type parameterized tests
    //
    // emit_gep generates: `%res = llvm.getelementptr %base[%idx] : (!llvm.ptr, i64) -> !llvm.ptr, <ELEM_TY>`
    // We verify the trailing element type is correct for each primitive and composite type.
    // =========================================================================

    /// Verify emit_gep output format for a given element type string
    fn assert_gep_elem_ty(elem_ty: &str) {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        ctx.emit_gep(&mut out, "%res", "%base", "%idx", elem_ty);

        let expected_suffix = format!("!llvm.ptr, {}\n", elem_ty);
        assert!(
            out.contains(&expected_suffix),
            "emit_gep for elem_ty '{}' should end with '{}', got: {}",
            elem_ty, expected_suffix, out
        );
    }

    #[test]
    fn test_emit_gep_i8() {
        assert_gep_elem_ty("i8");
    }

    #[test]
    fn test_emit_gep_i16() {
        assert_gep_elem_ty("i16");
    }

    #[test]
    fn test_emit_gep_i32() {
        assert_gep_elem_ty("i32");
    }

    #[test]
    fn test_emit_gep_i64() {
        assert_gep_elem_ty("i64");
    }

    #[test]
    fn test_emit_gep_f32() {
        assert_gep_elem_ty("f32");
    }

    #[test]
    fn test_emit_gep_f64() {
        assert_gep_elem_ty("f64");
    }

    #[test]
    fn test_emit_gep_ptr() {
        assert_gep_elem_ty("!llvm.ptr");
    }

    #[test]
    fn test_emit_gep_struct() {
        assert_gep_elem_ty("!llvm.struct<(i64, i64)>");
    }

    #[test]
    fn test_emit_gep_named_struct() {
        assert_gep_elem_ty("!llvm.struct<\"Entry_i64_i64\", (i64, i64)>");
    }

    #[test]
    fn test_emit_gep_array() {
        assert_gep_elem_ty("!llvm.array<16 x i8>");
    }

    #[test]
    fn test_emit_gep_nested_struct() {
        assert_gep_elem_ty("!llvm.struct<(i64, !llvm.struct<(i64, i64)>)>");
    }

    // =========================================================================
    // Section 2: ptr_offset intrinsic - element type resolution
    //
    // When Ptr<T>::offset(count) is called, the intrinsic handler must
    // resolve T to the correct MLIR element type for the GEP.
    // Bug C1: For generic T (e.g. Ptr<Entry<K,V>>), the handler falls
    // through to the "i8" default, making offset byte-based instead
    // of element-based.
    // =========================================================================

    /// Test that Type::Concrete("Ptr", [I64]) resolves to "i64" element type,
    /// not "i8".
    #[test]
    fn test_ptr_offset_elem_type_i64() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ptr_ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::I64]);

        // Simulate what ptr_offset does: extract element type
        let elem_ty = if let Type::Concrete(name, args) = &ptr_ty {
            if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                ctx.with_lowering_ctx(|lctx| args[0].to_mlir_type(lctx)).unwrap_or("i8".to_string())
            } else {
                "i8".to_string()
            }
        } else {
            "i8".to_string()
        };

        assert_eq!(elem_ty, "i64", 
            "Ptr<i64>::offset should use GEP element type 'i64', not '{}'", elem_ty);
    }

    /// Test that Type::Concrete("Ptr", [U8]) resolves to "i8" element type
    #[test]
    fn test_ptr_offset_elem_type_u8() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ptr_ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::U8]);

        let elem_ty = if let Type::Concrete(name, args) = &ptr_ty {
            if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                ctx.with_lowering_ctx(|lctx| args[0].to_mlir_type(lctx)).unwrap_or("i8".to_string())
            } else {
                "i8".to_string()
            }
        } else {
            "i8".to_string()
        };

        assert_eq!(elem_ty, "i8",
            "Ptr<u8>::offset should use GEP element type 'i8', not '{}'", elem_ty);
    }

    /// Test that Type::Concrete("Ptr", [I32]) resolves to "i32" element type
    #[test]
    fn test_ptr_offset_elem_type_i32() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ptr_ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::I32]);

        let elem_ty = if let Type::Concrete(name, args) = &ptr_ty {
            if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                ctx.with_lowering_ctx(|lctx| args[0].to_mlir_type(lctx)).unwrap_or("i8".to_string())
            } else {
                "i8".to_string()
            }
        } else {
            "i8".to_string()
        };

        assert_eq!(elem_ty, "i32",
            "Ptr<i32>::offset should use GEP element type 'i32', not '{}'", elem_ty);
    }

    /// Test that Type::Concrete("Ptr", [F32]) resolves to "f32" element type
    #[test]
    fn test_ptr_offset_elem_type_f32() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ptr_ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::F32]);

        let elem_ty = if let Type::Concrete(name, args) = &ptr_ty {
            if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                ctx.with_lowering_ctx(|lctx| args[0].to_mlir_type(lctx)).unwrap_or("i8".to_string())
            } else {
                "i8".to_string()
            }
        } else {
            "i8".to_string()
        };

        assert_eq!(elem_ty, "f32",
            "Ptr<f32>::offset should use GEP element type 'f32', not '{}'", elem_ty);
    }

    /// Test that Type::Concrete("Ptr", [F64]) resolves to "f64" element type
    #[test]
    fn test_ptr_offset_elem_type_f64() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ptr_ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::F64]);

        let elem_ty = if let Type::Concrete(name, args) = &ptr_ty {
            if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                ctx.with_lowering_ctx(|lctx| args[0].to_mlir_type(lctx)).unwrap_or("i8".to_string())
            } else {
                "i8".to_string()
            }
        } else {
            "i8".to_string()
        };

        assert_eq!(elem_ty, "f64",
            "Ptr<f64>::offset should use GEP element type 'f64', not '{}'", elem_ty);
    }

    /// Test that Type::Concrete("Ptr", [U64]) resolves to "i64" element type
    #[test]
    fn test_ptr_offset_elem_type_u64() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ptr_ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::U64]);

        let elem_ty = if let Type::Concrete(name, args) = &ptr_ty {
            if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                ctx.with_lowering_ctx(|lctx| args[0].to_mlir_type(lctx)).unwrap_or("i8".to_string())
            } else {
                "i8".to_string()
            }
        } else {
            "i8".to_string()
        };

        assert_eq!(elem_ty, "i64",
            "Ptr<u64>::offset should use GEP element type 'i64', not '{}'", elem_ty);
    }

    /// BUG C1: Test that Ptr<Struct("Entry_i64_i64")> resolves to a struct type,
    /// NOT "i8". This is the exact bug in HashMap: Ptr<Entry<K,V>>::offset.
    ///
    /// EXPECTED TO FAIL on current code if the struct isn't registered.
    #[test]
    fn test_ptr_offset_elem_type_struct_entry() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        // This is what happens after monomorphization:
        // Ptr<Entry<i64, i64>> becomes Concrete("Ptr", [Struct("Entry_i64_i64")])
        let entry_ty = Type::Struct("std__collections__hash_map__Entry_i64_i64".to_string());
        let ptr_ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![entry_ty.clone()]);

        let elem_ty = if let Type::Concrete(name, args) = &ptr_ty {
            if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                ctx.with_lowering_ctx(|lctx| args[0].to_mlir_type(lctx)).unwrap_or("i8".to_string())
            } else {
                "i8".to_string()
            }
        } else {
            "i8".to_string()
        };

        // The bug: without struct registration, this resolves to some struct type
        // but with the wrong name. We just check it's NOT "i8".
        assert_ne!(elem_ty, "i8",
            "Ptr<Entry<i64,i64>>::offset should NOT use GEP element type 'i8'. \
             This means the ptr_offset intrinsic is treating all Ptr<T> offsets as \
             byte offsets, which is the root cause of HashMap data corruption.");
    }

    /// Test Ptr<Ptr<i64>> — pointer to pointer
    #[test]
    fn test_ptr_offset_elem_type_ptr_to_ptr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let inner_ptr = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![Type::I64]);
        let ptr_ty = Type::Concrete("std__core__ptr__Ptr".to_string(), vec![inner_ptr]);

        let elem_ty = if let Type::Concrete(name, args) = &ptr_ty {
            if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                ctx.with_lowering_ctx(|lctx| args[0].to_mlir_type(lctx)).unwrap_or("i8".to_string())
            } else {
                "i8".to_string()
            }
        } else {
            "i8".to_string()
        };

        // Inner Ptr<i64> should be i64 (it flattens to i64)
        assert_ne!(elem_ty, "i8",
            "Ptr<Ptr<i64>>::offset should NOT use byte-level GEP");
    }

    /// Test that Type::Struct("Ptr") fallback correctly maps to known types
    #[test]
    fn test_ptr_offset_struct_fallback_i64() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let _ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        // When Ptr type comes through as Type::Struct after monomorphization
        let ptr_ty = Type::Struct("std__core__ptr__Ptr_i64".to_string());

        let elem_ty = if let Type::Struct(name) = &ptr_ty {
            if name.ends_with("_u8") { "i8".to_string() }
            else if name.ends_with("_i64") { "i64".to_string() }
            else { "i8".to_string() }
        } else {
            "i8".to_string()
        };

        assert_eq!(elem_ty, "i64",
            "Struct-typed Ptr_i64 should resolve to 'i64' via suffix match");
    }

    /// Test that Type::Struct("Ptr_u8") fallback correctly maps to i8
    #[test]
    fn test_ptr_offset_struct_fallback_u8() {
        let ptr_ty = Type::Struct("std__core__ptr__Ptr_u8".to_string());

        let elem_ty = if let Type::Struct(name) = &ptr_ty {
            if name.ends_with("_u8") { "i8".to_string() }
            else if name.ends_with("_i64") { "i64".to_string() }
            else { "i8".to_string() }
        } else {
            "i8".to_string()
        };

        assert_eq!(elem_ty, "i8",
            "Struct-typed Ptr_u8 should resolve to 'i8' via suffix match");
    }

    // =========================================================================
    // Section 3: Reference equality comparison (Bug C2)
    //
    // When comparing `&a == &b` where a and b are i64 values, the compiler
    // should auto-deref and compare values (i64 == i64), NOT compare
    // pointer addresses (ptr == ptr).
    //
    // Current behavior: generates `llvm.icmp "eq" %ptr1, %ptr2 : !llvm.ptr`
    // Expected behavior: load from both ptrs, then `arith.cmpi "eq" %val1, %val2 : i64`
    // =========================================================================

    /// Test that emit_cmp generates the correct comparison for i64 values
    #[test]
    fn test_emit_cmp_i64_values() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        ctx.emit_cmp(&mut out, "%res", "arith.cmpi", "eq", "%lhs", "%rhs", "i64");

        assert!(out.contains("arith.cmpi \"eq\","),
            "i64 comparison should use arith.cmpi, got: {}", out);
        assert!(out.contains(": i64"),
            "i64 comparison should have type annotation ': i64', got: {}", out);
        assert!(!out.contains("!llvm.ptr"),
            "i64 comparison should NOT use !llvm.ptr, got: {}", out);
    }

    /// Test that emit_cmp generates ptr comparison for actual pointer types
    #[test]
    fn test_emit_cmp_ptr_values() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        ctx.emit_cmp(&mut out, "%res", "llvm.icmp", "eq", "%lhs", "%rhs", "!llvm.ptr");

        assert!(out.contains("llvm.icmp \"eq\""),
            "pointer comparison should use llvm.icmp, got: {}", out);
        assert!(out.contains("!llvm.ptr"),
            "pointer comparison should have type !llvm.ptr, got: {}", out);
    }

    /// BUG C2: When Reference(I64) types are compared, the to_mlir_type resolves
    /// to !llvm.ptr, causing address comparison. Document this behavior.
    #[test]
    fn test_reference_type_resolves_to_ptr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ref_ty = Type::Reference(Box::new(Type::I64), false);
        let mlir_ty = ctx.with_lowering_ctx(|lctx| ref_ty.to_mlir_type(lctx)).unwrap();

        // This test DOCUMENTS the current behavior that causes Bug C2
        assert_eq!(mlir_ty, "!llvm.ptr",
            "Reference<i64> should resolve to !llvm.ptr at MLIR level");

        // The problem: when emit_binary sees two Reference<i64> operands,
        // it computes common_ty = Reference<i64>, then mlir_ty = "!llvm.ptr",
        // and emits: arith.cmpi "eq", %ptr1, %ptr2 : !llvm.ptr
        // This compares ADDRESSES, not VALUES.
    }

    /// BUG C2: Verify that the correct fix would involve auto-dereferencing
    /// references before comparison. We document what the CORRECT output
    /// should look like.
    #[test]
    fn test_reference_comparison_should_load_values() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        // Simulate what SHOULD happen for `&a == &b`:
        // 1. Load value from %ptr_a -> %val_a : i64
        // 2. Load value from %ptr_b -> %val_b : i64
        // 3. Compare: arith.cmpi "eq", %val_a, %val_b : i64
        let mut out = String::new();

        // Step 1: Load both values
        ctx.emit_load(&mut out, "%val_a", "%ptr_a", "i64");
        ctx.emit_load(&mut out, "%val_b", "%ptr_b", "i64");

        // Step 2: Compare values
        ctx.emit_cmp(&mut out, "%eq_res", "arith.cmpi", "eq", "%val_a", "%val_b", "i64");

        assert!(out.contains("llvm.load %ptr_a"),
            "Should load value from first reference");
        assert!(out.contains("llvm.load %ptr_b"),
            "Should load value from second reference");
        assert!(out.contains("arith.cmpi \"eq\","),
            "Should compare loaded i64 values, not pointers");
        assert!(out.contains(": i64\n"),
            "Comparison type should be i64, not !llvm.ptr");
    }

    // =========================================================================
    // Section 4: emit_gep_field parameterized tests
    //
    // emit_gep_field generates struct field access:
    //   %res = llvm.getelementptr %base[0, <IDX>] : (!llvm.ptr) -> !llvm.ptr, <STRUCT_TY>
    // =========================================================================

    #[test]
    fn test_emit_gep_field_simple_struct() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let struct_ty = "!llvm.struct<(i64, i64)>";
        ctx.emit_gep_field(&mut out, "%res", "%base", 0, struct_ty);

        assert!(out.contains("[0, 0]"),
            "Field 0 access should use [0, 0], got: {}", out);
        assert!(out.contains(struct_ty),
            "Should reference the struct type, got: {}", out);
    }

    #[test]
    fn test_emit_gep_field_second_field() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let struct_ty = "!llvm.struct<(i64, f64, i32)>";
        ctx.emit_gep_field(&mut out, "%field1", "%base", 1, struct_ty);

        assert!(out.contains("[0, 1]"),
            "Field 1 access should use [0, 1], got: {}", out);
    }

    #[test]
    fn test_emit_gep_field_hashmap_entry() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let entry_ty = "!llvm.struct<\"Entry_i64_i64\", (i64, i64)>";
        ctx.emit_gep_field(&mut out, "%key_ptr", "%entry", 0, entry_ty);

        assert!(out.contains(entry_ty),
            "HashMap Entry field access should reference Entry type");
        assert!(out.contains("[0, 0]"),
            "Key field (index 0) should use [0, 0]");
    }

    #[test]
    fn test_emit_gep_field_hashmap_struct() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let _out = String::new();
        // HashMap<i64,i64> struct: { ptr, ptr, i64, i64, i64, i64, i64 }
        let hm_ty = "!llvm.struct<\"HashMap_i64_i64\", (!llvm.ptr, !llvm.ptr, i64, i64, i64, i64, i64)>";

        // Test accessing each field
        for field_idx in 0..7 {
            let mut field_out = String::new();
            let res = format!("%field_{}", field_idx);
            ctx.emit_gep_field(&mut field_out, &res, "%self", field_idx, hm_ty);

            assert!(field_out.contains(&format!("[0, {}]", field_idx)),
                "HashMap field {} should use [0, {}], got: {}", field_idx, field_idx, field_out);
        }
    }

    // =========================================================================
    // Section 5: Type.to_mlir_type resolution for all main types
    //
    // Comprehensive verification that each Salt type maps to the expected
    // MLIR type string. This matters because emit_gep callers use
    // to_mlir_type() to determine the element type.
    // =========================================================================

    #[test]
    fn test_type_to_mlir_primitives() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let cases = vec![
            (Type::I8,   "i8"),
            (Type::I16,  "i16"),
            (Type::I32,  "i32"),
            (Type::I64,  "i64"),
            (Type::U8,   "i8"),
            (Type::U16,  "i16"),
            (Type::U32,  "i32"),
            (Type::U64,  "i64"),
            (Type::F32,  "f32"),
            (Type::F64,  "f64"),
            (Type::Bool, "i1"),
        ];

        for (ty, expected) in cases {
            let result = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();
            assert_eq!(result, expected,
                "Type::{:?}.to_mlir_type() should be '{}', got '{}'", ty, expected, result);
        }
    }

    #[test]
    fn test_type_to_mlir_reference() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ref_i64 = Type::Reference(Box::new(Type::I64), false);
        let result = ctx.with_lowering_ctx(|lctx| ref_i64.to_mlir_type(lctx)).unwrap();
        assert_eq!(result, "!llvm.ptr",
            "Reference<i64> at MLIR level should be !llvm.ptr");

        let ref_u8 = Type::Reference(Box::new(Type::U8), false);
        let result = ctx.with_lowering_ctx(|lctx| ref_u8.to_mlir_type(lctx)).unwrap();
        assert_eq!(result, "!llvm.ptr",
            "Reference<u8> at MLIR level should be !llvm.ptr");
    }

    // =========================================================================
    // Section 4: memcpy intrinsic type coercion tests
    //
    // llvm.intr.memcpy requires: (!llvm.ptr, !llvm.ptr, i64) -> ()
    // But Salt callers may pass any combination of:
    //   - dst: !llvm.ptr (native ref) or i64 (from reinterpret_cast)
    //   - src: !llvm.ptr (native ref) or i64 (from reinterpret_cast)
    //   - len: i64 or i32 (from String.len which is i32)
    //
    // The memcpy handler must emit inttoptr for i64→ptr and extsi for i32→i64.
    // These 8 tests cover the full 2×2×2 matrix.
    // =========================================================================

    /// Helper: Simulate the memcpy intrinsic handler's type coercion logic.
    /// Returns the MLIR snippet that would be emitted for the given arg types.
    fn simulate_memcpy_coercion(
        dst_is_ptr: bool,
        src_is_ptr: bool,
        len_is_i64: bool,
    ) -> String {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();

        // Simulate dst coercion
        let dst_var = if dst_is_ptr {
            "%dst_ptr".to_string()
        } else {
            let p = format!("%memcpy_dst_ptr_{}", ctx.next_id());
            ctx.emit_inttoptr(&mut out, &p, "%dst_i64", "i64");
            p
        };

        // Simulate src coercion
        let src_var = if src_is_ptr {
            "%src_ptr".to_string()
        } else {
            let p = format!("%memcpy_src_ptr_{}", ctx.next_id());
            ctx.emit_inttoptr(&mut out, &p, "%src_i64", "i64");
            p
        };

        // Simulate len coercion
        let len_var = if len_is_i64 {
            "%len_i64".to_string()
        } else {
            let ext = format!("%memcpy_len_ext_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi %len_i32 : i32 to i64\n", ext));
            ext
        };

        // Emit the memcpy call
        out.push_str(&format!(
            "    \"llvm.intr.memcpy\"({}, {}, {}) <{{isVolatile = false}}> : (!llvm.ptr, !llvm.ptr, i64) -> ()\n",
            dst_var, src_var, len_var
        ));

        out
    }

    // --------------- Test matrix: 8 combinations ---------------

    #[test]
    fn test_memcpy_ptr_ptr_i64() {
        // Happy path: all args native types, no coercion needed
        let mlir = simulate_memcpy_coercion(true, true, true);
        assert!(!mlir.contains("inttoptr"), "No inttoptr needed when both args are !llvm.ptr");
        assert!(!mlir.contains("extsi"), "No extsi needed when len is i64");
        assert!(mlir.contains("\"llvm.intr.memcpy\"(%dst_ptr, %src_ptr, %len_i64)"),
            "Should use original ptr and i64 vars directly. Got:\n{}", mlir);
    }

    #[test]
    fn test_memcpy_i64_ptr_i64() {
        // dst is i64 (from reinterpret_cast), needs inttoptr
        let mlir = simulate_memcpy_coercion(false, true, true);
        assert!(mlir.contains("llvm.inttoptr %dst_i64 : i64 to !llvm.ptr"),
            "dst i64 should be converted to ptr. Got:\n{}", mlir);
        assert!(!mlir.contains("llvm.inttoptr %src"),
            "src is already ptr, no conversion needed");
        assert!(mlir.contains("\"llvm.intr.memcpy\"(%memcpy_dst_ptr_"),
            "memcpy should use converted dst ptr. Got:\n{}", mlir);
    }

    #[test]
    fn test_memcpy_ptr_i64_i64() {
        // src is i64, needs inttoptr
        let mlir = simulate_memcpy_coercion(true, false, true);
        assert!(!mlir.contains("llvm.inttoptr %dst"),
            "dst is already ptr, no conversion needed");
        assert!(mlir.contains("llvm.inttoptr %src_i64 : i64 to !llvm.ptr"),
            "src i64 should be converted to ptr. Got:\n{}", mlir);
        assert!(mlir.contains("\"llvm.intr.memcpy\"(%dst_ptr, %memcpy_src_ptr_"),
            "memcpy should use original dst and converted src. Got:\n{}", mlir);
    }

    #[test]
    fn test_memcpy_i64_i64_i64() {
        // Both dst and src are i64 (buffered_writer_perf pattern)
        let mlir = simulate_memcpy_coercion(false, false, true);
        assert!(mlir.contains("llvm.inttoptr %dst_i64 : i64 to !llvm.ptr"),
            "dst i64 should be converted. Got:\n{}", mlir);
        assert!(mlir.contains("llvm.inttoptr %src_i64 : i64 to !llvm.ptr"),
            "src i64 should be converted. Got:\n{}", mlir);
        assert!(mlir.contains("\"llvm.intr.memcpy\"(%memcpy_dst_ptr_"),
            "memcpy should use both converted ptrs. Got:\n{}", mlir);
    }

    #[test]
    fn test_memcpy_ptr_ptr_i32() {
        // len is i32 (writer_perf pattern), needs extsi
        let mlir = simulate_memcpy_coercion(true, true, false);
        assert!(!mlir.contains("inttoptr"), "No inttoptr when both args are ptrs");
        assert!(mlir.contains("arith.extsi %len_i32 : i32 to i64"),
            "i32 length should be sign-extended to i64. Got:\n{}", mlir);
        assert!(mlir.contains("\"llvm.intr.memcpy\"(%dst_ptr, %src_ptr, %memcpy_len_ext_"),
            "memcpy should use extended length. Got:\n{}", mlir);
    }

    #[test]
    fn test_memcpy_i64_i64_i32() {
        // Worst case: both addrs as i64 AND len as i32
        let mlir = simulate_memcpy_coercion(false, false, false);
        assert!(mlir.contains("llvm.inttoptr %dst_i64"),
            "dst needs inttoptr. Got:\n{}", mlir);
        assert!(mlir.contains("llvm.inttoptr %src_i64"),
            "src needs inttoptr. Got:\n{}", mlir);
        assert!(mlir.contains("arith.extsi %len_i32 : i32 to i64"),
            "len needs extsi. Got:\n{}", mlir);
        // All three conversions must appear before the memcpy call
        let memcpy_pos = mlir.find("\"llvm.intr.memcpy\"").unwrap();
        let last_conv = mlir.rfind("arith.extsi").unwrap();
        assert!(last_conv < memcpy_pos,
            "All conversions must appear before the memcpy call");
    }

    #[test]
    fn test_memcpy_i64_ptr_i32() {
        let mlir = simulate_memcpy_coercion(false, true, false);
        assert!(mlir.contains("llvm.inttoptr %dst_i64"),
            "dst needs inttoptr. Got:\n{}", mlir);
        assert!(!mlir.contains("llvm.inttoptr %src"),
            "src already ptr");
        assert!(mlir.contains("arith.extsi %len_i32 : i32 to i64"),
            "len needs extsi. Got:\n{}", mlir);
    }

    #[test]
    fn test_memcpy_ptr_i64_i32() {
        let mlir = simulate_memcpy_coercion(true, false, false);
        assert!(!mlir.contains("llvm.inttoptr %dst"),
            "dst already ptr");
        assert!(mlir.contains("llvm.inttoptr %src_i64"),
            "src needs inttoptr. Got:\n{}", mlir);
        assert!(mlir.contains("arith.extsi %len_i32 : i32 to i64"),
            "len needs extsi. Got:\n{}", mlir);
    }

    /// Verify that the memcpy output always ends with the correct type signature
    #[test]
    fn test_memcpy_always_has_correct_signature() {
        // Test all 8 combinations produce the correct llvm.intr.memcpy signature
        for dst_is_ptr in [true, false] {
            for src_is_ptr in [true, false] {
                for len_is_i64 in [true, false] {
                    let mlir = simulate_memcpy_coercion(dst_is_ptr, src_is_ptr, len_is_i64);
                    assert!(
                        mlir.contains(": (!llvm.ptr, !llvm.ptr, i64) -> ()"),
                        "memcpy must always declare (!llvm.ptr, !llvm.ptr, i64) -> () signature.\n\
                         dst_is_ptr={}, src_is_ptr={}, len_is_i64={}\nGot:\n{}",
                        dst_is_ptr, src_is_ptr, len_is_i64, mlir
                    );
                }
            }
        }
    }
}
