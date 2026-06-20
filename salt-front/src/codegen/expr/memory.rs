use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::type_bridge::*;
use std::collections::HashMap;
use super::{emit_expr, emit_lvalue, LValueKind};


fn emit_field_get_base(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &syn::ExprField,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<(String, Type, bool), String> {
    if let Ok((addr, ty, _kind)) = emit_lvalue(ctx, out, &f.base, local_vars) {
        let is_aggregate = matches!(&ty, Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _) | Type::Tuple(_))
            || matches!(&ty, Type::Reference(inner, _) if matches!(inner.as_ref(), Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _) | Type::Tuple(_)));
        if is_aggregate {
            if matches!(&ty, Type::Reference(_, _)) {
                let actual_addr = if _kind == LValueKind::Local {
                    let loaded_ref = format!("%loaded_ref_{}", ctx.next_id());
                    ctx.emit_load(out, &loaded_ref, &addr, "!llvm.ptr");
                    loaded_ref
                } else {
                    addr
                };
                Ok((actual_addr, ty, true))
            } else {
                Ok((addr, ty, true))
            }
        } else {
            let val = if _kind == LValueKind::SSA {
                addr.clone()
            } else {
                let v = format!("%field_base_load_{}", ctx.next_id());
                let mlir_ty = ty.to_mlir_storage_type(ctx)?;
                ctx.emit_load(out, &v, &addr, &mlir_ty);
                v
            };
            Ok((val, ty, false))
        }
    } else {
        let (bv, bt) = emit_expr(ctx, out, &f.base, local_vars, None)?;
        Ok((bv, bt, false))
    }
}

fn emit_field_safety_check(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &syn::ExprField,
    base_ty: &Type,
    current_val: &str,
) -> Result<Option<(String, Type)>, String> {
    if let syn::Expr::Path(path_expr) = &*f.base {
        if let Some(ident) = path_expr.path.get_ident() {
            let var_name = ident.to_string();
            let field_name = if let syn::Member::Named(id) = &f.member { id.to_string() } else { "unnamed".to_string() };
            let is_ptr = matches!(base_ty, Type::Reference(..) | Type::Pointer { .. }) || base_ty.k_is_ptr_type();
            if is_ptr {
                if field_name == "addr"
                    && (matches!(base_ty, Type::Pointer { .. } | Type::Reference(..)) || base_ty.k_is_ptr_type()) {
                         let addr_val = format!("%ptr_addr_{}", ctx.next_id());
                         out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", addr_val, current_val));
                         return Ok(Some((addr_val, Type::U64)));
                    }
                let is_dynamic = *ctx.is_dynamic_check_block() || ctx.emission.in_dynamic_check_fn;
                if !is_dynamic {
                    ctx.pointer_tracker.check_deref(&var_name)?;
                }
            }
        }
    }
    Ok(None)
}

fn emit_field_auto_deref(
    ctx: &mut LoweringContext,
    out: &mut String,
    mut current_ty: Type,
    mut current_val: String,
    mut was_ref: bool,
) -> Result<(Type, String, bool), String> {
    loop {
        let ty_clone = current_ty.clone();
        if let Type::Reference(inner, _) = ty_clone {
            was_ref = true;
            match *inner {
                Type::Struct(_) | Type::Tuple(_) | Type::Concrete(_, _) => {
                    current_ty = *inner;
                    break;
                }
                _ => {
                    let loaded = format!("%deref_{}", ctx.next_id());
                    let mlir_ty = inner.to_mlir_type(ctx)?;
                    ctx.emit_load(out, &loaded, &current_val, &mlir_ty);
                    current_val = loaded;
                    current_ty = *inner;
                }
            }
        } else if let Type::Pointer { element, .. } = ty_clone {
            was_ref = true;
            match *element {
                Type::Struct(_) | Type::Tuple(_) | Type::Concrete(_, _) => {
                    current_ty = *element; 
                    break;
                }
                _ => {
                    let loaded = format!("%deref_ptr_{}", ctx.next_id());
                    let mlir_ty = element.to_mlir_type(ctx)?;
                    ctx.emit_load(out, &loaded, &current_val, &mlir_ty);
                    current_val = loaded;
                    current_ty = *element;
                }
            }
        } else {
            break;
        }
    }
    Ok((current_ty, current_val, was_ref))
}

