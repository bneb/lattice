// ============================================================================
// Ptr<T> Struct Field & Cast Regression Tests
//
// Guards against three issues:
// 1. u64 as Ptr<T> (inttoptr) and Ptr<T> as u64 (ptrtoint) casts
// 2. Ptr<T> sizing for struct layout (8 bytes on 64-bit)
// 3. Ptr<T> as struct field — promote_numeric must not reject Pointer types
// ============================================================================

#[cfg(test)]
mod tests {
    use salt_front::codegen::context::CodegenContext;
    use salt_front::types::{Type, TypeKey, Provenance};
    use salt_front::grammar::SaltFile;
    use salt_front::registry::StructInfo;
    use std::collections::{BTreeMap, HashMap};

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

    fn ptr_ty(inner: Type) -> Type {
        Type::Pointer {
            element: Box::new(inner),
            provenance: Provenance::Naked,
            is_mutable: false,
        }
    }

    // ========================================================================
    // Test 1: Pointer type has correct size (8 bytes) for struct layout
    // ========================================================================

    #[test]
    fn test_pointer_size_is_8_bytes() {
        let reg = HashMap::new();
        let ptr = ptr_ty(Type::I32);
        assert_eq!(ptr.size_of(&reg), 8, "Ptr<i32> should be 8 bytes");

        let ptr_struct = ptr_ty(Type::Struct("Node".to_string()));
        assert_eq!(ptr_struct.size_of(&reg), 8, "Ptr<Node> should be 8 bytes");
    }

    // ========================================================================
    // Test 2: Struct with Pointer field has correct layout
    // ========================================================================

    #[test]
    fn test_struct_with_pointer_field_layout() {
        // struct ListNode { val: i32, next: Ptr<ListNode> }
        // Expected: 4 (i32) + 4 (padding) + 8 (ptr) = 16 bytes
        let reg = HashMap::new();
        let node_ptr = ptr_ty(Type::Struct("ListNode".to_string()));
        let fields = vec![Type::I32, node_ptr];

        // Compute size manually using field_order
        let mut offset = 0usize;
        let mut max_align = 1usize;
        for ty in &fields {
            let align = ty.align_of(&reg);
            max_align = max_align.max(align);
            offset = (offset + align - 1) & !(align - 1);
            offset += ty.size_of(&reg);
        }
        let total = (offset + max_align - 1) & !(max_align - 1);

        assert_eq!(total, 16, "ListNode {{ i32, Ptr<ListNode> }} should be 16 bytes");
    }

    // ========================================================================
    // Test 3: cast_numeric u64 → Pointer emits inttoptr
    // ========================================================================

    #[test]
    fn test_cast_u64_to_pointer_emits_inttoptr() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let target = ptr_ty(Type::I32);

            let result = ctx.with_lowering_ctx(|lctx| salt_front::codegen::type_bridge::cast_numeric(
                lctx, &mut out, "%addr", &Type::U64, &target
            ));

