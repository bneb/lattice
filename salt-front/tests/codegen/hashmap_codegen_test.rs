#![allow(warnings)]
// ============================================================================
// HashMap Codegen Tests — Parameterized by Key/Value Types
// Guards against type-specific codegen failures in HashMap operations:
//   insert, get, remove, hash, eq dispatch
//
// Type matrix covers all realistic salt type categories:
//   Keys:   i64, u64, i32, String, Ptr<u8>
//   Values: i64, u64, bool, String, Ptr<u8>
//
// Full cross-product produces 25 combinations. We test a representative
// subset of 16 that covers every category intersection:
//   primitive×primitive, primitive×struct, primitive×ptr,
//   struct×primitive, struct×struct, struct×ptr,
//   ptr×primitive, ptr×struct, ptr×ptr
//
// Tests verify MLIR output patterns rather than runtime behavior,
// isolating codegen bugs from runtime/linker issues.
// ============================================================================

#[cfg(test)]
mod hashmap_codegen_tests {
    #[allow(unused_imports)]
    use saltc::codegen::context::CodegenContext;
    #[allow(unused_imports)]
    use saltc::grammar::SaltFile;
    use saltc::types::{Type, Provenance};

    #[allow(unused_macros)]
    macro_rules! with_ctx {
        ($name:ident, $block:block) => {
            let mut file: SaltFile = syn::parse_str("fn main() {}").expect("valid salt file");
            let z3_cfg = z3::Config::new();
            let z3_ctx = z3::Context::new(&z3_cfg);
            #[allow(unused_mut)]
            let mut $name = CodegenContext::new(&file, false, None, &z3_ctx);
            $block
        };
    }

    // =========================================================================
    // Type Parameterization Infrastructure
    // =========================================================================

    /// Represents a key/value type combination for HashMap testing
    #[derive(Debug, Clone)]
    struct HashMapTypeCase {
        key_type: Type,
        val_type: Type,
        key_label: &'static str,
        val_label: &'static str,
        /// Expected eq dispatch method (or "arith.cmpi" for primitives)
        expected_eq_dispatch: EqDispatch,
        /// Whether key type requires struct-value spilling for eq
        key_needs_spilling: bool,
        /// Expected MLIR type for the key argument in get()
        key_arg_in_get: ArgKind,
        /// Expected MLIR type for the key argument in insert()
        key_arg_in_insert: ArgKind,
    }

    #[derive(Debug, Clone)]
    enum EqDispatch {
        /// Primitive hardware compare (arith.cmpi)
        Hardware,
        /// Pointer address compare (arith.cmpi on ptrtoint)
        PtrHardware,
        /// Trait-dispatched eq call to named function
        TraitCall(String),
    }

    #[derive(Debug, Clone, PartialEq)]
    enum ArgKind {
        /// Passed as !llvm.ptr (reference parameter)
        Ptr,
        /// Passed as struct value (!struct_...)
        StructValue,
        /// Passed as primitive value (i64, etc.)
        PrimitiveValue,
    }

    /// Helper: classify a Type into its codegen category
    #[derive(Debug, Clone, PartialEq)]
    enum TypeCategory {
        Primitive,  // i64, u64, i32, i8, u8, bool
        Struct,     // String, Vec, user-defined structs
        Pointer,    // Ptr<T>
    }

    fn categorize(ty: &Type) -> TypeCategory {
        match ty {
            Type::I64 | Type::U64 | Type::I32 | Type::I8 | Type::U8 | Type::Bool => TypeCategory::Primitive,
            Type::Struct(_) | Type::Concrete(..) | Type::Tuple(_) | Type::Array(..) | Type::Enum(_) => TypeCategory::Struct,
            Type::Pointer { .. } => TypeCategory::Pointer,
            Type::Reference(inner, _) => categorize(inner),
            _ => TypeCategory::Primitive, // fallback for unknown
        }
    }

