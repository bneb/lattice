use crate::types::Type;
use crate::codegen::context::LoweringContext;

pub fn extract_ptr_inner(name: &str) -> Option<String> {
    if let Some(idx) = name.rfind("Ptr") {
        let after = &name[idx + "Ptr".len()..];
        let inner = after.trim_start_matches('_');
        if !inner.is_empty() { return Some(inner.to_string()); }
    }
    None
}

/// Flattening Loop
#[allow(clippy::only_used_in_recursion)]
pub fn flatten_nested_ptr(ty: &Type, depth: usize, debug_ctx: &str) -> Type {
    if depth > 10 { return ty.clone(); }
    match ty {
        Type::Concrete(template, args) if template.contains("Ptr") && !args.is_empty() => {
            if args[0].k_is_ptr_type() {
                // Drill down to the innermost non-pointer type
                return flatten_nested_ptr(&args[0], depth + 1, debug_ctx);
            }
            // If it's a pointer but the inner is NOT a pointer, we stay as is
            // EXCEPT if we are already in a recursion (depth > 0), in which case we strip this last layer too
            if depth > 0 { return args[0].clone(); }
            ty.clone()
        }
        Type::Struct(name) if name.contains("Ptr") => {
            if let Some(inner_name) = extract_ptr_inner(name) {
                let t = Type::Struct(inner_name);
                return flatten_nested_ptr(&t, depth + 1, debug_ctx);
            }
            ty.clone()
        }
        _ => ty.clone(),
    }
}

/// Layout Prover
pub fn prove_layout_compatibility(struct_registry: &std::collections::HashMap<crate::types::TypeKey, crate::registry::StructInfo>, from: &Type, to: &Type) -> bool {
    if from == to { return true; }
    from.size_of(struct_registry) == to.size_of(struct_registry) && from.align_of(struct_registry) == to.align_of(struct_registry)
}

/// Convenience wrapper: extracts struct_registry from CodegenContext.
pub fn prove_layout_compatibility_ctx(ctx: &mut LoweringContext, from: &Type, to: &Type) -> bool {
    let reg = ctx.struct_registry();
    prove_layout_compatibility(reg, from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::types::TypeKey;
    use crate::registry::StructInfo;

    fn empty_reg() -> HashMap<TypeKey, StructInfo> {
        HashMap::new()
    }

    #[test]
    fn test_extract_ptr_inner() {
        assert_eq!(extract_ptr_inner("FooPtr_i32"), Some("i32".into()));
        assert_eq!(extract_ptr_inner("FooPtr_"), None);
        assert_eq!(extract_ptr_inner("Foo"), None);
        assert_eq!(extract_ptr_inner("Ptr_T"), Some("T".into()));
        assert_eq!(extract_ptr_inner(""), None);
    }

    #[test]
    fn test_flatten_nested_ptr_depth_guard() {
        let ty = Type::Struct("Ptr_i32".into());
        assert_eq!(flatten_nested_ptr(&ty, 11, "test"), ty);
    }

    #[test]
    fn test_flatten_nested_ptr_single() {
        let p = Type::Concrete("Ptr".into(), vec![Type::I32]);
        assert_eq!(flatten_nested_ptr(&p, 0, "test"), p);
        assert_eq!(flatten_nested_ptr(&p, 1, "test"), Type::I32);
    }

    #[test]
    fn test_flatten_nested_ptr_nested() {
        let inner = Type::Concrete("Ptr".into(), vec![Type::I32]);
        let outer = Type::Concrete("Ptr".into(), vec![inner]);
        assert_eq!(flatten_nested_ptr(&outer, 0, "test"), Type::I32);
    }

    #[test]
    fn test_flatten_nested_ptr_non_ptr() {
        assert_eq!(flatten_nested_ptr(&Type::I32, 0, "test"), Type::I32);
        assert_eq!(flatten_nested_ptr(&Type::F64, 3, "test"), Type::F64);
        assert_eq!(flatten_nested_ptr(&Type::Unit, 0, "test"), Type::Unit);
    }

    #[test]
    fn test_flatten_nested_ptr_struct() {
        let s = Type::Struct("Ptr_i32".into());
        assert_eq!(flatten_nested_ptr(&s, 1, "test"), Type::Struct("i32".into()));
    }

    #[test]
    fn test_flatten_nested_ptr_triple() {
        let inner = Type::Concrete("Ptr".into(), vec![Type::I32]);
        let middle = Type::Concrete("Ptr".into(), vec![inner]);
        let outer = Type::Concrete("Ptr".into(), vec![middle]);
        assert_eq!(flatten_nested_ptr(&outer, 0, "test"), Type::I32);
    }

    #[test]
    fn test_flatten_nested_ptr_struct_ptr_not_real() {
        // Struct("Ptr") contains "Ptr" but is not a real Ptr<T> pointer
        // extract_ptr_inner("Ptr") returns None because after is empty
        let s = Type::Struct("Ptr".into());
        assert_eq!(flatten_nested_ptr(&s, 0, "test"), s);
    }

    #[test]
    fn test_prove_layout_same_type() {
        let reg = empty_reg();
        assert!(prove_layout_compatibility(&reg, &Type::F64, &Type::F64));
        assert!(prove_layout_compatibility(&reg, &Type::Bool, &Type::Bool));
    }

    #[test]
    fn test_prove_layout_compatible_primitives() {
        let reg = empty_reg();
        // Same size and alignment
        assert!(prove_layout_compatibility(&reg, &Type::I32, &Type::U32));
        assert!(prove_layout_compatibility(&reg, &Type::I64, &Type::Usize));
        assert!(prove_layout_compatibility(&reg, &Type::I16, &Type::U16));
    }

    #[test]
    fn test_prove_layout_incompatible_primitives() {
        let reg = empty_reg();
        // Different sizes
        assert!(!prove_layout_compatibility(&reg, &Type::I32, &Type::I64));
        assert!(!prove_layout_compatibility(&reg, &Type::U8, &Type::U16));
        assert!(!prove_layout_compatibility(&reg, &Type::Bool, &Type::I16));
    }
}
