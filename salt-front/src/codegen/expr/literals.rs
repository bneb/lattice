
fn infer_struct_generics(ctx: &mut LoweringContext, s: &syn::ExprStruct, full_name: &str, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Vec<Type> {
    let template_data = ctx.struct_templates().get(full_name).map(|template| {
        let generics = template.generics.clone();
        let field_raw_types: Vec<(String, Option<String>)> = template.fields.iter().map(|f| {
            let raw_name = if let crate::grammar::SynType::Path(sp) = &f.ty {
                if sp.segments.len() == 1 && sp.segments[0].args.is_empty() {
                    Some(sp.segments[0].ident.to_string())
                } else { None }
            } else { None };
            (f.name.to_string(), raw_name)
        }).collect();
        (generics, field_raw_types)
    });
    
    if let Some((Some(generics), field_raw_types)) = template_data {
        let type_map = ctx.current_type_map().clone();
        let param_names: Vec<String> = generics.params.iter().map(|param| {
            match param {
                crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
            }
        }).collect();
        
        let mut inferred_map = type_map;
        for field in &s.fields {
            if let syn::Member::Named(id) = &field.member {
                let field_name = id.to_string();
                if let Some((_, Some(ref gname))) = field_raw_types.iter().find(|(n, _)| n == &field_name) {
                    if param_names.contains(gname) {
                        if let Ok(actual_ty) = emit_expr(ctx, &mut String::new(), &field.expr, local_vars, None) {
                            inferred_map.insert(gname.clone(), actual_ty.1);
                        }
                    }
                }
            }
        }

        infer_phantom_generics(&param_names, &mut inferred_map);
        
        param_names.iter().filter_map(|pname| {
            inferred_map.get(pname).cloned()
        }).collect()
    } else { let mut s: Vec<_> = ctx.current_type_map().iter().collect(); s.sort_by_key(|(k,_)|*k); s.into_iter().map(|(_,v)|v.clone()).collect() }
}

fn eval_struct_fields(
    ctx: &mut LoweringContext,
    out: &mut String,
    s: &syn::ExprStruct,
    info: &crate::registry::StructInfo,
    mangled_name: &str,
    local_vars: &mut HashMap<String, (Type, LocalKind)>
) -> Result<HashMap<String, (String, Type)>, String> {
    let mut field_vals = HashMap::new();
    for field in &s.fields {
        if let syn::Member::Named(id) = &field.member {
            let name = id.to_string();
            *ctx.pending_pointer_state = None;

            let f_ty = if let Some(template_name) = &info.template_name {
                 let field_ty_owned = ctx.struct_templates().get(template_name)
                     .and_then(|t| t.fields.iter().find(|f| f.name == name).map(|tf| tf.ty.clone()));
                 if let Some(field_ty_ast) = field_ty_owned {
                     crate::codegen::type_bridge::resolve_type(ctx, &field_ty_ast)
                 } else {
                     info.fields.get(&name).map(|(_, ty)| ty).cloned().unwrap_or(Type::Unit)
                 }
            } else {
                info.fields.get(&name).map(|(_, ty)| ty).cloned().unwrap_or(Type::Unit)
            };

            let (val, actual_ty) = emit_expr(ctx, out, &field.expr, local_vars, Some(&f_ty))?;
            *ctx.pending_pointer_state = None;

            if f_ty == Type::Unit && actual_ty == Type::I64 {
                crate::ice!("Type Poisoning Detected: Field '{}' resolved to Unit but assigned I64. \nGeneric substitution failed during monomorphization. \nStructure: {}\nArg Type: {:?}", name, mangled_name, actual_ty);
            }

            field_vals.insert(name, (val, actual_ty));

            let mut field_expr = &field.expr;
            while let syn::Expr::Cast(c) = field_expr {
                field_expr = &c.expr;
            }
            if let syn::Expr::Path(p) = field_expr {
                if p.path.segments.len() == 1 {
                    let var_name = p.path.segments[0].ident.to_string();
                    let alloc_id = format!("malloc:{}", var_name);
                    if ctx.malloc_tracker.contains_alloc(&alloc_id) {
                        ctx.malloc_tracker.link_dependency("__pending_struct", alloc_id);
                    }
                    if ctx.malloc_tracker.has_dependencies(&var_name) {
                        ctx.malloc_tracker.link_dependency("__pending_struct", var_name);
                    }
                }
            }
        }
    }
    Ok(field_vals)
}
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::type_bridge::*;
use crate::common::mangling::Mangler;
use std::collections::HashMap;
use super::{emit_expr, infer_phantom_generics};

// Stub handlers for now, to be moved to separate files
pub fn emit_lit(ctx: &mut LoweringContext, out: &mut String, lit: &syn::ExprLit, expected: Option<&Type>) -> Result<(String, Type), String> {
    match &lit.lit {
        syn::Lit::Int(n) => {
            let val_u64 = match n.base10_parse::<u64>() {
                Ok(v) => v,
                Err(_) => {
                    let s = n.token().to_string();
                    if s.starts_with("0x") || s.starts_with("0X") {
                        u64::from_str_radix(&s[2..], 16).map_err(|e| format!("Invalid hex literal {}: {}", s, e))?
                    } else if s.starts_with("0b") || s.starts_with("0B") {
                        u64::from_str_radix(&s[2..], 2).map_err(|e| format!("Invalid binary literal {}: {}", s, e))?
                    } else if s.starts_with("0o") || s.starts_with("0O") {
                        u64::from_str_radix(&s[2..], 8).map_err(|e| format!("Invalid octal literal {}: {}", s, e))?
                    } else if let Ok(signed) = s.parse::<i64>() {
                        signed as u64
                    } else {
                         return Err(format!("Invalid integer literal: {}", s));
                    }
                }
            };
            let val = val_u64 as i64;
            
            let suffix = n.suffix();
            let (target_ty, ty_str) = if !suffix.is_empty() {
                match suffix {
                    "u8" => (Type::U8, "i8"),
                    "i8" => (Type::I8, "i8"),
                    "u16" => (Type::U16, "i16"),
                    "i16" => (Type::I16, "i16"),
                    "u32" => (Type::U32, "i32"),
                    "i32" => (Type::I32, "i32"),
                    "u64" => (Type::U64, "i64"),
                    "i64" => (Type::I64, "i64"),
                    "usize" => (Type::Usize, "index"),
                    _ => (if val_u64 > 0x7FFFFFFF { Type::I64 } else { Type::I32 }, if val_u64 > 0x7FFFFFFF { "i64" } else { "i32" })
                }
            } else if let Some(exp) = expected {
                 match exp {
                     Type::U8 | Type::I8 => (exp.clone(), "i8"),
                     Type::U16 | Type::I16 => (exp.clone(), "i16"),
                     Type::U32 | Type::I32 => (exp.clone(), "i32"),
                     Type::U64 | Type::I64 => (exp.clone(), "i64"),
                     Type::Usize => (exp.clone(), "index"),
                     _ => {
                         (Type::I64, "i64") // Default to I64 for safety in mixed context
                     }
                 }
            } else {
                 let use_i64 = val_u64 > 0x7FFFFFFF;
                 (if use_i64 { Type::I64 } else { Type::I32 }, if use_i64 { "i64" } else { "i32" })
            };
            
            let res = format!("%c{}", ctx.next_id());
            ctx.emit_const_int(out, &res, val, ty_str);
            Ok((res, target_ty))
        }
        syn::Lit::Bool(b) => {
            let res = format!("%c{}", ctx.next_id());
            let val = if b.value { 1 } else { 0 };
            ctx.emit_const_int(out, &res, val, "i1"); // Salt booleans are i1 in SSA
            Ok((res, Type::Bool))
        }
        syn::Lit::Str(s) => {
             let val = s.value();
             let str_len = val.len();
             // Search for existing string literal by content
             let existing = ctx.string_literals().iter()
                 .find(|(_, content, _)| *content == val)
                 .map(|(name, _, _)| name.clone());
             let global_id = if let Some(existing_id) = existing {
                 existing_id
             } else {
                 let new_id = format!("str_{}", ctx.next_id());
                 ctx.string_literals_mut().push((new_id.clone(), val.clone(), str_len));
                 // NOTE: String global emission happens in mod.rs from string_literals list
                 // Do NOT emit to decl_out here to avoid duplicate symbols
                 new_id
             };

             // String literals produce StringView { ptr, len } by default.
             // This enables "hello".length() and other method calls on string literals.
             // Auto-extraction to !llvm.ptr happens in promote_numeric for FFI compat.
             if matches!(expected, Some(Type::Reference(..)) | Some(Type::Pointer { .. })) {
                 // Legacy path: when we KNOW the caller expects a raw pointer (e.g. explicit cast),
                 // emit as raw ptr for zero-overhead FFI.
                 let res = format!("%ptr_{}", ctx.next_id());
                 ctx.emit_addressof(out, &res, &global_id)?;
                 Ok((res, Type::Reference(Box::new(Type::U8), false)))
             } else {
                 // Default path: construct StringView { ptr: !llvm.ptr, len: i64 }
                 let ptr_var = format!("%slit_ptr_{}", ctx.next_id());
                 ctx.emit_addressof(out, &ptr_var, &global_id)?;

                 let len_var = format!("%slit_len_{}", ctx.next_id());
                 out.push_str(&format!("    {} = arith.constant {} : i64\n", len_var, str_len));

                 let sv_type = Type::Struct("std__core__str__StringView".to_string());
                 let sv_mlir = sv_type.to_mlir_type(ctx)?;
                 let undef = format!("%slit_undef_{}", ctx.next_id());
                 let with_ptr = format!("%slit_wptr_{}", ctx.next_id());
                 let result = format!("%slit_{}", ctx.next_id());
                 out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", undef, sv_mlir));
                 out.push_str(&format!("    {} = llvm.insertvalue {}, {}[0] : {}\n", with_ptr, ptr_var, undef, sv_mlir));
                 out.push_str(&format!("    {} = llvm.insertvalue {}, {}[1] : {}\n", result, len_var, with_ptr, sv_mlir));

                 Ok((result, sv_type))
              }
        }
        syn::Lit::Float(f) => {
             let val = f.base10_parse::<f64>().map_err(|e| format!("Invalid float literal: {}", e))?;
             let res = format!("%f{}", ctx.next_id());
             
             let suffix = f.suffix();
             let (target_ty, ty_str) = if suffix == "f32" {
                 (Type::F32, "f32")
             } else if suffix == "f64" {
                 (Type::F64, "f64")
             } else if let Some(Type::F32) = expected {
                 (Type::F32, "f32")
             } else {
                 (Type::F64, "f64")
             };
             
             ctx.emit_const_float(out, &res, val, ty_str);
             Ok((res, target_ty))
        }
        syn::Lit::Char(c) => {
            let val = c.value() as u32;
            let ssa = format!("%char_{}", ctx.next_id());
            ctx.emit_const_int(out, &ssa, val as i64, "i8");
            Ok((ssa, Type::I8))
        }
        _ => Err(format!("Unsupported literal: {:?}", lit.lit)),
    }
}


fn resolve_local_variable_path(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    local_vars: &HashMap<String, (Type, LocalKind)>,
    expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    if let Some((ty, kind)) = local_vars.get(name) {
        if ctx.consumed_vars().contains(name) {
            return Err(format!("Use of moved value: {}", name));
        }
        if let Some(state) = ctx.pointer_tracker.get_state(name) {
            if state == crate::codegen::verification::PointerState::Uninitialized {
                return Err(format!("Use of uninitialized pointer variable: {}", name));
            }
            let is_dynamic = *ctx.is_dynamic_check_block() || ctx.emission.in_dynamic_check_fn;
                    if !is_dynamic
                && state == crate::codegen::verification::PointerState::Freed {
                    return Err(format!("Use of freed pointer variable: {}", name));
                }
        }
        match kind {
            LocalKind::SSA(val) => return Ok(Some((val.clone(), ty.clone()))),
            LocalKind::Ptr(ptr) => {
                if let Some(exp) = expected_ty {
                    if matches!(exp, Type::Owned(..)) {
                        return Ok(Some((ptr.clone(), Type::Owned(Box::new(ty.clone())))));
                    }
                }
                let res = format!("%load_{}_{}", name, ctx.next_id());
                let scopes = Some(("#scope_local", "#scope_global"));
                ctx.emit_load_logical_with_scope(out, &res, ptr, ty, scopes)?;
                return Ok(Some((res, ty.clone())));
            }
        }
    }
    Ok(None)
}

fn resolve_constant_path(
    ctx: &mut LoweringContext,
    out: &mut String,
    mangled: &str,
) -> Result<Option<(String, Type)>, String> {
    if let Some(val) = ctx.evaluator.constant_table.get(mangled).cloned() {
        let ty = ctx.resolve_global(mangled).unwrap_or_else(|| {
            match val {
                crate::evaluator::ConstValue::Integer(_) => Type::I64,
                crate::evaluator::ConstValue::Bool(_) => Type::Bool,
                crate::evaluator::ConstValue::Float(_) => Type::F64,
                crate::evaluator::ConstValue::String(_) => Type::Reference(Box::new(Type::U8), false),
                crate::evaluator::ConstValue::Array(_) => Type::Array(Box::new(Type::I64), 0, false),
                crate::evaluator::ConstValue::Complex => Type::Unit,
            }
        });
        
        match val {
            crate::evaluator::ConstValue::Integer(i) => {
                let res = format!("%const_{}_{}", mangled, ctx.next_id());
                if matches!(ty, Type::Bool) {
                     ctx.emit_const_int(out, &res, i, "i1");
                } else if ty.is_float() {
                     ctx.emit_const_float(out, &res, i as f64, "f64");
                } else {
                     let ty_str = ty.to_mlir_type(ctx)?;
                     ctx.emit_const_int(out, &res, i, &ty_str);
                }
                return Ok(Some((res, ty)));
            }
            crate::evaluator::ConstValue::Bool(b) => {
                 let res = format!("%const_bool_{}_{}", mangled, ctx.next_id());
                 ctx.emit_const_int(out, &res, if b { 1 } else { 0 }, "i1");
                 return Ok(Some((res, Type::Bool)));
            }
            crate::evaluator::ConstValue::Float(f) => {
                 let res = format!("%const_float_{}_{}", mangled, ctx.next_id());
                 ctx.emit_const_float(out, &res, f, "f64");
                 return Ok(Some((res, Type::F64)));
            }
            crate::evaluator::ConstValue::String(_) => {
                 return Err("String constants not supported".to_string());
            }
            crate::evaluator::ConstValue::Array(_) => {
                 return Err("Array constants not supported".to_string());
            }
            crate::evaluator::ConstValue::Complex => return Err("Complex constants not supported".to_string()),
        }
    }
    Ok(None)
}

fn resolve_global_variable_path(
    ctx: &mut LoweringContext,
    out: &mut String,
    p: &syn::ExprPath,
    mangled: &str,
) -> Result<Option<(String, Type)>, String> {
    let mut global_ty = ctx.resolve_global(mangled);
    let mut valid_mangled = mangled.to_string();
    
    if global_ty.is_none() && p.path.segments.len() == 2 {
        let type_name = p.path.segments[0].ident.to_string();
        let method_name = p.path.segments[1].ident.to_string();
        
        let struct_ty = crate::types::Type::Struct(type_name);
        let resolved = crate::codegen::type_bridge::resolve_codegen_type(ctx, &struct_ty);
        
        if let crate::types::Type::Struct(fqn) = resolved {
             let candidate = format!("{}__{}", fqn, method_name);
             if let Some(ty) = ctx.resolve_global(&candidate) {
                  valid_mangled = candidate;
                  global_ty = Some(ty);
             }
        }
    }

    match global_ty {
        Some(ty) => {
            let mangled = valid_mangled;
            let scopes = Some(("#scope_global", "#scope_local"));
            
             if let Type::Fn(ref param_tys, ref ret_ty) = ty {
                 let is_local = ctx.require_local_function(&mangled);
                 if !is_local {
                     ctx.ensure_func_declared(&mangled, param_tys, ret_ty)?;
                 }
                 
                 let typed_ref = format!("%fn_typed_{}", ctx.next_id());
                 let ptr_var = format!("%fn_ref_{}", ctx.next_id());
                 let arg_tys_mlir: Vec<String> = param_tys.iter()
                     .map(|t| t.to_mlir_type(ctx).unwrap_or("!llvm.ptr".to_string()))
                     .collect();
                 let ret_ty_mlir = if **ret_ty == Type::Unit {
                     "()".to_string()
                 } else {
                     ret_ty.to_mlir_type(ctx).unwrap_or("!llvm.ptr".to_string())
                 };
                 let fn_type_str = if **ret_ty == Type::Unit {
                     format!("({}) -> ()", arg_tys_mlir.join(", "))
                 } else {
                     format!("({}) -> {}", arg_tys_mlir.join(", "), ret_ty_mlir)
                 };
                 out.push_str(&format!("    {} = func.constant @{} : {}\n",
                     typed_ref, mangled, fn_type_str));
                 out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : {} to !llvm.ptr\n",
                     ptr_var, typed_ref, fn_type_str));
                 return Ok(Some((ptr_var, ty.clone())));
             }
             
             ctx.ensure_global_declared(&mangled, &ty)?;
             let ptr = format!("%global_ptr_{}", ctx.next_id());
             ctx.emit_addressof(out, &ptr, &mangled)?;
             
             let val_loaded = format!("%global_val_{}", ctx.next_id());
             ctx.emit_load_logical_with_scope(out, &val_loaded, &ptr, &ty, scopes)?;
             
             Ok(Some((val_loaded, ty.clone())))
        },
        None => Ok(None)
    }
}

