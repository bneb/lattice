use std::collections::{HashMap, BTreeMap};
use syn;

use crate::codegen::context::{LoweringContext, LocalKind};
use crate::types::Type;
use crate::codegen::expr::{emit_expr, promote_numeric, get_path_from_expr, infer_generics, emit_lvalue};
use crate::codegen::type_bridge::{resolve_type, resolve_codegen_type};
use crate::codegen::expr::utils::resolve_package_prefix_ctx;


fn get_type_based_pkg(ctx: &LoweringContext, receiver_ty: &Type, cached_receiver_val: &Option<String>) -> Option<String> {
    let type_based_pkg = match receiver_ty {
        Type::Struct(name) | Type::Concrete(name, _) => Some(name.clone()),
        Type::Pointer { .. } => Some("std__core__ptr__Ptr".to_string()),
        _ => None,
    };
    if let Some(name) = type_based_pkg.as_ref() {
        if cached_receiver_val.is_none() {
            let is_known = ctx.struct_registry().values().any(|i| &i.name == name) || ctx.enum_registry().values().any(|i| &i.name == name);
            if is_known { type_based_pkg } else { None }
        } else { type_based_pkg }
    } else { None }
}

fn get_receiver_generic_suffix(receiver_ty: &Type) -> Option<String> {
    match receiver_ty {
        Type::Concrete(_, args) if !args.is_empty() => Some(args.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_")),
        Type::Pointer { element, .. } => Some(element.mangle_suffix()),
        _ => None,
    }
}

fn get_receiver_concrete_args(ctx: &mut LoweringContext, receiver_ty: &Type) -> Vec<Type> {
    match receiver_ty {
        Type::Concrete(_, args) => args.clone(),
        Type::Reference(inner, _) => match inner.as_ref() {
            Type::Concrete(_, args) => args.clone(),
            Type::Pointer { element, .. } => vec![crate::codegen::type_bridge::resolve_codegen_type(ctx, &(**element))],
            _ => vec![],
        },
        Type::Pointer { element, .. } => vec![crate::codegen::type_bridge::resolve_codegen_type(ctx, &(**element))],
        _ => vec![],
    }
}

fn try_resolve_static_method(
    ctx: &mut LoweringContext,
    out: &mut String,
    m: &syn::ExprMethodCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    expected_ty: Option<&Type>,
    receiver_ty: &Type,
    cached_receiver_val: &Option<String>,
    cached_receiver_ty: &Type,
    type_based_pkg: &Option<String>,
    receiver_generic_suffix: &Option<String>,
) -> Result<Option<(String, Type)>, String> {
    let method = m.method.to_string();
    if let Some(segments) = get_path_from_expr(&m.receiver) {
        let is_local_var = segments.len() == 1 && local_vars.contains_key(&segments[0]);
        if let Some((pkg, item)) = if is_local_var { None } else { resolve_package_prefix_ctx(ctx, &segments) } {
             // Use type-based pkg_name if receiver has a concrete type
             // This fixes GLOBAL_ALLOC.alloc() → GlobalSlabAlloc__alloc (not GLOBAL_ALLOC__alloc)
             let pkg_name = if let Some(type_name) = &type_based_pkg {

                 type_name.clone()
             } else if item.is_empty() { 
                 pkg.clone() 
             } else if pkg.is_empty() { 
                 item.clone() 
             } else { 
                 format!("{}__{}", pkg, item) 
             };
             
             // Build mangled name: pkg__method (then add suffix for specialized version)
             // Naming convention for specializations is: pkg__method_suffix (e.g., Ptr__offset_u8)
             let base_mangled = format!("{}__{}", pkg_name, method);
             let original_mangled = if let Some(ref suffix) = receiver_generic_suffix {
                 format!("{}_{}", base_mangled, suffix)
             } else {
                 base_mangled.clone()
             };
             let mut mangled = original_mangled.clone();
             
             // [CROSS-MODULE @no_mangle FIX]
             // Check if `mangled` refers to a function in the registry that is `@no_mangle`.
             // If it is, `mangled` MUST be truncated to just `method`.
             if let Some(registry) = ctx.config.registry {
                 let pkg_path = pkg_name.replace("__", ".");
                 if let Some(mod_info) = registry.modules.get(&pkg_path) {
                     if let Some(func) = mod_info.function_templates.get(&method) {
                         if func.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" ) {
                             mangled = method.clone();
                         }
                     }
                 }
             }
             
             // Check both specialized and non-specialized names for generic impls
             let is_generic = ctx.generic_impls().contains_key(&base_mangled) 
                           || ctx.generic_impls().contains_key(&original_mangled);
             
             // Trigger method body hydration for type-based methods (e.g., GlobalSlabAlloc::alloc)
             // This is critical: without this, methods are declared but not defined, causing linker errors
             if type_based_pkg.is_some() {
                 // [GRAYDON FIX - IDENTITY CRISIS] Extract concrete type args from receiver
                 // This is CRITICAL: we must pass the receiver's specialization args (e.g., [i64, i64] from HashMap<i64, i64>)
                 // so that request_specialization can generate the correct mangled name (HashMap__get_i64_i64)
                 // instead of the template name (HashMap__get_K_V)
                 let receiver_concrete_args: Vec<Type> = get_receiver_concrete_args(ctx, &receiver_ty);

                 
                 let _ = ctx.request_specialization(&base_mangled, receiver_concrete_args, Some(receiver_ty.clone()));

             }
             
             let mut emitted_vals = Vec::new();
             let mut emitted_tys = Vec::new();
             let mut specialized_sig = None;

             if is_generic {
                  for arg_expr in &m.args {
                       let (val, ty) = emit_expr(ctx, out, arg_expr, local_vars, None)?;
                       emitted_vals.push(val);
                       emitted_tys.push(ty);
                  }
                  
                  // Clone func_def data to release generic_impls borrow before request_specialization
                  let func_data = ctx.generic_impls().get(&original_mangled).map(|(func_def, _)| {
                      (func_def.generics.clone(), func_def.args.clone(), func_def.ret_type.clone())
                  });
                  if let Some((Some(generics), func_args, func_ret_type)) = func_data {
                            if !generics.params.is_empty() {
                                let generic_names: std::collections::HashSet<String> = generics.params.iter().map(|p| match p {
                                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                }).collect();
                                let mut params: Vec<Type> = func_args.iter()
                                     .filter_map(|arg| arg.ty.as_ref().and_then(|t| Type::from_syn_with_generics(t, &generic_names)))
                                     .collect();
                                let mut args_for_infer = emitted_tys.clone();
                                
                                // Infer from Return Type expectation
                                if let Some(ret_def) = &func_ret_type {
                                     if let Some(exp) = expected_ty {
                                          if let Some(ret_ty_gen) = Type::from_syn_with_generics(ret_def, &generic_names) {
                                               params.push(ret_ty_gen);
                                               args_for_infer.push(exp.clone());
                                          }
                                     }
                                }

                                let concrete = infer_generics(&params, &args_for_infer, &generics);
                                mangled = ctx.request_specialization(&original_mangled, concrete.clone(), Some(receiver_ty.clone()));

                                // [Fix] Substitute generics in return type and args locally
                                let mut subst_map = BTreeMap::new();
                                for (i, param) in generics.params.iter().enumerate() {
                                    if let crate::grammar::GenericParam::Type { name, .. } = param {
                                         if let Some(c) = concrete.get(i) {
                                              subst_map.insert(name.to_string(), c.clone());
                                         }
                                    }
                                }

                                let ret_ty_base = if let Some(rt) = &func_ret_type {
                                    Type::from_syn_with_generics(rt, &generic_names).unwrap_or(Type::Unit)
                                } else { Type::Unit };
                                let ret_ty_subst = ret_ty_base.substitute(&subst_map);

                                let args_subst = func_args.iter().filter_map(|arg| {
                                     arg.ty.as_ref().and_then(|t| Type::from_syn_with_generics(t, &generic_names)).map(|t| t.substitute(&subst_map))
                                }).collect::<Vec<_>>();
                                
                                specialized_sig = Some((ret_ty_subst, args_subst));
                           }
                      }
             }
             

             // NOTE: The previous HACK that redirected GlobalSlabAlloc/GLOBAL_ALLOC methods 
             // to free functions was REMOVED because it caused infinite recursion in the 
             // allocator (alloc() calling itself instead of the actual method).
             
             let (ret_ty, expected_arg_tys) = if let Some((r, a)) = specialized_sig {
                 (r, a)
             } else if let Some(sig) = ctx.resolve_global(&mangled) {
                  if let Type::Fn(p, r) = sig { (*r, p) } else { 
                      return Err(format!("Symbol '{}' is not a function", mangled));
                  }
             } else if type_based_pkg.is_some() {
                  // For typed receiver methods (e.g., Ptr<u8>::offset), the method may not be in
                  // globals yet. Lookup the template from method_registry and substitute generics.
                  // Note: method_registry stores templates with NON-specialized TypeKey, so we need
                  // to create a base key without the specialization args for lookup.
                  let type_key = crate::codegen::type_bridge::type_to_type_key(&receiver_ty);
                  let base_type_key = crate::types::TypeKey {
                      path: type_key.path.clone(),
                      name: type_key.name.clone(),
                      specialization: None,  // Templates are stored without specialization
                  };
                  

                  
                  // [V4.0 KEUOS] Use TraitRegistry for method lookup  
                  let mut registry_result = ctx.trait_registry().get_legacy(&base_type_key, &method);
                  
                  // If exact match fails, try matching via find_method_by_name which matches on path and name
                  if registry_result.is_none() {
                      for key in ctx.trait_registry().iter_type_keys() {
                          if key.path == base_type_key.path && key.name == base_type_key.name {
                              if let Some(result) = ctx.trait_registry().get_legacy(&key, &method) {
                                  registry_result = Some(result);

                                  break;
                              }
                          }
                      }
                  }
                  
                  if let Some((func_def, _, _)) = registry_result {
                      // Build substitution map from receiver type args
                      let mut subst_map = BTreeMap::new();
                      
                      // CRITICAL: Map Self to the EFFECTIVE receiver type after TYPE-OVERRIDE
                      // If type_based_pkg was used (meaning there's a TYPE-OVERRIDE), construct
                      // the effective type from type_based_pkg + receiver's generic args.
                      // Example: If receiver_ty is Vec<u8> but type_based_pkg is "std__core__ptr__Ptr",
                      // the effective type for Self should be Ptr<u8>, not Vec<u8>.
                      let effective_receiver_ty = if let Some(ref override_pkg) = type_based_pkg {
                          // Extract generic args from the receiver for the overridden type
                          if let Type::Concrete(_, args) = &receiver_ty {
                              Type::Concrete(override_pkg.clone(), args.clone())
                          } else {
                              Type::Struct(override_pkg.clone())
                          }
                      } else {
                          receiver_ty.clone()
                      };
                      subst_map.insert("Self".to_string(), effective_receiver_ty.clone());
                      
                      // KEUOS FIX: Extract receiver_concrete_args again for mapping generics!
                      let receiver_concrete_args: Vec<Type> = match &receiver_ty {
                          Type::Concrete(_, args) => args.clone(),
                          Type::Reference(inner, _) => match inner.as_ref() {
                              Type::Concrete(_, args) => args.clone(),
                              Type::Pointer { element, .. } => {
                                  let canonical_element = crate::codegen::type_bridge::resolve_codegen_type(ctx, &(**element));
                                  vec![canonical_element]
                              },
                              _ => vec![],
                          },
                          Type::Pointer { element, .. } => {
                              let canonical_element = crate::codegen::type_bridge::resolve_codegen_type(ctx, &(**element));
                              vec![canonical_element]
                          },
                          _ => vec![],
                      };
                                       if let Some(generics) = &func_def.generics {
                          for (i, param) in generics.params.iter().enumerate() {
                              if let crate::grammar::GenericParam::Type { name, .. } = param {
                                  if let Some(arg) = receiver_concrete_args.get(i) {
                                      subst_map.insert(name.to_string(), arg.clone());
                                  }
                              }
                          }
                      }
                      // Also add T -> arg[0] mapping for simple cases as fallback
                      if receiver_concrete_args.len() == 1 {
                          subst_map.insert("T".to_string(), receiver_concrete_args[0].clone());
                      }
                      
                      
                      // Temporarily override current_self_ty to the effective receiver type
                      // This ensures resolve_type uses correct Self (e.g., Ptr<u8>) not caller's Self (e.g., Vec<u8>)
                      let old_self_ty = ctx.current_self_ty().clone();
                      *ctx.current_self_ty_mut() = Some(effective_receiver_ty.clone());
                      
                      let ret_ty_base = if let Some(rt) = &func_def.ret_type {
                          crate::codegen::type_bridge::resolve_type(ctx, rt)
                      } else { Type::Unit };
                      let ret_ty_subst = ret_ty_base.substitute(&subst_map);
                      
                      let args_subst: Vec<Type> = func_def.args.iter().filter_map(|arg| {
                          arg.ty.as_ref().map(|t| {
                              let resolved = crate::codegen::type_bridge::resolve_type(ctx, t);
                              let substed = resolved.substitute(&subst_map);
                              substed
                          })
                      }).collect();
                      
                      // Restore original self_ty
                      *ctx.current_self_ty_mut() = old_self_ty;
                      

                      (ret_ty_subst, args_subst)
                  } else {
                      // Clone pending task data to release pending_generations borrow before resolve_type
                      let pending_task_data = ctx.pending_generations().iter().find_map(|task| {
                          if task.mangled_name == mangled {
                              Some((task.func.ret_type.clone(), task.func.args.clone(), task.type_map.clone(), task.self_ty.clone()))
                          } else { None }
                      });
                      if pending_task_data.is_none() {
                      }
                      let pending_sig = if let Some((ret_type, func_args, type_map, self_ty)) = pending_task_data {
                          let old_type_map = ctx.current_type_map().clone();
                          let old_self_ty = ctx.current_self_ty().clone();
                          *ctx.current_type_map_mut() = type_map.clone();
                          *ctx.current_self_ty_mut() = self_ty;
                          let ret_ty = if let Some(rt) = &ret_type {
                              crate::codegen::type_bridge::resolve_type(ctx, rt).substitute(&type_map)
                          } else { Type::Unit };
                          let args: Vec<Type> = func_args.iter().filter_map(|arg| {
                              arg.ty.as_ref().map(|t| {
                                  crate::codegen::type_bridge::resolve_type(ctx, t).substitute(&type_map)
                              })
                          }).collect();
                          *ctx.current_type_map_mut() = old_type_map;
                          *ctx.current_self_ty_mut() = old_self_ty;
                          Some((ret_ty, args))
                      } else { None };
                      
                      if let Some((ret_ty, args)) = pending_sig {

                          (ret_ty, args)
                      } else {
                          // Fallback: Check for free function in package
                          let mut short_mangled = format!("{}__{}", pkg, method);
                          // Check registry for @no_mangle
                          if let Some(registry) = ctx.config.registry {
                              let pkg_path = pkg.replace("__", ".");
                              if let Some(mod_info) = registry.modules.get(&pkg_path) {
                                  if let Some(func) = mod_info.function_templates.get(&method) {
                                      if func.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" ) {
                                          short_mangled = method.clone();
                                      }
                                  }
                              }
                          }


                          if let Some(sig) = ctx.resolve_global(&short_mangled) {
                              mangled = short_mangled;
                              if let Type::Fn(p, r) = sig { (*r, p) } else {
                                   return Err(format!("Symbol '{}' is not a function", mangled));
                              }
                          } else {
                              return Err(format!("Linker Error: Function '{}' not found in symbol table.", mangled));
                          }
                      }
                  }
             } else {
                  // Fallback: Check for free function in package (e.g. std__core__slab_alloc__dealloc)
                  // If pkg_name is std__core__slab_alloc__GlobalSlabAlloc, we want std__core__slab_alloc
                  // This assumes the struct name is the last component.
                  let mut short_mangled = format!("{}__{}", pkg, method); // pkg is from resolve_package_prefix, likely the module path?
                  if let Some(registry) = ctx.config.registry {
                      let pkg_path = pkg.replace("__", ".");
                      if let Some(mod_info) = registry.modules.get(&pkg_path) {
                          if let Some(func) = mod_info.function_templates.get(&method) {
                              if func.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" ) {
                                  short_mangled = method.clone();
                              }
                          }
                      }
                  }


                  if let Some(sig) = ctx.resolve_global(&short_mangled) {
                      mangled = short_mangled;
                      if let Type::Fn(p, r) = sig { (*r, p) } else {
                           return Err(format!("Symbol '{}' is not a function", mangled));
                      }
                  } else {
                      // [ATOMIC INTERCEPTION - LIB MODE] Before erroring, check if the
                      // receiver is a global of type Atomic<T>. In --lib mode, globals get
                      // package-prefixed names (e.g., kernel__mem__free_list_head) which
                      // causes method resolution to construct wrong function names.
                      let method = m.method.to_string();
                      if matches!(method.as_str(), "fetch_add" | "fetch_sub" | "load" | "store") {
                          if let Ok((receiver_addr, receiver_ty, _kind)) = emit_lvalue(ctx, out, &m.receiver, local_vars) {
                              if let Type::Atomic(inner) = receiver_ty {
                                  let mlir_ty = inner.to_mlir_type(ctx)?;
                                  if method == "fetch_add" || method == "fetch_sub" {
                                      let op = if method == "fetch_add" { "add" } else { "sub" };
                                      let (val, ty) = emit_expr(ctx, out, &m.args[0], local_vars, Some(&inner))?;
                                      let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                                      let res = format!("%atomic_res_{}", ctx.next_id());
                                      ctx.emit_atomicrmw(out, &res, op, &receiver_addr, &val_prom, &mlir_ty);
                                      return Ok(Some((res, *inner)));
                                  } else if method == "load" {
                                      let res = format!("%atomic_load_{}", ctx.next_id());
                                      ctx.emit_load_atomic(out, &res, &receiver_addr, &mlir_ty);
                                      return Ok(Some((res, *inner)));
                                  } else if method == "store" {
                                      let (val, ty) = emit_expr(ctx, out, &m.args[0], local_vars, Some(&inner))?;
                                      let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                                      ctx.emit_store_atomic(out, &val_prom, &receiver_addr, &mlir_ty);
                                      return Ok(Some(("%unit".to_string(), Type::Unit)));
                                  }
                              }
                          }
                      }
                      return Err(format!("Linker Error: Function '{}' not found in symbol table.", mangled));
                  }
             };

             let mut final_args = Vec::new();
             let mut final_arg_tys = Vec::new();
             
             // If we used type-based pkg (method on a typed receiver), prepend receiver as &self
             if type_based_pkg.is_some() {
                 // Use cached receiver (MEMOIZATION FIX - no duplicate emission)
                 let (recv_val, recv_ty) = if let Some(ref val) = cached_receiver_val {
                     (val.clone(), cached_receiver_ty.clone())
                 } else {
                     // Fallback for static methods - emit once
                     emit_expr(ctx, out, &m.receiver, local_vars, None)?
                 };
                 // Use the resolved signature type for self if available, otherwise fallback to Reference
                 let self_arg_ty = if !expected_arg_tys.is_empty() {
                     expected_arg_tys[0].clone()
                 } else {
                     Type::Reference(Box::new(receiver_ty.clone()), true)
                 };

                 // Methods expect a reference to self - take address if needed
                 let recv_ref = if matches!(recv_ty, Type::Reference(_, _)) {
                     recv_val
                 } else {
                     // If the expected self argument is a reference/pointer, we must spill the value
                     if matches!(self_arg_ty, Type::Reference(_, _)) {
                         // Check if it's a global first
                         let mut is_global = false;
                         let mut global_ptr = None;
                         if let syn::Expr::Path(p) = &*m.receiver {
                             let name = p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("__");
                             if let Some((canonical, _)) = resolve_package_prefix_ctx(ctx, &[name.clone()]) {
                                 let full_name = if canonical.is_empty() { name } else { canonical };
                                 let ptr_var = format!("%recv_ptr_{}", ctx.next_id());
                                 out.push_str(&format!("    {} = llvm.mlir.addressof @{} : !llvm.ptr\n", ptr_var, full_name));
                                 global_ptr = Some(ptr_var);
                                 is_global = true;
                             }
                         }
                         
                         if is_global {
                             global_ptr.ok_or_else(|| "Compiler bug: global_ptr missing".to_string())?
                         } else {
                             // Spill to alloca!
                             let ptr_var = format!("%spill_recv_{}", ctx.next_id());
                             let mlir_ty = recv_ty.to_mlir_storage_type(ctx)?;
                             out.push_str(&format!("    {} = llvm.alloca %c1_i64 x {} : (i64) -> !llvm.ptr\n", ptr_var, mlir_ty));
                             ctx.emit_store(out, &recv_val, &ptr_var, &mlir_ty);
                             ptr_var
                         }
                     } else {
                         recv_val
                     }
                 };
                 final_args.push(recv_ref);
                 final_arg_tys.push(self_arg_ty);
             }
             
             if is_generic {
                   // When type_based_pkg is set, the method has &self as first arg but user only provides remaining args
                   let self_offset = if type_based_pkg.is_some() { 1 } else { 0 };
                   let user_expected_len = expected_arg_tys.len().saturating_sub(self_offset);
                   if emitted_vals.len() != user_expected_len {
                        return Err(format!("Arity Mismatch: {} expects {} args, got {}", mangled, user_expected_len, emitted_vals.len()));
                   }
                    for (i, val) in emitted_vals.iter().enumerate() {
                         let src_ty = &emitted_tys[i];
                         let dst_ty = &expected_arg_tys[i + self_offset]; // Skip &self
                        let val_coerced = crate::codegen::type_bridge::cast_numeric(ctx, out, val, src_ty, dst_ty)?;
                        final_args.push(val_coerced);
                        final_arg_tys.push(dst_ty.clone());
                  }
             } else {
                   // Apply self_offset for non-generic path too
                   let self_offset = if type_based_pkg.is_some() { 1 } else { 0 };
                   for (i, arg_expr) in m.args.iter().enumerate() {
                        let expected = expected_arg_tys.get(i + self_offset);
                       let (val, ty) = emit_expr(ctx, out, arg_expr, local_vars, expected)?;
                       let val_prom = if let Some(target) = expected {
                            crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &ty, target)?
                       } else { val };
                       final_args.push(val_prom);
                       final_arg_tys.push(if let Some(t) = expected { t.clone() } else { ty });
                  }
             }
             
             let args_str = final_args.join(", ");
             let arg_tys_str = final_arg_tys.iter().map(|t| t.to_mlir_type(ctx)).collect::<Result<Vec<_>, String>>()?.join(", ");
             
             let res = if ret_ty != Type::Unit { format!("%mcall_res_{}", ctx.next_id()) } else { "".to_string() };

             ctx.ensure_func_declared(&mangled, &final_arg_tys, &ret_ty)?;

             if res.is_empty() {
                 out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled, args_str, arg_tys_str));
             } else {
                 out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", res, mangled, args_str, arg_tys_str, ret_ty.to_mlir_type(ctx)?));
             }

             // [SALT MEMORY MODEL] Conservative Aliasing for methods
             let method_name = m.method.to_string();
             if method_name != "free" && method_name != "drop" {
                 let is_extern = ctx.external_decls().contains(&method_name);
                 if is_extern || ctx.config.freeing_functions.contains(&method_name) {
                     if let Some(var_name) = super::extract_ident_name(&m.receiver) {
                         ctx.pointer_tracker.mark_optional(&var_name);
                     }
                     for arg in &m.args {
                         if let Some(var_name) = super::extract_ident_name(arg) {
                             ctx.pointer_tracker.mark_optional(&var_name);
                         }
                     }
                 }
             }

             ctx.emission.global_lvn.clear();
             return Ok(Some((res, ret_ty)));
        }
    }
    Ok(None)
}

