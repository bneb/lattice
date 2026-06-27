use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::evaluator::ConstValue;
use crate::codegen::type_bridge::resolve_type;

pub fn emit_const(ctx: &mut LoweringContext, _out: &mut String, c: &crate::grammar::ConstDef) -> Result<(), String> {
    let val = ctx.evaluator.eval_expr(&c.value).map_err(|e| format!("Const eval failed for {}: {:?}", c.name, e))?;
    match &val {
        ConstValue::Integer(_) | ConstValue::Bool(_) | ConstValue::Float(_) | ConstValue::String(_) => {
            ctx.evaluator.constant_table.insert(c.name.to_string(), val.clone());
        }
        _ => {} // Complex/struct: skip inlining, handled via global below
    }

    let ty = resolve_type(ctx, &c.ty);
    let mlir_ty = ty.to_mlir_type(ctx)?;
    let name = ctx.mangle_fn_name(&c.name.to_string());
    if ctx.initialized_globals().contains(&*name) {
        return Ok(());
    }
    ctx.initialized_globals_mut().insert(name.to_string());
    let val_attr = match val {
        ConstValue::Integer(i) => {
             let suffix = match ty {
                 Type::I64 | Type::U64 | Type::Usize => "i64",
                 Type::I32 | Type::U32 => "i32",
                 Type::I16 | Type::U16 => "i16",
                 Type::I8 | Type::U8 => "i8",
                 Type::Bool => "i1",
                 _ => "i64"
             };
             format!("{} : {}", i, suffix)
        }
        ConstValue::Float(f) => {
             let suffix = if matches!(ty, Type::F32) { "f32" } else { "f64" };
             format!("{} : {}", f, suffix)
        }
        ConstValue::Bool(b) => format!("{} : i1", if b { 1 } else { 0 }),
        _ => {
             if let syn::Expr::Struct(s) = &c.value {
                 if s.fields.len() == 1 {
                     if let Some(field) = s.fields.first() {
                         if let Ok(ConstValue::Integer(i)) = ctx.evaluator.eval_expr(&field.expr) {
                             let i32_val = i as i32;
                             ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global internal constant @{}() {{alignment = 4}} : {} {{\n", name, mlir_ty));
                             ctx.decl_out_mut().push_str(&format!("    %0 = llvm.mlir.constant({} : i32) : i32\n", i32_val));
                             ctx.decl_out_mut().push_str(&format!("    %1 = llvm.mlir.undef : {}\n", mlir_ty));
                             ctx.decl_out_mut().push_str(&format!("    %2 = llvm.insertvalue %0, %1[0] : {}\n", mlir_ty));
                             ctx.decl_out_mut().push_str(&format!("    llvm.return %2 : {}\n", mlir_ty));
                             ctx.decl_out_mut().push_str("  }\n");
                             return Ok(());
                         }
                     }
                 }
             }

             let alignment = match &ty {
                 Type::Array(_, len, _) if *len >= 16 => 64,
                 Type::Struct(_) | Type::Concrete(_, _) => 16,
                 _ => 8,
             };
             ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global internal @{}() {{alignment = {}}} : {} {{\n", name, alignment, mlir_ty));
             ctx.decl_out_mut().push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_ty));
             ctx.decl_out_mut().push_str(&format!("    llvm.return %0 : {}\n", mlir_ty));
             ctx.decl_out_mut().push_str("  }\n");
             return Ok(());
        }
    };

    ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global internal constant @{}({}) : {}\n", name, val_attr, mlir_ty));
    Ok(())
}

pub fn emit_global_def(ctx: &mut LoweringContext, _out: &mut String, g: &crate::grammar::GlobalDef) -> Result<(), String> {
    let ty_raw = resolve_type(ctx, &g.ty);
    let ty_storage = match &ty_raw {
        Type::Atomic(inner) => (**inner).clone(),
        other => other.clone(),
    };
    let name = ctx.mangle_fn_name(&g.name.to_string());

    if ctx.initialized_globals().contains(&*name) {
        return Ok(());
    }

    ctx.globals_mut().insert(name.to_string(), ty_raw.clone());
    ctx.initialized_globals_mut().insert(name.to_string());

    let mlir_ty = ty_storage.to_mlir_storage_type(ctx)?;

    let init_val = if let Some(val_expr) = &g.init {
        let eval = crate::evaluator::Evaluator::new();
        match eval.eval_expr(val_expr) {
            Ok(crate::evaluator::ConstValue::Integer(i)) => {
                let suffix = match &ty_storage {
                    Type::I64 | Type::U64 | Type::Usize => "i64",
                    Type::I32 | Type::U32 => "i32",
                    Type::I16 | Type::U16 => "i16",
                    Type::I8 | Type::U8 => "i8",
                    Type::Bool => "i1",
                    _ => "i64",
                };
                format!("{} : {}", i, suffix)
            }
            Ok(crate::evaluator::ConstValue::Float(f)) => {
                let suffix = if matches!(&ty_storage, Type::F32) { "f32" } else { "f64" };
                format!("{} : {}", f, suffix)
            }
            Ok(crate::evaluator::ConstValue::Bool(b)) => {
                format!("{} : i1", if b { 1 } else { 0 })
            }
            Ok(crate::evaluator::ConstValue::Array(elements)) => {
                let inner_mlir_ty = if let Type::Array(inner_ty, _, _) = &ty_storage {
                    inner_ty.to_mlir_storage_type(ctx).unwrap_or_else(|_| "i64".to_string())
                } else {
                    "i64".to_string()
                };
                if inner_mlir_ty.contains("struct") {
                    "".to_string()
                } else {
                    let mut elem_strs = Vec::new();
                    for e in elements {
                        match e {
                            crate::evaluator::ConstValue::Integer(i) => elem_strs.push(i.to_string()),
                            crate::evaluator::ConstValue::Float(f) => elem_strs.push(f.to_string()),
                            crate::evaluator::ConstValue::Bool(b) => elem_strs.push(if b { "1".to_string() } else { "0".to_string() }),
                            _ => elem_strs.push("0".to_string()),
                        }
                    }
                    format!("dense<[{}]> : tensor<{}x{}>", elem_strs.join(", "), elem_strs.len(), inner_mlir_ty)
                }
            }
            _ => "".to_string(),
        }
    } else {
        "".to_string()
    };

    let linkage = if g.is_pub { "external" } else { "internal" };

    if init_val.is_empty() {
        let alignment = match &ty_storage {
            Type::Array(_, len, _) if *len >= 16 => 64,
            Type::Struct(_) | Type::Concrete(_, _) => 16,
            _ => 8,
        };

        ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global {} @{}() {{alignment = {}}} : {} {{\n", linkage, name, alignment, mlir_ty));
        ctx.decl_out_mut().push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_ty));
        ctx.decl_out_mut().push_str(&format!("    llvm.return %0 : {}\n", mlir_ty));
        ctx.decl_out_mut().push_str("  }\n");
    } else {
        ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global {} @{}({}) : {}\n", linkage, name, init_val, mlir_ty));
    }
    Ok(())
}
