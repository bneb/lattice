
#[cfg(test)]
mod tests {
    use salt_front::codegen::context::CodegenContext;
    use salt_front::types::Type;

    use salt_front::grammar::SaltFile;

    // Helper macro to avoid lifetime complexity in return types
    macro_rules! with_ctx {
        ($name:ident, $block:block) => {
            let file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
            let z3_cfg = z3::Config::new();
            let z3_ctx = z3::Context::new(&z3_cfg);
            #[allow(unused_mut)]
            let mut $name = CodegenContext::new(&file, false, None, &z3_ctx);
            $block
        }
    }

    #[test]
    fn test_resolve_type_simple() {
        with_ctx!(ctx, {
            let ty = Type::I32;
            let mlir_ty = ty.to_mlir_type(&ctx).unwrap();
            assert_eq!(mlir_ty, "i32");
        });
    }

    #[test]
    fn test_resolve_type_bool_storage() {
        with_ctx!(ctx, {
            let ty = Type::Bool;
            // Logical is i1, Storage is i8. 
            // Call storage type conversion to verify storage ABI
            assert_eq!(ty.to_mlir_storage_type(&ctx).unwrap(), "i8"); 
        });
    }

    #[test]
    fn test_resolve_type_tuple() {
        with_ctx!(ctx, {
            let ty = Type::Tuple(vec![Type::I32, Type::F32]);
            assert_eq!(ty.to_mlir_type(&ctx).unwrap(), "!llvm.struct<(i32, f32)>"); 
        });
    }