fn try_resolve_atomic_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    m: &syn::ExprMethodCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    expected_ty: Option<&Type>,
    method_name: &str,
) -> Result<Option<(String, Type)>, String> {
    // 1. Try Intrinsic (e.g. popcount)
    let mut intrinsic_args = vec![*m.receiver.clone()];
    intrinsic_args.extend(m.args.iter().cloned());
    if let Ok(Some(res)) = ctx.emit_intrinsic(out, method_name, &intrinsic_args, local_vars, expected_ty) {
         return Ok(Some(res));
    }

    // Special handling for Atomic intrinsics (fetch_add, fetch_sub, load, store)
    if let Ok((receiver_addr, receiver_ty, _kind)) = emit_lvalue(ctx, out, &m.receiver, local_vars) {
        if let Type::Atomic(inner) = receiver_ty {
            let mlir_ty = inner.to_mlir_type(ctx)?;
            if method_name == "fetch_add" {
                 let (val, ty) = emit_expr(ctx, out, &m.args[0], local_vars, Some(&inner))?;
                 let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                 let res = format!("%atomic_res_{}", ctx.next_id());
                 ctx.emit_atomicrmw(out, &res, "add", &receiver_addr, &val_prom, &mlir_ty);
                 return Ok(Some((res, *inner)));
            } else if method_name == "fetch_sub" {
                 let (val, ty) = emit_expr(ctx, out, &m.args[0], local_vars, Some(&inner))?;
                 let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                 let res = format!("%atomic_res_{}", ctx.next_id());
                 ctx.emit_atomicrmw(out, &res, "sub", &receiver_addr, &val_prom, &mlir_ty);
                 return Ok(Some((res, *inner)));
            } else if method_name == "load" {
                 let res = format!("%atomic_load_{}", ctx.next_id());
                 ctx.emit_load_atomic(out, &res, &receiver_addr, &mlir_ty);
                 return Ok(Some((res, *inner)));
            } else if method_name == "store" {
                 let (val, ty) = emit_expr(ctx, out, &m.args[0], local_vars, Some(&inner))?;
                 let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                 ctx.emit_store_atomic(out, &val_prom, &receiver_addr, &mlir_ty);
                 return Ok(Some(("%unit".to_string(), Type::Unit)));
            }
        }
    }
    Ok(None)
}

