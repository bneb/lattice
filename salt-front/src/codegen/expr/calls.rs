use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::type_bridge::*;
use super::resolver;
use std::collections::HashMap;
use super::emit_expr;
use super::literals::emit_enum_constructor;
use super::call_helpers::{emit_low_level_call, handle_post_call_state};
#[allow(clippy::too_many_arguments)] // REASON: all 9 params independently meaningful; bundling would obscure intent
fn emit_function_call(
    ctx: &mut LoweringContext,
    out: &mut String,
    c: &syn::ExprCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    _expected: Option<&Type>,
    mangled_name: String,
    ret_ty: Type,
    arg_tys: Vec<Type>,
    lazy_task: Option<Box<crate::codegen::collector::MonomorphizationTask>>,
) -> Result<(String, Type), String> {
             let args_vec: Vec<syn::Expr> = c.args.iter().cloned().collect();
             hydrate_function_if_needed(ctx, &mangled_name, &arg_tys, &ret_ty, &lazy_task)?;
             let (requires, ensures, param_names) = extract_verification_data(ctx, &mangled_name, &lazy_task);

             let (args_vals, inferred_tys) = emit_function_args(
                 ctx, out, &args_vec, local_vars, &arg_tys, &requires, &param_names
             )?;

             let final_arg_tys = if arg_tys.is_empty() && !c.args.is_empty() { inferred_tys } else { arg_tys };

             let (res_val, final_ret_ty) = emit_low_level_call(
                 ctx, out, &mangled_name, &args_vec, &args_vals, &final_arg_tys, &ret_ty, &ensures, &param_names
             )?;

             handle_post_call_state(ctx, &mangled_name);

             // Flow callee postconditions into caller's Z3 solver
             if !ensures.is_empty() && !res_val.is_empty() {
                 crate::codegen::expr::call_helpers::apply_ensures_to_solver(
                     ctx, &ensures, &param_names, &args_vec, &res_val,
                 );
             }

             let mut final_res = res_val;
             let mut final_ret_ty_out = final_ret_ty.clone();

             if let Some(exp) = _expected {
                 if exp.is_numeric() && final_ret_ty_out.is_numeric() {
                     if let Ok(promoted) = promote_numeric(ctx, out, &final_res, &final_ret_ty_out, exp) {
                         final_res = promoted;
                         final_ret_ty_out = exp.clone();
                     }
                 }
             }

             Ok((final_res, final_ret_ty_out))
}

pub fn emit_call(ctx: &mut LoweringContext, out: &mut String, c: &syn::ExprCall, local_vars: &mut HashMap<String, (Type, LocalKind)>, _expected: Option<&Type>) -> Result<(String, Type), String> {
    // __z3_forall is a symbolic quantifier handled by the Z3 translator.
    // At the MLIR emission level, emit `true` since the contract is Z3-proven.
    if let syn::Expr::Path(p) = &*c.func {
        if p.path.is_ident("__z3_forall") || p.path.is_ident("__z3_exists") {
            let res = format!("%z3q_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant true\n", res));
            return Ok((res, Type::Bool));
        }
    }

    // TENSOR CONSTRUCTOR: Tensor<T>(value, [dims])
    // Intercept before resolver to handle as builtin type constructor
    if let syn::Expr::Path(p) = &*c.func {
        if let Some(first_seg) = p.path.segments.first() {
            if first_seg.ident == "Tensor" {
                return emit_tensor_constructor(ctx, out, c, &first_seg.arguments, local_vars);
            }
        }
    }

    // INDIRECT FUNCTION CALL: f(args) or (self.func)(args)
    // When the call target is an expression that evaluates to Type::Fn,
    // bypass the resolver and emit an LLVM indirect call through the pointer.
    // This enables zero-cost combinators: monomorphized generics receive function
    // pointers which LLVM devirtualizes when the pointer is a known constant.
    if let Some(res) = try_emit_indirect_call(ctx, out, c, local_vars)? {
        return Ok(res);
    }

    let mut resolver = resolver::CallSiteResolver::new(ctx);
    let resolved_call = resolver.resolve_call(c, local_vars, _expected)?;

    match resolved_call {
        resolver::CallKind::Intrinsic(name, explicit_generics) => {
            emit_intrinsic_call(ctx, out, c, name, explicit_generics, local_vars, _expected)
        },
        resolver::CallKind::EnumConstructor(res) => {
             let args_vec: Vec<syn::Expr> = c.args.iter().cloned().collect();
             emit_enum_constructor(ctx, out, res, &args_vec, local_vars)
        },
        resolver::CallKind::StructLiteral(struct_name, fields) => {
            emit_struct_literal_call(ctx, out, c, struct_name, fields, local_vars)
        },
        resolver::CallKind::TransparentVecAccess { method, element_ty, receiver, args } => {
            emit_transparent_vec_access(ctx, out, method, element_ty, *receiver, args, local_vars)
        },
        resolver::CallKind::Function(mangled_name, ret_ty, arg_tys, lazy_task) => {
             emit_function_call(ctx, out, c, local_vars, _expected, mangled_name, ret_ty, arg_tys, lazy_task)
        }
    }
}