fn resolve_enum_variant_suffix(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    mangled: &str,
) -> Result<Option<(String, Type)>, String> {
    if !ctx.globals().contains_key(mangled) && !ctx.evaluator.constant_table.contains_key(mangled) {
        let mut found_variant = None;

        let mut sorted_enums: Vec<_> = ctx.enum_registry().values().collect();
        sorted_enums.sort_by_key(|e| &e.name);
        for info in sorted_enums {
             for (var_name, payload, disc) in &info.variants {
                 if var_name == name {
                     found_variant = Some((info.name.clone(), *disc, payload.is_none()));
                     break;
                 }
             }
             if found_variant.is_some() { break; }
        }

        if let Some((enum_name, disc, is_unit)) = found_variant {
             if is_unit {
                 let enum_ty = Type::Enum(enum_name.clone());
                 let mlir_ty = enum_ty.to_mlir_type(ctx)?;
                 
                 let res = format!("%enum_val_{}", ctx.next_id());
                 let undef = format!("{}_undef", res);
                 
                 out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", undef, mlir_ty));
                 
                 let tag_val = format!("{}_tag", res);
                 ctx.emit_const_int(out, &tag_val, disc as i64, "i32");
                 
                 out.push_str(&format!("    {} = llvm.insertvalue {}, {}[0] : {}\n", 
                     res, tag_val, undef, mlir_ty));
                 
                 return Ok(Some((res, enum_ty)));
             } else {
                 return Err(format!("Cannot use tuple variant '{}' as value without arguments", name));
             }
        }
    }
    Ok(None)
}