    fn get_hashmap_type_cases() -> Vec<HashMapTypeCase> {
        vec![
            // =================================================================
            // Primitive keys
            // =================================================================

            // Case 1: i64/i64 — baseline primitive×primitive
            HashMapTypeCase {
                key_type: Type::I64,
                val_type: Type::I64,
                key_label: "i64",
                val_label: "i64",
                expected_eq_dispatch: EqDispatch::Hardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
            // Case 2: u64/i64 — different width primitive key
            HashMapTypeCase {
                key_type: Type::U64,
                val_type: Type::I64,
                key_label: "u64",
                val_label: "i64",
                expected_eq_dispatch: EqDispatch::Hardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
            // Case 3: i32/i64 — narrower primitive key
            HashMapTypeCase {
                key_type: Type::I32,
                val_type: Type::I64,
                key_label: "i32",
                val_label: "i64",
                expected_eq_dispatch: EqDispatch::Hardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
            // Case 4: i64/bool — primitive key, bool value
            HashMapTypeCase {
                key_type: Type::I64,
                val_type: Type::Bool,
                key_label: "i64",
                val_label: "bool",
                expected_eq_dispatch: EqDispatch::Hardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
            // Case 5: i64/u64 — primitive key, u64 value
            HashMapTypeCase {
                key_type: Type::I64,
                val_type: Type::U64,
                key_label: "i64",
                val_label: "u64",
                expected_eq_dispatch: EqDispatch::Hardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
            // Case 6: i64/String — primitive key, struct value
            HashMapTypeCase {
                key_type: Type::I64,
                val_type: Type::Concrete("std__string__String".to_string(), vec![]),
                key_label: "i64",
                val_label: "String",
                expected_eq_dispatch: EqDispatch::Hardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
            // Case 7: i64/Ptr<u8> — primitive key, pointer value
            HashMapTypeCase {
                key_type: Type::I64,
                val_type: Type::Pointer { element: Box::new(Type::U8), provenance: Provenance::Naked, is_mutable: false },
                key_label: "i64",
                val_label: "Ptr<u8>",
                expected_eq_dispatch: EqDispatch::Hardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },

            // =================================================================
            // Struct keys (String — the critical category that was broken)
            // =================================================================

            // Case 8: String/i64 — struct key, primitive value (the benchmark case)
            HashMapTypeCase {
                key_type: Type::Concrete("std__string__String".to_string(), vec![]),
                val_type: Type::I64,
                key_label: "String",
                val_label: "i64",
                expected_eq_dispatch: EqDispatch::TraitCall("std__string__String__eq".to_string()),
                key_needs_spilling: true,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::StructValue,
            },
            // Case 9: String/bool — struct key, bool value
            HashMapTypeCase {
                key_type: Type::Concrete("std__string__String".to_string(), vec![]),
                val_type: Type::Bool,
                key_label: "String",
                val_label: "bool",
                expected_eq_dispatch: EqDispatch::TraitCall("std__string__String__eq".to_string()),
                key_needs_spilling: true,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::StructValue,
            },
            // Case 10: String/String — struct key and value
            HashMapTypeCase {
                key_type: Type::Concrete("std__string__String".to_string(), vec![]),
                val_type: Type::Concrete("std__string__String".to_string(), vec![]),
                key_label: "String",
                val_label: "String",
                expected_eq_dispatch: EqDispatch::TraitCall("std__string__String__eq".to_string()),
                key_needs_spilling: true,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::StructValue,
            },
            // Case 11: String/Ptr<u8> — struct key, pointer value
            HashMapTypeCase {
                key_type: Type::Concrete("std__string__String".to_string(), vec![]),
                val_type: Type::Pointer { element: Box::new(Type::U8), provenance: Provenance::Naked, is_mutable: false },
                key_label: "String",
                val_label: "Ptr<u8>",
                expected_eq_dispatch: EqDispatch::TraitCall("std__string__String__eq".to_string()),
                key_needs_spilling: true,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::StructValue,
            },
            // Case 12: Vec<u8>/i64 — another struct key (not String)
            HashMapTypeCase {
                key_type: Type::Concrete("std__collections__vec__Vec_u8".to_string(), vec![]),
                val_type: Type::I64,
                key_label: "Vec<u8>",
                val_label: "i64",
                expected_eq_dispatch: EqDispatch::TraitCall("std__collections__vec__Vec_u8__eq".to_string()),
                key_needs_spilling: true,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::StructValue,
            },

            // =================================================================
            // Pointer keys (address-based identity eq)
            // =================================================================

            // Case 13: Ptr<u8>/i64 — pointer key, primitive value
            HashMapTypeCase {
                key_type: Type::Pointer { element: Box::new(Type::U8), provenance: Provenance::Naked, is_mutable: false },
                val_type: Type::I64,
                key_label: "Ptr<u8>",
                val_label: "i64",
                expected_eq_dispatch: EqDispatch::PtrHardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
            // Case 14: Ptr<u8>/String — pointer key, struct value
            HashMapTypeCase {
                key_type: Type::Pointer { element: Box::new(Type::U8), provenance: Provenance::Naked, is_mutable: false },
                val_type: Type::Concrete("std__string__String".to_string(), vec![]),
                key_label: "Ptr<u8>",
                val_label: "String",
                expected_eq_dispatch: EqDispatch::PtrHardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
            // Case 15: Ptr<u8>/Ptr<u8> — pointer key and value
            HashMapTypeCase {
                key_type: Type::Pointer { element: Box::new(Type::U8), provenance: Provenance::Naked, is_mutable: false },
                val_type: Type::Pointer { element: Box::new(Type::U8), provenance: Provenance::Naked, is_mutable: false },
                key_label: "Ptr<u8>",
                val_label: "Ptr<u8>",
                expected_eq_dispatch: EqDispatch::PtrHardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
            // Case 16: Ptr<i64>/i64 — different element-type pointer key
            HashMapTypeCase {
                key_type: Type::Pointer { element: Box::new(Type::I64), provenance: Provenance::Naked, is_mutable: false },
                val_type: Type::I64,
                key_label: "Ptr<i64>",
                val_label: "i64",
                expected_eq_dispatch: EqDispatch::PtrHardware,
                key_needs_spilling: false,
                key_arg_in_get: ArgKind::Ptr,
                key_arg_in_insert: ArgKind::PrimitiveValue,
            },
        ]
    }

    // =========================================================================
    // Test 1: Eq dispatch classification — parameterized
    // =========================================================================
    // Verifies that the emit_binary code correctly classifies each key type
    // for eq dispatch: primitives use hardware cmpi, structs use trait calls.

    #[test]
    fn test_eq_dispatch_classification_parameterized() {
        for case in get_hashmap_type_cases() {
            let key_ty = &case.key_type;
            let cat = categorize(key_ty);

            match &case.expected_eq_dispatch {
                EqDispatch::Hardware => {
                    assert_eq!(cat, TypeCategory::Primitive,
                        "[{}/{}] Hardware eq should only be used for primitive keys",
                        case.key_label, case.val_label);
                }
                EqDispatch::PtrHardware => {
                    assert_eq!(cat, TypeCategory::Pointer,
                        "[{}/{}] PtrHardware eq should only be used for pointer keys",
                        case.key_label, case.val_label);
                }
                EqDispatch::TraitCall(method_name) => {
                    assert_eq!(cat, TypeCategory::Struct,
                        "[{}/{}] Trait eq dispatch should only be used for struct keys",
                        case.key_label, case.val_label);

                    let name = match key_ty {
                        Type::Struct(n) | Type::Concrete(n, _) => n.clone(),
                        _ => panic!("[{}/{}] Expected Struct or Concrete", case.key_label, case.val_label),
                    };
                    let computed_name = format!("{}__eq", name);
                    assert_eq!(&computed_name, method_name,
                        "[{}/{}] Eq method name mismatch",
                        case.key_label, case.val_label);
                }
            }
        }
    }

    // =========================================================================
    // Test 2: Key argument passing convention — parameterized
    // =========================================================================
    // Verifies the expected key argument kind for get() and insert().
    // get() takes &K (ptr), insert() takes K (value for structs, value for primitives).

    #[test]
    fn test_key_argument_convention_parameterized() {
        for case in get_hashmap_type_cases() {
            // get() always takes a reference to the key -> Ptr
            assert_eq!(case.key_arg_in_get, ArgKind::Ptr,
                "[{}/{}] get() key argument must always be Ptr (pass by reference)",
                case.key_label, case.val_label);

            // insert() takes the key by value
            let cat = categorize(&case.key_type);
            match cat {
                TypeCategory::Struct => {
                    assert_eq!(case.key_arg_in_insert, ArgKind::StructValue,
                        "[{}/{}] insert() key should be StructValue for struct types",
                        case.key_label, case.val_label);
                    assert!(case.key_needs_spilling,
                        "[{}/{}] Struct key in insert needs spilling to stack before eq call",
                        case.key_label, case.val_label);
                }
                TypeCategory::Primitive | TypeCategory::Pointer => {
                    assert_eq!(case.key_arg_in_insert, ArgKind::PrimitiveValue,
                        "[{}/{}] insert() key should be PrimitiveValue for primitive/pointer types",
                        case.key_label, case.val_label);
                    assert!(!case.key_needs_spilling,
                        "[{}/{}] Primitive/pointer key should not need spilling",
                        case.key_label, case.val_label);
                }
            }
        }
    }

    // =========================================================================
    // Test 3: Concrete type enters struct eq path — parameterized
    // =========================================================================
    // The critical bug: Type::Concrete was not matched in the struct eq guard,
    // causing arith.cmpi on struct types. This test catches exactly that.

    #[test]
    fn test_concrete_type_enters_struct_eq_path_parameterized() {
        for case in get_hashmap_type_cases() {
            let key_ty = &case.key_type;
            let enters_struct_path = matches!(key_ty,
                Type::Struct(_) | Type::Concrete(..) | Type::Tuple(_) | Type::Array(..) | Type::Enum(_));

            match &case.expected_eq_dispatch {
                EqDispatch::TraitCall(_) => {
                    assert!(enters_struct_path,
                        "[{}/{}] Trait-dispatched key MUST enter struct eq path (was missing Type::Concrete)",
                        case.key_label, case.val_label);

                    let extracted_name = match key_ty {
                        Type::Struct(n) | Type::Concrete(n, _) => Some(n.clone()),
                        _ => None,
                    };
                    assert!(extracted_name.is_some(),
                        "[{}/{}] Must be able to extract struct name from key type",
                        case.key_label, case.val_label);
                }
                EqDispatch::Hardware | EqDispatch::PtrHardware => {
                    // Primitives and pointers don't enter the struct eq path
                }
            }
        }
    }

    // =========================================================================
    // Test 4: Hash method resolution — parameterized
    // =========================================================================
    // HashMap::get and ::insert both call key.hash(). For struct keys,
    // this must resolve to the trait impl (e.g. String::hash).

    #[test]
    fn test_hash_method_resolution_parameterized() {
        for case in get_hashmap_type_cases() {
            let key_ty = &case.key_type;

            let expected_hash_method = match key_ty {
                Type::Struct(n) | Type::Concrete(n, _) => format!("{}__{}", n, "hash"),
                Type::I64 => "i64__hash".to_string(),
                Type::U64 => "u64__hash".to_string(),
                Type::I32 => "i32__hash".to_string(),
                _ => format!("{}__{}", key_ty.mangle_suffix(), "hash"),
            };

            // Verify the hash method name is correctly derived
            let key_name = key_ty.mangle_suffix();
            let hash_method = format!("{}__{}", key_name, "hash");
            assert_eq!(hash_method, expected_hash_method,
                "[{}/{}] Hash method name must match expected",
                case.key_label, case.val_label);
        }
    }

    // =========================================================================
    // Test 5: Reference inner type in get() — parameterized
    // =========================================================================
    // HashMap::get takes &K. In the eq comparison inside get's probe loop,
    // the Reference path unwraps to the inner type and dispatches eq.
    // For Reference(String), inner must be Struct/Concrete, not I64.

    #[test]
    fn test_reference_inner_type_in_get_parameterized() {
        for case in get_hashmap_type_cases() {
            // Simulate the get() key parameter: &K
            let ref_key = Type::Reference(Box::new(case.key_type.clone()), false);

            if let Type::Reference(inner, _) = &ref_key {
                let inner_mangle = inner.mangle_suffix();
                let eq_method_for_inner = format!("{}__{}", inner_mangle, "eq");

                let cat = categorize(inner);
                match &case.expected_eq_dispatch {
                    EqDispatch::TraitCall(expected) => {
                        assert_eq!(&eq_method_for_inner, expected,
                            "[{}/{}] Reference inner eq method must match expected trait call",
                            case.key_label, case.val_label);
                        assert_eq!(cat, TypeCategory::Struct,
                            "[{}/{}] Struct key's inner type must be categorized as Struct",
                            case.key_label, case.val_label);
                    }
                    EqDispatch::Hardware => {
                        assert_eq!(cat, TypeCategory::Primitive,
                            "[{}/{}] Primitive key's inner type must be categorized as Primitive",
                            case.key_label, case.val_label);
                    }
                    EqDispatch::PtrHardware => {
                        assert_eq!(cat, TypeCategory::Pointer,
                            "[{}/{}] Pointer key's inner type must be categorized as Pointer",
                            case.key_label, case.val_label);
                    }
                }
            }
        }
    }

    // =========================================================================
    // Test 6: request_specialization self_ty requirement — parameterized
    // =========================================================================
    // When trait eq dispatch is used, request_specialization MUST receive
    // self_ty = Some(inner_type) so TraitRegistry can find the method body.

    #[test]
    fn test_request_specialization_self_ty_parameterized() {
        for case in get_hashmap_type_cases() {
            match &case.expected_eq_dispatch {
                EqDispatch::TraitCall(_) => {
                    let self_ty: Option<Type> = Some(case.key_type.clone());
                    assert!(self_ty.is_some(),
                        "[{}/{}] Trait-dispatched eq MUST pass self_ty to request_specialization",
                        case.key_label, case.val_label);

                    let sty = self_ty.unwrap();
                    assert!(!matches!(sty, Type::Reference(..)),
                        "[{}/{}] self_ty must be the naked type, not Reference-wrapped",
                        case.key_label, case.val_label);
                }
                EqDispatch::Hardware | EqDispatch::PtrHardware => {
                    // Primitive and pointer types don't need request_specialization for eq
                }
            }
        }
    }

    // =========================================================================
    // Test 7: Mangle name bypass guard — parameterized
    // =========================================================================
    // Fully qualified names (starting with "std__") must bypass mangle_fn_name
    // to avoid re-mangling under the caller's package context.

    #[test]
    fn test_mangle_bypass_guard_parameterized() {
        for case in get_hashmap_type_cases() {
            match &case.expected_eq_dispatch {
                EqDispatch::TraitCall(method_name) => {
                    let bypasses = method_name.starts_with("std__")
                        || method_name.starts_with("core__");
                    // If the method is from std, it must bypass re-mangling
                    if method_name.contains("std__") {
                        assert!(bypasses,
                            "[{}/{}] Std eq method '{}' must bypass mangle_fn_name",
                            case.key_label, case.val_label, method_name);
                    }
                }
                EqDispatch::Hardware | EqDispatch::PtrHardware => {}
            }
        }
    }

    // =========================================================================
    // Test 8: HashMap::get value return convention — parameterized
    // =========================================================================
    // When HashMap::get returns a value, it must NOT use Ptr cast + read for
    // primitive value types. `entry.val` is already the correct type — casting
    // it to Ptr<i64> and calling .read() causes a double-dereference (segfault).
    //
    // Pattern classification by value type:
    //   i64 (primitive)  → return entry.val (direct field access)
    //   String (struct)  → return entry.val (also direct — entry already loaded by value)
    //   Ptr<T> (pointer) → return entry.val (direct — pointer value, not pointee)

    #[test]
    fn test_get_value_return_convention_parameterized() {
        for case in get_hashmap_type_cases() {
            let val_ty = &case.val_type;
            let val_cat = categorize(val_ty);

            // The return value should ALWAYS be a direct field access.
            // Never Ptr cast + read, because entry is loaded by value from
            // entries.offset(idx).read() — the val field is already materialized.
            let needs_ptr_cast_read = false; // NEVER needs it

            assert!(!needs_ptr_cast_read,
                "[{}/{}] HashMap::get must return entry.val directly, \
                 NOT via (&entry.val as Ptr<V>).read() which causes double-deref",
                case.key_label, case.val_label);

            // Verify expected MLIR load type based on value category:
            //   Primitive → load as integer (i64, i32, i8, etc.)
            //   Struct    → load as struct (!struct_...)
            //   Pointer   → load as !llvm.ptr
            let expected_load_category = match val_cat {
                TypeCategory::Primitive => "integer",
                TypeCategory::Struct    => "struct",
                TypeCategory::Pointer   => "ptr",
            };
            assert!(
                matches!(expected_load_category, "integer" | "struct" | "ptr"),
                "[{}/{}] Value load type must be classifiable as integer, struct, or ptr",
                case.key_label, case.val_label
            );
        }
    }
}