pub fn emit_method_call(ctx: &mut LoweringContext, out: &mut String, m: &syn::ExprMethodCall, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected_ty: Option<&Type>) -> Result<(String, Type), String> {
    
    // Mark malloc'd pointers as escaped when passed
    // as function arguments to method calls.
    for arg_expr in m.args.iter() {
        super::mark_expression_escaped(ctx, arg_expr);
    }

    // When .free() or .drop() is called on a variable,
    // mark it as released in the Z3 ownership tracker so verify_leak_free passes.
    // Also remove from cleanup stack to prevent double-free in RAII cleanup.
    let method_name = m.method.to_string();
    if method_name == "free" || method_name == "drop" {
        if let syn::Expr::Path(p) = &*m.receiver {
            if let Some(ident) = p.path.get_ident() {
                let var_name = ident.to_string();
                let _ = ctx.ownership_tracker.mark_released(
                    &var_name,
                    ctx.z3_solver
                );
                // Remove from RAII cleanup stack to prevent double-free
                ctx.release_by_var_name(&var_name);
            }
        }
    }
    
    // 0. Try Intrinsic (Primitive Methods like popcount)
    let mut intrinsic_args = Vec::new();
    intrinsic_args.push(*m.receiver.clone());
    intrinsic_args.extend(m.args.iter().cloned());
    if let Ok(Some(res)) = ctx.emit_intrinsic(out, &m.method.to_string(), &intrinsic_args, local_vars, expected_ty) {
         return Ok(res);
    }
    

    // Emit the receiver expression EXACTLY ONCE at the top of emit_method_call.
    let (cached_receiver_val, cached_receiver_ty): (Option<String>, Type) = 
        if let syn::Expr::Path(p) = &*m.receiver {
            if let Some(ident) = p.path.get_ident() {
                let var_name = ident.to_string();
                if let Some((ty, kind)) = local_vars.get(&var_name) {
                    match kind {
                        crate::codegen::context::LocalKind::Ptr(ptr) => {
                            fn is_aggregate_type(ty: &Type) -> bool {
                                match ty {
                                    Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _) => true,
                                    Type::Owned(inner) => is_aggregate_type(inner),
                                    _ => false,
                                }
                            }
                            let is_aggregate = is_aggregate_type(ty);
                            if is_aggregate {
                                (Some(ptr.clone()), Type::Reference(Box::new(ty.clone()), false))
                            } else {
                                let val = format!("%local_load_{}", ctx.next_id());
                                // Default to i64 if mlir_storage_type fails, though it shouldn't
                                let mlir_ty = ty.to_mlir_storage_type(ctx).unwrap_or_else(|_| "i64".to_string());
                                ctx.emit_load(out, &val, ptr, &mlir_ty);
                                (Some(val), ty.clone())
                            }
                        },
                        crate::codegen::context::LocalKind::SSA(val) => {
                            (Some(val.clone()), ty.clone())
                        },
                    }
                } else {
                    match emit_expr(ctx, out, &m.receiver, local_vars, None) {
                        Ok((val, ty)) => (Some(val), ty),
                        Err(_) => {
                            let syn_ty = crate::grammar::SynType::from_std(
                                syn::Type::Path(syn::TypePath { qself: None, path: p.path.clone() })
                            ).map_err(|e| e.to_string())?;
                            let ty = crate::codegen::type_bridge::resolve_type(ctx, &syn_ty);
                            (None, ty)
                        }
                    }
                }
            } else {
                match emit_expr(ctx, out, &m.receiver, local_vars, None) {
                    Ok((val, ty)) => (Some(val), ty),
                    Err(_) => {
                        let syn_ty = crate::grammar::SynType::from_std(
                            syn::Type::Path(syn::TypePath { qself: None, path: p.path.clone() })
                        ).map_err(|e| e.to_string())?;
                        let ty = crate::codegen::type_bridge::resolve_type(ctx, &syn_ty);
                        (None, ty)
                    }
                }
            }
        } else {
            match emit_expr(ctx, out, &m.receiver, local_vars, None) {
                Ok((val, ty)) => (Some(val), ty),
                Err(_) => (None, Type::Unit),
            }
        };
        
    // Substitute generics in cached receiver type at the source
    let mut cached_receiver_ty = cached_receiver_ty.substitute(ctx.current_type_map());
    // Canonicalize receiver type to prevent raw Struct("Node")
    cached_receiver_ty = crate::codegen::type_bridge::resolve_codegen_type(ctx, &cached_receiver_ty);

    // Try special methods
    if let Ok(Some(res)) = crate::codegen::expr::special_methods::try_emit_special_method(
        ctx, out, m, local_vars, expected_ty, &cached_receiver_val, &cached_receiver_ty
    ) {
        return Ok(res);
    }

    // Try standard method resolution
    crate::codegen::expr::method_resolution::resolve_and_emit_method(
        ctx, out, m, local_vars, expected_ty, &cached_receiver_val, &cached_receiver_ty
    )
}
pub(crate) fn emit_tensor_constructor(
    ctx: &mut LoweringContext, 
    out: &mut String, 
    c: &syn::ExprCall,
    generics: &syn::PathArguments,
    local_vars: &mut HashMap<String, (Type, LocalKind)>
) -> Result<(String, Type), String> {
    
    // 1. Extract element type from generics: Tensor<f64>
    let elem_ty = if let syn::PathArguments::AngleBracketed(args) = generics {
        if let Some(syn::GenericArgument::Type(ty)) = args.args.first() {
            let syn_ty = crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?;
            resolve_type(ctx, &syn_ty)
        } else {
            return Err("Tensor requires type parameter: Tensor<f64>(...)".to_string());
        }
    } else {
        return Err("Tensor requires type parameter: Tensor<f64>(...)".to_string());
    };
    
    // 2. Parse arguments: (value, [d1, d2, ...])
    if c.args.len() != 2 {
        return Err("Tensor constructor requires 2 args: Tensor<T>(value, [dims])".to_string());
    }
    
    // Evaluate initial value
    let (init_val, init_ty) = emit_expr(ctx, out, &c.args[0], local_vars, Some(&elem_ty))?;
    let init_val = promote_numeric(ctx, out, &init_val, &init_ty, &elem_ty)?;
    
    // Parse shape array literal [d1, d2, ...]
    let shape: Vec<usize> = if let syn::Expr::Array(arr) = &c.args[1] {
        let mut dims = Vec::new();
        for elem in &arr.elems {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(lit), .. }) = elem {
                dims.push(lit.base10_parse::<usize>().map_err(|e| e.to_string())?);
            } else {
                return Err("Tensor shape must be integer literals: [512, 512]".to_string());
            }
        }
        dims
    } else {
        return Err("Tensor shape must be array literal: Tensor<f64>(0.0, [512, 512])".to_string());
    };
    
    // 3. Create Tensor type
    let tensor_ty = Type::Tensor(Box::new(elem_ty.clone()), shape.clone());
    let total_elements: usize = shape.iter().product();
    
    // 4. Emit MLIR: memref.alloc + linalg.fill
    // For now, use stack allocation for small tensors, heap for large
    let elem_mlir = elem_ty.to_mlir_storage_type(ctx)?;
    let shape_str: String = shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
    let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
    
    let tensor_ptr = format!("%tensor_{}", ctx.next_id());
    
    if total_elements * 8 > 1024 * 1024 {
        // Large tensor: heap allocation
        out.push_str(&format!("    {} = memref.alloc() : {}\n", tensor_ptr, memref_ty));
    } else {
        // Small tensor: stack allocation
        out.push_str(&format!("    {} = memref.alloca() : {}\n", tensor_ptr, memref_ty));
    }
    
    // Fill with initial value using linalg.fill
    let _filled = format!("%filled_{}", ctx.next_id());
    out.push_str(&format!("    linalg.fill ins({} : {}) outs({} : {})\n", 
        init_val, elem_mlir, tensor_ptr, memref_ty));
    
    // Return the memref pointer and tensor type
    Ok((tensor_ptr, tensor_ty))
}