fn resolve_enum_variant_full(
    ctx: &mut LoweringContext,
    out: &mut String,
    p: &syn::ExprPath,
    segments: &[String],
    expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    if segments.len() < 2 {
        return Ok(None);
    }
    let enum_name = &segments[0];
    let variant_name = segments.last().ok_or_else(|| "Empty segments in Enum Variant Lookup".to_string())?;

    let mut resolved_enum_name = enum_name.clone();
    let is_local = ctx.enum_templates().contains_key(enum_name) || ctx.enum_registry().values().any(|i| i.name == *enum_name);
    for imp in ctx.imports() {
         if is_local { break; }
         if let Some(alias) = &imp.alias {
             if *alias == *enum_name {
                 resolved_enum_name = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                 break;
             }
         } else if let Some(group) = &imp.group {
             if group.iter().any(|id| *id == *enum_name) {
                 let mut parts: Vec<String> = imp.name.iter().map(|id| id.to_string()).collect();
                 parts.push(enum_name.clone());
                 resolved_enum_name = Mangler::mangle(&parts);
                 break;
             }
         } else if let Some(last) = imp.name.last() {
             if *last == *enum_name {
                  resolved_enum_name = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                  break;
             }
         }
    }
    let mut generic_args = Vec::new();
    if let Some(seg) = p.path.segments.first() {
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
            for arg in &args.args {
                if let syn::GenericArgument::Type(ty) = arg {
                    let syn_ty = crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?;
                    generic_args.push(crate::codegen::type_bridge::resolve_type(ctx, &syn_ty));
                }
            }
        }
    }
    let found_enum = if ctx.enum_registry().values().any(|i| i.name == resolved_enum_name) {
         Some(Type::Enum(resolved_enum_name.clone()))
    } else if ctx.enum_templates().get(&resolved_enum_name).is_some() {
         if !generic_args.is_empty() {
             Some(Type::Concrete(resolved_enum_name.clone(), generic_args))
         } else {
             let inferred = if let Some(exp) = expected_ty {
                 if let Type::Concrete(exp_name, exp_args) = exp {
                     if exp_name == &resolved_enum_name {
                         Some(Type::Concrete(resolved_enum_name.clone(), exp_args.clone()))
                     } else { None }
                 } else if let Type::Enum(exp_name) = exp {
                      if exp_name.starts_with(&resolved_enum_name) {
                          Some(Type::Enum(exp_name.clone()))
                      } else { None }
                 } else { None }
             } else { None };
             
             inferred.or(Some(Type::Enum(resolved_enum_name.clone())))
         }
    } else { None };

    if let Some(base_ty) = found_enum {
        let resolved = crate::codegen::type_bridge::resolve_codegen_type(ctx, &base_ty);
        
        let mangled_enum_opt = if let Type::Enum(n) = &resolved {
            Some(n.clone())
        } else if let Type::Concrete(base, args) = &resolved {
            let suffix = args.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_");
            Some(format!("{}_{}", base, suffix))
        } else { None };

        if let Some(mangled_enum) = mangled_enum_opt {
            if let Some(info) = ctx.enum_registry().values().find(|i| i.name == mangled_enum).cloned() {
                if let Some((_idx, payload, disc)) = info.variants.iter().enumerate().find(|(_, (n, _, _))| n == variant_name).map(|(i, v)| (i, v.1.as_ref(), v.2)) {
                    let payload_opt: Option<&Type> = payload;
                    if payload_opt.is_none() {
                         let res = format!("%variant_{}", ctx.next_id());
                         let mlir_ty = resolved.to_mlir_type(ctx)?;
                         out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", res, mlir_ty));
                         
                         let tag_val = format!("%tag_{}", ctx.next_id());
                         ctx.emit_const_int(out, &tag_val, disc as i64, "i32");
                         
                         let final_res = format!("%variant_final_{}", ctx.next_id());
                         ctx.emit_insertvalue(out, &final_res, &tag_val, &res, 0, &mlir_ty);
                         return Ok(Some((final_res, resolved)));
                    }
                }
            }
        }
    }
    Ok(None)
}

