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
/// Extract the compile-time element count from an array type, unwrapping
/// references and pointers. Returns None for non-array types.
fn extract_array_length(ty: &Type) -> Option<i64> {
    match ty {
        Type::Array(_, len, _) => Some(*len as i64),
        Type::Reference(inner, _) | Type::Pointer { element: inner, .. } => extract_array_length(inner),
        _ => None,
    }
}

/// Prove Ptr<T> bounds safety using loop invariants and requires clauses.
/// Returns Ok(true) if Z3 proved the index is within bounds, Ok(false) if not.
fn prove_ptr_bounds_loop_aware(
    ctx: &mut LoweringContext,
    index: &syn::Expr,
    local_vars: &HashMap<String, (Type, LocalKind)>,
    _func_name: &str,
) -> Result<bool, String> {
    let loop_stack = crate::codegen::verification::loop_bounds::get_loop_bound_stack();
    let req_params = crate::codegen::verification::loop_bounds::get_requires_params();

    // Resolve a bound name to its Z3 Int
    let resolve_z3 = |name: &str| -> Option<crate::z3_shim::ast::Int<'_>> {
        if let Some((_, LocalKind::SSA(ssa))) = local_vars.get(name) {
            ctx.symbolic_tracker.get(ssa).cloned()
        } else { ctx.symbolic_tracker.get(name).cloned() }
    };

    // Candidate bounds: individual loop bounds + pairwise products + requires params
    let mut bounds: Vec<crate::z3_shim::ast::Int<'_>> = Vec::new();
    for ub_name in &loop_stack {
        if let Some(z3_ub) = resolve_z3(ub_name) { bounds.push(z3_ub); }
    }
    for a in 0..loop_stack.len() {
        for b in (a + 1)..loop_stack.len() {
            if let (Some(za), Some(zb)) = (resolve_z3(&loop_stack[a]), resolve_z3(&loop_stack[b])) {
                bounds.push(za * zb);
            }
        }
    }
    for param in &req_params {
        if let Some(z3_ub) = resolve_z3(param) { bounds.push(z3_ub); }
    }
    if bounds.is_empty() { return Ok(false); }

    // Translate the index expression to a Z3 integer
    let z3_idx: Option<crate::z3_shim::ast::Int<'_>> = match index {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(li), .. }) => {
            li.base10_parse::<i64>().ok().map(|v| ctx.mk_int(v))
        }
        syn::Expr::Path(p) => {
            p.path.get_ident().and_then(|ident| {
                let name = ident.to_string();
                let ssa = local_vars.get(&name)
                    .and_then(|(_, kind)| if let LocalKind::SSA(s) = kind { Some(s.clone()) } else { None });
                ctx.symbolic_tracker.get(ssa.as_deref().unwrap_or(&name)).cloned()
            })
        }
        _ => translate_to_z3(ctx, index, local_vars).ok(),
    };
    let Some(z3_idx) = z3_idx else { return Ok(false) };

    // Check each candidate bound
    for z3_ub in &bounds {
        *ctx.total_checks += 1;
        ctx.z3_solver.push();
        ctx.z3_solver.assert(&z3_idx.ge(z3_ub));
        let proven = ctx.z3_solver.check() == crate::z3_shim::SatResult::Unsat;
        ctx.z3_solver.pop(1);
        if proven { *ctx.elided_checks += 1; return Ok(true); }
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)] // REASON: all 8 params independently meaningful; bundling would obscure intent
fn emit_index_ptr_ref(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, base_ty: &Type, kind: LValueKind, element: &Type) -> Result<(String, Type), String> {
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
                 // Extract array length from the type to prove bounds safety.
                 // &[T; N] → N elements known at compile time.
                 let func_name = ctx.current_fn_name().clone();
                 let array_len = extract_array_length(base_ty);
                 let mut need_runtime_check: Option<i64> = None;
                 if let Some(n) = array_len {
                     let mut proven = false;
                     // Fast path: if the index is a for-loop induction variable
                     // tracked in the Z3 solver, assert the violation directly.
                     if let syn::Expr::Path(p) = &*i.index {
                         if let Some(ident) = p.path.get_ident() {
                             let src_name = ident.to_string();
                             let ssa_name = local_vars.get(&src_name)
                                 .and_then(|(_, kind)| {
                                     if let LocalKind::SSA(ssa) = kind { Some(ssa.clone()) }
                                     else { None }
                                 });
                             let lookup_name = ssa_name.as_deref().unwrap_or(&src_name);
                             if let Some(z3_idx) = ctx.symbolic_tracker.get(lookup_name).cloned() {
                                 ctx.z3_solver.push();
                                 let alloc = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, n);
                                 ctx.z3_solver.assert(&z3_idx.ge(&alloc));
                                 proven = ctx.z3_solver.check() == crate::z3_shim::SatResult::Unsat;
                                 ctx.z3_solver.pop(1);
                             }
                         }
                     }
                     if proven {
                         *ctx.elided_checks += 1;
                     } else {
                         need_runtime_check = Some(n);
                     }
                 } else if !ctx.config.no_verify && !*ctx.is_unsafe_block() {
                     let proven = prove_ptr_bounds_loop_aware(ctx, &i.index, local_vars, &func_name)?;
                     if !proven {
                         let info = crate::codegen::verification::ptr_bounds_verifier::PtrBoundsInfo::new(&func_name);
                         let proof_result = crate::codegen::verification::ptr_bounds_verifier::verify_ptr_dynamic_index(ctx.z3_ctx, ctx.z3_solver, &info);
                         if proof_result != crate::codegen::verification::ptr_bounds_verifier::PtrProofResult::Proven {
                             return Err(format!("Unsafe pointer indexing in '{}': Z3 could not prove bounds safety for raw pointer indexing. You must provide explicit bounds constraints via @requires, loop invariants, or wrap the access in an unsafe block.", func_name));
                         }
                     }
                 }

                 // Emit the index computation
                 let idx_expr = &*i.index;
                 let (raw_idx_val, raw_idx_ty) = emit_expr(ctx, out, idx_expr, local_vars, None)?;
                 let idx_final = if raw_idx_ty == Type::I64 {
                     raw_idx_val
                 } else {
                     promote_numeric(ctx, out, &raw_idx_val, &raw_idx_ty, &Type::I64)?
                 };

                 // Emit runtime bounds check if Z3 couldn't prove the index is safe
                 if let Some(n) = need_runtime_check {
                     let ok = format!("%bounds_ok_{}", ctx.next_id());
                     let upper = format!("%upper_{}", ctx.next_id());
                     out.push_str(&format!("    {} = arith.constant {} : i64\n", upper, n));
                     out.push_str(&format!("    {} = arith.cmpi ult, {}, {} : i64\n", ok, idx_final, upper));
                     out.push_str(&format!("    scf.if {} {{\n", ok));
                     out.push_str("      scf.yield\n");
                     out.push_str("    } else {\n");
                     out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
                     out.push_str("    }\n");
                 }
                                // LValueKind: Ptr/Local → load alloca; SSA & Reference → use pointer directly.
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
                  if let Type::Array(ref arr_elem, _, _) = *element {
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
                 
                 Ok((load_res, (*element).clone()))
}
fn emit_index_tensor(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, inner: &Type, shape: &[usize]) -> Result<(String, Type), String> {
// Tensors are memref types (SSA values from memref.alloc)
                 // For SSA, base_ptr is already the memref value
                 // For Ptr/Local, a memref.load from a ptr would be needed, but tensors should always be SSA
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
                         // But the type of the aggregate logic is needed.
                         // Usually type is inferred or the struct type is passed.
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
                 
                 // Z3 Bounds Check Elision: proves index in bounds at compile time; fallback: runtime check.
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
                 // Tensors are allocated as memref<DxMxT>, so memref ops must be used, not llvm.gep
                 
                 let elem_mlir = inner.to_mlir_storage_type(ctx)?;
                 
                 // Build the memref type string: memref<D0xD1x...xDnxElemType>
                 let shape_str = shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
                 let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
                 
                 // Build index list for memref.load: [%i0, %i1, ...]
                 let indices_str = indices.join(", ");
                 
                 let res = format!("%tensor_val_{}", ctx.next_id());
                 out.push_str(&format!("    {} = memref.load {}[{}] : {}\n", 
                     res, tensor_ptr, indices_str, memref_ty));
                 
                 Ok((res, (*inner).clone()))
}
#[allow(clippy::too_many_arguments)] // REASON: all 8 params independently meaningful; bundling would obscure intent
fn emit_index_array(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, base_ty: &Type, inner: &Type, packed: &bool) -> Result<(String, Type), String> {
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
                  ctx.emit_load_logical(out, &res, &elem_ptr, inner)?;
                  Ok((res, (*inner).clone()))
}