pub fn emit_field(
    ctx: &mut LoweringContext,
    out: &mut String,
    f: &syn::ExprField,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
) -> Result<(String, Type), String> {
    let (base_val, base_ty, was_reference) = emit_field_get_base(ctx, out, f, local_vars)?;
    let mut current_ty = base_ty.clone();
    let mut current_val = base_val.clone();
    let mut was_ref = was_reference;

    if let Some(res) = emit_field_safety_check(ctx, out, f, &base_ty, &current_val)? {
        return Ok(res);
    }

    let (cty, cval, wref) = emit_field_auto_deref(ctx, out, current_ty, current_val, was_ref)?;
    current_ty = cty;
    current_val = cval;
    was_ref = wref;

    let is_dynamic = *ctx.is_dynamic_check_block() || ctx.emission.in_dynamic_check_fn;
    if was_ref && is_dynamic {
        out.push_str(&format!("    llvm.call @salt_verify_epoch({}) : (!llvm.ptr) -> ()\n", current_val));
        let as_int = format!("%tag_int_{}", ctx.next_id());
        let mask = format!("%tag_mask_{}", ctx.next_id());
        let stripped_int = format!("%stripped_int_{}", ctx.next_id());
        let stripped_ptr = format!("%stripped_ptr_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", as_int, current_val));
        out.push_str(&format!("    {} = llvm.mlir.constant(281474976710655 : i64) : i64\n", mask));
        out.push_str(&format!("    {} = llvm.and {}, {} : i64\n", stripped_int, as_int, mask));
        out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", stripped_ptr, stripped_int));
        let _ = ctx.ensure_external_declaration("salt_verify_epoch", &[Type::Pointer { element: Box::new(Type::U8), is_mutable: false, provenance: crate::types::Provenance::Naked }], &Type::Unit);
        current_val = stripped_ptr;
    }

    let current_ty_resolved = if let Type::Concrete(base, args) = &current_ty {
        let specialized = ctx.ensure_struct_exists(base, args)?;
        Type::Struct(specialized)
    } else {
        current_ty.clone()
    };

    if let Type::Struct(name) = &current_ty_resolved {
        let current_ty = current_ty_resolved.clone();
        let info = ctx.lookup_struct_by_type(&current_ty)
            .or_else(|| {
                let canonical = current_ty.to_canonical_name();
                ctx.struct_registry().values()
                    .find(|i| {
                        let i_canonical = Type::Struct(i.name.clone()).to_canonical_name();
                        i.name == canonical || i_canonical == canonical || i.name == *name
                    })
                    .cloned()
            })
            .ok_or_else(|| {
                let available: Vec<String> = ctx.struct_registry().values().map(|i| i.name.clone()).collect();
                format!("Undefined struct: {} (Available: {:?})", name, available)
            })?;
            
        let field_name = if let syn::Member::Named(id) = &f.member { id.to_string() } else { "unnamed".to_string() };
        
        if let Some((idx, raw_field_ty)) = info.fields.get(&field_name) {
            let mut local_spec_map = ctx.current_type_map().clone();
            
            if !info.specialization_args.is_empty() {
                if let Some(template_name) = &info.template_name {
                    if let Some(template_def) = ctx.struct_templates().get(template_name).cloned() {
                        if let Some(ref generics) = template_def.generics {
                            for (i, param) in generics.params.iter().enumerate() {
                                if let crate::grammar::GenericParam::Type { name: param_name, .. } = param {
                                    if i < info.specialization_args.len() {
                                        let concrete_ty = info.specialization_args[i].clone();
                                        local_spec_map.insert(param_name.to_string(), concrete_ty);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            let field_ty = &raw_field_ty.substitute(&local_spec_map);
            let struct_mlir_ty = Type::Struct(info.name.clone()).to_mlir_type(ctx)?;
            
            if struct_mlir_ty == "i64" {
                return Ok((current_val, field_ty.clone()));
            }
            
            if let Type::Pointer { .. } = current_ty {
                 if field_name == "addr" {
                     let addr_val = format!("%ptr_addr_{}", ctx.next_id());
                     out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", addr_val, current_val));
                     return Ok((addr_val, Type::U64));
                 }
            }

            let is_native_ptr = name.contains("NativePtr") && current_ty.k_is_ptr_type();
            if is_native_ptr
                && field_name == "addr" {
                    let is_lvalue = was_ref 
                        || matches!(base_ty, Type::Reference(_, _)) 
                        || current_val.contains("spill")
                        || current_val.contains("local_")
                        || current_val.contains("alloca");
                    
                    let ptr_val = if is_lvalue {
                        let loaded = format!("%nativeptr_loaded_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !llvm.ptr\n", loaded, current_val));
                        loaded
                    } else {
                        current_val.clone()
                    };
                    
                    let addr_val = format!("%nativeptr_addr_extract_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", addr_val, ptr_val));
                    return Ok((addr_val, Type::U64));
                }

            let phys_idx = ctx.get_physical_index(&info.field_order, *idx);
            let is_ephemeral_ref = ctx.emission.ephemeral_refs.contains(&current_val);
            
            if !was_ref && !matches!(base_ty, Type::Reference(_, _)) && !matches!(base_ty, Type::Owned(_)) && !is_ephemeral_ref {
                 let spill = format!("%spill_field_base_{}", ctx.next_id());
                 ctx.emit_alloca(out, &spill, &struct_mlir_ty);
                 ctx.emit_store(out, &current_val, &spill, &struct_mlir_ty);
                 
                 let ptr = format!("%field_ptr_{}_{}", field_name, ctx.next_id());
                 ctx.emit_gep_field(out, &ptr, &spill, phys_idx, &struct_mlir_ty);
                 
                 let is_large_aggregate = matches!(field_ty, Type::Array(_, _, _)) || {
                     let size = ctx.size_of(field_ty);
                     size > 64
                 };
                 if is_large_aggregate {
                     return Ok((ptr, Type::Reference(Box::new(field_ty.clone()), false)));
                 }
                 
                 let res = format!("%field_val_{}_{}", field_name, ctx.next_id());
                 ctx.emit_load_logical(out, &res, &ptr, field_ty)?;
                 return Ok((res, field_ty.clone()));
            } else {
                let ptr = format!("%field_ptr_{}_{}", field_name, ctx.next_id());
                ctx.emit_gep_field(out, &ptr, &current_val, phys_idx, &struct_mlir_ty);
                
                let is_large_aggregate = matches!(field_ty, Type::Array(_, _, _)) || {
                    let size = ctx.size_of(field_ty);
                    size > 64
                };
                if is_large_aggregate {
                    return Ok((ptr, Type::Reference(Box::new(field_ty.clone()), false)));
                }
                
                let res = format!("%field_val_{}_{}", field_name, ctx.next_id());
                ctx.emit_load_logical(out, &res, &ptr, field_ty)?;
                return Ok((res, field_ty.clone()));
            }
        }
    } else if let Type::Tuple(elems) = &current_ty {
        if let syn::Member::Unnamed(idx) = &f.member {
            let i = idx.index as usize;
            if let Some(elem_ty) = elems.get(i) {
                let mlir_tuple = current_ty.to_mlir_type(ctx)?;
                let ptr = format!("%tuple_field_{}_{}", i, ctx.next_id());
                ctx.emit_gep_field(out, &ptr, &current_val, i, &mlir_tuple);
                let res = format!("%tuple_val_{}_{}", i, ctx.next_id());
                ctx.emit_load_logical(out, &res, &ptr, elem_ty)?;
                return Ok((res, elem_ty.clone()));
            }
        }
    } else if let Type::Owned(inner) = &current_ty {
        let inner_resolved = if let Type::Concrete(base, args) = &**inner {
             Type::Struct(ctx.ensure_struct_exists(base, args)?)
        } else {
             *inner.clone()
        };

        match inner_resolved {
             Type::Struct(ref sn) | Type::Concrete(ref sn, _) => {
                  let field_name = if let syn::Member::Named(id) = &f.member { id.to_string() } else { return Err("Named field expected".to_string()); };
                  let struct_ty = Type::Struct(sn.clone());
                  let info = ctx.lookup_struct_by_type(&struct_ty)
                      .or_else(|| ctx.struct_registry().values().find(|i| i.name == *sn).cloned())
                      .unwrap_or_else(|| panic!("Struct info missing for '{}' (available: {:?})", sn, ctx.struct_registry().keys().map(|k| k.name.clone()).collect::<Vec<_>>()));
                  if let Some((idx, field_ty)) = info.fields.get(&field_name) {
                       let gep_var = format!("%owned_gep_{}", ctx.next_id());
                       let mlir_ty = inner.to_mlir_type(ctx)?;
                       let phys_idx = ctx.get_physical_index(&info.field_order, *idx);
                       ctx.emit_gep_field(out, &gep_var, &current_val, phys_idx, &mlir_ty);
                       let res = format!("%owned_res_{}", ctx.next_id());
                       ctx.emit_load_logical(out, &res, &gep_var, field_ty)?;
                       return Ok((res, field_ty.clone()));
                  } else { return Err(format!("Field not found {}", field_name)); }
             }
             Type::Tuple(ref elems) => {
                 let idx = if let syn::Member::Unnamed(idx) = &f.member { idx.index as usize } else { return Err("Tuple access requires index".to_string()); };
                 if idx >= elems.len() { return Err(format!("Tuple index out of bounds: {} >= {}", idx, elems.len())); }
                 let field_ty = &elems[idx];
                 let gep_var = format!("%owned_gep_tup_{}", ctx.next_id());
                 let mlir_ty = inner.to_mlir_type(ctx)?;
                 ctx.emit_gep_field(out, &gep_var, &current_val, idx, &mlir_ty);
                 let res = format!("%owned_res_tup_{}", ctx.next_id());
                 ctx.emit_load_logical(out, &res, &gep_var, field_ty)?;
                 return Ok((res, field_ty.clone()));
             }
             _ => return Err(format!("Cannot access field {:?} on type Owned({:?})", f.member, inner_resolved)),
        }
    }
    Err(format!("Cannot access field {:?} on type {:?}", f.member, base_ty))
}



fn emit_index_ptr_ref(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, base_ty: &Type, kind: LValueKind, element: &Box<Type>) -> Result<(String, Type), String> {
// Check deref validity
                 if let syn::Expr::Path(path_expr) = &*i.expr {
                     if let Some(ident) = path_expr.path.get_ident() {
                         let var_name = ident.to_string();
                         let is_dynamic = *ctx.is_dynamic_check_block() || ctx.emission.in_dynamic_check_fn;
                         if !is_dynamic {
                             ctx.pointer_tracker.check_deref(&var_name)?;
                         }
                     }
                 }

                 // Z3 Bounds Verification Integration
                 let func_name = ctx.current_fn_name().clone();
                 let info = crate::codegen::verification::ptr_bounds_verifier::PtrBoundsInfo::new(&func_name);
                 let proof_result = crate::codegen::verification::ptr_bounds_verifier::verify_ptr_dynamic_index(ctx.z3_ctx, ctx.z3_solver, &info);
                 
                 match proof_result {
                     crate::codegen::verification::ptr_bounds_verifier::PtrProofResult::Proven => {
                         *ctx.elided_checks += 1;
                     }
                     _ => {
                         if !ctx.config.no_verify && !*ctx.is_unsafe_block() {
                             return Err(format!("Unsafe pointer indexing in '{}': Z3 could not prove bounds safety for raw pointer indexing. You must provide explicit bounds constraints via @requires, loop invariants, or wrap the access in an unsafe block.", func_name));
                         }
                     }
                 }

                 // [DEBUG] Trace array-ref indexing path
                 // [ZERO-TRUST INDEX EVALUATION] Pass None to sever Context Contamination
                 let idx_expr = &*i.index;
                 let (raw_idx_val, raw_idx_ty) = emit_expr(ctx, out, idx_expr, local_vars, None)?;
                 
                 // 
                 let idx_final = if raw_idx_ty == Type::I64 {
                     raw_idx_val
                 } else {
                     promote_numeric(ctx, out, &raw_idx_val, &raw_idx_ty, &Type::I64)?
                 };
                                // Use LValueKind to determine if we need to load.
                  // If kind is Ptr or Local, base_ptr is an alloca containing the pointer - must load first.
                  // If kind is SSA, base_ptr IS the pointer value (no load needed).
                  // For Reference types, the SSA value IS the pointer - don't load!
                  // A &u8 parameter like %arg_s is already the pointer to the data, not a pointer-to-pointer.
                  let ptr_for_gep = if matches!(base_ty, Type::Reference(_, _)) {
                      // For references, base_ptr IS the address of the data (even if kind is Ptr)
                      base_ptr.clone()
                  } else {
                      match kind {
                          LValueKind::Ptr | LValueKind::Local => {
                              // base_ptr is an alloca, need to load the pointer value
                              let loaded_ptr = format!("%ptr_lvalue_loaded_{}", ctx.next_id());
                              ctx.emit_load(out, &loaded_ptr, &base_ptr, "!llvm.ptr");
                              loaded_ptr
                          }
                          LValueKind::SSA | LValueKind::Global(_) | LValueKind::Bit(_) | LValueKind::Tensor { .. } => {
                              // base_ptr is already the SSA value (the pointer itself)
                              base_ptr.clone()
                          }
                      }
                  };
                 
                 // Tier 3: @dynamic_check Epoch Verification
                 let is_dynamic = *ctx.is_dynamic_check_block() || ctx.emission.in_dynamic_check_fn;
                 let ptr_for_gep = if is_dynamic {
                     out.push_str(&format!("    llvm.call @salt_verify_epoch({}) : (!llvm.ptr) -> ()\n", ptr_for_gep));
                     let as_int = format!("%tag_int_{}", ctx.next_id());
                     let mask = format!("%tag_mask_{}", ctx.next_id());
                     let stripped_int = format!("%stripped_int_{}", ctx.next_id());
                     let stripped_ptr = format!("%stripped_ptr_{}", ctx.next_id());
                     out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", as_int, ptr_for_gep));
                     out.push_str(&format!("    {} = llvm.mlir.constant(281474976710655 : i64) : i64\n", mask));
                     out.push_str(&format!("    {} = llvm.and {}, {} : i64\n", stripped_int, as_int, mask));
                     out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", stripped_ptr, stripped_int));
                     let _ = ctx.ensure_external_declaration("salt_verify_epoch", &[Type::Pointer { element: Box::new(Type::U8), is_mutable: false, provenance: crate::types::Provenance::Naked }], &Type::Unit);
                     stripped_ptr
                 } else {
                     ptr_for_gep
                 };
                 
                  // Handle Reference(Array(T, N)) - read index into array element
                  // When element is Array(I32, 10, false), use [0, idx] GEP and return element type
                  if let Type::Array(ref arr_elem, _, _) = **element {
                      let arr_mlir = element.to_mlir_type(ctx)?;
                      let elem_ptr = format!("%ref_arr_elem_ptr_{}", ctx.next_id());
                      out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n",
                          elem_ptr, ptr_for_gep, idx_final, arr_mlir));
                      let load_res = format!("%ref_arr_val_{}", ctx.next_id());
                      ctx.emit_load_logical(out, &load_res, &elem_ptr, arr_elem.as_ref())?;
                      return Ok((load_res, (**arr_elem).clone()));
                  }

                 let res = format!("%ptr_idx_{}", ctx.next_id());
                 let elem_mlir = element.to_mlir_storage_type(ctx)?;

                 // LOWERING: Becomes a direct LLVM GEP + LOAD
                 ctx.emit_gep(out, &res, &ptr_for_gep, &idx_final, &elem_mlir);
                 let load_res = format!("%val_{}", ctx.next_id());
                 ctx.emit_load(out, &load_res, &res, &elem_mlir);
                 
                 Ok((load_res, (**element).clone()))
}

fn emit_index_tensor(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, inner: &Box<Type>, shape: &Vec<usize>) -> Result<(String, Type), String> {
// Tensors are memref types (SSA values from memref.alloc)
                 // For SSA, base_ptr is already the memref value
                 // For Ptr/Local, we would need memref.load from a ptr, but tensors should always be SSA
                 let tensor_ptr = base_ptr.clone();

                 // Unwrap Paren: (i, j) may be wrapped in syn::Expr::Paren
                 let index_expr = if let syn::Expr::Paren(p) = &*i.index {
                     &*p.expr
                 } else {
                     &*i.index
                 };
                 let indices = if let syn::Expr::Tuple(tup) = index_expr {
                     let mut v = Vec::new();
                     for e in &tup.elems {
                         let (val, ty) = emit_expr(ctx, out, e, local_vars, Some(&Type::Usize))?;
                         // Skip cast if already index type (Usize) - important for affine.for IVs
                         let idx_index = if ty == Type::Usize {
                             val
                         } else {
                             let i64_val = promote_numeric(ctx, out, &val, &ty, &Type::I64)?;
                             let idx = format!("%idx_index_{}", ctx.next_id());
                             out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", idx, i64_val));
                             idx
                         };
                         v.push(idx_index);
                     }
                     v
                 } else {
                     let (idx_val, idx_ty) = emit_expr(ctx, out, index_expr, local_vars, None)?;
                     
                     if let Type::Tuple(elem_tys) = &idx_ty {
                         // Handle Tuple Index (e.g. let t = (1, 2); tensor[t])
                         let mut v = Vec::new();
                         // Need the MLIR type of the tuple for extractvalue? 
                         // llvm.extractvalue takes the aggregate.
                         // But we need the type of the aggregate logic.
                         // Usually type is inferred or we pass the struct type.
                         // In MLIR llvm.extractvalue: `llvm.extractvalue %struct[0] : !llvm.struct<(...)>`
                         let tuple_mlir_ty = idx_ty.to_mlir_type(ctx)?;
                         
                         for (i, elem_ty) in elem_tys.iter().enumerate() {
                             let extracted = format!("%idx_extract_{}_{}", i, ctx.next_id());
                             out.push_str(&format!("    {} = llvm.extractvalue {}[{}]{} : {}\n", 
                                extracted, 
                                idx_val, 
                                i, 
                                "", // No extra indices
                                tuple_mlir_ty
                             ));
                             
                             let idx_index = if *elem_ty == Type::Usize {
                                 extracted
                             } else {
                                 let i64_val = promote_numeric(ctx, out, &extracted, elem_ty, &Type::I64)?;
                                 let idx = format!("%idx_index_cast_{}_{}", i, ctx.next_id());
                                 out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", idx, i64_val));
                                 idx
                             };
                             v.push(idx_index);
                         }
                         v
                     } else {
                         // Scalar Index
                         let idx_index = if idx_ty == Type::Usize {
                             idx_val
                         } else {
                             let i64_val = promote_numeric(ctx, out, &idx_val, &idx_ty, &Type::I64)?;
                             let idx = format!("%idx_index_{}", ctx.next_id());
                             out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", idx, i64_val));
                             idx
                         };
                         vec![idx_index]
                     }
                 };
                 
                 // Z3 Bounds Check Elision 
                 // Attempts to prove bounds are safe at compile time using Z3.
                 // If proven safe, emits no runtime check. Otherwise, falls through to memref.load
                 // which has implicit bounds checking in debug mode.
                 let sym_ctx = crate::codegen::verification::SymbolicContext::new(ctx.z3_ctx);
                 let mut all_safe = true;
                 
                 if let syn::Expr::Tuple(tup) = index_expr {
                     for (dim, e) in tup.elems.iter().enumerate() {
                         if let Some(dim_size) = shape.get(dim) {
                             if let Ok(z3_idx) = translate_to_z3(ctx, e, local_vars) {
                                  let z3_size = ctx.mk_int(*dim_size as i64);
                                  let z3_zero = ctx.mk_int(0);
                                  let lt_zero = z3_idx.lt(&z3_zero);
                                  let ge_size = z3_idx.ge(&z3_size);
                                  let violation = crate::z3_shim::ast::Bool::or(ctx.z3_ctx, &[&lt_zero, &ge_size]);
                                  *ctx.total_checks += 1;
                                  ctx.z3_solver.push();
                                  ctx.z3_solver.assert(&violation);
                                  let z3_result = ctx.z3_solver.check();
                                  ctx.z3_solver.pop(1);
                                  if z3_result == crate::z3_shim::SatResult::Unsat {
                                      *ctx.elided_checks += 1;
                                  } else {
                                      all_safe = false;
                                  }
                             } else { all_safe = false; }
                         } else { all_safe = false; }
                     }
                 } else if shape.len() == 1 {
                     if let Ok(z3_idx) = translate_to_z3(ctx, index_expr, local_vars) {
                          let z3_size = ctx.mk_int(shape[0] as i64);
                          let z3_zero = ctx.mk_int(0);
                          let lt_zero = z3_idx.lt(&z3_zero);
                          let ge_size = z3_idx.ge(&z3_size);
                          let violation = crate::z3_shim::ast::Bool::or(ctx.z3_ctx, &[&lt_zero, &ge_size]);
                          *ctx.total_checks += 1;
                          ctx.z3_solver.push();
                          ctx.z3_solver.assert(&violation);
                          let z3_result = ctx.z3_solver.check();
                          ctx.z3_solver.pop(1);
                          if z3_result == crate::z3_shim::SatResult::Unsat {
                              *ctx.elided_checks += 1;
                          } else {
                              all_safe = false;
                          }
                     } else { all_safe = false; }
                 } else { all_safe = false; }
                 
                 // Log elision status for debugging (can be removed in production)
                 let _ = (sym_ctx, all_safe); // Suppress unused warnings for now

                 // TENSOR LOAD : Use memref.load with multi-dimensional indices
                 // Tensors are allocated as memref<DxMxT>, so we must use memref ops, not llvm.gep
                 
                 let elem_mlir = inner.to_mlir_storage_type(ctx)?;
                 
                 // Build the memref type string: memref<D0xD1x...xDnxElemType>
                 let shape_str = shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
                 let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
                 
                 // Build index list for memref.load: [%i0, %i1, ...]
                 let indices_str = indices.join(", ");
                 
                 let res = format!("%tensor_val_{}", ctx.next_id());
                 out.push_str(&format!("    {} = memref.load {}[{}] : {}\n", 
                     res, tensor_ptr, indices_str, memref_ty));
                 
                 Ok((res, *inner.clone()))
}

fn emit_index_array(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, base_ty: &Type, inner: &Box<Type>, packed: &bool) -> Result<(String, Type), String> {
let (idx_val, idx_ty) = emit_expr(ctx, out, &i.index, local_vars, Some(&Type::I64))?;
                 let idx_prom = promote_numeric(ctx, out, &idx_val, &idx_ty, &Type::I64)?;
                 
                 if *packed {
                      // Packed Boolean Array Read
                      // 1. Word Index = idx / 64
                      let word_idx = format!("%word_idx_{}", ctx.next_id());
                      let c64 = format!("%c64_{}", ctx.next_id());
                      ctx.emit_const_int(out, &c64, 64, "i64");
                      ctx.emit_binop(out, &word_idx, "arith.divui", &idx_prom, &c64, "i64");
                      
                      // 2. Bit Offset = idx % 64
                      let bit_off = format!("%bit_off_{}", ctx.next_id());
                      ctx.emit_binop(out, &bit_off, "arith.remui", &idx_prom, &c64, "i64");
                      
                      // 3. GEP Word
                      let elem_ptr = format!("%word_ptr_{}", ctx.next_id());
                      let arr_mlir = base_ty.to_mlir_type(ctx)?;
                      // base_ptr is pointer to array. GEP into word array.
                      // Note: Packed array storage is !llvm.array<N x i64>.
                      out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", elem_ptr, base_ptr, word_idx, arr_mlir));
                      
                      // 4. Load Word
                      let word_val = format!("%word_val_{}", ctx.next_id());
                      ctx.emit_load(out, &word_val, &elem_ptr, "i64");
                      
                      // 5. Shift & Mask
                      let shifted = format!("%shifted_{}", ctx.next_id());
                      ctx.emit_binop(out, &shifted, "arith.shrui", &word_val, &bit_off, "i64");
                      
                      let trunc = format!("%trunc_{}", ctx.next_id());
                      ctx.emit_cast(out, &trunc, "arith.trunci", &shifted, "i64", "i1");
                      
                      // Salt boolean storage is i8? But emit_expr for Bool usually expects i1 in SSA.
                      // Let's return i1 (Type::Bool SSA is i1).
                      return Ok((trunc, Type::Bool));
                 }

                 let elem_ptr = format!("%elem_ptr_{}", ctx.next_id());
                 let arr_mlir = base_ty.to_mlir_type(ctx)?;
                 out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", elem_ptr, base_ptr, idx_prom, arr_mlir));
                 
                 let res = format!("%index_res_{}", ctx.next_id());
                  ctx.emit_load_logical(out, &res, &elem_ptr, inner.as_ref())?;
                  Ok((res, *inner.clone()))
}

fn emit_index_owned(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, kind: LValueKind, inner: &Box<Type>) -> Result<(String, Type), String> {
let (idx_val, idx_ty) = emit_expr(ctx, out, &i.index, local_vars, Some(&Type::I64))?;
                 let idx_prom = promote_numeric(ctx, out, &idx_val, &idx_ty, &Type::I64)?;

                 let loaded_ptr = if kind == LValueKind::SSA {
                     base_ptr
                 } else {
                     let res = format!("%loaded_base_{}", ctx.next_id());
                     ctx.emit_load(out, &res, &base_ptr, "!llvm.ptr");
                     res
                 };
                 
                 if let Type::Array(ref elem_ty, _, packed) = inner.as_ref() {
                     let elem_ptr = format!("%elem_ptr_{}", ctx.next_id());
                     let arr_mlir = inner.to_mlir_type(ctx)?;  
                     
                     if *packed {
                          let c64 = format!("%c64_{}", ctx.next_id());
                          ctx.emit_const_int(out, &c64, 64, "i64");
                          let word_idx = format!("%word_idx_{}", ctx.next_id());
                          ctx.emit_binop(out, &word_idx, "arith.divui", &idx_prom, &c64, "i64");
                          let bit_off = format!("%bit_off_{}", ctx.next_id());
                          ctx.emit_binop(out, &bit_off, "arith.remui", &idx_prom, &c64, "i64");

                          out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", elem_ptr, loaded_ptr, word_idx, arr_mlir));
                          
                          let word_val = format!("%word_val_{}", ctx.next_id());
                          ctx.emit_load(out, &word_val, &elem_ptr, "i64");
                          let shifted = format!("%shifted_{}", ctx.next_id());
                          ctx.emit_binop(out, &shifted, "arith.shrui", &word_val, &bit_off, "i64");
                          let trunc = format!("%trunc_{}", ctx.next_id());
                          ctx.emit_cast(out, &trunc, "arith.trunci", &shifted, "i64", "i1");
                          return Ok((trunc, Type::Bool));
                     }

                     out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", elem_ptr, loaded_ptr, idx_prom, arr_mlir));
                     
                     let res = format!("%index_res_{}", ctx.next_id());
                      ctx.emit_load_logical(out, &res, &elem_ptr, elem_ty.as_ref())?;
                     
                     return Ok((res, *elem_ty.clone()));
                 }

                 let elem_ptr = format!("%elem_ptr_{}", ctx.next_id());
                 let inner_storage = inner.to_mlir_storage_type(ctx)?;
                 ctx.emit_gep(out, &elem_ptr, &loaded_ptr, &idx_prom, &inner_storage);
                 let res = format!("%index_res_{}", ctx.next_id());
                 ctx.emit_load_logical(out, &res, &elem_ptr, inner)?;
                 Ok((res, *inner.clone()))
}

fn emit_index_window(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, base_ty: &Type, inner: &Box<Type>) -> Result<(String, Type), String> {
let (idx_val, idx_ty) = emit_expr(ctx, out, &i.index, local_vars, Some(&Type::I64))?;
                 let idx_prom = promote_numeric(ctx, out, &idx_val, &idx_ty, &Type::I64)?;

                 let data_ptr_ptr = format!("%win_ptr_{}", ctx.next_id());
                 let win_storage_ty = base_ty.to_mlir_storage_type(ctx)?;
                 out.push_str(&format!("    {} = llvm.getelementptr {}[0, 0] : (!llvm.ptr) -> !llvm.ptr, {}\n", data_ptr_ptr, base_ptr, win_storage_ty));
                 
                 let data_ptr = format!("%win_data_ptr_{}", ctx.next_id());
                 ctx.emit_load(out, &data_ptr, &data_ptr_ptr, "!llvm.ptr");
                 
                 let elem_ptr = format!("%elem_ptr_{}", ctx.next_id());
                 let inner_storage = inner.to_mlir_storage_type(ctx)?;
                 ctx.emit_gep(out, &elem_ptr, &data_ptr, &idx_prom, &inner_storage);
                 let res = format!("%index_res_{}", ctx.next_id());
                 ctx.emit_load_logical(out, &res, &elem_ptr, inner)?;
                 Ok((res, *inner.clone()))
}

pub fn emit_index(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, _expected: Option<&Type>) -> Result<(String, Type), String> {
    // Try LValue first (Handles Arrays/Windows properly, and Tensors)
    // Try LValue first (Handles Arrays/Windows properly, and Tensors)
    // typo in original code? i.expr is the thing being indexed.
    
    // Correct logic:
    if let Ok((base_ptr, base_ty, kind)) = emit_lvalue(ctx, out, &i.expr, local_vars) {
         match base_ty {
             // : Native Pointer Indexing
             // This replaces the legacy "NativePtr" string-matching logic.
             Type::Pointer { ref element, .. } | Type::Reference(ref element, _) => return emit_index_ptr_ref(ctx, out, i, local_vars, base_ptr.clone(), &base_ty, kind.clone(), element),

             Type::Tensor(ref inner, ref shape) => return emit_index_tensor(ctx, out, i, local_vars, base_ptr.clone(), inner, shape),

             Type::Array(ref inner, _, ref packed) => return emit_index_array(ctx, out, i, local_vars, base_ptr.clone(), &base_ty, inner, packed),

             Type::Owned(ref inner) => return emit_index_owned(ctx, out, i, local_vars, base_ptr.clone(), kind, inner),

             Type::Window(ref inner, _) => return emit_index_window(ctx, out, i, local_vars, base_ptr.clone(), &base_ty, inner),

             _ => {} // Fallback
         }
    }

    // Fallback R-Value (Handles basic pointers and arrays)
    eprintln!("EMIT_INDEX_FALLBACK_RVALUE");
    let (base_val, base_ty) = emit_expr(ctx, out, &i.expr, local_vars, None)?;
    let (idx_val, idx_ty) = emit_expr(ctx, out, &i.index, local_vars, Some(&Type::I64))?;
    let idx_prom = promote_numeric(ctx, out, &idx_val, &idx_ty, &Type::I64)?;

    match base_ty {
        Type::Reference(ref inner, _) | Type::Owned(ref inner) => {
             // Handle Reference(Array) - need to index into array and get element
             if let Type::Array(ref elem_ty, _, _) = inner.as_ref() {
                 // base_val is the pointer to the array
                 let arr_mlir = inner.to_mlir_type(ctx)?;
                 let elem_ptr = format!("%ref_arr_elem_ptr_{}", ctx.next_id());
                 out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", 
                     elem_ptr, base_val, idx_prom, arr_mlir));
                 
                 let res = format!("%ref_arr_index_res_{}", ctx.next_id());
                 ctx.emit_load_logical(out, &res, &elem_ptr, elem_ty.as_ref())?;
                 return Ok((res, *elem_ty.clone()));
             }
             
             // Default path for non-array references
             let ptr = format!("%index_ptr_{}", ctx.next_id());
             let storage_inner = inner.to_mlir_storage_type(ctx)?;
             ctx.emit_gep(out, &ptr, &base_val, &idx_prom, &storage_inner);
             
             let res = format!("%index_res_{}", ctx.next_id());
             ctx.emit_load_logical(out, &res, &ptr, inner)?;
             Ok((res, *inner.clone()))
        }
        // Handle direct Array indexing (e.g., field access returns Array without Reference wrapper)
        Type::Array(ref elem_ty, _, _) => {
             // base_val is a pointer to the array in memory
             let arr_mlir = base_ty.to_mlir_type(ctx)?;
             let elem_ptr = format!("%arr_elem_ptr_{}", ctx.next_id());
             out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", 
                 elem_ptr, base_val, idx_prom, arr_mlir));
             
             let res = format!("%arr_index_res_{}", ctx.next_id());
             ctx.emit_load_logical(out, &res, &elem_ptr, elem_ty.as_ref())?;
             Ok((res, *elem_ty.clone()))
        }
        // : First-Class Pointer Indexing (Fallback Path)
        // This handles Ptr<T> when emit_lvalue didn't catch it
        Type::Pointer { ref element, .. } => {
             // Z3 Bounds Verification Integration
             let func_name = ctx.current_fn_name().clone();
             let info = crate::codegen::verification::ptr_bounds_verifier::PtrBoundsInfo::new(&func_name);
             
             // Extract loop invariant upper bounds or known local array sizes
             // Note: In an advanced version we would query MallocTracker or loop invariants
             // but for now, we rely on the Z3 context having the path conditions mapped.
             let proof_result = crate::codegen::verification::ptr_bounds_verifier::verify_ptr_dynamic_index(ctx.z3_ctx, ctx.z3_solver, &info);
             
             match proof_result {
                 crate::codegen::verification::ptr_bounds_verifier::PtrProofResult::Proven => {
                     *ctx.elided_checks += 1;
                 }
                 _ => {
                     // Since Ptr<T> has no runtime length, we cannot emit a runtime bounds check.
                     // Therefore, to enforce memory safety, we MUST reject unproven pointer indexing at compile time.
                     if !ctx.config.no_verify && !*ctx.is_unsafe_block() {
                         return Err(format!("Unsafe pointer indexing in '{}': Z3 could not prove bounds safety for raw pointer indexing. You must provide explicit bounds constraints via @requires, loop invariants, or wrap the access in an unsafe block.", func_name));
                     }
                 }
             }
             let elem_mlir = element.to_mlir_storage_type(ctx)?;
             let res_ptr = format!("%ptr_idx_{}", ctx.next_id());

             // Emit KeuOS GEP (No indirection, just register offset)
             ctx.emit_gep(out, &res_ptr, &base_val, &idx_prom, &elem_mlir);
             
             // Load the value directly into a scalar register
             let val_res = format!("%val_{}", ctx.next_id());
             ctx.emit_load(out, &val_res, &res_ptr, &elem_mlir);
             
             Ok((val_res, (**element).clone()))
        }
        _ => Err(format!("Index operator not supported on type {:?}", base_ty))
    }
}