    #[test]
    fn test_resolve_type_enum() {
        with_ctx!(ctx, {
            // Unknown Enum falls back to opaque struct
            let ty = Type::Enum("Option".to_string());
            let res = ty.to_mlir_type(&ctx);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), "!llvm.struct<\"Option\">");
        });
    }

    #[test]
    fn test_zero_attr_recursive() {
        with_ctx!(ctx, {
            // Test zero initialization for a complex type: Array of Tuples
            // [ (i32, bool), ... ] x 2
            let inner = Type::Tuple(vec![Type::I32, Type::Bool]);
            let arr = Type::Array(Box::new(inner), 2);
            
            // zero_attr should return a defined constant string for MLIR
            let res = salt_front::codegen::type_bridge::zero_attr(&mut ctx, &arr);
            assert!(res.is_ok());
            let s = res.unwrap();
            
            // Explicit recursive list
            assert!(s.contains("[0 : i32, 0 : i8]")); 
        });
    }

    #[test]
    fn test_promote_numeric_logic() {
        with_ctx!(ctx, {
            // Test numeric type promotion/demotion logic
            let mut out = String::new();
            
            // i32 -> i64 (Extension)
            let res = salt_front::codegen::type_bridge::promote_numeric(&mut ctx, &mut out, "%val", &Type::I32, &Type::I64);
            assert!(res.is_ok());
            assert!(out.contains("arith.extsi"));
            
            out.clear();
            // u8 -> i32 (Extension)
            let res = salt_front::codegen::type_bridge::promote_numeric(&mut ctx, &mut out, "%val_u8", &Type::U8, &Type::I32);
            assert!(res.is_ok());
            assert!(out.contains("arith.extui")); // Unsigned extension
            
            out.clear();
            // f32 -> f64 (Float extension)
            let res = salt_front::codegen::type_bridge::promote_numeric(&mut ctx, &mut out, "%val_f", &Type::F32, &Type::F64);
            assert!(res.is_ok());
            assert!(out.contains("arith.extf"));
        });
    }

    #[test]
    fn test_struct_alignment_layout() {
        with_ctx!(ctx, {
            // Struct with i8, i64, i8 -> Expects padding
            // Layout: 
            // 0: i8 (size 1)
            // 1..8: Padding (7 bytes) -> aligned to 8
            // 8: i64 (size 8)
            // 16: i8 (size 1)
            // 17..24: Padding (7 bytes) -> struct size aligned to max align (8)
            
            // We need to manually register the struct info to simulate a parsed struct
            use salt_front::registry::StructInfo;
            use std::collections::{BTreeMap, HashMap};
            use salt_front::types::TypeKey;
            
            let name = "PaddingTest".to_string();
            let fields = vec![Type::I8, Type::I64, Type::I8];
            
            let info = StructInfo {
                name: name.clone(),
                fields: HashMap::new(), 
                field_order: fields,
                template_name: None,
                specialization_args: vec![],
            };
            
            let key = TypeKey { path: vec![], name: name.clone(), specialization: None };
            ctx.struct_registry_mut().insert(key, info);
            
            let ty = Type::Struct(name);
            let mlir = ty.to_mlir_type(&ctx).unwrap();
            
            // Check for padding fields
            assert!(mlir.contains("!llvm.array<7 x i8>"), "Missing internal padding: {}", mlir);
            // We expect two 7-byte paddings (one internal, one tail)
            let matches: Vec<_> = mlir.match_indices("!llvm.array<7 x i8>").collect();
            assert_eq!(matches.len(), 2, "Expected 2 padding regions (internal and tail)");
        });
    }

    #[test]
    fn test_recursion_cycle_detection() {
        with_ctx!(ctx, {
            // Struct Node { val: i32, next: Owned<Node> }
            // Owned<Node> -> !llvm.ptr, so it breaks the Cycle at the Storage level usually.
            // BUT to_mlir_type for Struct calculates layout.
            // Wait, Type::Owned is !llvm.ptr, it doesn't recurse into inner for MlirType of the POINTER.
            // But if we have Type::Struct("Node") which contains Type::Struct("Node") directly (illegal, infinite size), 
            // then VISITED check prevents infinite loop.
            
            let name = "Cycle".to_string();
            // Direct recursion (infinite size type, but legal to define, illegal to instantiate?)
            // Salt parser might allow it.
            let fields = vec![Type::I32, Type::Struct(name.clone())];
            
            use salt_front::registry::StructInfo;
            use std::collections::{BTreeMap, HashMap};
             let info = StructInfo {
                name: name.clone(),
                fields: HashMap::new(),
                field_order: fields,
                template_name: None,
                specialization_args: vec![],
            };
            let key = salt_front::types::TypeKey { path: vec![], name: name.clone(), specialization: None };
            ctx.struct_registry_mut().insert(key, info);
            
            let ty = Type::Struct(name.clone());
            let mlir = ty.to_mlir_type(&ctx).unwrap();
            
            // Should contain "!llvm.struct<"Cycle">" inside the definition
            // The outer definition will be !llvm.struct<"Cycle", (i32, !llvm.struct<"Cycle">)>
            // Wait, typically we don't allow value recursion. Pointers break it.
            // But verify the VISITED set works to stop infinite emission.
            assert!(mlir.contains(&format!("!llvm.struct<\"{}\">", name)));
        });
    }

    #[test]
    fn test_exhaustive_numeric_promotions() {
        with_ctx!(ctx, {
            let numeric_types = vec![
                Type::I8, Type::U8, Type::I16, Type::U16, 
                Type::I32, Type::U32, Type::I64, Type::U64,
                Type::Usize, // Index type - critical for affine loop compatibility
                Type::F32, Type::F64
            ];
            
            for t1 in &numeric_types {
                for t2 in &numeric_types {
                    let mut out = String::new();
                    let res = salt_front::codegen::type_bridge::promote_numeric(&mut ctx, &mut out, "%val", t1, t2);
                    
                    // Self-promotion is always Ok
                    if t1 == t2 {
                        assert!(res.is_ok(), "Self promotion failed for {:?}", t1);
                        continue;
                    }
                    
                    // Specific invalid promotions
                    if matches!(t1, Type::F32 | Type::F64) && matches!(t2, Type::I8 | Type::U8 | Type::I16 | Type::U16 | Type::I32 | Type::U32 | Type::I64 | Type::U64) {
                        assert!(res.is_err(), "Implicit Float->Int promotion should fail: {:?} -> {:?}", t1, t2);
                    } else if matches!(t2, Type::F32) && matches!(t1, Type::F64) {
                         assert!(res.is_err(), "Implicit F64->F32 should fail");
                    } else {
                         // Widening Int->Int, Int->Float should be ok
                         if matches!(t1, Type::I32) && matches!(t2, Type::I64) {
                             assert!(res.is_ok());
                             assert!(out.contains("arith.extsi"));
                         }
                    }
                }
            }
            
            // Boundary checks
            let mut out = String::new();
            let res = salt_front::codegen::type_bridge::promote_numeric(&mut ctx, &mut out, "%val", &Type::Bool, &Type::I32);
            assert!(res.is_err());
        });
    }

    #[test]
    fn test_enum_recursive_equality() {
         with_ctx!(ctx, {
            use salt_front::registry::EnumInfo;

            // Enum List { Cons(Box<List>), Nil }
            let name = "List".to_string();
            let variants = vec![
                ("Cons".to_string(), Some(Type::Owned(Box::new(Type::Enum(name.clone())))), 0),
                ("Nil".to_string(), None, 1),
            ];
            
            let info = EnumInfo {
                name: name.clone(),
                variants,
                max_payload_size: 8, // Pointer size
                template_name: None,
                specialization_args: vec![],
            };
            let key = salt_front::types::TypeKey { path: vec![], name: name.clone(), specialization: None };
            ctx.enum_registry_mut().insert(key, info);
            
            let ty = Type::Enum(name.clone());
            let mut out = String::new();
            let op: syn::BinOp = syn::parse_str("==").unwrap();
            
            // Should generate tag comparison + payload comparison (ptr)
            let res = salt_front::codegen::expr::aggregate_eq::emit_aggregate_eq(&mut ctx, &mut out, &op, "%lhs", "%rhs", &ty);
            assert!(res.is_ok());
            
            // Check for Tag comparison
            assert!(out.contains("%cmp_tag_"), "Missing tag comparison");
            
            // Check for Payload (Ptr) comparison
            // It recursively calls emit_aggregate_eq for payload array?
            // "Field 2: Payload [u8; max]" -> It treats payload as byte array for equality!
            // Wait, aggregate_eq for Enum converts payload to [u8; N] and compares bytes?
            // Code line 147: let pay_ty = Type::Array(Box::new(Type::U8), info.max_payload_size);
            // Yes. It blindly compares bytes.
            // This confirms the "Boolean Law" issue: Uninitialized bytes in smaller variants might cause spurious inequality 
            // if we don't zero-init. But layout-wise, coverage touches lines 137-152.
            assert!(out.contains("cmp_arr_"), "Missing payload byte comparison");
         });
    }

    #[test]
    fn test_zero_sized_variant_mix() {
         with_ctx!(ctx, {
            use salt_front::registry::EnumInfo;
            
            // Enum Mixed { Unit, Large([u8; 100]) }
            let name = "Mixed".to_string();
            // max_payload_size = 100
            
            let info = EnumInfo {
                name: name.clone(),
                variants: vec![], // Variants don't matter for aggregate_eq logic, only max_payload_size
                max_payload_size: 100,
                template_name: None,
                specialization_args: vec![],
            };
            let key = salt_front::types::TypeKey { path: vec![], name: name.clone(), specialization: None };
            ctx.enum_registry_mut().insert(key, info);
            
            let ty = Type::Enum(name);
            let mut out = String::new();
            let op: syn::BinOp = syn::parse_str("==").unwrap();
            
            let res = salt_front::codegen::expr::aggregate_eq::emit_aggregate_eq(&mut ctx, &mut out, &op, "%lhs", "%rhs", &ty);
            assert!(res.is_ok());
            
            // Should contain padding check (Field 1) and Payload check (Field 2)
            // Padding [u8; 4] implies loop 0..4
            assert!(out.contains("pad_l_"), "Missing padding extract");
            assert!(out.contains("pay_l_"), "Missing payload extract");
         });
    }
    #[test]
    fn test_heterogeneous_enum_payload() {
        with_ctx!(ctx, {
            use salt_front::registry::EnumInfo;
            // Enum Hetero { Tiny(u8), Huge([f64; 8]) }
            // Tiny: size 1
            // Huge: size 64
            // Max payload size = 64.
            
            let name = "Hetero".to_string();
            let tiny_ty = Type::U8;
            let huge_ty = Type::Array(Box::new(Type::F64), 8);
            
            let reg = std::collections::HashMap::new();
            assert_eq!(tiny_ty.size_of(&reg), 1);
            assert_eq!(huge_ty.size_of(&reg), 64);
            
            let max_payload = 64;
            
            let info = EnumInfo {
                name: name.clone(),
                variants: vec![
                    ("Tiny".to_string(), Some(tiny_ty), 0),
                    ("Huge".to_string(), Some(huge_ty), 1),
                ],
                max_payload_size: max_payload,
                template_name: None,
                specialization_args: vec![],
            };
            
            let key = salt_front::types::TypeKey { path: vec![], name: name.clone(), specialization: None };
            ctx.enum_registry_mut().insert(key, info);
            
            let ty = Type::Enum(name.clone());
            let mlir = ty.to_mlir_type(&ctx).unwrap();
            
            // Expected: !llvm.struct<"Hetero", (i32, !llvm.array<4 x i8>, !llvm.array<64 x i8>)>
            // Note: Padding calc logic check:
            // "if info.max_payload_size > 0 { fields.push("!llvm.array<4 x i8>"); fields.push("!llvm.array<64 x i8>"); }"
            assert!(mlir.contains("!llvm.array<64 x i8>"), "Missing payload array of size 64: {}", mlir);
        });
    }

    #[test]
    fn test_recursive_list_equality() {
        with_ctx!(ctx, {
             use salt_front::registry::EnumInfo;
             // Enum List { Cons(i32, Box<List>), Nil }
             // Payload: Tuple(i32, Owned) -> Size 16 (4 + 4pad + 8).
             
             let name = "List".to_string();
             let list_ref = Type::Enum(name.clone());
             
             let cons_payload = Type::Tuple(vec![
                 Type::I32,
                 Type::Owned(Box::new(list_ref))
             ]);
             
             let reg = std::collections::HashMap::new();
             assert_eq!(cons_payload.size_of(&reg), 16, "Cons payload size mismatch");
             
             let info = EnumInfo {
                 name: name.clone(),
                 variants: vec![
                     ("Cons".to_string(), Some(cons_payload), 0),
                     ("Nil".to_string(), None, 1),
                 ],
                 max_payload_size: 16,
                 template_name: None,
                 specialization_args: vec![],
             };
             let key = salt_front::types::TypeKey { path: vec![], name: name.clone(), specialization: None };
             ctx.enum_registry_mut().insert(key, info);
             
             let ty = Type::Enum(name);
             let mut out = String::new();
             let op: syn::BinOp = syn::parse_str("==").unwrap();
             
             let res = salt_front::codegen::expr::aggregate_eq::emit_aggregate_eq(&mut ctx, &mut out, &op, "%lhs", "%rhs", &ty);
             assert!(res.is_ok());
             
             assert!(out.contains("cmp_tag_"), "Missing tag comparison");
             // Payload [u8; 16] -> unrolled loop -> cmp_arr_ or similar
             assert!(out.contains("cmp_arr_") || out.contains("icmp"), "Missing payload comparison");
        });
    }

    // ============================================================================
    // Index Type (Usize) Conversion Tests
    // Guards against MLIR type mismatches with affine loop induction variables
    // ============================================================================

    #[test]
    fn test_usize_to_i64_emits_index_cast() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let res = salt_front::codegen::type_bridge::promote_numeric(
                &mut ctx, &mut out, "%iv", &Type::Usize, &Type::I64
            );
            assert!(res.is_ok(), "Usize->I64 promotion failed: {:?}", res);
            assert!(
                out.contains("arith.index_cast") && out.contains("index to i64"),
                "Expected arith.index_cast ... : index to i64, got: {}", out
            );
        });
    }

    #[test]
    fn test_i64_to_usize_emits_index_cast() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let res = salt_front::codegen::type_bridge::promote_numeric(
                &mut ctx, &mut out, "%val", &Type::I64, &Type::Usize
            );
            assert!(res.is_ok(), "I64->Usize promotion failed: {:?}", res);
            assert!(
                out.contains("arith.index_cast") && out.contains("i64 to index"),
                "Expected arith.index_cast ... : i64 to index, got: {}", out
            );
        });
    }

    #[test]
    fn test_usize_to_i32_emits_index_cast_then_trunci() {
        with_ctx!(ctx, {
            let mut out = String::new();
            // Use cast_numeric for truncation (smaller type)
            let res = salt_front::codegen::type_bridge::cast_numeric(
                &mut ctx, &mut out, "%idx", &Type::Usize, &Type::I32
            );
            assert!(res.is_ok(), "Usize->I32 cast failed: {:?}", res);
            // Should emit index_cast to i64 first, then trunci to i32
            assert!(
                out.contains("arith.index_cast") && out.contains("index to i64"),
                "Missing index_cast to i64: {}", out
            );
            assert!(
                out.contains("arith.trunci") && out.contains("i64 to i32"),
                "Missing trunci to i32: {}", out
            );
        });
    }

    #[test]
    fn test_i32_to_usize_emits_extsi_then_index_cast() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let res = salt_front::codegen::type_bridge::promote_numeric(
                &mut ctx, &mut out, "%val32", &Type::I32, &Type::Usize
            );
            assert!(res.is_ok(), "I32->Usize promotion failed: {:?}", res);
            // Should emit extsi i32 to i64, then index_cast to index
            assert!(
                out.contains("arith.extsi") && out.contains("i32 to i64"),
                "Missing extsi to i64: {}", out
            );
            assert!(
                out.contains("arith.index_cast") && out.contains("i64 to index"),
                "Missing index_cast to index: {}", out
            );
        });
    }

    #[test]
    fn test_usize_self_promotion_is_noop() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let res = salt_front::codegen::type_bridge::promote_numeric(
                &mut ctx, &mut out, "%idx", &Type::Usize, &Type::Usize
            );
            assert!(res.is_ok());
            // Self promotion should return same variable, no MLIR emitted
            assert!(out.is_empty() || !out.contains("arith."), "Usize self-promotion should be no-op: {}", out);
        });
    }

    #[test]
    fn test_usize_to_mlir_type_is_index() {
        with_ctx!(ctx, {
            let mlir = Type::Usize.to_mlir_type(&ctx).unwrap();
            assert_eq!(mlir, "index", "Usize should map to MLIR 'index' type, got: {}", mlir);
        });
    }

    // ============================================================================
    // Layout-Aware Cast Validation Tests (Audit Fix)
    // Guards against unsound struct-to-struct casts
    // ============================================================================

    #[test]
    fn test_prove_layout_compatibility_same_size() {
        with_ctx!(ctx, {
            use salt_front::registry::StructInfo;
            use std::collections::{BTreeMap, HashMap};
            use salt_front::types::TypeKey;
            
            // Register two structs with identical layouts: { x: i64, y: i64 }
            for name in &["StructA", "StructB"] {
                let fields = vec![Type::I64, Type::I64];
                let info = StructInfo {
                    name: name.to_string(),
                    fields: HashMap::new(),
                    field_order: fields,
                    template_name: None,
                    specialization_args: vec![],
                };
                let key = TypeKey { path: vec![], name: name.to_string(), specialization: None };
                ctx.struct_registry_mut().insert(key, info);
            }
            
            let compatible = salt_front::codegen::type_bridge::prove_layout_compatibility(
                &ctx,
                &Type::Struct("StructA".to_string()),
                &Type::Struct("StructB".to_string())
            );
            assert!(compatible, "Identically-sized structs should be compatible");
        });
    }

    #[test]
    fn test_prove_layout_compatibility_different_size() {
        with_ctx!(ctx, {
            use salt_front::registry::StructInfo;
            use std::collections::{BTreeMap, HashMap};
            use salt_front::types::TypeKey;
            
            // Small: { x: i32 } = 4 bytes
            let small_info = StructInfo {
                name: "Small".to_string(),
                fields: HashMap::new(),
                field_order: vec![Type::I32],
                template_name: None,
                specialization_args: vec![],
            };
            let small_key = TypeKey { path: vec![], name: "Small".to_string(), specialization: None };
            ctx.struct_registry_mut().insert(small_key, small_info);
            
            // Large: { x: i64, y: i64 } = 16 bytes  
            let large_info = StructInfo {
                name: "Large".to_string(),
                fields: HashMap::new(),
                field_order: vec![Type::I64, Type::I64],
                template_name: None,
                specialization_args: vec![],
            };
            let large_key = TypeKey { path: vec![], name: "Large".to_string(), specialization: None };
            ctx.struct_registry_mut().insert(large_key, large_info);
            
            let compatible = salt_front::codegen::type_bridge::prove_layout_compatibility(
                &ctx,
                &Type::Struct("Small".to_string()),
                &Type::Struct("Large".to_string())
            );
            assert!(!compatible, "Differently-sized structs should NOT be compatible");
        });
    }

    #[test]
    fn test_struct_cast_rejects_incompatible_layouts() {
        with_ctx!(ctx, {
            use salt_front::registry::StructInfo;
            use std::collections::{BTreeMap, HashMap};
            use salt_front::types::TypeKey;
            
            // Small: { x: i32 } = 4 bytes
            let small_info = StructInfo {
                name: "SmallCast".to_string(),
                fields: HashMap::new(),
                field_order: vec![Type::I32],
                template_name: None,
                specialization_args: vec![],
            };
            let small_key = TypeKey { path: vec![], name: "SmallCast".to_string(), specialization: None };
            ctx.struct_registry_mut().insert(small_key, small_info);
            
            // Large: { x: i64, y: i64 } = 16 bytes
            let large_info = StructInfo {
                name: "LargeCast".to_string(),
                fields: HashMap::new(),
                field_order: vec![Type::I64, Type::I64],
                template_name: None,
                specialization_args: vec![],
            };
            let large_key = TypeKey { path: vec![], name: "LargeCast".to_string(), specialization: None };
            ctx.struct_registry_mut().insert(large_key, large_info);
            
            let mut out = String::new();
            let result = salt_front::codegen::type_bridge::cast_numeric(
                &ctx, &mut out, "%val",
                &Type::Struct("SmallCast".to_string()),
                &Type::Struct("LargeCast".to_string())
            );
            
            assert!(result.is_err(), "Should reject incompatible struct cast");
            let err = result.unwrap_err();
            assert!(err.contains("FORMAL INTEGRITY ERROR"), "Error message should mention FORMAL INTEGRITY ERROR: {}", err);
            assert!(err.contains("Layout compatibility"), "Error should mention layout: {}", err);
        });
    }

    #[test]
    fn test_struct_cast_accepts_compatible_layouts() {
        with_ctx!(ctx, {
            use salt_front::registry::StructInfo;
            use std::collections::{BTreeMap, HashMap};
            use salt_front::types::TypeKey;
            
            // Both structs have identical layout: { i64, i64 } = 16 bytes, align 8
            for name in &["CompatA", "CompatB"] {
                let info = StructInfo {
                    name: name.to_string(),
                    fields: HashMap::new(),
                    field_order: vec![Type::I64, Type::I64],
                    template_name: None,
                    specialization_args: vec![],
                };
                let key = TypeKey { path: vec![], name: name.to_string(), specialization: None };
                ctx.struct_registry_mut().insert(key, info);
            }
            
            let mut out = String::new();
            let result = salt_front::codegen::type_bridge::cast_numeric(
                &ctx, &mut out, "%val",
                &Type::Struct("CompatA".to_string()),
                &Type::Struct("CompatB".to_string())
            );
            
            assert!(result.is_ok(), "Should accept compatible struct cast: {:?}", result);
            assert!(out.contains("llvm.bitcast"), "Should emit llvm.bitcast for compatible cast: {}", out);
        });
    }

    #[test]
    fn test_prove_layout_compatibility_primitives() {
        with_ctx!(ctx, {
            // Same-size primitives should be compatible
            assert!(
                salt_front::codegen::type_bridge::prove_layout_compatibility(&ctx, &Type::I64, &Type::U64),
                "i64 and u64 should be compatible (same size/align)"
            );
            assert!(
                salt_front::codegen::type_bridge::prove_layout_compatibility(&ctx, &Type::I32, &Type::F32),
                "i32 and f32 should be compatible (both 4 bytes, align 4)"
            );
            
            // Different-size primitives should NOT be compatible
            assert!(
                !salt_front::codegen::type_bridge::prove_layout_compatibility(&ctx, &Type::I32, &Type::I64),
                "i32 and i64 should NOT be compatible (4 vs 8 bytes)"
            );
            assert!(
                !salt_front::codegen::type_bridge::prove_layout_compatibility(&ctx, &Type::I8, &Type::I32),
                "i8 and i32 should NOT be compatible (1 vs 4 bytes)"
            );
        });
    }

    #[test]
    fn test_prove_layout_compatibility_same_size_different_align() {
        with_ctx!(ctx, {
            use salt_front::registry::StructInfo;
            use std::collections::{BTreeMap, HashMap};
            use salt_front::types::TypeKey;
            
            // StructAlign8: { x: i64 } = 8 bytes, align 8
            let align8_info = StructInfo {
                name: "StructAlign8".to_string(),
                fields: HashMap::new(),
                field_order: vec![Type::I64],
                template_name: None,
                specialization_args: vec![],
            };
            let align8_key = TypeKey { path: vec![], name: "StructAlign8".to_string(), specialization: None };
            ctx.struct_registry_mut().insert(align8_key, align8_info);
            
            // StructAlign4: { x: i32, y: i32 } = 8 bytes, align 4
            let align4_info = StructInfo {
                name: "StructAlign4".to_string(),
                fields: HashMap::new(),
                field_order: vec![Type::I32, Type::I32],
                template_name: None,
                specialization_args: vec![],
            };
            let align4_key = TypeKey { path: vec![], name: "StructAlign4".to_string(), specialization: None };
            ctx.struct_registry_mut().insert(align4_key, align4_info);
            
            // Both 8 bytes, but different alignments (8 vs 4)
            let compatible = salt_front::codegen::type_bridge::prove_layout_compatibility(
                &ctx,
                &Type::Struct("StructAlign8".to_string()),
                &Type::Struct("StructAlign4".to_string())
            );
            // In practice, these may or may not be compatible depending on the ABI.
            // Our implementation checks both size AND alignment, so this should fail.
            assert!(!compatible, "Same size but different alignment should NOT be compatible");
        });
    }

    #[test]
    fn test_prove_layout_compatibility_self() {
        with_ctx!(ctx, {
            // A type should always be compatible with itself
            assert!(
                salt_front::codegen::type_bridge::prove_layout_compatibility(&ctx, &Type::I64, &Type::I64),
                "i64 should be compatible with itself"
            );
            assert!(
                salt_front::codegen::type_bridge::prove_layout_compatibility(&ctx, &Type::F64, &Type::F64),
                "f64 should be compatible with itself"
            );
        });
    }
}