fn get_receiver_lvalue(
    ctx: &mut LoweringContext,
    out: &mut String,
    receiver_expr: &syn::Expr,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    cached_receiver_val: &Option<String>,
    cached_receiver_ty: &Type,
    method_name: &str,
) -> Result<(String, Type), String> {
    if let Ok((addr, raw_ty, _kind)) = emit_lvalue(ctx, out, receiver_expr, local_vars) {
        let ty = raw_ty.substitute(&ctx.current_type_map());

        fn is_aggregate_type(ty: &Type) -> bool {
            match ty {
                Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _) => true,
                Type::Owned(inner) => is_aggregate_type(inner),
                _ => false,
            }
        }
        let is_aggregate = is_aggregate_type(&ty);
        let is_ref_ssa = matches!(ty, Type::Reference(_, _)) && matches!(_kind, crate::codegen::expr::LValueKind::SSA);
        if is_aggregate {
            Ok((addr, Type::Reference(Box::new(ty), false)))
        } else if is_ref_ssa {
            Ok((addr, ty))
        } else {
            let val = format!("%recv_load_{}", ctx.next_id());
            let mlir_ty = ty.to_mlir_storage_type(ctx)?;
            ctx.emit_load(out, &val, &addr, &mlir_ty);
            Ok((val, ty))
        }
    } else {
        if let Some(ref val) = cached_receiver_val {
            Ok((val.clone(), cached_receiver_ty.substitute(&ctx.current_type_map())))
        } else {
            Err(format!("Method call '{}' requires a receiver value", method_name))
        }
    }
}

