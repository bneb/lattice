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

/// [ZERO-TRUST] Layout Prover
pub fn prove_layout_compatibility(struct_registry: &std::collections::HashMap<crate::types::TypeKey, crate::registry::StructInfo>, from: &Type, to: &Type) -> bool {
    if from == to { return true; }
    from.size_of(struct_registry) == to.size_of(struct_registry) && from.align_of(struct_registry) == to.align_of(struct_registry)
}

/// Convenience wrapper: extracts struct_registry from CodegenContext.
pub fn prove_layout_compatibility_ctx(ctx: &mut LoweringContext, from: &Type, to: &Type) -> bool {
    let reg = ctx.struct_registry();
    prove_layout_compatibility(&reg, from, to)
}