fn resolve_generics_and_functions(
    ctx: &mut LoweringContext,
    out: &mut String,
    segments: &[String],
) -> Result<Option<(String, Type)>, String> {
    if segments.len() == 1 {
         let name = &segments[0];
         if let Some(Type::Struct(s)) = ctx.current_type_map().get(name) {
                 if let Ok(i) = s.parse::<i64>() {
                     let res = format!("%const_gen_{}", ctx.next_id());
                     ctx.emit_const_int(out, &res, i, "i64");
                     return Ok(Some((res, Type::I64)));
                 } else {
                     let res = format!("%undef_const_{}", ctx.next_id());
                     out.push_str(&format!("    {} = llvm.mlir.undef : i64\n", res));
                     return Ok(Some((res, Type::I64)));
                 }
         }
    }
    if segments.len() == 1 {
        let name = &segments[0];
        let current_pkg_prefix = ctx.package_prefix();
        let mangled_fn = format!("{}{}", current_pkg_prefix, name);

        let fn_sig = ctx.config.file.items.iter().find_map(|item| {
            if let crate::grammar::Item::Fn(f) = item {
                let my_mangled = if f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" ) {
                    f.name.to_string()
                } else {
                    format!("{}{}", current_pkg_prefix, f.name)
                };
                if my_mangled == mangled_fn {
                    let arg_types: Vec<Type> = f.args.iter().filter_map(|arg| {
                        arg.ty.as_ref().and_then(Type::from_syn)
                    }).collect();
                    let ret_type = f.ret_type.as_ref()
                        .and_then(Type::from_syn)
                        .unwrap_or(Type::Unit);
                    Some((mangled_fn.clone(), Type::Fn(arg_types, Box::new(ret_type))))
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some((fn_mangled, fn_ty)) = fn_sig {
            ctx.globals_mut().insert(fn_mangled.clone(), fn_ty.clone());
            if let Type::Fn(ref param_tys, ref ret_ty) = fn_ty {
                let typed_ref = format!("%fn_typed_{}", ctx.next_id());
                let ptr_var = format!("%fn_ref_{}", ctx.next_id());
                let arg_tys_mlir: Vec<String> = param_tys.iter()
                    .map(|t| t.to_mlir_type(ctx).unwrap_or("!llvm.ptr".to_string()))
                    .collect();
                let ret_ty_mlir = if **ret_ty == Type::Unit {
                    "()".to_string()
                } else {
                    ret_ty.to_mlir_type(ctx).unwrap_or("!llvm.ptr".to_string())
                };
                let fn_type_str = if **ret_ty == Type::Unit {
                    format!("({}) -> ()", arg_tys_mlir.join(", "))
                } else {
                    format!("({}) -> {}", arg_tys_mlir.join(", "), ret_ty_mlir)
                };
                out.push_str(&format!("    {} = func.constant @{} : {}\n",
                    typed_ref, fn_mangled, fn_type_str));
                out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : {} to !llvm.ptr\n",
                    ptr_var, typed_ref, fn_type_str));
                return Ok(Some((ptr_var, fn_ty)));
            }
        }
    }
    Ok(None)
}

pub fn emit_path(ctx: &mut LoweringContext, out: &mut String, p: &syn::ExprPath, local_vars: &mut HashMap<String, (Type, LocalKind)>, _expected: Option<&Type>) -> Result<(String, Type), String> {
    let segments: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
    let name = segments[0].clone();
    
    if segments.len() == 1 {
        if let Some(res) = resolve_local_variable_path(ctx, out, &name, local_vars, _expected)? {
            return Ok(res);
        }
    }

    let mut mangled = Mangler::mangle(&segments);
    if segments.len() == 1 {
        if let Some(pkg) = &*ctx.current_package {
            let pkg_mangled = Mangler::mangle(&pkg.name.iter().map(|id: &syn::Ident| id.to_string()).collect::<Vec<_>>());
            let local_mangled = Mangler::mangle(&[&pkg_mangled, &mangled]);
            if name == "GLOBAL_ALLOC" {
                 let _resol = ctx.resolve_global(&local_mangled);
            }
            if ctx.resolve_global(&local_mangled).is_some() || ctx.evaluator.constant_table.contains_key(&local_mangled) {
                mangled = local_mangled;
            }
        }
    }

    if let Some(res) = resolve_enum_variant_suffix(ctx, out, &name, &mangled)? {
        return Ok(res);
    }

    if let Some(res) = resolve_constant_path(ctx, out, &mangled)? {
        return Ok(res);
    }

    if let Some(res) = resolve_global_variable_path(ctx, out, p, &mangled)? {
        return Ok(res);
    }

    if let Some(res) = resolve_enum_variant_full(ctx, out, p, &segments, _expected)? {
        return Ok(res);
    }

    if let Some(res) = resolve_generics_and_functions(ctx, out, &segments)? {
        return Ok(res);
    }

    Err(format!("Undefined variable or constant: {}", segments.join(".")))
}

pub fn emit_array(ctx: &mut LoweringContext, out: &mut String, a: &syn::ExprArray, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected_ty: Option<&Type>) -> Result<(String, Type), String> {
    if a.elems.is_empty() {
        return Err("Empty array literal not supported yet".to_string());
    }
    let mut elem_vals = Vec::new();
    let mut elem_ty = Type::Unit;
    
    // Determine expected element type
    let expected_elem = if let Some(Type::Array(inner, _, _)) = expected_ty {
        Some(inner.as_ref())
    } else {
        None
    };

    // 1. Evaluate elements and determine type
    for (i, expr) in a.elems.iter().enumerate() {
        let (val, ty) = emit_expr(ctx, out, expr, local_vars, expected_elem)?;
        if i == 0 {
            elem_ty = ty;
        } else if ty != elem_ty {
             // Basic type unification: strict MLIR type equality.
             // Supertypes are not silently promoted to maintain predictable bounds.
             if ty.to_mlir_type(ctx)? != elem_ty.to_mlir_type(ctx)? {
                 return Err(format!("Array element type mismatch at index {}: expected {:?}, found {:?}", i, elem_ty, ty));
             }
        }
        elem_vals.push(val);
    }
    
    let len = elem_vals.len();
    let array_ty = Type::Array(Box::new(elem_ty.clone()), len, false);
    let mlir_ty = array_ty.to_mlir_type(ctx)?;
    
    let mut current_array = format!("%array_init_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", current_array, mlir_ty));
    
    for (i, val) in elem_vals.iter().enumerate() {
        let next_array = format!("%array_step_{}_{}", i, ctx.next_id());
        
        let val_insert = if elem_ty == Type::Bool {
             let zext = format!("%b_zext_arr_{}_{}", i, ctx.next_id());
             ctx.emit_cast(out, &zext, "arith.extui", val, "i1", "i8");
             zext
        } else {
             val.clone()
        };
        
        ctx.emit_insertvalue(out, &next_array, &val_insert, &current_array, i, &mlir_ty);
        current_array = next_array;
    }
    
    Ok((current_array, array_ty))
}