fn populate_type_map_from_receiver(
    ctx: &mut LoweringContext,
    receiver_ty: &Type
) -> (Vec<Type>, Option<String>, Type) {
    let mut concrete_tys = Vec::new();
    let mut template_name_opt = None;

    let mut peeled_ty = receiver_ty.clone();
    while let Type::Reference(inner, _) = peeled_ty {
        peeled_ty = *inner;
    }

    if let Type::Struct(name) = &peeled_ty {
        if let Some(info) = ctx.struct_registry().values().find(|i| i.name == *name).cloned() {
             concrete_tys.extend(info.specialization_args);
             template_name_opt = info.template_name;
        } else if ctx.struct_templates().contains_key(name) {
             template_name_opt = Some(name.clone());
        }
    } else if let Type::Enum(name) = &peeled_ty {
        if let Some(info) = ctx.enum_registry().values().find(|i| i.name == *name).cloned() {
             concrete_tys.extend(info.specialization_args);
             template_name_opt = info.template_name;
        } else if ctx.enum_templates().contains_key(name) {
             template_name_opt = Some(name.clone());
        }
    } else if let Type::Concrete(name, args) = &peeled_ty {
         concrete_tys.extend(args.iter().cloned());
         template_name_opt = Some(name.clone());
    } else if let Type::Pointer { element, .. } = &peeled_ty {
         let canonical_element = crate::codegen::type_bridge::resolve_codegen_type(ctx, &(**element));
         concrete_tys.push(canonical_element);
         template_name_opt = Some("std__core__ptr__Ptr".to_string());
    }

    if let Some(t_name) = &template_name_opt {
         let mut gen_params = if let Some(s) = ctx.struct_templates().get(t_name) {
              s.generics.as_ref().map(|g| g.params.clone())
         } else if let Some(e) = ctx.enum_templates().get(t_name) {
              e.generics.as_ref().map(|g| g.params.clone())
         } else {
              None
         };

         if gen_params.is_none() {
              if let Some(template_name) = ctx.find_struct_template_by_name(&t_name) {
                  if let Some(template) = ctx.struct_templates().get(&template_name) {
                      gen_params = template.generics.as_ref().map(|g| g.params.clone());
                  }
              }
              if gen_params.is_none() {
                  if let Some(template_name) = ctx.find_enum_template_by_name(&t_name) {
                      if let Some(template) = ctx.enum_templates().get(&template_name) {
                          gen_params = template.generics.as_ref().map(|g| g.params.clone());
                      }
                  }
              }
         }

         if let Some(params) = gen_params {
             for (i, param) in params.iter().enumerate() {
                  if let Some(arg) = concrete_tys.get(i) {
                       let name = match param {
                           crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                           crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                       };
                       ctx.current_type_map_mut().insert(name, arg.clone());
                  }
             }
         }
    }
    
    (concrete_tys, template_name_opt, peeled_ty)
}