fn emit_index_owned(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, kind: LValueKind, inner: &Type) -> Result<(String, Type), String> {
let (idx_val, idx_ty) = emit_expr(ctx, out, &i.index, local_vars, Some(&Type::I64))?;
                 let idx_prom = promote_numeric(ctx, out, &idx_val, &idx_ty, &Type::I64)?;

                 let loaded_ptr = if kind == LValueKind::SSA {
                     base_ptr
                 } else {
                     let res = format!("%loaded_base_{}", ctx.next_id());
                     ctx.emit_load(out, &res, &base_ptr, "!llvm.ptr");
                     res
                 };
                 
                 if let Type::Array(ref elem_ty, _, packed) = inner {
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
                 Ok((res, (*inner).clone()))
}

fn emit_index_window(ctx: &mut LoweringContext, out: &mut String, i: &syn::ExprIndex, local_vars: &mut HashMap<String, (Type, LocalKind)>, base_ptr: String, base_ty: &Type, inner: &Type) -> Result<(String, Type), String> {
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
                 Ok((res, (*inner).clone()))
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
             let func_name = ctx.current_fn_name().clone();
             if !ctx.config.no_verify && !*ctx.is_unsafe_block() {
                 let proven = prove_ptr_bounds_loop_aware(ctx, &i.index, local_vars, &func_name)?;
                 if !proven {
                     let info = crate::codegen::verification::ptr_bounds_verifier::PtrBoundsInfo::new(&func_name);
                     let proof_result = crate::codegen::verification::ptr_bounds_verifier::verify_ptr_dynamic_index(ctx.z3_ctx, ctx.z3_solver, &info);
                     if proof_result != crate::codegen::verification::ptr_bounds_verifier::PtrProofResult::Proven {
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
/// Resolve a bound name to an i64 if it's a compile-time constant.
fn resolve_bound_as_i64(
    name: &str,
    local_vars: &HashMap<String, (Type, LocalKind)>,
) -> Option<i64> {
    // Check if the bound is a literal integer in the AST context.
    // We check local_vars for a constant SSA mapping.
    None // Stub: always return None for non-constant bounds
}

/// Resolve a loop bound name to its Z3 Int via local_vars→symbolic_tracker.
fn resolve_bound<'a>(
    name: &str,
    local_vars: &HashMap<String, (Type, LocalKind)>,
    ctx: &LoweringContext<'a, '_>,
) -> Option<crate::z3_shim::ast::Int<'a>> {
    if let Some((_, LocalKind::SSA(ssa))) = local_vars.get(name) {
        ctx.symbolic_tracker.get(ssa).cloned()
    } else {
        ctx.symbolic_tracker.get(name).cloned()
    }
}

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
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Float(lf), .. }) => {
            // Float literals are truncated to integers for Z3 comparison.
            // This handles zero-checks (0.0 != 0) and basic comparisons.
            // Full Real support requires Z3Numeric (see verification/numeric.rs).
            let val = lf.base10_parse::<f64>().map_err(|e| e.to_string())?;
            Ok(ctx.mk_int(val as i64))
        }
        syn::Expr::Path(p) => {
            let name = p.path.segments.last().ok_or_else(|| "Empty path segments".to_string())?.ident.to_string();
            // First check local variables for SSA value
            if let Some((_, LocalKind::SSA(ssa))) = local_vars.get(&name) {
                    if let Some(z3_val) = ctx.get_symbolic_int(ssa) {
                                                return Ok(z3_val);
                    }
            }
            // Check symbolic tracker by original name (function params, etc.)
            if let Some(_z3_val) = ctx.symbolic_tracker.get(&name).cloned() {
                                return Ok(_z3_val);
            }
            // Fallback to fresh variable — store it so subsequent lookups find it
                        let fresh = ctx.mk_var(&name);
            ctx.symbolic_tracker.insert(name.clone(), fresh.clone());
            Ok(fresh)
        }
        syn::Expr::Binary(b) => {
            let lhs = translate_to_z3(ctx, &b.left, local_vars)?;
            let rhs = translate_to_z3(ctx, &b.right, local_vars)?;
            match b.op {
                syn::BinOp::Add(_) => Ok(lhs + rhs),
                syn::BinOp::Sub(_) => Ok(lhs - rhs),
                syn::BinOp::Mul(_) => Ok(lhs * rhs),
                syn::BinOp::Div(_) => Ok(lhs / rhs),
                syn::BinOp::BitAnd(_) | syn::BinOp::BitOr(_) | syn::BinOp::BitXor(_)
                | syn::BinOp::Shl(_) | syn::BinOp::Shr(_) => crate::codegen::expr::z3_translate::translate_bitwise_op(ctx, &lhs, &rhs, &b.op),
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
        // Array indexing: arr[i] → versioned uninterpreted function arr_vN(i)
        // Each indexed store bumps the version; reads use the latest version.
        syn::Expr::Index(idx) => {
            let base_name = if let syn::Expr::Path(p) = &*idx.expr {
                p.path.get_ident().map(|i| i.to_string()).unwrap_or_else(|| "unknown_arr".to_string())
            } else {
                "unknown_arr".to_string()
            };
            let index_z3 = translate_to_z3(ctx, &idx.index, local_vars)?;
            let ver = crate::codegen::verification::array_tracker::get_version(&base_name);
            // Lazy store emission: update assertion + bounded frame axiom
            let stores = crate::codegen::verification::array_tracker::get_stores(&base_name);
            let applied = crate::codegen::verification::array_tracker::stores_applied(&base_name);
            if applied < stores.len() {
                crate::codegen::verification::array_tracker::mark_stores_applied(&base_name, stores.len());
                let mut cur_ver = applied;
                let loop_bounds = crate::codegen::verification::loop_bounds::get_loop_bound_stack();
                let frame_bound = loop_bounds.last();
                for store in &stores[applied..] {
                    let old_ver = cur_ver;
                    let new_ver = cur_ver + 1;
                    cur_ver = new_ver;
                    let old_name = format!("{}_v{}", base_name, old_ver);
                    let new_name = format!("{}_v{}", base_name, new_ver);
                    let int_sort = crate::z3_shim::Sort::int(ctx.z3_ctx);
                    let old_func = crate::z3_shim::FuncDecl::new(ctx.z3_ctx, crate::z3_shim::Symbol::String(old_name), &[&int_sort], &int_sort);
                    let new_func = crate::z3_shim::FuncDecl::new(ctx.z3_ctx, crate::z3_shim::Symbol::String(new_name), &[&int_sort], &int_sort);
                    if let (Ok(s_idx), Ok(s_val)) = (
                        translate_to_z3(ctx, &store.index_expr, local_vars),
                        translate_to_z3(ctx, &store.value_expr, local_vars),
                    ) {
                        use crate::z3_shim::ast::Ast;
                        let new_at_idx = new_func.apply(&[&s_idx]);
                        if let Some(new_int) = new_at_idx.as_int() {
                            ctx.z3_solver.assert(&new_int._eq(&s_val));
                        }
                        // Frame axiom: try concrete expansion first, then ForAll
                        if let Some(bound_val) = crate::codegen::verification::loop_bounds::get_concrete_bound() {
                            // Expand to concrete assertions for each index in 0..bound
                            for k_val in 0..bound_val {
                                let k_z3 = crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, k_val);
                                let k_eq_store = s_idx._eq(&k_z3);
                                ctx.z3_solver.push();
                                ctx.z3_solver.assert(&k_eq_store);
                                let k_not_store = ctx.z3_solver.check() == crate::z3_shim::SatResult::Unsat;
                                ctx.z3_solver.pop(1);
                                if !k_not_store { continue; }
                                let old_at_k = old_func.apply(&[&k_z3]);
                                let new_at_k = new_func.apply(&[&k_z3]);
                                if let (Some(old_int), Some(new_int)) = (old_at_k.as_int(), new_at_k.as_int()) {
                                    ctx.z3_solver.assert(&new_int._eq(&old_int));
                                }
                            }
                        } else if let Some(bound_name) = frame_bound {
                            if let Some(z3_bound) = resolve_bound(bound_name, local_vars, ctx) {
                                let k_name = format!("k_fr_{}", ctx.next_id());
                                let k_fr = crate::z3_shim::ast::Int::new_const(ctx.z3_ctx, k_name.as_str());
                                let k_in_range = crate::z3_shim::ast::Bool::and(ctx.z3_ctx, &[
                                    &k_fr.ge(&crate::z3_shim::ast::Int::from_i64(ctx.z3_ctx, 0)),
                                    &k_fr.lt(&z3_bound),
                                ]);
                                let k_ne_i = k_fr._eq(&s_idx).not();
                                let old_at_k = old_func.apply(&[&k_fr]);
                                let new_at_k = new_func.apply(&[&k_fr]);
                                if let (Some(old_int), Some(new_int)) = (old_at_k.as_int(), new_at_k.as_int()) {
                                    let frame_eq = new_int._eq(&old_int);
                                    let frame_body = crate::z3_shim::ast::Bool::or(ctx.z3_ctx, &[&k_in_range.not(), &k_ne_i.not(), &frame_eq]);
                                    ctx.z3_solver.assert(&crate::z3_shim::ast::forall_const(ctx.z3_ctx, &[&k_fr], &[], &frame_body));
                                }
                            }
                        }
                    }
                }
            }
            let func_name = format!("{}_v{}", base_name, ver);
            let sym = crate::z3_shim::Symbol::String(func_name);
            let domain = &[&crate::z3_shim::Sort::int(ctx.z3_ctx)];
            let range = &crate::z3_shim::Sort::int(ctx.z3_ctx);
            let func = crate::z3_shim::FuncDecl::new(ctx.z3_ctx, sym, domain, range);
            let result = func.apply(&[&index_z3]);
            result.as_int().ok_or_else(|| format!("Array access {}[idx] did not return Int", base_name))
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
        // Function calls in contracts → Z3 uninterpreted functions
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

/// Translate a Salt expression to a Z3 String value.
/// Handles string literals (compile-time constants) and variable references.
pub fn translate_string_to_z3<'a, 'ctx>(
    ctx: &mut LoweringContext<'a, 'ctx>,
    expr: &syn::Expr,
    _local_vars: &HashMap<String, (Type, LocalKind)>,
) -> Result<crate::z3_shim::ast::String<'a>, String> {
    match expr {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => {
            crate::z3_shim::ast::String::from_str(ctx.z3_ctx, &s.value())
                .map_err(|e| format!("invalid string literal: {}", e))
        }
        syn::Expr::Path(p) => {
            let name = p.path.segments.last()
                .ok_or_else(|| "Empty path in string context".to_string())?
                .ident.to_string();
            Ok(crate::z3_shim::ast::String::new_const(ctx.z3_ctx, name))
        }
        _ => Err("Expected a string expression (literal or variable)".to_string()),
    }
}

/// Translate a symbolic exists expression: __z3_exists(var_name, lo, hi, body)
fn translate_z3_exists<'a, 'ctx>(
    ctx: &mut LoweringContext<'a, 'ctx>,
    call: syn::ExprCall,
    local_vars: &HashMap<String, (Type, LocalKind)>,
    sym_ctx: &crate::codegen::verification::SymbolicContext<'a>,
) -> Result<crate::z3_shim::ast::Bool<'a>, String> {
    let args: Vec<&syn::Expr> = call.args.iter().collect();
    let var_name = match args[0] { syn::Expr::Lit(l) => match &l.lit { syn::Lit::Str(s) => s.value(), _ => return Err("exists: arg 0 must be string".into()) }, _ => return Err("exists: arg 0 must be string".into()) };
    let lo = args[1]; let hi = args[2]; let body = args[3];
    let z3_var = ctx.mk_var(&var_name);
    let mut body_vars = local_vars.clone();
    body_vars.insert(var_name.clone(), (Type::I64, crate::codegen::context::LocalKind::SSA(var_name.clone())));
    let old_val = ctx.symbolic_tracker.get(&var_name).cloned();
    ctx.symbolic_tracker.insert(var_name.clone(), z3_var.clone());
    let z3_lo = translate_to_z3(ctx, lo, &body_vars)?;
    let z3_hi = translate_to_z3(ctx, hi, &body_vars)?;
    let z3_body = translate_bool_to_z3(ctx, body, &body_vars, sym_ctx)?;
    if let Some(old) = old_val { ctx.symbolic_tracker.insert(var_name.clone(), old); } else { ctx.symbolic_tracker.remove(&var_name); }
    // Check empty range: exists k in empty → false
    let check_k = ctx.mk_var("_ec");
    ctx.z3_solver.push();
    ctx.z3_solver.assert(&check_k.ge(&z3_lo)); ctx.z3_solver.assert(&check_k.lt(&z3_hi));
    let range_empty = ctx.z3_solver.check() == crate::z3_shim::SatResult::Unsat;
    ctx.z3_solver.pop(1);
    if range_empty { return Ok(crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, false)); }
    // exists var. (lo <= var < hi) && body
    let ge = z3_var.ge(&z3_lo); let lt = z3_var.lt(&z3_hi);
    let range = crate::z3_shim::ast::Bool::and(ctx.z3_ctx, &[&ge, &lt]);
    let conjunction = crate::z3_shim::ast::Bool::and(ctx.z3_ctx, &[&range, &z3_body]);
    let bound: &dyn crate::z3_shim::ast::Ast = &z3_var;
    Ok(crate::z3_shim::ast::exists_const(ctx.z3_ctx, &[bound], &[], &conjunction))
}