fn try_emit_indirect_call(
    ctx: &mut LoweringContext,
    out: &mut String,
    c: &syn::ExprCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>
) -> Result<Option<(String, Type)>, String> {
    let is_indirect = match &*c.func {
        syn::Expr::Path(p) if p.path.segments.len() == 1 => {
            let name = p.path.segments[0].ident.to_string();
            local_vars.get(&name).map(|(ty, _)| matches!(ty, Type::Fn(_, _))).unwrap_or(false)
        },
        syn::Expr::Paren(_) => true,
        syn::Expr::Field(_) => true,
        _ => false,
    };

    if !is_indirect {
        return Ok(None);
    }

    let fn_result = super::emit_expr(ctx, out, &c.func, local_vars, None);
    if let Ok((fn_ptr_val, Type::Fn(param_tys, ret_ty))) = fn_result {
            let mut arg_vals = Vec::new();
            let mut arg_mlir_tys = Vec::new();
            for (i, arg_expr) in c.args.iter().enumerate() {
                let hint = param_tys.get(i);
                let (mut val, mut ty) = super::emit_expr(ctx, out, arg_expr, local_vars, hint)?;
                if let Some(target) = param_tys.get(i) {
                    if !ty.structural_eq(target) {
                        val = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &ty, target)?;
                        ty = target.clone();
                    }
                }
                arg_vals.push(val);
                arg_mlir_tys.push(ty.to_mlir_type(ctx)?);
            }

            let args_str = arg_vals.join(", ");
            let args_tys_str = arg_mlir_tys.join(", ");
            let ret_mlir_ty = ret_ty.to_mlir_type(ctx)?;

            let mut res_val = String::new();
            if *ret_ty == Type::Unit {
                out.push_str(&format!("    llvm.call {}({}) : !llvm.ptr, ({}) -> ()\n",
                    fn_ptr_val, args_str, args_tys_str));
            } else {
                res_val = format!("%indirect_call_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.call {}({}) : !llvm.ptr, ({}) -> {}\n",
                    res_val, fn_ptr_val, args_str, args_tys_str, ret_mlir_ty));
            }

            ctx.emission.global_lvn.clear();
            return Ok(Some((res_val, *ret_ty.clone())));
    }
    Ok(None)
}