fn append_method_generics(
    ctx: &mut LoweringContext,
    m: &syn::ExprMethodCall,
    concrete_tys: &mut Vec<Type>,
    template_name_opt: &Option<String>,
    func: &crate::grammar::SaltFn,
    method_generic_map: &std::collections::BTreeMap<String, Type>,
) -> Result<(), String> {
    // Add method-level generic arguments if present (from sync ExprMethodCall.turbofish?)
    if let Some(tf) = &m.turbofish {
        for arg in &tf.args {
            if let syn::GenericArgument::Type(ty_arg) = arg {
                 concrete_tys.push(resolve_codegen_type(ctx, &crate::types::Type::from_syn(&crate::grammar::SynType::from_std(ty_arg.clone()).map_err(|e| e.to_string())?).ok_or_else(|| "Failed to parse type".to_string())?));
            }
        }
    }
    
    // If concrete_tys is empty (no explicit args), try to infer from context map if template matches
    if concrete_tys.is_empty() {
         if let Some(t_name) = template_name_opt {
              let gen_params = if let Some(s) = ctx.struct_templates().get(t_name) {
                   s.generics.as_ref().map(|g| g.params.clone())
              } else if let Some(e) = ctx.enum_templates().get(t_name) {
                   e.generics.as_ref().map(|g| g.params.clone())
              } else { None };

              if let Some(params) = gen_params {
                   let current_map = ctx.current_type_map();
                   for param in &params {
                        let name = match param {
                             crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                             crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                        };
                        if let Some(arg) = current_map.get(&name) {
                             concrete_tys.push(arg.clone());
                        }
                   }
                   // If valid inference, concrete_tys should match params len
                   if concrete_tys.len() != params.len() {
                        concrete_tys.clear(); // Abort partially filled args
                   }
              }
         }
    }

    // [PHASE 4.1 BIDIR BRIDGE]
    if let Some(fn_generics) = &func.generics {
        let turbofish_count = if let Some(tf) = &m.turbofish { tf.args.len() } else { 0 };
        
        let struct_generic_names: std::collections::HashSet<String> = {
            let mut names = std::collections::HashSet::new();
            if let Some(t_name) = template_name_opt {
                let gen_params = {
                    let templates = ctx.struct_templates();
                    if let Some(s) = templates.get(t_name) {
                        s.generics.as_ref().map(|g| g.params.clone())
                    } else {
                        let etemplates = ctx.enum_templates();
                        etemplates.get(t_name).and_then(|e| e.generics.as_ref()).map(|g| g.params.clone())
                    }
                };
                if let Some(params) = gen_params {
                    for p in &params {
                        let name = match p {
                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                        };
                        names.insert(name);
                    }
                }
            }
            names
        };

        // Append only METHOD-level generics
        let mut turbofish_remaining = turbofish_count;
        for param in fn_generics.params.iter() {
             let name = match param {
                 crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                 crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
             };
             
             if struct_generic_names.contains(&name) {
                 continue;
             }
             
             if turbofish_remaining > 0 {
                 turbofish_remaining -= 1;
                 continue;
             }
             
             if let Some(resolved) = method_generic_map.get(&name) {
                  concrete_tys.push(resolved.clone());
             }
        }
    }
    Ok(())
}