/// Translate a symbolic forall expression: __z3_forall(var_name, lo, hi, body)
fn translate_z3_forall<'a, 'ctx>(
    ctx: &mut LoweringContext<'a, 'ctx>,
    call: syn::ExprCall,
    local_vars: &HashMap<String, (Type, LocalKind)>,
    sym_ctx: &crate::codegen::verification::SymbolicContext<'a>,
) -> Result<crate::z3_shim::ast::Bool<'a>, String> {
    let args: Vec<&syn::Expr> = call.args.iter().collect();
    let var_name = match args[0] {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Str(s) => s.value(),
            _ => return Err("forall: first arg must be a string literal".to_string()),
        },
        _ => return Err("forall: first arg must be a string literal".to_string()),
    };
    let lo = args[1];
    let hi = args[2];
    let body = args[3];

    let z3_var = ctx.mk_var(&var_name);
    let mut body_vars = local_vars.clone();
    body_vars.insert(var_name.clone(), (Type::I64, crate::codegen::context::LocalKind::SSA(var_name.clone())));

    let old_val = ctx.symbolic_tracker.get(&var_name).cloned();
    ctx.symbolic_tracker.insert(var_name.clone(), z3_var.clone());

    // Translate lo and hi to Z3 Ints for range constraints
    let z3_lo = translate_to_z3(ctx, lo, &body_vars)?;
    let z3_hi = translate_to_z3(ctx, hi, &body_vars)?;

    // Translate the body
    let z3_body = translate_bool_to_z3(ctx, body, &body_vars, sym_ctx)?;

    // Restore the symbolic tracker
    if let Some(old) = old_val {
        ctx.symbolic_tracker.insert(var_name.clone(), old);
    } else {
        ctx.symbolic_tracker.remove(&var_name);
    }

    // Check if the range is provably empty (e.g., 0..0 with i=1)
    // If so, the forall is vacuously true — skip the quantifier.
    let check_k = ctx.mk_var("_fc");
    ctx.z3_solver.push();
    ctx.z3_solver.assert(&check_k.ge(&z3_lo));
    ctx.z3_solver.assert(&check_k.lt(&z3_hi));
    let range_empty = ctx.z3_solver.check() == crate::z3_shim::SatResult::Unsat;
    ctx.z3_solver.pop(1);
    if range_empty {
        return Ok(crate::z3_shim::ast::Bool::from_bool(ctx.z3_ctx, true));
    }

    // Emit Z3 ForAll: forall var. (lo <= var < hi) => body
    let ge = z3_var.ge(&z3_lo);
    let lt = z3_var.lt(&z3_hi);
    let range_holds = crate::z3_shim::ast::Bool::and(ctx.z3_ctx, &[&ge, &lt]);
    let implication = crate::z3_shim::ast::Bool::or(ctx.z3_ctx, &[&range_holds.not(), &z3_body]);
    let bound: &dyn crate::z3_shim::ast::Ast = &z3_var;
    Ok(crate::z3_shim::ast::forall_const(
        ctx.z3_ctx,
        &[bound],
        &[],
        &implication,
    ))
}