#[allow(unused)]
pub fn translate_to_z3<'a, 'ctx>(
    ctx: &mut LoweringContext<'a, 'ctx>, 
    expr: &syn::Expr, 
    local_vars: &HashMap<String, (Type, LocalKind)>,
    // sym_ctx: &SymbolicContext<'a>
) -> Result<crate::z3_shim::ast::Int<'a>, String> {
    match expr {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(li), .. }) => {
            let val = li.base10_parse::<i64>().map_err(|e| e.to_string())?;
            Ok(ctx.mk_int(val))
        }
        syn::Expr::Path(p) => {
            let name = p.path.segments.last().ok_or_else(|| "Empty path segments".to_string())?.ident.to_string();
            // First check local variables for SSA value
            if let Some((_, kind)) = local_vars.get(&name) {
                if let LocalKind::SSA(ssa) = kind {
                    if let Some(z3_val) = ctx.get_symbolic_int(ssa) {
                        return Ok(z3_val);
                    }
                }
            }
            // Fallback to fresh variable
            Ok(ctx.mk_var(&name))
        }
        syn::Expr::Binary(b) => {
            let lhs = translate_to_z3(ctx, &b.left, local_vars)?;
            let rhs = translate_to_z3(ctx, &b.right, local_vars)?;
            match b.op {
                syn::BinOp::Add(_) => Ok(lhs + rhs),
                syn::BinOp::Sub(_) => Ok(lhs - rhs),
                syn::BinOp::Mul(_) => Ok(lhs * rhs),
                syn::BinOp::Div(_) => Ok(lhs / rhs),
                _ => Err(format!("Unsupported symbolic operator: {:?}", b.op)),
            }
        }
        syn::Expr::Paren(p) => translate_to_z3(ctx, &p.expr, local_vars),
        syn::Expr::Field(f) => {
            let base_z3 = translate_to_z3(ctx, &f.base, local_vars)?;
            // Model field access as Z3 uninterpreted function: field(base) → Int
            if let syn::Member::Named(id) = &f.member {
                let field_name = id.to_string();
                let func = crate::z3_shim::FuncDecl::new(
                    ctx.z3_ctx,
                    crate::z3_shim::Symbol::String(format!("field_{}", field_name)),
                    &[&crate::z3_shim::Sort::int(ctx.z3_ctx)],
                    &crate::z3_shim::Sort::int(ctx.z3_ctx),
                );
                let result = func.apply(&[&base_z3]);
                result.as_int().ok_or_else(|| format!("Field access {} did not return Int", field_name))
            } else {
                Err("Unsupported unnamed field access in verification".to_string())
            }
        }
        syn::Expr::Cast(c) => translate_to_z3(ctx, &c.expr, local_vars),
        syn::Expr::Group(g) => translate_to_z3(ctx, &g.expr, local_vars),
        syn::Expr::Unary(u) => {
             let inner = translate_to_z3(ctx, &u.expr, local_vars)?;
             match u.op {
                 syn::UnOp::Neg(_) => Ok(-inner),
                 _ => Err(format!("Unsupported symbolic unary operator: {:?}", u.op)),
             }
        }
        // @pure function calls → Z3 uninterpreted functions
        syn::Expr::Call(call) => {
            // Extract function name from the call expression
            let func_name = if let syn::Expr::Path(p) = &*call.func {
                p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("_")
            } else {
                "unknown_fn".to_string()
            };
            // Translate arguments to Z3
            let mut arg_z3s: Vec<crate::z3_shim::ast::Int> = Vec::new();
            for arg in &call.args {
                arg_z3s.push(translate_to_z3(ctx, arg, local_vars)?);
            }
            // Build Z3 uninterpreted function: func(args...) → Int
            let sorts: Vec<crate::z3_shim::Sort> = arg_z3s.iter().map(|_| crate::z3_shim::Sort::int(ctx.z3_ctx)).collect();
            let sort_refs: Vec<&crate::z3_shim::Sort> = sorts.iter().collect();
            let func = crate::z3_shim::FuncDecl::new(
                ctx.z3_ctx,
                crate::z3_shim::Symbol::String(func_name),
                &sort_refs,
                &crate::z3_shim::Sort::int(ctx.z3_ctx),
            );
            let arg_refs: Vec<&dyn crate::z3_shim::ast::Ast> = arg_z3s.iter().map(|a| a as &dyn crate::z3_shim::ast::Ast).collect();
            let result = func.apply(&arg_refs);
            result.as_int().ok_or_else(|| "Function call did not return Int in Z3".to_string())
        }
        syn::Expr::MethodCall(mc) => {
            // Model method calls as func(receiver, args...) → Int
            let method_name = mc.method.to_string();
            let mut arg_z3s = Vec::new();
            arg_z3s.push(translate_to_z3(ctx, &mc.receiver, local_vars)?);
            for arg in &mc.args {
                arg_z3s.push(translate_to_z3(ctx, arg, local_vars)?);
            }
            let sorts: Vec<crate::z3_shim::Sort> = arg_z3s.iter().map(|_| crate::z3_shim::Sort::int(ctx.z3_ctx)).collect();
            let sort_refs: Vec<&crate::z3_shim::Sort> = sorts.iter().collect();
            let func = crate::z3_shim::FuncDecl::new(
                ctx.z3_ctx,
                crate::z3_shim::Symbol::String(format!("method_{}", method_name)),
                &sort_refs,
                &crate::z3_shim::Sort::int(ctx.z3_ctx),
            );
            let arg_refs: Vec<&dyn crate::z3_shim::ast::Ast> = arg_z3s.iter().map(|a| a as &dyn crate::z3_shim::ast::Ast).collect();
            let result = func.apply(&arg_refs);
            result.as_int().ok_or_else(|| format!("Method call {} did not return Int in Z3", method_name))
        }
        _ => {
            // Treat unknown complex expressions as fresh symbolic variables
            Ok(ctx.mk_var("unknown"))
        }
    }
}