pub fn emit_tuple(ctx: &mut LoweringContext, out: &mut String, t: &syn::ExprTuple, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(String, Type), String> {
    let mut elem_vals = Vec::new();
    let mut elem_tys = Vec::new();
    
    for expr in &t.elems {
        let (val, ty) = emit_expr(ctx, out, expr, local_vars, None)?;
        elem_vals.push(val);
        elem_tys.push(ty);
    }
    
    let tuple_ty = Type::Tuple(elem_tys.clone());
    let mlir_ty = tuple_ty.to_mlir_type(ctx)?;
    
    let mut current_tuple = format!("%tuple_init_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", current_tuple, mlir_ty));
    
    for (i, val) in elem_vals.iter().enumerate() {
        let next_tuple = format!("%tuple_step_{}_{}", i, ctx.next_id());
        
        let val_insert = if elem_tys[i] == Type::Bool {
             let zext = format!("%b_zext_tup_{}_{}", i, ctx.next_id());
             ctx.emit_cast(out, &zext, "arith.extui", val, "i1", "i8");
             zext
        } else {
             val.clone()
        };
        
        ctx.emit_insertvalue(out, &next_tuple, &val_insert, &current_tuple, i, &mlir_ty);
        current_tuple = next_tuple;
    }
    
    Ok((current_tuple, tuple_ty))
}

