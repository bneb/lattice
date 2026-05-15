use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::codegen::abi::Layout;

impl Type {
    pub fn to_mlir_storage_type(&self, ctx: &mut LoweringContext) -> Result<String, String> {
        match self {
            Type::Owned(inner) => return inner.to_mlir_storage_type(ctx),
            Type::Atomic(inner) => return inner.to_mlir_storage_type(ctx),
            _ => {}
        }
        
        if self.k_is_ptr_type() {
            return Ok("!llvm.ptr".to_string());
        }

        if let Type::Tensor(_inner, _shape) = self {
             return Ok("!llvm.ptr".to_string());
        }

        if let Type::Concrete(base, args) = self {
           if (base.contains("Simd") && !base.contains("ptr")) || base == "Simd" {
               if args.len() >= 2 {
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

pub fn to_mlir_type(ctx: &mut LoweringContext, ty: &Type) -> Result<String, String> {
    match ty {
        Type::I8 | Type::U8 => Ok("i8".to_string()),
        Type::I16 | Type::U16 => Ok("i16".to_string()),
        Type::I32 | Type::U32 => Ok("i32".to_string()),
        Type::I64 | Type::U64 => Ok("i64".to_string()),
        Type::F32 => Ok("f32".to_string()),
        Type::F64 => Ok("f64".to_string()),
        Type::Usize => Ok("index".to_string()),
        Type::Bool => Ok("i1".to_string()),
        Type::Unit => Ok("!llvm.void".to_string()),
        Type::Never => Ok("!llvm.void".to_string()),
        Type::Reference(_, _) | Type::Pointer { .. } | Type::Owned(_) => Ok("!llvm.ptr".to_string()),
        Type::Tensor(_inner, _shape) => Ok("!llvm.ptr".to_string()),
        Type::Array(inner, len, packed) => {
            if *packed {
                let bit_len = *len;
                let word_count = (bit_len + 63) / 64;
                Ok(format!("!llvm.array<{} x i64>", word_count))
            } else {
                Ok(format!("!llvm.array<{} x {}>", len, inner.to_mlir_type(ctx)?))
            }
        }
        Type::Struct(_) | Type::Concrete(_, _) => ty.to_mlir_storage_type(ctx),
        Type::Tuple(elems) => {
            if elems.is_empty() { return Ok("!llvm.void".to_string()); }
            let mut inner = Vec::new();
            for e in elems {
                inner.push(e.to_mlir_type(ctx)?);
            }
            Ok(format!("!llvm.struct<({})>", inner.join(", ")))
        }
        Type::Generic(name) => {
            if let Some(resolved) = ctx.resolve_type(name) {
                if &resolved != ty {
                    return resolved.to_mlir_type(ctx);
                }
            }
            Ok("i64".to_string())
        }
        _ => Err(format!("Cannot convert type {:?} to MLIR", ty)),
    }
}