#[allow(clippy::only_used_in_recursion)] // pub API: params passed in recursive calls
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
                    // Float operands: use Real (exact rational) comparison instead of Int truncation
                    if crate::codegen::expr::z3_translate::is_float_expr(&b.left, local_vars)
                        || crate::codegen::expr::z3_translate::is_float_expr(&b.right, local_vars)
                    {
                        let lhs = crate::codegen::expr::z3_translate::translate_real_to_z3(ctx, &b.left, local_vars)?;
                        let rhs = crate::codegen::expr::z3_translate::translate_real_to_z3(ctx, &b.right, local_vars)?;
                        return Ok(match b.op {
                            syn::BinOp::Eq(_) => lhs._eq(&rhs), syn::BinOp::Ne(_) => lhs._eq(&rhs).not(),
                            syn::BinOp::Lt(_) => lhs.lt(&rhs), syn::BinOp::Le(_) => lhs.le(&rhs),
                            syn::BinOp::Gt(_) => lhs.gt(&rhs), syn::BinOp::Ge(_) => lhs.ge(&rhs),
                            _ => unreachable!(),
                        });
                    }
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
        // __z3_forall(var_name, lo, hi, body) — symbolic forall quantifier
        syn::Expr::Call(call) => {
            // Check for symbolic forall before general function call handling
            if let syn::Expr::Path(p) = &*call.func {
                let func_name = p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("_");
                if func_name == "__z3_forall" && call.args.len() == 4 {
                    return translate_z3_forall(ctx, call.clone(), local_vars, sym_ctx);
                }
                if func_name == "__z3_exists" && call.args.len() == 4 {
                    return translate_z3_exists(ctx, call.clone(), local_vars, sym_ctx);
                }
            }

            // General function calls returning bool in contracts
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

            // String content operations — Z3-str
            if method_name == "contains" || method_name == "starts_with" || method_name == "ends_with" {
                let receiver = translate_string_to_z3(ctx, &mc.receiver, local_vars)?;
                let arg = translate_string_to_z3(ctx, &mc.args[0], local_vars)?;
                return Ok(match method_name.as_str() {
                    "contains" => receiver.contains(&arg),
                    "starts_with" => receiver.prefix(&arg),
                    "ends_with" => receiver.suffix(&arg),
                    _ => unreachable!(),
                });
            }
            if method_name == "matches" {
                let receiver = translate_string_to_z3(ctx, &mc.receiver, local_vars)?;
                let pattern = if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &mc.args[0] {
                    s.value()
                } else {
                    return Err("regex pattern must be a string literal".to_string());
                };
                let regex = crate::z3_shim::ast::Regexp::literal(ctx.z3_ctx, &pattern);
                return Ok(receiver.regex_matches(&regex));
            }

            // Generic: uninterpreted boolean function
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