            assert!(result.is_ok(), "u64 → Ptr<i32> cast should succeed: {:?}", result);
            assert!(
                out.contains("llvm.inttoptr") && out.contains("i64 to !llvm.ptr"),
                "Should emit inttoptr: {}", out
            );
        });
    }

    #[test]
    fn test_cast_i64_to_pointer_emits_inttoptr() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let target = ptr_ty(Type::Struct("Node".to_string()));

            let result = ctx.with_lowering_ctx(|lctx| salt_front::codegen::type_bridge::cast_numeric(
                lctx, &mut out, "%addr", &Type::I64, &target
            ));

            assert!(result.is_ok(), "i64 → Ptr<Node> cast should succeed: {:?}", result);
            assert!(
                out.contains("llvm.inttoptr"),
                "Should emit inttoptr: {}", out
            );
        });
    }

    // ========================================================================
    // Test 4: cast_numeric Pointer → u64 emits ptrtoint
    // ========================================================================

    #[test]
    fn test_cast_pointer_to_u64_emits_ptrtoint() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let source = ptr_ty(Type::I32);

            let result = ctx.with_lowering_ctx(|lctx| salt_front::codegen::type_bridge::cast_numeric(
                lctx, &mut out, "%ptr", &source, &Type::U64
            ));

            assert!(result.is_ok(), "Ptr<i32> → u64 cast should succeed: {:?}", result);
            assert!(
                out.contains("llvm.ptrtoint") && out.contains("!llvm.ptr to i64"),
                "Should emit ptrtoint: {}", out
            );
        });
    }

    // ========================================================================
    // Test 5: Round-trip Pointer cast is identity
    // ========================================================================

    #[test]
    fn test_pointer_roundtrip_u64_ptr_u64() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let ptr = ptr_ty(Type::I64);

            // u64 → Ptr<i64>
            let res1 = ctx.with_lowering_ctx(|lctx| salt_front::codegen::type_bridge::cast_numeric(
                lctx, &mut out, "%addr", &Type::U64, &ptr
            ));
            assert!(res1.is_ok(), "u64 → Ptr should succeed");
            let ptr_var = res1.unwrap();

            // Ptr<i64> → u64
            let res2 = ctx.with_lowering_ctx(|lctx| salt_front::codegen::type_bridge::cast_numeric(
                lctx, &mut out, &ptr_var, &ptr, &Type::U64
            ));
            assert!(res2.is_ok(), "Ptr → u64 should succeed");

            // Both operations should have been emitted
            assert!(out.contains("llvm.inttoptr"), "Missing inttoptr");
            assert!(out.contains("llvm.ptrtoint"), "Missing ptrtoint");
        });
    }

    // ========================================================================
    // Test 6: promote_numeric rejects integer → pointer (Constitutional Guard)
    // This is EXPECTED — implicit promotion should fail; only explicit casts work
    // ========================================================================

    #[test]
    fn test_promote_numeric_rejects_integer_to_pointer() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let target = ptr_ty(Type::I32);

            let result = ctx.with_lowering_ctx(|lctx| salt_front::codegen::type_bridge::promote_numeric(
                lctx, &mut out, "%val", &Type::U64, &target
            ));

            assert!(result.is_err(), "Implicit u64 → Ptr promotion should be rejected");
            let err = result.unwrap_err();
            assert!(
                err.contains("Context Contamination") || err.contains("Cannot promote"),
                "Error should mention context contamination: {}", err
            );
        });
    }

    // ========================================================================
    // Test 7: Pointer → Pointer promotion is identity (same inner type)
    // ========================================================================

    #[test]
    fn test_promote_pointer_to_same_pointer_is_noop() {
        with_ctx!(ctx, {
            let mut out = String::new();
            let ptr = ptr_ty(Type::I32);

            let result = ctx.with_lowering_ctx(|lctx| salt_front::codegen::type_bridge::promote_numeric(
                lctx, &mut out, "%ptr", &ptr, &ptr
            ));

            assert!(result.is_ok(), "Ptr<i32> → Ptr<i32> promotion should be identity: {:?}", result);
            assert!(out.is_empty(), "No MLIR should be emitted for identity promotion: {}", out);
        });
    }

    // ========================================================================
    // Test 8: Struct with Pointer field — MLIR type includes !llvm.ptr
    // ========================================================================

    #[test]
    fn test_struct_with_pointer_field_mlir_type() {
        with_ctx!(ctx, {
            let name = "LinkedNode".to_string();
            let ptr_field = ptr_ty(Type::Struct(name.clone()));
            let fields = vec![Type::I32, ptr_field];

            let mut field_map = HashMap::new();
            field_map.insert("val".to_string(), (0, Type::I32));
            field_map.insert("next".to_string(), (1, ptr_ty(Type::Struct(name.clone()))));

            let info = StructInfo {
                name: name.clone(),
                fields: field_map,
                field_order: fields,
                field_alignments: vec![],
                template_name: None,
                specialization_args: vec![],
            };
            let key = TypeKey { path: vec![], name: name.clone(), specialization: None };
            ctx.struct_registry_mut().insert(key, info);

            let ty = Type::Struct(name);
            let mlir = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();

            // Named structs use opaque references like !struct_LinkedNode
            assert!(
                mlir.contains("LinkedNode"),
                "MLIR type should reference LinkedNode: {}", mlir
            );

            // More importantly: verify the struct registry has correct field layout
            let registry = ctx.struct_registry();
            let info = registry.values().find(|i| i.name == "LinkedNode").unwrap();
            assert_eq!(info.field_order.len(), 2, "LinkedNode should have 2 fields");
            assert_eq!(info.field_order[0], Type::I32, "First field should be i32");
            assert!(
                matches!(info.field_order[1], Type::Pointer { .. }),
                "Second field should be Pointer, got: {:?}", info.field_order[1]
            );
        });
    }
}
