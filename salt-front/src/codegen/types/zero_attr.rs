use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::codegen::type_bridge::resolve_codegen_type;

fn zero_attr_struct_enum(ctx: &mut LoweringContext<'_, '_>, ty: &Type) -> Result<String, String> {
    match ty {
        Type::Struct(name) => {
            let info_opt = ctx.struct_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).cloned();
            if let Some(info) = info_opt {
                let mut parts = Vec::new();
                for ty in &info.field_order {
                    let attr = zero_attr(ctx, ty)?;
                    if attr.is_empty() { return Ok("".to_string()); }
                    parts.push(attr);
                }
                Ok(format!("[{}]", parts.join(", ")))
            } else {
                Ok("".to_string())
            }
        }
        Type::Enum(name) => {
            if let Some(info) = ctx.enum_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).cloned() {
                let mut parts = vec!["0 : i32".to_string()];
                if info.max_payload_size > 0 {
                    parts.push("[0 : i8, 0 : i8, 0 : i8, 0 : i8]".to_string());
                    let mut zeros = Vec::new();
                    for _ in 0..info.max_payload_size {
                        zeros.push("0 : i8".to_string());
                    }
                    parts.push(format!("[{}]", zeros.join(", ")));
                }
                Ok(format!("[{}]", parts.join(", ")))
            } else {
                Ok("".to_string())
            }
        }
        _ => unreachable!(),
    }
}

fn zero_attr_aggregate(ctx: &mut LoweringContext<'_, '_>, ty: &Type) -> Result<String, String> {
    match ty {
        Type::Array(inner, len, _) => {
            let inner_attr = zero_attr(ctx, inner)?;
            if inner_attr.is_empty() { return Ok("".to_string()); }
            let mut parts = Vec::new();
            for _ in 0..*len {
                parts.push(inner_attr.clone());
            }
            Ok(format!("[{}]", parts.join(", ")))
        }
        Type::Tuple(elems) => {
            let mut parts = Vec::new();
            for e in elems {
                let attr = zero_attr(ctx, e)?;
                if attr.is_empty() { return Ok("".to_string()); }
                parts.push(attr);
            }
            Ok(format!("[{}]", parts.join(", ")))
        }
        _ => unreachable!(),
    }
}

pub fn zero_attr(ctx: &mut LoweringContext<'_, '_>, ty: &Type) -> Result<String, String> {
    match ty {
        Type::Bool => Ok("0 : i8".to_string()),
        Type::I8 | Type::U8 => Ok("0 : i8".to_string()),
        Type::I16| Type::U16 => Ok("0 : i16".to_string()),
        Type::I32| Type::U32 => Ok("0 : i32".to_string()),
        Type::I64| Type::U64 | Type::Usize => Ok("0 : i64".to_string()),
        Type::F32 => Ok("0.0 : f32".to_string()),
        Type::F64 => Ok("0.0 : f64".to_string()),
        Type::Owned(_) | Type::Reference(_, _) | Type::Fn(_, _) => Ok("null : !llvm.ptr".to_string()),
        Type::Atomic(inner) => zero_attr(ctx, inner),
        Type::Array(..) | Type::Tuple(..) => zero_attr_aggregate(ctx, ty),
        Type::Struct(..) | Type::Enum(..) => zero_attr_struct_enum(ctx, ty),
        Type::Concrete(..) => {
             let resolved = resolve_codegen_type(ctx, ty);
             zero_attr(ctx, &resolved)
        }
        Type::Never => Ok("".to_string()),
        Type::SelfType => Err("Unresolved 'Self' type reached zero_attr. This is a compiler bug.".to_string()),
        _ => Err(format!("No zero attribute for type {:?}", ty)),
    }
}