fn resolve_specialized_method_name(
    ctx: &mut LoweringContext,
    target_name: &str,
    template_name_opt: &Option<String>,
    peeled_ty: &Type,
    method_name: &str,
    method_lookup_ty: &Type,
    concrete_tys: &[Type],
) -> String {
    let mut actual_target_name = target_name.to_string();
    let is_specialized = !concrete_tys.is_empty();
    if is_specialized {
        let mut handled = false;
        if let Some(t_name) = template_name_opt {
             let specialized_mangled_raw = ctx.get_mangled(peeled_ty).to_string();
             let specialized_mangled = specialized_mangled_raw.strip_prefix("!struct_")
                 .unwrap_or(&specialized_mangled_raw).to_string();
             
             if specialized_mangled != *t_name {
                  let (_base_prefix, override_name) = if let Type::Pointer { element, .. } = peeled_ty {
                      let suffix = element.mangle_suffix();
                      ("std__core__ptr__Ptr".to_string(), format!("std__core__ptr__Ptr__{}_{}", method_name, suffix))
                  } else {
                      (specialized_mangled.clone(), format!("{}__{}", specialized_mangled, method_name))
                  };
                  
                  let func_name_to_request = format!("{}__{}", t_name, method_name);
                  actual_target_name = ctx.request_explicit_specialization(
                      &func_name_to_request,
                      &override_name,
                      concrete_tys.to_vec(),
                      Some(method_lookup_ty.clone())
                  );
                  handled = true;
             }
        }
        
        if !handled {
            let base_prefix = template_name_opt.as_ref().unwrap_or(&target_name.to_string()).clone();
            actual_target_name = ctx.request_specialization(&format!("{}__{}", base_prefix, method_name), concrete_tys.to_vec(), Some(method_lookup_ty.clone()));
        }
    }
    actual_target_name
}

fn apply_method_memory_model(
    ctx: &mut LoweringContext,
    m: &syn::ExprMethodCall,
    method_name: &str,
) {
    if method_name != "free" && method_name != "drop" {
        let is_extern = ctx.external_decls().contains(method_name);
        if is_extern || ctx.config.freeing_functions.contains(method_name) {
            if let Some(var_name) = super::extract_ident_name(&m.receiver) {
                ctx.pointer_tracker.mark_optional(&var_name);
            }
            for arg in &m.args {
                if let Some(var_name) = super::extract_ident_name(arg) {
                    ctx.pointer_tracker.mark_optional(&var_name);
                }
            }
        }
    }
}