pub fn emit_repeat(ctx: &mut LoweringContext, out: &mut String, r: &syn::ExprRepeat, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected_ty: Option<&Type>) -> Result<(String, Type), String> {
    let expected_elem = if let Some(Type::Array(inner, _, _)) = expected_ty {
        Some(inner.as_ref())
    } else {
        None
    };
    let (val, ty) = emit_expr(ctx, out, &r.expr, local_vars, expected_elem)?;
    let len = match ctx.evaluator.eval_expr(&r.len) {
        Ok(crate::evaluator::ConstValue::Integer(v)) => v as usize,
        _ => return Err("Array repeat length must be a constant integer".to_string()),
    };
    
    let array_ty = Type::Array(Box::new(ty.clone()), len, false);
    let mlir_array_ty = array_ty.to_mlir_type(ctx)?;
    
    // BULK INIT FAST PATH
    // Detect zero-initialization pattern [0; N] for large arrays
    let is_zero_init = match &*r.expr {
        syn::Expr::Lit(l) => match &l.lit {
            syn::Lit::Int(n) => n.base10_parse::<i64>().unwrap_or(1) == 0,
            _ => false,
        },
        _ => false,
    };
    
    // For large zero-init arrays, use alloca + memset instead of O(n) insertvalues
    if is_zero_init && len > 64 {
        // Calculate element size in bytes
        let elem_size = match &ty {
            Type::U8 | Type::I8 | Type::Bool => 1,
            Type::U16 | Type::I16 => 2,
            Type::U32 | Type::I32 | Type::F32 => 4,
            Type::U64 | Type::I64 | Type::F64 | Type::Usize => 8,
            _ => return Err(format!("Bulk init for element type {:?} not supported", ty)),
        };
        let total_size = elem_size * len;
        
        // Limit to 1MB to avoid stack overflow
        if total_size > 1024 * 1024 {
            return Err(format!("Array size {} bytes exceeds 1MB limit for stack allocation", total_size));
        }
        
        // Allocate raw bytes on stack
        let buf_ptr = format!("%array_buf_{}", ctx.next_id());
        let size_const = format!("%array_size_{}", ctx.next_id());
        let one_const = format!("%one_{}", ctx.next_id());
        
        // Emit size constant and alloca
        out.push_str(&format!("    {} = arith.constant {} : i64\n", size_const, total_size));
        out.push_str(&format!("    {} = arith.constant 1 : i64\n", one_const));
        out.push_str(&format!("    {} = llvm.alloca {} x i8 : (i64) -> !llvm.ptr\n", buf_ptr, one_const));
        
        // Emit memset to zero: "llvm.intr.memset"(ptr, val, len) <{isVolatile = false}>
        let zero_val = format!("%zero_byte_{}", ctx.next_id());
        out.push_str(&format!("    {} = arith.constant 0 : i8\n", zero_val));
        out.push_str(&format!("    \"llvm.intr.memset\"({}, {}, {}) <{{isVolatile = false}}> : (!llvm.ptr, i8, i64) -> ()\n", 
            buf_ptr, zero_val, size_const));
        
        // Load as the array type
        let result = format!("%array_zeroed_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", result, buf_ptr, mlir_array_ty));
        
        return Ok((result, array_ty));
    }
    
    // For non-zero or small arrays, use insertvalue loop (existing logic)
    if len > 128 {
        return Err(format!("Array repeat length {} with non-zero init too large. Use zero-init [0; N] for large arrays.", len));
    }
    
    let mut current_array = format!("%array_init_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", current_array, mlir_array_ty));
    
    // Fix for boolean array storage (i1 -> i8)
    let val_insert = if ty == Type::Bool {
          let zext = format!("%b_zext_rep_{}", ctx.next_id());
          ctx.emit_cast(out, &zext, "arith.extui", &val, "i1", "i8");
          zext
    } else {
          val.clone()
    };
    
    for i in 0..len {
        let next_array = format!("%array_step_{}_{}", i, ctx.next_id());
        out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", next_array, val_insert, current_array, i, mlir_array_ty));
        current_array = next_array;
    }
    
    Ok((current_array, array_ty))
}

