use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::codegen::abi::Layout;
use crate::codegen::types::substitution::substitute_generics_ctx;

impl Type {
    pub fn to_mlir_storage_type(&self, ctx: &mut LoweringContext) -> Result<String, String> {
        match self {
            Type::Owned(inner) => return inner.to_mlir_storage_type(ctx),
            Type::Atomic(inner) => return inner.to_mlir_storage_type(ctx),
            _ => {}
        }

        if self.k_is_ptr_type() || matches!(self, Type::Reference(_, _)) {
            return Ok("!llvm.ptr".to_string());
        }

        if let Type::Tensor(inner, shape) = self {
            let elem = inner.to_mlir_storage_type(ctx)?;
            let dims: Vec<String> = shape.iter().map(|d| d.to_string()).collect();
            return Ok(format!("memref<{}x{}>", dims.join("x"), elem));
        }

        if let Type::Concrete(base, args) = self {
           if ((base.contains("Simd") && !base.contains("ptr")) || base == "Simd")
               && args.len() >= 2
           {
                let inner_ty = &args[0];
                let size_arg = &args[1];
                let size = if let Type::Struct(s) = size_arg {
                    s.parse::<usize>().unwrap_or(0)
                } else if let Type::Concrete(val_str, _) = size_arg {
                     val_str.parse::<usize>().unwrap_or(0)
                } else { 0 };

                if size > 0 {
                    let inner_mlir = inner_ty.to_mlir_type(ctx)?;
                    return Ok(format!("vector<{}x{}>", size, inner_mlir));
                }
           }

           if base == "Vector4f32" { return Ok("vector<4xf32>".to_string()); }
           if base == "Vector8f32" { return Ok("vector<8xf32>".to_string()); }
           if base == "Vector4f64" { return Ok("vector<4xf64>".to_string()); }
           if base == "Vector16f32" { return Ok("vector<16xf32>".to_string()); }
        }

        match self {
            Type::Struct(name) => {
                match name.as_str() {
                    "Vector4f32"  => return Ok("vector<4xf32>".to_string()),
                    "Vector8f32"  => return Ok("vector<8xf32>".to_string()),
                    "Vector4f64"  => return Ok("vector<4xf64>".to_string()),
                    "Vector16f32" => return Ok("vector<16xf32>".to_string()),
                    _ => {}
                }
                let full_name = {
                    let registry = ctx.struct_registry();
                    let target = name;
                    registry.values()
                        .find(|info| {
                            info.name == *target
                            || info.name.ends_with(&format!("__{}", target))
                            || (info.name.contains("__") && info.name.split("__").last() == Some(target.as_str()))
                        })
                        .map(|info| info.name.clone())
                        .unwrap_or_else(|| name.clone())
                };
                return Ok(format!("!struct_{}", full_name));
            }
            Type::Concrete(base, args) => {
                if args.is_empty() {
                    match base.as_str() {
                        "Vector4f32"  => return Ok("vector<4xf32>".to_string()),
                        "Vector8f32"  => return Ok("vector<8xf32>".to_string()),
                        "Vector4f64"  => return Ok("vector<4xf64>".to_string()),
                        "Vector16f32" => return Ok("vector<16xf32>".to_string()),
                        _ => {}
                    }
                }
                let full_base = {
                    let templates = ctx.struct_templates();
                    templates.keys()
                        .find(|k| k.ends_with(base) || *k == base)
                        .cloned()
                        .unwrap_or_else(|| base.clone())
                };
                let suffix = args.iter().map(|t| t.to_canonical_name()).collect::<Vec<_>>().join("_");
                let mangled = if args.is_empty() { full_base } else { format!("{}_{}", full_base, suffix) };
                return Ok(format!("!struct_{}", mangled));
            }
            _ => {}
        }

        let layout = Layout::compute(ctx, self);
        Ok(layout.to_mlir_storage(ctx))
    }

    pub fn to_mlir_type(&self, ctx: &mut LoweringContext) -> Result<String, String> {
        to_mlir_type(ctx, self)
    }
}

fn canonicalize_struct_arg_t(ctx: &mut LoweringContext, t: &Type) -> Type {
    if let Type::Struct(sname) = t {
        if !sname.contains("__") {
            let suffix = format!("__{}", sname);
            if let Some(canonical) = ctx.struct_templates().keys()
                .find(|k| k.ends_with(&suffix))
                .cloned()
            {
                return Type::Struct(canonical);
            }
        }
    }
    t.clone()
}