fn emit_transparent_vec_access(
    ctx: &mut LoweringContext,
    out: &mut String,
    method: String,
    element_ty: Type,
    receiver: syn::Expr,
    args: Vec<syn::Expr>,
    local_vars: &mut HashMap<String, (Type, LocalKind)>
) -> Result<(String, Type), String> {
    let (vec_val, vec_ty) = super::emit_expr(ctx, out, &receiver, local_vars, None)?;
    
    let (base_ptr_val, _) = {
        let vec_mlir_ty = vec_ty.to_mlir_type(ctx)?;
        let data_ptr = format!("%vec_data_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", 
            data_ptr, vec_val, vec_mlir_ty));
        (data_ptr, Type::I64)
    };
    
    let index_expr = args.first().ok_or("get_unchecked/set_unchecked requires index argument")?;
    let (index_val, _) = super::emit_expr(ctx, out, index_expr, local_vars, Some(&Type::I64))?;
    
    let stride = ctx.size_of(&element_ty) as i64;
    let stride_val = format!("%stride_{}", ctx.next_id());
    ctx.emit_const_int(out, &stride_val, stride, "i64");
    
    let offset_val = format!("%offset_{}", ctx.next_id());
    ctx.emit_binop(out, &offset_val, "arith.muli", &index_val, &stride_val, "i64");
    
    let final_addr = format!("%elem_addr_{}", ctx.next_id());
    ctx.emit_binop(out, &final_addr, "arith.addi", &base_ptr_val, &offset_val, "i64");
    
    let elem_ptr = format!("%elem_ptr_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", 
        elem_ptr, final_addr));
    
    let elem_mlir_ty = element_ty.to_mlir_type(ctx)?;
    
    if method == "get_unchecked" {
        let result_val = format!("%vec_get_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", 
            result_val, elem_ptr, elem_mlir_ty));
        Ok((result_val, element_ty))
    } else {
        let value_expr = args.get(1).ok_or("set_unchecked requires value argument")?;
        let (value_val, _) = super::emit_expr(ctx, out, value_expr, local_vars, Some(&element_ty))?;
        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", 
            value_val, elem_ptr, elem_mlir_ty));
        Ok(("".to_string(), Type::Unit))
    }
}