pub fn translate_bool_to_z3<'a, 'ctx>(
    ctx: &mut LoweringContext<'a, 'ctx>, 
    expr: &syn::Expr, 
    local_vars: &HashMap<String, (Type, LocalKind)>,
    sym_ctx: &crate::codegen::verification::SymbolicContext<'a>
) -> Result<crate::z3_shim::ast::Bool<'a>, String> {
    use crate::z3_shim::ast::Ast;
    match expr {
        syn::Expr::Binary(b) => {
            match b.op {
                syn::BinOp::Eq(_) | syn::BinOp::Ne(_) | syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_) => {
                    let lhs = translate_to_z3(ctx, &b.left, local_vars)?;
                    let rhs = translate_to_z3(ctx, &b.right, local_vars)?;
                    match b.op {
                        syn::BinOp::Eq(_) => Ok(lhs._eq(&rhs)),
                        syn::BinOp::Ne(_) => Ok(lhs._eq(&rhs).not()),
                        syn::BinOp::Lt(_) => Ok(lhs.lt(&rhs)),
                        syn::BinOp::Le(_) => Ok(lhs.le(&rhs)),
                        syn::BinOp::Gt(_) => Ok(lhs.gt(&rhs)),
                        syn::BinOp::Ge(_) => Ok(lhs.ge(&rhs)),
                        _ => unreachable!(),
                    }
                }
                syn::BinOp::And(_) => {
                    let bl = translate_bool_to_z3(ctx, &b.left, local_vars, sym_ctx)?;
                    let br = translate_bool_to_z3(ctx, &b.right, local_vars, sym_ctx)?;
                    Ok(crate::z3_shim::ast::Bool::and(ctx.z3_ctx, &[&bl, &br]))
                }
                syn::BinOp::Or(_) => {
                    let bl = translate_bool_to_z3(ctx, &b.left, local_vars, sym_ctx)?;
                    let br = translate_bool_to_z3(ctx, &b.right, local_vars, sym_ctx)?;
                    Ok(crate::z3_shim::ast::Bool::or(ctx.z3_ctx, &[&bl, &br]))
                }
                _ => Err(format!("Unsupported symbolic boolean operator: {:?}", b.op)),
            }
        }
        syn::Expr::Unary(u) => {
             match u.op {
                 syn::UnOp::Not(_) => {
                      let inner = translate_bool_to_z3(ctx, &u.expr, local_vars, sym_ctx)?;
                      Ok(inner.not())
                 },
                 _ => Err("Arithmetic unary op in boolean context".to_string()),
             }
        }
        syn::Expr::Group(g) => translate_bool_to_z3(ctx, &g.expr, local_vars, sym_ctx),
        syn::Expr::Paren(p) => translate_bool_to_z3(ctx, &p.expr, local_vars, sym_ctx),
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. }) => {
            Ok(crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, b.value))
        }
        // @pure function calls returning bool → Z3 uninterpreted Bool functions
        syn::Expr::Call(call) => {
            let func_name = if let syn::Expr::Path(p) = &*call.func {
                p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("_")
            } else {
                "unknown_bool_fn".to_string()
            };
            let mut arg_z3s: Vec<crate::z3_shim::ast::Int<'a>> = Vec::new();
            for arg in &call.args {
                arg_z3s.push(translate_to_z3(ctx, arg, local_vars)?);
            }
            let sorts: Vec<crate::z3_shim::Sort> = arg_z3s.iter().map(|_| crate::z3_shim::Sort::int(ctx.z3_ctx)).collect();
            let sort_refs: Vec<&crate::z3_shim::Sort> = sorts.iter().collect();
            let func = crate::z3_shim::FuncDecl::new(
                ctx.z3_ctx,
                crate::z3_shim::Symbol::String(func_name),
                &sort_refs,
                &crate::z3_shim::Sort::bool(ctx.z3_ctx),
            );
            let arg_refs: Vec<&dyn crate::z3_shim::ast::Ast> = arg_z3s.iter().map(|a| a as &dyn crate::z3_shim::ast::Ast).collect();
            let result = func.apply(&arg_refs);
            result.as_bool().ok_or_else(|| "Function call did not return Bool in Z3".to_string())
        }
        syn::Expr::MethodCall(mc) => {
            let method_name = mc.method.to_string();
            let mut arg_z3s: Vec<crate::z3_shim::ast::Int<'a>> = Vec::new();
            arg_z3s.push(translate_to_z3(ctx, &mc.receiver, local_vars)?);
            for arg in &mc.args {
                arg_z3s.push(translate_to_z3(ctx, arg, local_vars)?);
            }
            let sorts: Vec<crate::z3_shim::Sort> = arg_z3s.iter().map(|_| crate::z3_shim::Sort::int(ctx.z3_ctx)).collect();
            let sort_refs: Vec<&crate::z3_shim::Sort> = sorts.iter().collect();
            let func = crate::z3_shim::FuncDecl::new(
                ctx.z3_ctx,
                crate::z3_shim::Symbol::String(format!("method_{}", method_name)),
                &sort_refs,
                &crate::z3_shim::Sort::bool(ctx.z3_ctx),
            );
            let arg_refs: Vec<&dyn crate::z3_shim::ast::Ast> = arg_z3s.iter().map(|a| a as &dyn crate::z3_shim::ast::Ast).collect();
            let result = func.apply(&arg_refs);
            result.as_bool().ok_or_else(|| format!("Method call {} did not return Bool in Z3", method_name))
        }
        _ => Err("Unsupported symbolic boolean expression".to_string()),
    }
}