pub fn resolve_and_emit_method(
    ctx: &mut LoweringContext,
    out: &mut String,
    m: &syn::ExprMethodCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    expected_ty: Option<&Type>,
    cached_receiver_val: &Option<String>,
    cached_receiver_ty: &Type,
) -> Result<(String, Type), String> {
    let mut receiver_ty = cached_receiver_ty.clone();
    receiver_ty = receiver_ty.substitute(&ctx.current_type_map());
    receiver_ty = resolve_codegen_type(ctx, &receiver_ty);
    
    // instead of the variable name (e.g., GLOBAL_ALLOC) to construct the method name.
    let method = m.method.to_string();
    let method = m.method.to_string();
    let type_based_pkg = get_type_based_pkg(ctx, &receiver_ty, cached_receiver_val);
    let receiver_generic_suffix = get_receiver_generic_suffix(&receiver_ty);

    // 2. Check for Static Package Call (Namespace Lookahead)
    if let Some(res) = try_resolve_static_method(
        ctx, out, m, local_vars, expected_ty, &receiver_ty,
        cached_receiver_val, cached_receiver_ty, &type_based_pkg, &receiver_generic_suffix
    )? {
        return Ok(res);
    }

    let method_name = m.method.to_string();

    if let Some(res) = try_resolve_atomic_intrinsic(ctx, out, m, local_vars, expected_ty, &method_name)? {
        return Ok(res);
    }


    let (receiver_ptr, receiver_ty) = get_receiver_lvalue(
        ctx, out, &m.receiver, local_vars, cached_receiver_val, cached_receiver_ty, &method_name
    )?;
    let receiver_val = receiver_ptr.clone();
    // Extract inner type for method resolution (strip the Reference wrapper we added)
    let raw_lookup_ty = if let Type::Reference(inner, _) = &receiver_ty { *inner.clone() } else { receiver_ty.clone() };
    // KEUOS FIX: Apply current type_map substitution to resolve generics like T -> F32
    // This ensures that method calls inside generic functions use concrete types
    let current_map = ctx.current_type_map().clone();

    let method_lookup_ty = resolve_codegen_type(ctx, &raw_lookup_ty.substitute(&current_map));

    // [KEUOS FIX] Use fully-qualified TEMPLATE name for target_name (without specialization suffix)
    // The specialization suffix is added separately via request_specialization
    // Example: Type::Pointer { element: U8 } -> "std__core__ptr__Ptr" (not "std__core__ptr__Ptr_u8")
    let target_name = match &method_lookup_ty {
        Type::Pointer { .. } => "std__core__ptr__Ptr".to_string(),
        Type::Struct(name) => name.clone(),
        Type::Concrete(base, _) => base.clone(),  // Use base template name without args
        _ => method_lookup_ty.mangle_suffix(),
    };

    // [LAZY HYDRATION FIX] Auto-hydrate Ptr<T> impl methods before method lookup.
    // Ptr is a built-in type — its methods (write, read, offset, etc.) should always
    // be available without explicit `use std::core::ptr::*` imports.
    // ensure_struct_exists triggers template discovery which loads the impl block.
    if matches!(&method_lookup_ty, Type::Pointer { .. }) {
        let _ = ctx.ensure_struct_exists("std__core__ptr__Ptr", &[]);
    }

    // Helper closure for lookup
    let lookup_recursive = |ty: &Type| -> Option<((crate::grammar::SaltFn, Option<Type>, Vec<crate::grammar::ImportDecl>), Type)> {
        let _cached_receiver_ty = Type::Unit;
    
    // [DEBUG]
    
    // 1. Try to resolve the package name from the receiver type if it's an aggregatef?
        let mut current_ty = ty.clone();
        
        // Loop max 10 times to prevent infinite autoderef?
        for _ in 0..10 {
            // 1. Try Key Lookup (Deep Peeler logic in resolve_method handles the rest)
            if let Ok(info) = ctx.resolve_method(&current_ty, &method_name) {
                 return Some((info, current_ty.clone()));
            }
            
            // 2. Autoderef & Phantom Limb Peeling (Local Loop)
            match current_ty {
                Type::Owned(inner) => {
                    current_ty = *inner;
                },
                Type::Struct(ref name) if name.starts_with("RefMut_") => {
                    // Manual peel for recursion if resolve_method didn't catch it at this level
                    let inner_name = &name["RefMut_".len()..];
                    current_ty = Type::Struct(inner_name.to_string());
                },
                _ => break,
            }
        }
        None
    };

    let method_info_res = lookup_recursive(&receiver_ty);
    
    // Unpack (method_info is Option<((Fn, RecTy, Imports), ActualReceiverTy)>)
    // We update the 'rec_ty' (middle tuple element) to be the *actual* type found (e.g. T instead of &T)
    // capable of resolving Self properly.
    let method_info = method_info_res.map(|(info, actual_ty)| (info.0, Some(actual_ty), info.2));

    // (Proceed with existing logic using method_info)

    if let Some((func, _rec_ty, _)) = method_info {
        // [FIX] Context Setup: Self + Generics
        // NOTE: We use the ORIGINAL receiver_ty (with Reference wrapper) here because:
        // 1. It's needed for correct hydration context (monomorphized functions need correct Self type)
        // 2. The is_bare_self check below uses the parsed syn::Type, not current_self_ty
        let old_self = ctx.current_self_ty().clone();
        *ctx.current_self_ty_mut() = Some(method_lookup_ty.clone());

        let old_map = ctx.current_type_map().clone();
        
        let (mut concrete_tys, template_name_opt, peeled_ty) = populate_type_map_from_receiver(ctx, &receiver_ty);

        // [KEUOS FIX] Insert method generic names into type_map BEFORE calling resolve_type
        // so that they are resolved as Type::Generic instead of Type::Struct.
        if let Some(generics) = &func.generics {
            for param in &generics.params {
                let name = match param {
                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                };
                ctx.current_type_map_mut().insert(name.clone(), Type::Generic(name));
            }
        }

        // [STATIC METHOD GUARD] Static methods (e.g., Arena::new) have no self parameter.
        // Detect this before indexing func.args[0].
        // Parser (grammar.rs:1389) creates self args with name = Ident::new("self", ...) 
        // and ty = Some(Self/&Self). Static methods have regular named first args.
        let is_static_method = func.args.is_empty() || func.args[0].name != "self";

        let (self_arg_ty_raw, _is_bare_self, signature_arg_tys_raw) = if !is_static_method {
            let self_arg = &func.args[0];
            // [ABI FIX] Detect if self is by-value or by-reference by checking the RESOLVED type
            let ty_raw = if let Some(t) = &self_arg.ty { 
                resolve_type(ctx, t)
            } else { 
                method_lookup_ty.clone()
            };
            let bare = !matches!(&ty_raw, Type::Reference(..));
            let sig_tys = func.args.iter().skip(1).map(|a| resolve_type(ctx, a.ty.as_ref().expect("Missing param type"))).collect::<Vec<_>>();
            (Some(ty_raw), bare, sig_tys)
        } else {
            // Static method: all args are regular parameters
            let sig_tys = func.args.iter().map(|a| resolve_type(ctx, a.ty.as_ref().expect("Missing param type"))).collect::<Vec<_>>();
            (None, false, sig_tys)
        };
        let signature_ret_raw_unsubst = if let Some(rt) = &func.ret_type { resolve_type(ctx, rt) } else { Type::Unit };
        
        // [KEUOS FIX] Remove the method generics we temporarily added to ctx.current_type_map()
        // so that GenericResolver can infer them properly and substitute doesn't infinite loop.
        if let Some(generics) = &func.generics {
            for param in &generics.params {
                let name = match param {
                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                };
                ctx.current_type_map_mut().remove(&name);
            }
        }

        // KEUOS FIX V9.10: Build method-level generic mapping
        // Start with struct-level generics from context, then layer method-level on top
        let mut method_generic_map = ctx.current_type_map().clone();
        
        // [GENERIC RESOLVER] Consolidated generic inference via GenericResolver
        // This replaces ~150 lines of inline turbofish + bidir + arg inference + phantom logic
        {
            // Extract turbofish args
            let mut turbofish_args = Vec::new();
            if let Some(tf) = &m.turbofish {
                for arg in &tf.args {
                    if let syn::GenericArgument::Type(ty_arg) = arg {
                        let syn_ty = crate::grammar::SynType::from_std(ty_arg.clone()).map_err(|e| e.to_string())?;
                        let ty = crate::types::Type::from_syn(&syn_ty).ok_or_else(|| "Failed to parse type".to_string())?;
                        turbofish_args.push(resolve_codegen_type(ctx, &ty));
                    }
                }
            }

            // Extract struct generic params for the resolver
            let struct_gen_params: Option<Vec<crate::grammar::GenericParam>> = template_name_opt.as_ref().and_then(|t_name| {
                // Direct lookup first
                if let Some(s) = ctx.struct_templates().get(t_name) {
                    if let Some(g) = s.generics.as_ref() {
                        return Some(g.params.iter().cloned().collect());
                    }
                }
                if let Some(e) = ctx.enum_templates().get(t_name) {
                    if let Some(g) = e.generics.as_ref() {
                        return Some(g.params.iter().cloned().collect());
                    }
                }
                // Fallback: centralized template lookup
                if let Some(template_name) = ctx.find_struct_template_by_name(t_name) {
                    if let Some(template) = ctx.struct_templates().get(&template_name) {
                        if let Some(g) = template.generics.as_ref() {
                            return Some(g.params.iter().cloned().collect());
                        }
                    }
                }
                None
            });

            let struct_gen_slice = struct_gen_params.as_deref();

            // Use GenericResolver for method-level inference
            let mut resolver = crate::codegen::generic_resolver::GenericResolver::new(ctx);
            let call_args_vec: Vec<syn::Expr> = m.args.iter().cloned().collect();
            match resolver.resolve_generics(
                &func,
                &turbofish_args,
                &call_args_vec,
                local_vars,
                expected_ty,
                Some(&method_lookup_ty),
                struct_gen_slice,
                &concrete_tys,
            ) {
                Ok(resolved_map) => {
                    // Merge resolver results into method_generic_map
                    for (k, v) in resolved_map {
                        method_generic_map.insert(k, v);
                    }
                }
                Err(_e) => {

                    // Non-fatal: proceed with whatever we have
                }
            }
        }


        let signature_arg_tys = signature_arg_tys_raw.iter().map(|t| t.substitute(&method_generic_map)).collect::<Vec<_>>();
        let signature_ret_raw = signature_ret_raw_unsubst.substitute(&method_generic_map);


        *ctx.current_self_ty_mut() = old_self;
        *ctx.current_type_map_mut() = old_map;

        // [ABI FIX] For bare 'self' (by-value), use the inner type from method_lookup_ty
        // This avoids passing a Reference-wrapped type when the method expects a value
        // [ABI V12] Substitute generics in self-arg type for correct ABI detection
        let self_arg_ty = self_arg_ty_raw.as_ref().map(|t| t.substitute(&method_generic_map));

        
        let mut final_receiver_val = receiver_val;
        // [KEUOS FIX] Use substituted receiver_ty to preserve Type::Reference wrapper
        // method_lookup_ty strips the Reference, which causes invalid struct allocations!
        let mut final_receiver_ty = receiver_ty.substitute(&ctx.current_type_map());
        
        // COERCION: Treat Phantom RefMut as Reference
        if let Type::Struct(ref name) = final_receiver_ty {
            if name.starts_with("RefMut_") {
                 let inner_name = &name["RefMut_".len()..];
                 // Strip potential suffix if needed or assume inner name is valid Struct
                 // But wait, RefMut_std__collections__vec__Vec_u8 -> std__collections__vec__Vec_u8
                 let inner_ty = Type::Struct(inner_name.to_string());
                 
                 final_receiver_ty = Type::Reference(Box::new(inner_ty), true);
            }
        }
        

        
        // [KEUOS FIX] Check if receiver is ALREADY a pointer (from alloca/local variable/GEP)
        // These are identified by SSA name patterns: %local_, %alloca, %spill, %gep_f_, %field_ptr_
        let is_already_pointer = final_receiver_val.starts_with("%local_") 
            || final_receiver_val.starts_with("%alloca")
            || final_receiver_val.starts_with("%spill")
            || final_receiver_val.starts_with("%gep_f_")
            || final_receiver_val.starts_with("%field_ptr_")
            || final_receiver_val.starts_with("%iter_ptr_");
        
        if let Some(ref self_arg_ty_inner) = self_arg_ty {
            if matches!(self_arg_ty_inner, Type::Reference(..)) && !matches!(final_receiver_ty, Type::Reference(..)) {
                if is_already_pointer {
                    final_receiver_ty = Type::Reference(Box::new(final_receiver_ty), true);
                } else {
                    let ptr = format!("%self_ptr_{}", ctx.next_id());
                    let mlir_ty = final_receiver_ty.to_mlir_storage_type(ctx)?;
                    ctx.emit_alloca(out, &ptr, &mlir_ty);
                    ctx.emit_store(out, &final_receiver_val, &ptr, &mlir_ty);
                    final_receiver_val = ptr;
                    final_receiver_ty = Type::Reference(Box::new(final_receiver_ty), true);
                }
            } else if !matches!(self_arg_ty_inner, Type::Reference(..)) && matches!(final_receiver_ty, Type::Reference(..)) {
                if let Type::Reference(inner, _) = final_receiver_ty {
                    let mlir_ty = inner.to_mlir_storage_type(ctx)?;
                    let val = format!("%self_val_{}", ctx.next_id());
                    ctx.emit_load(out, &val, &final_receiver_val, &mlir_ty);
                    final_receiver_val = val;
                    final_receiver_ty = *inner;
                }
            } else if !matches!(self_arg_ty_inner, Type::Reference(..)) && !matches!(final_receiver_ty, Type::Reference(..)) && is_already_pointer {
                let mlir_ty = final_receiver_ty.to_mlir_storage_type(ctx)?;
                let val = format!("%self_val_{}", ctx.next_id());
                ctx.emit_load(out, &val, &final_receiver_val, &mlir_ty);
                final_receiver_val = val;
            }
        }
        
        // For instance methods, prepend receiver. For static methods, no receiver.
        let mut args_vals = if !is_static_method {
            vec![final_receiver_val]
        } else {
            vec![]
        };
        
        // Specialization Strategy:
        // If the method is generic OR the struct it belongs to is specialized,
        // we need to request a specialization for the method too.
        
            append_method_generics(ctx, m, &mut concrete_tys, &template_name_opt, &func, &method_generic_map)?;
            
            let actual_target_name = resolve_specialized_method_name(
                ctx, &target_name, &template_name_opt, &peeled_ty, &method_name, &method_lookup_ty, &concrete_tys
            );
            let is_specialized = !concrete_tys.is_empty();

        
        // Remove the redundant let mut args_vals = vec![receiver_val]; that followed
        // and fix the use of moved value.
        // Remove the redundant let mut args_vals = vec![receiver_val]; that followed
        // and fix the use of moved value.
        // Fix: Verification of Requires Clauses for Method Calls
        let _param_names: Vec<String> = func.args.iter().map(|a| a.name.to_string()).collect();
        // Construct full args list including receiver (self)
        let mut full_args_exprs = vec![*m.receiver.clone()];
        full_args_exprs.extend(m.args.iter().cloned());
        

        let arg_tys = signature_arg_tys;
        
        for (i, arg_expr) in m.args.iter().enumerate() {
            let expected = arg_tys.get(i);
            let (val, ty) = emit_expr(ctx, out, arg_expr, local_vars, expected)?;
            

            // Call-Site Guard Coercion
            let val_prom = if let Some(target) = expected {
                 // Explicitly check for type mismatch to force cast logic (bypassing promote_numeric optimism)
                 if &ty != target {
                      crate::codegen::type_bridge::cast_numeric(ctx, out, &val, &ty, target)?
                 } else {
                      val
                 }
            } else { val };
            args_vals.push(val_prom);
        }

        let ret_ty = resolve_codegen_type(ctx, &signature_ret_raw);
        let res = if ret_ty != Type::Unit { format!("%mcall_res_{}", ctx.next_id()) } else { "".to_string() };
        
        let mangled_method = if is_specialized {
            actual_target_name // Already mangled by request_specialization
        } else {
            let base_prefix = template_name_opt.as_ref().unwrap_or(&target_name);
            let m_name = format!("{}__{}", base_prefix, method_name);
            let _ = ctx.request_specialization(&m_name, vec![], Some(final_receiver_ty.clone()));
            m_name
        };
        
        // HACK: GlobalSlabAlloc methods are specialized as free functions in std
        let mangled_method = if mangled_method.contains("GlobalSlabAlloc") {
             let short = mangled_method.replace("GlobalSlabAlloc__", "");
             if ctx.resolve_global(&short).is_some() {

                 short
             } else {
                 mangled_method
             }
        } else {
             mangled_method
        };
        
        // [FIX] Arity Mismatch Correction for Redirected Calls (Method -> Free Fn)
        // Only applies when a method call was REDIRECTED to a free function
        // (e.g., GlobalSlabAlloc methods). Instance methods must KEEP their receiver.
        let mut final_args_vals = args_vals.clone();
        let mut final_arg_tys_vec = vec![final_receiver_ty];
        final_arg_tys_vec.extend(arg_tys.clone());
        
        if is_static_method {
            if let Some(sig) = ctx.resolve_global(&mangled_method) {
                 if let Type::Fn(expected_args, _) = sig {
                      if final_args_vals.len() == expected_args.len() + 1 {
                           final_args_vals.remove(0);
                           final_arg_tys_vec.remove(0);
                      }
                 }
            }
        }

        let args_str = final_args_vals.join(", ");
        ctx.ensure_func_declared(&mangled_method, &final_arg_tys_vec, &ret_ty)?;

        let mut mlir_arg_tys_code = Vec::new();
        for t in &final_arg_tys_vec {
            mlir_arg_tys_code.push(t.to_mlir_type(ctx)?);
        }
        let mlir_arg_tys = mlir_arg_tys_code.join(", ");
        
        if res.is_empty() {
            out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled_method, args_str, mlir_arg_tys));
        } else {
            out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", res, mangled_method, args_str, mlir_arg_tys, ret_ty.to_mlir_type(ctx)?));
        }
        
        // [SALT MEMORY MODEL] Conservative Aliasing for methods
        apply_method_memory_model(ctx, m, &method_name);

        ctx.emission.global_lvn.clear();
        Ok((res, ret_ty))
    } else {
        Err(format!("Method {} not found on type {}", method_name, target_name))
    }
}