fn emit_struct_literal_call(
    ctx: &mut LoweringContext,
    out: &mut String,
    c: &syn::ExprCall,
    struct_name: String,
    fields: Vec<(String, Type)>,
    local_vars: &mut HashMap<String, (Type, LocalKind)>
) -> Result<(String, Type), String> {
    let struct_ty = Type::Struct(struct_name.clone());
    let mlir_struct_ty = struct_ty.to_mlir_type(ctx)?;
    
    let alloca_var = format!("%struct_init_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.alloca %c1_i64 x {} : (i64) -> !llvm.ptr\n", 
        alloca_var, mlir_struct_ty));
    
    let args_vec: Vec<syn::Expr> = c.args.iter().cloned().collect();
    for (i, ((field_name, field_ty), arg_expr)) in fields.iter().zip(args_vec.iter()).enumerate() {
        let (arg_val, _arg_ty) = super::emit_expr(ctx, out, arg_expr, local_vars, Some(field_ty))?;
        
        let gep_var = format!("%field_ptr_{}", ctx.next_id());
        let field_mlir_ty = field_ty.to_mlir_type(ctx)?;
        out.push_str(&format!("    {} = llvm.getelementptr {} [0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n",
            gep_var, alloca_var, i, mlir_struct_ty));
        
        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", 
            arg_val, gep_var, field_mlir_ty));
        let _ = field_name; 
    }
    
    let load_var = format!("%struct_val_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", 
        load_var, alloca_var, mlir_struct_ty));
    
    Ok((load_var, struct_ty))
}

fn emit_intrinsic_call(
    ctx: &mut LoweringContext,
    out: &mut String,
    c: &syn::ExprCall,
    name: String,
    explicit_generics: Vec<Type>,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    _expected: Option<&Type>
) -> Result<(String, Type), String> {
    let args_vec: Vec<syn::Expr> = c.args.iter().cloned().collect();
    
    let lookup_ret_ty = if explicit_generics.is_empty() && _expected.is_none() {
         ctx.resolve_global_func(&name).map(|(ty, _)| {
             if let Type::Fn(_, ret) = ty { *ret } else { Type::Unit }
         })
    } else { None };

    let expected_for_intrinsic = if !explicit_generics.is_empty() {
        Some(&explicit_generics[0])
    } else if let Some(ty) = &lookup_ret_ty {
        Some(ty)
    } else {
        _expected
    };
    match ctx.emit_intrinsic(out, &name, &args_vec, local_vars, expected_for_intrinsic) {
        Ok(Some((val, ty))) => Ok((val, ty)),
        Ok(None) => Err(format!("Intrinsic '{}' not found", name)),
        Err(e) => Err(format!("Intrinsic '{}' emission failed: {}", name, e)),
    }
}