pub fn to_mlir_type(ctx: &mut LoweringContext, ty: &Type) -> Result<String, String> {
    let resolved_ty = substitute_generics_ctx(ctx, ty);

    if resolved_ty.k_is_ptr_type() || matches!(resolved_ty, Type::Reference(_, _)) {
        return Ok("!llvm.ptr".to_string());
    }
    match &resolved_ty {
        Type::I8 | Type::U8 => Ok("i8".to_string()),
        Type::I16 | Type::U16 => Ok("i16".to_string()),
        Type::I32 | Type::U32 => Ok("i32".to_string()),
        Type::I64 | Type::U64 => Ok("i64".to_string()),
        Type::F32 => Ok("f32".to_string()),
        Type::F64 => Ok("f64".to_string()),
        Type::Bool => Ok("i1".to_string()),
        Type::Usize => Ok("index".to_string()),
        Type::Unit => Ok("!llvm.void".to_string()),
        Type::Struct(name) => {
            match name.as_str() {
                "Vector4f32"  => return Ok("vector<4xf32>".to_string()),
                "Vector8f32"  => return Ok("vector<8xf32>".to_string()),
                "Vector4f64"  => return Ok("vector<4xf64>".to_string()),
                "Vector16f32" => return Ok("vector<16xf32>".to_string()),
                _ => {}
            }
            if let Some(concrete) = ctx.current_type_map().get(name).cloned() {
                return to_mlir_type(ctx, &concrete);
            }
            let full_name = {
                let registry = ctx.struct_registry();
                let target = name;
                let suffix = format!("__{}", target);
                let mut candidates: Vec<&str> = registry.values()
                    .filter(|info| {
                        info.name == *target
                        || info.name.ends_with(&suffix)
                    })
                    .map(|info| info.name.as_str())
                    .collect();
                candidates.sort_by_key(|c| c.len());
                candidates.first().map(|s| s.to_string())
                    .unwrap_or_else(|| name.clone())
            };
            Ok(format!("!struct_{}", full_name))
        },
        Type::Concrete(name, args) => {
            if args.is_empty() {
                match name.as_str() {
                    "Vector4f32"  => return Ok("vector<4xf32>".to_string()),
                    "Vector8f32"  => return Ok("vector<8xf32>".to_string()),
                    "Vector4f64"  => return Ok("vector<4xf64>".to_string()),
                    "Vector16f32" => return Ok("vector<16xf32>".to_string()),
                    _ => {}
                }
            }
            fn has_unresolved_generic(ty: &Type) -> bool {
                match ty {
                    Type::Generic(_) => true,
                    Type::Struct(n) if n.len() == 1 && n.chars().next().is_some_and(|c| c.is_ascii_uppercase()) => true,
                    Type::Concrete(_, inner_args) => inner_args.iter().any(has_unresolved_generic),
                    Type::Pointer { element, .. } => has_unresolved_generic(element),
                    Type::Reference(inner, _) => has_unresolved_generic(inner),
                    Type::Owned(inner) => has_unresolved_generic(inner),
                    _ => false,
                }
            }

            if args.iter().any(has_unresolved_generic) {
                return Ok("!llvm.ptr".to_string());
            }

            let full_base = {
                let templates = ctx.struct_templates();
                templates.keys()
                    .find(|k| *k == name || k.ends_with(&format!("__{}", name))
                              || (k.contains("__") && k.split("__").last() == Some(name.as_str())))
                    .cloned()
                    .unwrap_or_else(|| name.clone())
            };
            let canonical_args: Vec<Type> = args.iter().map(|t| canonicalize_struct_arg_t(ctx, t)).collect();
            let suffix = canonical_args.iter().map(|t| t.to_canonical_name()).collect::<Vec<_>>().join("_");
            let mangled = if args.is_empty() { full_base } else { format!("{}_{}", full_base, suffix) };
            Ok(format!("!struct_{}", mangled))
        },
        Type::Array(inner, len, _) => Ok(format!("!llvm.array<{} x {}>", len, to_mlir_type(ctx, inner)?)),
        Type::Tuple(elems) => {
            let parts: Result<Vec<_>, _> = elems.iter().map(|e| to_mlir_type(ctx, e)).collect();
            Ok(format!("!llvm.struct<({})>", parts?.join(", ")))
        }
        Type::Enum(name) => {
            let stripped_name = name.rsplit("__").next().unwrap_or(name);
            if let Some(enum_info) = ctx.enum_registry().values()
                .find(|i| i.name == *name || i.name == stripped_name)
            {
                return Ok(format!("!struct_{}", enum_info.name));
            }
            Ok(format!("!struct_{}", stripped_name))
        }
        _ => Err(format!("MLIR Lowering not implemented for type: {:?}", ty)),
    }
}