pub fn emit_struct(ctx: &mut LoweringContext, out: &mut String, s: &syn::ExprStruct, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(String, Type), String> {
    // Convert path to Type for resolution
    let path_ty = syn::Type::Path(syn::TypePath { qself: None, path: s.path.clone() });
    let syn_ty = crate::grammar::SynType::from_std(path_ty).map_err(|e| e.to_string())?;
    let raw_resolved_ty = resolve_type(ctx, &syn_ty);
    // Apply generic substitution for struct literals in specialized method contexts
    // This ensures RawVec in RawVec<T>::new() resolves to RawVec_u8 when T=u8
    let resolved_ty = raw_resolved_ty.substitute(ctx.current_type_map());
    
    // Check if this struct literal matches the current impl context
    // If inside RawVec<T>::new() and the struct literal is RawVec { ... },
    // the current generic arguments (T=u8) are applied to produce RawVec_u8
    let resolved_ty_with_context = match &resolved_ty {
        Type::Struct(name) => {
            // Check if this struct has a template in the registry
            // Use find_struct_template_by_name to get the fully-qualified name
            let template_name = ctx.find_struct_template_by_name(name);
            if template_name.is_some() && !ctx.current_type_map().is_empty() {
                let full_name = template_name.clone().expect("Template name must exist");
                let _has_template = ctx.struct_templates().contains_key(&full_name);
                // Build args in template's declared generic parameter order,
                // NOT HashMap::values() order which is non-deterministic.
                // Without this, Vec<T, A> with {T: I64, A: HeapAllocator} could produce
                // [HeapAllocator, I64] instead of [I64, HeapAllocator].
                let args: Vec<Type> = infer_struct_generics(ctx, s, &full_name, local_vars);
                if !args.is_empty() {
                    Type::Concrete(full_name, args)
                } else {
                    resolved_ty.clone()
                }
            } else {
                resolved_ty.clone()
            }
        }
        // Handle Concrete types with empty args in specialized method context
        // If we have Concrete(RawVec, []) inside RawVec<T>::new() with T=u8, produce Concrete(RawVec, [u8])
        Type::Concrete(base, args) if args.is_empty() && !ctx.current_type_map().is_empty() => {
            // Build args in template's declared generic parameter order,
            // NOT HashMap::values() order which is non-deterministic.
            let type_map_args: Vec<Type> = infer_struct_generics(ctx, s, base, local_vars);
            Type::Concrete(base.clone(), type_map_args)
        }
        _ => resolved_ty.clone(),
    };
    
    // Explicitly handle Concrete types by trigger specialization
    let mangled_name = match &resolved_ty_with_context {
        Type::Struct(n) => n.clone(),
        Type::Concrete(base, args) => {
             let is_enum = ctx.enum_templates().contains_key(base);
             ctx.specialize_template(base, args, is_enum)?.mangle()
        },
        _ => return Err(format!("Struct instantiation resolved to non-struct type: {:?}", resolved_ty_with_context)),
    };
    // TOP MINDS: Ensure struct exists (triggers instantiation if not in registry)
    // This handles cross-function hydration where the struct wasn't used in main
    let _ = ctx.ensure_struct_exists(&mangled_name, &[]);
    
    // Phase 5: Use centralized template lookup
    let short_name = s.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
    if let Some(template_name) = ctx.find_struct_template_by_name(&short_name) {
        let _ = ctx.ensure_struct_exists(&template_name, &[]);
    }
    
    // Phase 5: Use centralized struct lookup
    let info_opt = ctx.find_struct_by_name(&mangled_name);
    if let Some(info) = info_opt {
        // Temporarily swap type_map to include the struct's own
        // generic bindings during field emission. This prevents generic name collisions
        // when constructing a struct whose params shadow the enclosing scope
        // (e.g., Map<I,F,T> inside Filter<I,F>::map where I/F have different meanings).
        let prev_type_map = ctx.current_type_map().clone();
        if let Type::Concrete(base, args) = &resolved_ty_with_context {
            let generics_owned = ctx.struct_templates().get(base)
                .and_then(|t| t.generics.clone());
            if let Some(generics) = generics_owned {
                for (i, param) in generics.params.iter().enumerate() {
                    if let Some(arg) = args.get(i) {
                        let name = match param {
                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                        };
                        ctx.current_type_map_mut().insert(name, arg.clone());
                    }
                }
            }
        }

        let field_vals = eval_struct_fields(ctx, out, s, &info, &mangled_name, local_vars)?;
        
        
        // Fix: Use the computed mangled name (specialized) instead of the original resolved type
        // This ensures Concrete(Vec, [u8]) becomes Struct(Vec_u8) for MLIR emission.
        let struct_ty = Type::Struct(mangled_name.clone());

        // Use resolved_ty_with_context for MLIR type generation to include impl context specialization.
        // This ensures Concrete(Vec, [u8]) becomes Struct(Vec_u8) for MLIR emission, and
        // RawVec inside RawVec<T>::new() becomes RawVec_u8 when T=u8.
        let mlir_ty = resolved_ty_with_context.to_mlir_type(ctx)?;

        // SCALAR OPTIMIZATION: If struct lowers to i64 (implied wrapper like Ptr or SlabCache),
        // return the single field value directly.
        if mlir_ty == "i64" {
             // Exactly one field is assumed to be populated (or the first/only one is taken)
             if let Some((val_expr, _)) = field_vals.values().next() {
                 return Ok((val_expr.clone(), struct_ty));
             } else {
                 return Err(format!("Scalar wrapper struct construction (i64) requires 1 field. Found: {:?}", field_vals.keys()));
             }

        }
        
        // Clean Break: Removed NativePtr struct construction intercept
        // NativePtr is now Type::Pointer and cannot be constructed via struct syntax.
        
        let mut current_struct = format!("%struct_init_{}", ctx.next_id());
        
        // Start with undef
        out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", current_struct, mlir_ty));
        
        for (i, field_ty) in info.field_order.iter().enumerate() {
             let field_name = info.fields.iter().find(|(_, (idx, _))| *idx == i).map(|(k, _)| k).ok_or_else(|| format!("Field index {} not found", i))?;
             if let Some((val, actual_ty)) = field_vals.get(field_name) {
                  let next_struct = format!("%struct_step_{}", ctx.next_id());
                  let concrete_field_ty = field_ty.substitute(ctx.current_type_map());
                  let val_prom = promote_numeric(ctx, out, val, actual_ty, &concrete_field_ty)?;
                  let phys_idx = ctx.get_physical_index(&info.field_order, i);
                  ctx.emit_insertvalue_logical(out, &next_struct, &val_prom, &current_struct, phys_idx, &mlir_ty, &concrete_field_ty)?;
                  current_struct = next_struct;
             }
        }

        // Restore the type_map after struct field emission
        *ctx.current_type_map_mut() = prev_type_map;
        
        Ok((current_struct, struct_ty))
    } else {
        Err(format!("Undefined struct: {}", mangled_name))
    }
}

pub(crate) fn emit_enum_constructor(
    ctx: &mut LoweringContext, 
    out: &mut String, 
    info: crate::codegen::expr::utils::EnumVariantResolution,
    args: &[syn::Expr], 
    local_vars: &mut HashMap<String, (Type, LocalKind)>
) -> Result<(String, Type), String> {
    let resolved_ret = if !info.generic_args.is_empty() {
        Type::Concrete(info.enum_name.clone(), info.generic_args)
    } else {
        Type::Enum(info.enum_name.clone())
    };
    
    let mlir_ty = resolved_ret.to_mlir_type(ctx)?;
    let mut current_enum = format!("%enum_init_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", current_enum, mlir_ty));
    
    // 1. Discriminant
    let disc_reg = format!("%disc_{}", ctx.next_id());
    ctx.emit_const_int(out, &disc_reg, info.discriminant as i64, "i32");
    let next_enum = format!("%enum_disc_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.insertvalue {}, {}[0] : {}\n", next_enum, disc_reg, current_enum, mlir_ty));
    current_enum = next_enum;

    // 2. Payload
    // Check registry for max size using the full mangled name
    let mangled_key = resolved_ret.mangle_suffix();
    let enum_info = ctx.enum_registry().values().find(|i| i.name == mangled_key).cloned();
    
    // Fallback: If registry lookup fails (e.g. cross-crate issue), estimate from payload type
    let max_size = if let Some(ei) = enum_info { 
        ei.max_payload_size 
    } else {
        info.payload_ty.as_ref().map(|t| t.size_of(ctx.struct_registry())).unwrap_or(0)
    };
    

    if max_size > 0 {
        let array_mlir_ty = format!("!llvm.array<{} x i8>", max_size);
        let final_payload_val = if let Some(target_payload_ty) = &info.payload_ty {
             let payload_val = if args.len() > 1 {
                 // Multi-field variant: emit all args as a tuple, then store as bytes.
                 let mut field_vals = Vec::new();
                 let tuple_types: Vec<Type> = if let Type::Tuple(tys) = target_payload_ty { tys.clone() } else { vec![target_payload_ty.clone()] };
                 for (i, arg) in args.iter().enumerate() {
                     let expected = tuple_types.get(i).unwrap_or(target_payload_ty);
                     let (v, _) = emit_expr(ctx, out, arg, local_vars, Some(expected))?;
                     field_vals.push(v);
                 }
                 // Build the tuple value
                 let tuple_mlir = target_payload_ty.to_mlir_type(ctx)?;
                 let mut current = format!("%tuple_init_{}", ctx.next_id());
                 out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", current, tuple_mlir));
                 for (i, fv) in field_vals.iter().enumerate() {
                     let next = format!("%tuple_f{}_id{}", i, ctx.next_id());
                     out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", next, fv, current, i, tuple_mlir));
                     current = next;
                 }
                 current
             } else if let Some(arg_expr) = args.first() {
                 let (val, _ty) = emit_expr(ctx, out, arg_expr, local_vars, Some(target_payload_ty))?;
                 val
             } else {
                 let zero_array = format!("%zero_payload_{}", ctx.next_id());
                 out.push_str(&format!("    {} = llvm.mlir.zero : {}\n", zero_array, array_mlir_ty));
                 zero_array
             };
             // Store payload to buffer and load as byte array
             let payload_buffer = format!("%payload_buf_{}", ctx.next_id());
             out.push_str(&format!("    {} = llvm.alloca %c1_i64 x {} {{alignment = 8 : i64}} : (i64) -> !llvm.ptr\n", payload_buffer, array_mlir_ty));
             let target_payload_mlir: String = target_payload_ty.to_mlir_type(ctx)?;
             out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", payload_val, payload_buffer, target_payload_mlir));
             let loaded_array = format!("%payload_loaded_{}", ctx.next_id());
             out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", loaded_array, payload_buffer, array_mlir_ty));
             loaded_array
        } else {
             let zero_array = format!("%zero_payload_{}", ctx.next_id());
             out.push_str(&format!("    {} = llvm.mlir.zero : {}\n", zero_array, array_mlir_ty));
             zero_array
        };
        
        let next_enum_with_payload = format!("%enum_payload_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.insertvalue {}, {}[1] : {}\n", next_enum_with_payload, final_payload_val, current_enum, mlir_ty));
        current_enum = next_enum_with_payload;
    }

    Ok((current_enum, resolved_ret))
}