fn hydrate_function_if_needed(
    ctx: &mut LoweringContext,
    mangled_name: &str,
    arg_tys: &[Type],
    ret_ty: &Type,
    lazy_task: &Option<Box<crate::codegen::collector::MonomorphizationTask>>,
) -> Result<(), String> {
    if !ctx.is_function_defined(mangled_name) {
        if let Some(task) = lazy_task {
            let is_cross_module = ctx.config.lib_mode && {
                let current_pkg = &ctx.current_package;
                if let Some(pkg) = current_pkg.as_ref() {
                    let pkg_prefix = pkg.name.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("__");
                    let name_mismatches_prefix = !mangled_name.starts_with(&format!("{}__", pkg_prefix));
                    if name_mismatches_prefix {
                        let is_local_no_mangle = ctx.config.file.items.iter().any(|item| {
                            if let crate::grammar::Item::Fn(f) = item {
                                let is_nm = f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" );
                                is_nm && f.name == mangled_name
                            } else {
                                false
                            }
                        });
                        !is_local_no_mangle
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            if is_cross_module {
                ctx.ensure_external_declaration(mangled_name, arg_tys, ret_ty)?;
            } else {
                ctx.hydrate_specialization(*task.clone())?;
            }
        } else {
            ctx.ensure_external_declaration(mangled_name, arg_tys, ret_ty)?;
        }
    }
    Ok(())
}

fn extract_verification_data(
    ctx: &LoweringContext,
    mangled_name: &str,
    lazy_task: &Option<Box<crate::codegen::collector::MonomorphizationTask>>,
) -> (Vec<syn::Expr>, Vec<syn::Expr>, Vec<String>) {
    if let Some(t) = lazy_task.as_ref() {
        (
            t.func.requires.clone(),
            t.func.ensures.clone(),
            t.func.args.iter().map(|a| a.name.to_string()).collect::<Vec<_>>(),
        )
    } else if let Some((wrapper, _)) = ctx.generic_impls().get(mangled_name) {
        (
            wrapper.requires.clone(),
            wrapper.ensures.clone(),
            wrapper.args.iter().map(|a| a.name.to_string()).collect::<Vec<_>>(),
        )
    } else {
        (vec![], vec![], vec![])
    }
}

fn emit_function_args(
    ctx: &mut LoweringContext,
    out: &mut String,
    args_vec: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    arg_tys: &[Type],
    requires: &[syn::Expr],
    param_names: &[String],
) -> Result<(Vec<String>, Vec<Type>), String> {
    if !requires.is_empty() {
        crate::codegen::verification::VerificationEngine::verify(ctx, out, requires, param_names, args_vec, local_vars, arg_tys)?;
    }

    let mut args_vals = Vec::new();
    let mut inferred_tys = Vec::new();

    for (i, arg_expr) in args_vec.iter().enumerate() {
        let (mut val, mut ty) = super::emit_expr(ctx, out, arg_expr, local_vars, None)?;
        
        if let Some(target) = arg_tys.get(i) {
            if matches!(target, Type::Owned(..)) && !matches!(ty, Type::Owned(..)) {
                let mlir_ty = ty.to_mlir_type(ctx)?;
                if mlir_ty != "!llvm.ptr" {
                      let temp = format!("%owned_spill_{}", ctx.next_id());
                      ctx.emit_alloca(out, &temp, &mlir_ty);
                      ctx.emit_store(out, &val, &temp, &mlir_ty);
                      val = temp;
                      ty = target.clone();
                }
            }
            if !ty.structural_eq(target) {
                val = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &ty, target)?;
            }
            ty = target.clone();
        }

        if ty.is_affine() {
            if let Some(var_name) = super::extract_ident_name(arg_expr) {
                ctx.consumed_vars_mut().insert(var_name);
            }
        }
        
        args_vals.push(val);
        inferred_tys.push(ty);
    }
    
    Ok((args_vals, inferred_tys))
}
