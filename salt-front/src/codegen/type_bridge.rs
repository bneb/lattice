use crate::types::{Type, TypeKey};
use crate::codegen::context::LoweringContext;
use crate::registry::{StructInfo, EnumInfo};
use crate::evaluator::ConstValue;
use std::collections::HashMap;
pub use super::type_casts::cast_numeric;

pub use crate::codegen::types::numeric::get_numeric_idx;
pub use crate::codegen::types::numeric::PromotionTable;
pub use crate::codegen::types::numeric::PROMOTION_OPS;

pub use crate::codegen::types::numeric::get_arith_op;
pub use crate::codegen::types::numeric::get_comparison_pred;

pub fn promote_numeric(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<String, String> {    
    if from == to { return Ok(var.to_string()); }

    if let Some(res) = promote_numeric_linear(ctx, out, var, from, to)? {
        return Ok(res);
    }

    if from.is_integer() && to.k_is_ptr_type() {
        return Err(format!(
            "KeuOS Type Error: Cannot promote integer {:?} to pointer {:?}. var={} - This indicates Context Contamination in the loop engine.", 
            from, to, var
        ));
    }

    promote_numeric_cast(ctx, out, var, from, to)
}

fn promote_numeric_linear(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<Option<String>, String> {
    if let Type::Owned(inner) = to {
        if **inner == *from { 
            let temp_ptr = format!("%auto_box_{}", ctx.next_id());
            let mlir_ty = inner.to_mlir_storage_type(ctx).map_err(|e| format!("Failed to get storage type for auto-box: {}", e))?;
            ctx.emit_alloca(out, &temp_ptr, &mlir_ty);
            ctx.emit_store(out, var, &temp_ptr, &mlir_ty);
            return Ok(Some(temp_ptr));
        }
    }
    if let Type::Reference(inner, _) = to {
         if inner.structural_eq(from) {
             let temp_ptr = format!("%auto_ref_{}", ctx.next_id());
             let mlir_ty = from.to_mlir_storage_type(ctx).map_err(|e| format!("Auto-ref storage type error: {}", e))?;
             ctx.emit_alloca(out, &temp_ptr, &mlir_ty);
             ctx.emit_store(out, var, &temp_ptr, &mlir_ty);
             return Ok(Some(temp_ptr));
         }
    }
    if let Type::Owned(inner) = from {
        if **inner == *to { 
            let val_res = format!("%auto_unbox_{}", ctx.next_id());
            let mlir_ty = to.to_mlir_storage_type(ctx).map_err(|e| format!("Failed to get storage type for auto-unbox: {}", e))?;
            ctx.emit_load(out, &val_res, var, &mlir_ty);
            return Ok(Some(val_res));
        }
    }
    if from.structural_eq(to) {
        return Ok(Some(var.to_string()));
    }
    
    match (from, to) {
        (Type::Struct(n1), Type::Concrete(n2, _)) | (Type::Concrete(n2, _), Type::Struct(n1)) => {
            if Type::base_names_equal(n1, n2) { return Ok(Some(var.to_string())); }
        },
        (Type::Concrete(n1, args1), Type::Concrete(n2, args2)) => {
            if Type::base_names_equal(n1, n2) && args1.len() == args2.len() { return Ok(Some(var.to_string())); }
        },
        _ => {}
    }

    if matches!(from, Type::Fn(_, _)) && matches!(to, Type::I64 | Type::U64) {
        let res = format!("%fn_to_int_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
        return Ok(Some(res));
    }

    let is_stringview_from = match from {
        Type::Struct(name) | Type::Concrete(name, _) => name.contains("StringView"),
        _ => false,
    };
    if is_stringview_from && (to.k_is_ptr_type() || matches!(to, Type::Reference(..))) {
        let res = format!("%sv_extract_ptr_{}", ctx.next_id());
        let sv_mlir = from.to_mlir_type(ctx).unwrap_or("!llvm.struct<(ptr, i64)>".to_string());
        out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", res, var, sv_mlir));
        return Ok(Some(res));
    }
    Ok(None)
}

fn promote_numeric_cast(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<String, String> {
    let res = format!("%prom_{}", ctx.next_id());
    let mut emit = |op: &str, src_ty: &str, dst_ty: &str| {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, var, src_ty, dst_ty));
    };

    match (from, to) {
        (Type::Never, _) => {
             let dst_ty_mlir = to.to_mlir_type(ctx)?;
             out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", res, dst_ty_mlir));
             return Ok(res);
        },
        (Type::I8, Type::U8) | (Type::U8, Type::I8) | (Type::I8, Type::I8) | (Type::U8, Type::U8) => return Ok(var.to_string()),
        (Type::I16, Type::U16) | (Type::U16, Type::I16) | (Type::I16, Type::I16) | (Type::U16, Type::U16) => return Ok(var.to_string()),
        (Type::I32, Type::U32) | (Type::U32, Type::I32) | (Type::I32, Type::I32) | (Type::U32, Type::U32) => return Ok(var.to_string()),
        (Type::I64, Type::U64) | (Type::U64, Type::I64) | (Type::I64, Type::I64) | (Type::U64, Type::U64) | (Type::Usize, Type::Usize) => return Ok(var.to_string()),
        
        (Type::Usize, Type::I64) | (Type::Usize, Type::U64) => {
            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", res, var));
            return Ok(res);
        },
        (Type::I64, Type::Usize) | (Type::U64, Type::Usize) => {
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, var));
            return Ok(res);
        },
        
        (Type::I32, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi {} : i32 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::U32, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i32 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::I16, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi {} : i16 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::U16, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i16 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::I8, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi {} : i8 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::U8, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i8 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        
        (Type::Array(from_inner, f_len, false), Type::Array(to_inner, t_len, true)) 
            if f_len == t_len && **from_inner == Type::Bool && **to_inner == Type::Bool => {
             return promote_array_packing(ctx, out, var, *f_len, to);
        },
        (from, to) if from.is_integer() && to.is_integer() => {
             if *from == Type::Usize {
                 let intermediate = format!("%idx_i64_{}", ctx.next_id());
                 out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", intermediate, var));
                 let dst_width = get_bit_width(to);
                 if dst_width < 64 {
                     out.push_str(&format!("    {} = arith.trunci {} : i64 to {}\n", res, intermediate, to.to_mlir_type(ctx)?));
                     return Ok(res);
                 } else {
                     return Ok(intermediate);
                 }
             }
             let src_width = get_bit_width(from);
             let dst_width = get_bit_width(to);
             if src_width == dst_width {
                 return Ok(var.to_string());
             } else if src_width > dst_width {
                 emit("arith.trunci", &from.to_mlir_type(ctx)?, &to.to_mlir_type(ctx)?);
                 return Ok(res);
             } else {
                 let op = if from.is_unsigned() { "arith.extui" } else { "arith.extsi" };
                 emit(op, &from.to_mlir_type(ctx)?, &to.to_mlir_type(ctx)?);
                 return Ok(res);
             }
        },
        (from, to) if from.is_integer() && to.is_float() => {
             let op = if from.is_unsigned() { "arith.uitofp" } else { "arith.sitofp" };
             let src_str = from.to_mlir_type(ctx)?;
             let dst_str = to.to_mlir_type(ctx)?;
             emit(op, &src_str, &dst_str);
             return Ok(res);
        },
        (Type::F32, Type::F64) => { emit("arith.extf", "f32", "f64"); return Ok(res); },
        (Type::F64, Type::F32) => { emit("arith.truncf", "f64", "f32"); return Ok(res); },
        
        (Type::Reference(_, _), Type::Reference(_, _)) => return Ok(var.to_string()),
        
        (Type::Reference(inner_from, _), to) if inner_from.as_ref() == to => {
            let mlir_to = to.to_mlir_type(ctx)?;
            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, var, mlir_to));
            return Ok(res);
        },
        
        (Type::F32, Type::Bool) => {
             out.push_str("    %cst_0_f32 = arith.constant 0.0 : f32\n");
             out.push_str(&format!("    {} = arith.cmpf \"une\", {}, %cst_0_f32 : f32\n", res, var));
             return Ok(res);
        }
        (Type::F64, Type::Bool) => {
             out.push_str("    %cst_0_f64 = arith.constant 0.0 : f64\n");
             out.push_str(&format!("    {} = arith.cmpf \"une\", {}, %cst_0_f64 : f64\n", res, var));
             return Ok(res);
        }
        (from, Type::Bool) if from.is_integer() => {
             let zero = format!("%c0_{}", ctx.next_id());
             let mlir_from = from.to_mlir_type(ctx)?;
             ctx.emit_const_int(out, &zero, 0, &mlir_from);
             out.push_str(&format!("    {} = arith.cmpi \"ne\", {}, {} : {}\n", res, var, zero, mlir_from));
             return Ok(res);
        },
        (Type::Bool, to) if to.is_integer() => {
             let dst_ty = to.to_mlir_type(ctx)?;
             emit("arith.extui", "i1", &dst_ty);
             return Ok(res);
        }
        (Type::Tuple(fs), Type::Tuple(ts)) if fs.len() == ts.len() => {
             return promote_tuple(ctx, out, var, from, to, &res);
        }
        _ => {}
    }

    promote_numeric_fallback(ctx, out, var, from, to, &res)
}

fn promote_numeric_fallback(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type, res: &str) -> Result<String, String> {
    let mut emit = |op: &str, src_ty: &str, dst_ty: &str| {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, var, src_ty, dst_ty));
    };

    if let (Some(f_idx), Some(t_idx)) = (get_numeric_idx(from), get_numeric_idx(to)) {
        if let Some((op, src_ty, dst_ty)) = PROMOTION_OPS[f_idx][t_idx] {
            emit(op, src_ty, dst_ty);
            return Ok(res.to_string());
        }
    }

    if from.canonical_eq(to) {
        return Ok(var.to_string());
    }

    if let (Ok(mlir_from), Ok(mlir_to)) = (from.to_mlir_type(ctx), to.to_mlir_type(ctx)) {
        if mlir_from == mlir_to {
             let registry = ctx.struct_registry();
             if from.size_of(registry) == to.size_of(registry) {
                 return Ok(var.to_string());
             }
        }
    }

    match (from, to) {
        (Type::Struct(n), Type::Concrete(..)) | (Type::Concrete(..), Type::Struct(n)) => {
            let other = if matches!(from, Type::Struct(_)) { to } else { from };
            fn normalize_fqn(s: &str) -> String {
                let protected = s.replace("__", "\x01");
                let parts: Vec<&str> = protected.split('_').collect();
                let normalized: Vec<String> = parts.iter().map(|part| {
                    let restored = part.replace('\x01', "__");
                    if restored.contains("__") {
                        restored.rsplit("__").next().unwrap_or(&restored).to_string()
                    } else {
                        restored
                    }
                }).collect();
                normalized.join("_")
            }
            let n_norm = normalize_fqn(n);
            let other_norm = normalize_fqn(&other.mangle_suffix());
            if n_norm == other_norm {
                return Ok(var.to_string());
            }
        }
        _ => {}
    }

    if from.k_is_ptr_type() && to.k_is_ptr_type() {
        return Ok(var.to_string());
    }

    if let Type::Pointer { ref element, .. } = from {
        if element.as_ref() == to {
            return Ok(var.to_string());
        }
    }

    Err(format!("Numeric promotion not supported from {:?} to {:?} (var: {})", from, to, var))
}

fn promote_array_packing(ctx: &mut LoweringContext, out: &mut String, var: &str, f_len: usize, to: &Type) -> Result<String, String> {
     let packed_storage_ty = to.to_mlir_storage_type(ctx)?;
     let mut current_packed = format!("%packed_prom_{}", ctx.next_id());
     out.push_str(&format!("    {} = llvm.mlir.zero : {}\n", current_packed, packed_storage_ty));
     
     let unpacked_storage_ty_str = format!("!llvm.array<{} x i1>", f_len);
     
     let mut current_word_ssa = String::new();
     for i in 0..f_len {
         let bit_idx = i % 64;
         if bit_idx == 0 {
             let zero = format!("%zero_w_{}", ctx.next_id());
             ctx.emit_const_int(out, &zero, 0, "i64");
             current_word_ssa = zero;
         }
         
         let elem = format!("%elem_{}_{}", i, ctx.next_id());
         out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", elem, var, i, unpacked_storage_ty_str));
         
         let elem_ext = format!("%elem_ext_{}", ctx.next_id());
         ctx.emit_cast(out, &elem_ext, "arith.extui", &elem, "i8", "i64");
         
         let shifted = format!("%shifted_{}", ctx.next_id());
         let shift_amt = format!("%sh_amt_{}", ctx.next_id());
         ctx.emit_const_int(out, &shift_amt, bit_idx as i64, "i64");
         ctx.emit_binop(out, &shifted, "arith.shli", &elem_ext, &shift_amt, "i64");
         
         let new_word = format!("%accum_w_{}_{}", i, ctx.next_id());
         ctx.emit_binop(out, &new_word, "arith.ori", &current_word_ssa, &shifted, "i64");
         current_word_ssa = new_word;
         
         if bit_idx == 63 || i == f_len - 1 {
             let word_idx = i / 64;
             let inserted = format!("%packed_insert_{}", ctx.next_id());
             out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", inserted, current_word_ssa, current_packed, word_idx, packed_storage_ty));
             current_packed = inserted;
         }
     }
     Ok(current_packed)
}
fn promote_tuple(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type, res: &str) -> Result<String, String> {
     let (fs, ts) = match (from, to) { (Type::Tuple(f), Type::Tuple(t)) => (f, t), _ => return Err("promote_tuple requires tuple types".to_string()) };
     let target_mlir = to.to_mlir_storage_type(ctx)?;
     let src_mlir = from.to_mlir_storage_type(ctx)?;
     
     let first_init = format!("{}_init", res.replace("%", ""));
     out.push_str(&format!("    %{} = llvm.mlir.undef : {}\n", first_init, target_mlir));
     
     let mut current_struct_ssa = format!("%{}", first_init);
     
     for (i, (f_ty, t_ty)) in fs.iter().zip(ts.iter()).enumerate() {
        let elem_val = format!("%{}_elem_{}", res.replace("%", ""), i);
         ctx.emit_extractvalue(out, &elem_val, var, i, &src_mlir);
         
         let prom_elem = match promote_numeric(ctx, out, &elem_val, f_ty, t_ty) {
             Ok(r) => r,
             Err(_) => cast_numeric(ctx, out, &elem_val, f_ty, t_ty)?
         };
         
         let target_name = if i == fs.len() - 1 {
             res.to_string()
         } else {
             format!("{}_chain_{}", res, i)
         };
         
         out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", 
             target_name, prom_elem, current_struct_ssa, i, target_mlir));
         
         current_struct_ssa = target_name;
     }
     Ok(res.to_string())
}

pub(crate) fn get_bit_width(ty: &Type) -> u32 {
    match ty {
        Type::Bool | Type::I8 | Type::U8 => 8,
        Type::I16 | Type::U16 => 16,
        Type::I32 | Type::U32 | Type::F32 => 32,
        Type::I64 | Type::U64 | Type::Usize | Type::F64 => 64,
        _ => 0
    }
}

// to_mlir_type impl moved to crate::codegen::types::mlir

// ============================================================================
// Pointer flattening and layout validation
// ============================================================================

/// Extracts the inner type from mangled pointer names.
pub use crate::codegen::types::layout::extract_ptr_inner;
pub use crate::codegen::types::layout::flatten_nested_ptr;
pub use crate::codegen::types::layout::prove_layout_compatibility;
pub use crate::codegen::types::layout::prove_layout_compatibility_ctx;

pub use crate::codegen::types::substitution::substitute_generics;
pub use crate::codegen::types::substitution::substitute_generics_ctx;
pub use crate::codegen::types::mlir::to_mlir_type;


fn collect_self_concrete_args(ctx: &mut LoweringContext, struct_name: &str) -> Option<Vec<Type>> {
    let template = ctx.struct_templates().get(struct_name)?;
    let generics = template.generics.as_ref()?;
    let mut args = Vec::with_capacity(generics.params.len());
    for param in &generics.params {
        let p_name = match param {
            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
        };
        let arg = ctx.current_type_map().get(&p_name).cloned()?;
        args.push(arg);
    }
    if args.is_empty() { None } else { Some(args) }
}

fn resolve_struct_self_opt(ctx: &mut LoweringContext, r: Type) -> Type {
    if let Type::Struct(name) = &r {
        if let Some(args) = collect_self_concrete_args(ctx, name) {
            return Type::Concrete(name.clone(), args);
        }
    }
    r
}

fn resolve_codegen_type_self(ctx: &mut LoweringContext, _flattened: &Type) -> Type {
    let mut res = None;
    if let Some(concrete_ty) = ctx.current_type_map().get("Self").cloned() {
        res = Some(concrete_ty);
    }
    if res.is_none() {
        if let Some(self_ty) = ctx.current_self_ty() {
            res = Some(self_ty.clone());
        }
    }

    if let Some(r) = res {
        resolve_struct_self_opt(ctx, r)
    } else {
        panic!("MonomorphizationError: Failed to resolve SelfType. Map keys: {:?}", ctx.current_type_map().keys().collect::<Vec<_>>());
    }
}

fn resolve_codegen_type_struct(ctx: &mut LoweringContext, ty: &Type, name: &str) -> Type {
    if name.chars().all(|c| c.is_ascii_digit()) {
        return ty.clone();
    }
    if name.contains("__") {
        let resolved_base = name.to_string();
        let requires_generics = ctx.struct_templates().get(&resolved_base)
            .map(|t| t.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false))
            .unwrap_or(false);
        if requires_generics {
            return Type::Struct(resolved_base);
        }
        let resolved_params = vec![]; 
        let is_enum = ctx.enum_templates().contains_key(&resolved_base);
        if !ctx.suppress_specialization.get() {
            let _ = ctx.specialize_template(&resolved_base, &resolved_params, is_enum);
        }
        if is_enum {
            return Type::Enum(resolved_base);
        } else {
            return Type::Struct(resolved_base);
        }
    }

    let concrete_opt = ctx.current_type_map().get(name).cloned();
    if let Some(concrete_ty) = concrete_opt {
        concrete_ty
    } else {
        let suffix = format!("__{}", name);
        let canonical_candidate = ctx.struct_templates().keys()
            .find(|k| k.ends_with(&suffix))
            .cloned()
            .or_else(|| {
                ctx.enum_templates().keys()
                    .find(|k| k.ends_with(&suffix))
                    .cloned()
            });
        
        if let Some(ref candidate) = canonical_candidate {
            let resolved_base = candidate.clone();
            let requires_generics = ctx.struct_templates().get(&resolved_base)
                .map(|t| t.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false))
                .unwrap_or(false);
            if requires_generics {
                return Type::Struct(resolved_base);
            }
            let is_enum = ctx.enum_templates().contains_key(&resolved_base);
            if !ctx.suppress_specialization.get() {
                let _ = ctx.specialize_template(&resolved_base, &[], is_enum);
            }
            return if is_enum { Type::Enum(resolved_base) } else { Type::Struct(resolved_base) };
        }
        
        let segments: Vec<String> = name.split("::").map(|s| s.to_string()).collect();
        if let Some((pkg, item)) = crate::codegen::expr::utils::resolve_package_prefix_ctx(ctx, &segments) {
             let resolved_base = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) };
             let mut resolved_params = vec![];

             if resolved_params.is_empty() {
                  if let Some(template) = ctx.struct_templates().get(&resolved_base) {
                      if let Some(generics) = &template.generics {
                          let current_args = ctx.current_generic_args();
                           if current_args.len() == generics.params.len() {
                               resolved_params = current_args.clone();
                           } else {
                               let mut inferred = Vec::new();
                               let mut all_found = true;
                               for param in &generics.params {
                                   let p_name = match param {
                                       crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                       crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                   };
                                   let arg_opt = ctx.current_type_map().get(&p_name).cloned();
                                   if let Some(arg) = arg_opt {
                                       inferred.push(arg);
                                   } else {
                                       all_found = false;
                                       break;
                                   }
                               }
                               if all_found {
                                   resolved_params = inferred;
                               }
                           }
                      }
                  }
             }
             
             let is_enum = ctx.enum_templates().contains_key(&resolved_base);
             let requires_generics = ctx.struct_templates().get(&resolved_base)
                 .map(|t| t.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false))
                 .unwrap_or(false);
             
             if !ctx.suppress_specialization.get() && (!requires_generics || !resolved_params.is_empty()) {
                  let _ = ctx.specialize_template(&resolved_base, &resolved_params, is_enum);
             }
             
             if !resolved_params.is_empty() {
                 Type::Concrete(resolved_base, resolved_params)
             } else if is_enum {
                 Type::Enum(resolved_base)
             } else {
                 Type::Struct(resolved_base)
             }
        } else {
             Type::Struct(name.to_string())
        }
    }
}

fn resolve_codegen_type_concrete(ctx: &mut LoweringContext, base_name: &str, target_params: &[Type]) -> Type {
    if target_params.is_empty() {
        let concrete_opt = ctx.current_type_map().get(base_name).cloned();
        if let Some(concrete_ty) = concrete_opt {
            return resolve_codegen_type(ctx, &concrete_ty);
        }
    }
    if target_params.is_empty() && !ctx.current_type_map().is_empty() {
        if let Some(template) = ctx.struct_templates().get(base_name) {
            if let Some(generics) = &template.generics {
                let param_names: Vec<String> = generics.params.iter().map(|param| {
                    match param {
                        crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                        crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                    }
                }).collect();
                let type_map = ctx.current_type_map();
                let mut inferred_map = type_map.clone();
                crate::codegen::expr::infer_phantom_generics(&param_names, &mut inferred_map);
                let args: Vec<Type> = param_names.iter()
                    .filter_map(|pname| inferred_map.get(pname).cloned())
                    .collect();
                if args.len() == param_names.len() {
                    let resolved_args: Vec<Type> = args.iter()
                        .map(|a| resolve_codegen_type(ctx, a))
                        .collect();
                    return Type::Concrete(base_name.to_string(), resolved_args);
                }
            }
        }
    }
    let mut resolved_params = Vec::new();
    for param in target_params {
        resolved_params.push(resolve_codegen_type(ctx, param));
    }
    if base_name == "Owned" && !resolved_params.is_empty() {
        Type::Owned(Box::new(resolved_params[0].clone()))
    } else if !resolved_params.is_empty() && base_name == "Window" {
        let region = if resolved_params.len() >= 2 {
            if let Type::Struct(r) = &resolved_params[1] { r.clone() } else { "RAM".to_string() }
        } else { "RAM".to_string() };
        Type::Window(Box::new(resolved_params[0].clone()), region)
    } else if base_name == "Atomic" && !resolved_params.is_empty() {
        Type::Atomic(Box::new(resolved_params[0].clone()))
    } else {
        let mut resolved_base = base_name.to_string();
        if !resolved_base.contains("__") {
            let suffix = format!("__{}", base_name);
            let canonical_candidate = ctx.struct_templates().keys()
                .find(|k| k.ends_with(&suffix))
                .cloned()
                .or_else(|| {
                    ctx.enum_templates().keys()
                        .find(|k| k.ends_with(&suffix))
                        .cloned()
                });
            
            if let Some(candidate) = canonical_candidate {
                resolved_base = candidate;
            } else {
                let segments: Vec<String> = base_name.split("::").map(|s| s.to_string()).collect();
                if let Some((pkg, item)) = crate::codegen::expr::utils::resolve_package_prefix_ctx(ctx, &segments) {
                     resolved_base = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) };
                }
            }
        }

        let is_enum = ctx.enum_templates().contains_key(&resolved_base);
        if (ctx.struct_templates().contains_key(&resolved_base) || is_enum)
            && !ctx.suppress_specialization.get() {
                let _ = ctx.specialize_template(&resolved_base, &resolved_params, is_enum);
            }
        Type::Concrete(resolved_base, resolved_params)
    }
}

pub fn resolve_codegen_type(ctx: &mut LoweringContext, ty: &Type) -> Type {
    let flattened = flatten_nested_ptr(ty, 0, "codegen_resolve");
    match &flattened {
        Type::Enum(name) => Type::Enum(name.clone()),
        Type::Generic(name) => {
            let concrete_opt = ctx.current_type_map().get(name).cloned();
            if let Some(concrete_ty) = concrete_opt {
                 if let Type::Generic(ref n) = concrete_ty {
                     if n == name {
                         return concrete_ty;
                     }
                 }
                 resolve_codegen_type(ctx, &concrete_ty)
            } else if ctx.enum_registry().values().any(|i| i.name == *name) || ctx.enum_templates().contains_key(name) {
                Type::Enum(name.clone())
            } else {
                Type::Struct(name.clone())
            }
        }
        Type::SelfType => resolve_codegen_type_self(ctx, &flattened),
        Type::Struct(name) => resolve_codegen_type_struct(ctx, ty, name),
        Type::Concrete(base_name, target_params) => resolve_codegen_type_concrete(ctx, base_name, target_params),
        Type::Pointer { element, provenance, is_mutable } => Type::Pointer {
            element: Box::new(resolve_codegen_type(ctx, element)),
            provenance: provenance.clone(),
            is_mutable: *is_mutable,
        },
        Type::Reference(inner, mutability) => Type::Reference(Box::new(resolve_codegen_type(ctx, inner)), *mutability),
        Type::Owned(inner) => Type::Owned(Box::new(resolve_codegen_type(ctx, inner))),
        Type::Fn(args, ret) => Type::Fn(
            args.iter().map(|a| resolve_codegen_type(ctx, a)).collect(),
            Box::new(resolve_codegen_type(ctx, ret)),
        ),
        Type::Window(inner, region) => Type::Window(Box::new(resolve_codegen_type(ctx, inner)), region.clone()),
        Type::Array(inner, len, _) => Type::Array(Box::new(resolve_codegen_type(ctx, inner)), *len, false),
        Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| resolve_codegen_type(ctx, e)).collect()),
        Type::Tensor(inner, shape) => Type::Tensor(Box::new(resolve_codegen_type(ctx, inner)), shape.clone()),
        _ => ty.clone(),
    }
}



/// Bridges the gap between Rust's syn::Type (legacy/helper) and Salt's Type system.
pub fn resolve_type(ctx: &mut LoweringContext, ty: &crate::grammar::SynType) -> Type {
    // Handle context-dependent types (Array, Tensor) here.

    if let crate::grammar::SynType::Array(inner, len_expr) = ty {
        let inner_ty = resolve_type(ctx, inner);
        return match ctx.evaluator.eval_expr(len_expr) {
            Ok(crate::evaluator::ConstValue::Integer(val)) => Type::Array(Box::new(inner_ty), val as usize, false),
            Ok(_) => { crate::ice!("Array length must evaluate to an integer"); },
            Err(e) => { crate::ice!("Failed to evaluate array length: {:?}", e); }
        };
    }

    if let crate::grammar::SynType::Path(tp) = ty {
        if let Some(seg) = tp.segments.last() {
            if seg.ident == "Tensor"
                 && seg.args.len() >= 2 {
                     let inner_syn = &seg.args[0];
                     let inner = resolve_type(ctx, inner_syn);
                     let mut shape = Vec::new();
                     
                     // Check for __Shape_X_Y_Z__ marker (AUTO-RANK)
                     // Preprocessor prepends auto-computed rank: {128,784} -> __Shape_2_128_784__
                     // Format: __Shape_Rank_D1_D2_...__ where first element is auto-rank (skipped)
                     if let crate::grammar::SynType::Path(shape_path) = &seg.args[1] {
                         if let Some(shape_seg) = shape_path.segments.last() {
                             let shape_name = shape_seg.ident.to_string();
                             if shape_name.starts_with("__Shape_") && shape_name.ends_with("__") {
                                 // Parse __Shape_2_128_784__ -> skip auto-rank, dims = [128, 784]
                                 let shape_str = &shape_name[8..shape_name.len()-2]; // strip prefix/suffix
                                 let all_values: Vec<usize> = shape_str.split('_')
                                     .filter_map(|s| s.parse().ok())
                                     .collect();
                                 // Skip first value (rank indicator) and use rest as dimensions
                                 if all_values.len() > 1 {
                                     shape = all_values[1..].to_vec();
                                 } else if !all_values.is_empty() {
                                     // Single value: use as dimension (rank-1 tensor)
                                     shape = all_values;
                                 }
                                 return Type::Tensor(Box::new(inner), shape);
                             }
                         }
                     }
                     
                     // Legacy: Support old Tensor<f32, [128], [784]> syntax
                     for i in 1..seg.args.len() {
                         if let crate::grammar::SynType::Array(_dummy, len_expr) = &seg.args[i] {
                              if let Ok(crate::evaluator::ConstValue::Integer(val)) = ctx.evaluator.eval_expr(len_expr) {
                                  shape.push(val as usize);
                              }
                         }
                     }
                     return Type::Tensor(Box::new(inner), shape);
                 }
        }
    }

    // Default: Lower to Type and resolve imports/aliases (via resolve_codegen_type)
    // Note: Type::from_syn handles basic conversions (structs, primitives, etc.)
    if let Some(t) = Type::from_syn(ty) {
        resolve_codegen_type(ctx, &t)
    } else {
        Type::Unit
    }
}

/// Infers the type of a syn::Expr without emitting MLIR.
/// Used for receiver extraction in method call resolution.
pub fn infer_expr_type(
    ctx: &mut LoweringContext, 
    expr: &syn::Expr, 
    local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>
) -> Result<Type, String> {
    match expr {
        syn::Expr::Path(p) => {
            let name = p.path.segments.iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("__");
            
            // Check local vars first
            if let Some((ty, _)) = local_vars.get(&name) {
                return Ok(ty.clone());
            }
            
            // Check single-segment name in locals
            if p.path.segments.len() == 1 {
                let simple_name = p.path.segments[0].ident.to_string();
                if let Some((ty, _)) = local_vars.get(&simple_name) {
                    return Ok(ty.clone());
                }
            }
            
            // Check global variables/constants
            if let Some(ty) = ctx.globals().get(&name) {
                return Ok(ty.clone());
            }
            
            // Try canonical resolution with imports
            let canonical = crate::codegen::expr::utils::resolve_package_prefix_ctx(ctx, &p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>());
            if let Some((pkg, _)) = canonical {
                if let Some(ty) = ctx.globals().get(&pkg) {
                    return Ok(ty.clone());
                }
            }
            
            Err(format!("Cannot infer type for path expression: {:?}", name))
        }
        syn::Expr::Paren(p) => infer_expr_type(ctx, &p.expr, local_vars),
        syn::Expr::Field(f) => {
            let base_ty = infer_expr_type(ctx, &f.base, local_vars)?;
            // For field access, find the field type in the struct registry
            let _base_name = match &base_ty {
                Type::Struct(n) => n.clone(),
                Type::Concrete(n, _) => n.clone(),
                Type::Reference(inner, _) => {
                    match &**inner {
                        Type::Struct(n) => n.clone(),
                        Type::Concrete(n, _) => n.clone(),
                        _ => return Err(format!("Field access on non-struct reference: {:?}", base_ty)),
                    }
                }
                _ => return Err(format!("Field access on non-struct type: {:?}", base_ty)),
            };
            
            // Find the struct in the registry using TypeKey
            let type_key = type_to_type_key(&base_ty);
            if let Some(info) = ctx.struct_registry().get(&type_key) {
                if let syn::Member::Named(field_name) = &f.member {
                    // StructInfo.fields is HashMap<String, (usize, Type)>
                    if let Some((_, ft)) = info.fields.get(&field_name.to_string()) {
                        return Ok(ft.clone());
                    }
                }
            }
            Err(format!("Unknown field on type {:?}: {:?}", base_ty, f.member))
        }
        syn::Expr::Reference(r) => {
            let inner = infer_expr_type(ctx, &r.expr, local_vars)?;
            Ok(Type::Reference(Box::new(inner), r.mutability.is_some()))
        }
        syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
            let inner = infer_expr_type(ctx, &u.expr, local_vars)?;
            match inner {
                Type::Reference(inner_ty, _) => Ok(*inner_ty),
                Type::Owned(inner_ty) => Ok(*inner_ty),
                _ => Err(format!("Dereference on non-reference type: {:?}", inner)),
            }
        }
        _ => Err(format!("Cannot infer type for expression: {:?}", expr)),
    }
}

/// Converts a Type to a TypeKey for method_registry lookup.
pub use crate::codegen::types::resolution::type_to_type_key;

/// Trait Constraint Solver
/// Checks whether a concrete type satisfies a trait constraint.
/// 
/// This is called during generic instantiation when a type parameter has a bound:
/// `fn foo<T: Formattable>(x: T)` - when T is replaced with i64, i64: Formattable is verified.
pub use crate::codegen::types::traits::{check_trait_constraint, validate_trait_constraints, has_unresolved_type_params};

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    
    
    pub(crate) fn populate_explicit_specialization_map(
        &mut self,
        func: &crate::grammar::SaltFn,
        concrete_tys: &[Type],
        st: &Type,
        old_const_vals: &mut Vec<(String, Option<crate::evaluator::ConstValue>)>,
    ) {
        let template_name = if let Type::Struct(name) = st {
            self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
        } else if let Type::Enum(name) = st {
            self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
        } else if let Type::Concrete(name, _) = st {
            name.clone()
        } else if let Type::Pointer { .. } = st {
            "std__core__ptr__Ptr".to_string()
        } else {
            "".to_string()
        };
        
        if !template_name.is_empty() {
            let gen_params = if let Some(s) = self.struct_templates().get(&template_name) {
                s.generics.as_ref().map(|g| g.params.clone())
            } else if let Some(e) = self.enum_templates().get(&template_name) {
                e.generics.as_ref().map(|g| g.params.clone())
            } else { None };
            
            if let Some(params) = gen_params {
                for (i, param) in params.iter().enumerate() {
                    let pname = match param { crate::grammar::GenericParam::Type { name, .. } => name.to_string(), crate::grammar::GenericParam::Const { name, .. } => name.to_string() };
                    if let Type::Concrete(_, args) = &st {
                        if let Some(arg) = args.get(i) {
                            self.current_type_map_mut().insert(pname, arg.clone());
                        }
                    } else if let Type::Pointer { element, .. } = &st {
                        if i == 0 {
                            self.current_type_map_mut().insert(pname, (**element).clone());
                        }
                    } else if let Some(arg) = concrete_tys.get(i) {
                        self.current_type_map_mut().insert(pname, arg.clone());
                    }
                }
            }
        }
        
        if let Some(fn_generics) = &func.generics {
            let struct_generic_names: std::collections::HashSet<String> = {
                let mut names = std::collections::HashSet::new();
                let type_name = match st {
                    Type::Struct(name) | Type::Concrete(name, _) => Some(name.clone()),
                    _ => None
                };
                if let Some(ref tname) = type_name {
                    let gen_params = {
                        let templates = self.struct_templates();
                        if let Some(s) = templates.get(tname) {
                            s.generics.as_ref().map(|g| g.params.clone())
                        } else {
                            let _ = templates;
                            let etemplates = self.enum_templates();
                            etemplates.get(tname).and_then(|e| e.generics.as_ref()).map(|g| g.params.clone())
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
            
            let struct_generic_count = struct_generic_names.len();
            let method_args: Vec<Type> = concrete_tys.iter().skip(struct_generic_count).cloned().collect();
            
            if !method_args.is_empty() {
                let method_only_params: syn::punctuated::Punctuated<_, syn::token::Comma> = fn_generics.params.iter()
                    .filter(|p| {
                        let name = match p {
                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                        };
                        !struct_generic_names.contains(&name)
                    })
                    .cloned()
                    .collect();
                
                let method_only_generics = crate::grammar::Generics {
                    params: method_only_params,
                };
                self.map_generics(&Some(method_only_generics), &method_args, &func.name.to_string(), old_const_vals);
            }
        }
    }
    #[allow(clippy::too_many_arguments)] // All 8 parameters needed to construct MonomorphizationTask
    pub(crate) fn enqueue_monomorphization_task(
        &mut self,
        func_name: &str,
        mangled: &str,
        func: crate::grammar::SaltFn,
        concrete_tys: Vec<Type>,
        s_ty: Option<Type>,
        imports: Vec<crate::grammar::ImportDecl>,
        spec_map: std::collections::BTreeMap<String, Type>,
    ) {
        let mut pkg_path = Vec::new();
        if let Some((t_name, _method)) = func_name.rsplit_once("__") {
            if let Some(pkg) = self.discovery.type_origins.get(t_name) {
                pkg_path = pkg.split('.').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
            }
        }
        
        if pkg_path.is_empty() {
            let path_segments: Vec<String> = if func_name.contains("__") {
                 func_name.split("__").map(|s| s.to_string()).collect()
            } else {
                 vec![]
            };
            pkg_path = if path_segments.len() > 1 {
                path_segments[0..path_segments.len()-1].to_vec()
            } else {
                vec![]
            };
        }

        let task = crate::codegen::collector::MonomorphizationTask {
            identity: crate::types::TypeKey { 
                path: pkg_path, 
                name: func.name.to_string(), 
                specialization: None 
            },
            mangled_name: mangled.to_string(),
            func,
            concrete_tys,
            self_ty: s_ty,
            imports,
            type_map: spec_map,
        };
        self.expansion.pending_generations.push_back(task);
    }

    pub fn request_explicit_specialization(&mut self, func_name: &str, override_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        // Always strip Reference wrappers from self_ty.
        let self_ty = self_ty.map(|mut ty| {
            while let Type::Reference(inner, _) = ty {
                ty = *inner;
            }
            ty
        });
        
        let mangled = override_name.to_string();
        
        // Check strict map
        if let Some(existing) = self.specializations().get(&(func_name.to_string(), concrete_tys.clone())) {

            // If it exists in map but isn't defined or pending, queue it
            let defined = self.defined_functions().contains(existing);
            let pending = self.pending_generations().iter().any(|task| task.mangled_name == *existing);
            


            if !defined && !pending {

                 // Fall through to queue logic!
            } else {
                 return existing.clone();
            }
        }

        self.specializations_mut().insert((func_name.to_string(), concrete_tys.clone()), mangled.clone());
        
        let file = &self.config.file;
        // Search logic duplicated from request_specialization
        let found = if let Some(st) = &self_ty {
             let (st_base, method_name) = if let Some((base, method)) = func_name.rsplit_once("__") {
                 (base.to_string(), method.to_string())
             } else {
                 ("".to_string(), func_name.to_string())
             };
             
            let template_name = if let Type::Struct(name) = st {
                 self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Enum(name) = st {
                 self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             // Handle Type::Pointer method lookup with fully-qualified template name
             } else if let Type::Pointer { .. } = st {
                 "std__core__ptr__Ptr".to_string()
             } else {
                 st_base
             };
             // Use TraitRegistry for method lookup
             self.trait_registry().find_method_by_name(&template_name, &method_name, st)
        } else {
             file.items.iter().find_map(|item| {
                 if let crate::grammar::Item::Fn(f) = item {
                     if f.name == func_name { return Some((f.clone(), None, self.imports().clone())); }
                 }
                 None
             })
        };
        
        if let Some((func, s_ty, imports)) = found {

            let spec_map;
            {
                let old_imports = self.imports().clone();
                *self.imports_mut() = imports.clone();
                let old_map = self.current_type_map().clone();
                let old_args = self.current_generic_args().clone();
                let old_self = self.current_self_ty().clone();
                let mut old_const_vals = Vec::new();
                
                *self.current_generic_args_mut() = concrete_tys.clone();
                *self.current_self_ty_mut() = s_ty.clone();

                if let Some(st) = &s_ty {
                    self.populate_explicit_specialization_map(&func, &concrete_tys, st, &mut old_const_vals);
                }

                spec_map = self.current_type_map().clone();

                *self.current_type_map_mut() = old_map;
                *self.current_generic_args_mut() = old_args;
                *self.current_self_ty_mut() = old_self;
                *self.imports_mut() = old_imports;
            }

            self.enqueue_monomorphization_task(func_name, &mangled, func.clone(), concrete_tys.clone(), s_ty.clone(), imports.clone(), spec_map);
        }

        mangled
    }




    pub fn request_specialization(&mut self, func_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        // Always strip Reference wrappers from self_ty.
        // The self_ty identity should be the naked base type (e.g., Result), not Reference(Result).
        // This ensures correct type mangling and Self resolution during hydration.
        let self_ty = self_ty.map(|mut ty| {
            while let Type::Reference(inner, _) = ty {
                ty = *inner;
            }
            ty
        });

        // Prevent recursive specialization
        // Recursively flatten nested pointer wrappers
        let concrete_tys: Vec<Type> = concrete_tys.into_iter().enumerate().map(|(i, ty)| {
            let debug_ctx = format!("{}[arg {}]", func_name, i);
            flatten_nested_ptr(&ty, 0, &debug_ctx)
        }).collect();

        // Security check: ensure no generics leak into the monomorphization queue
        // Check for both Generic("T") and Struct("F") where F is not a known struct/enum
        if concrete_tys.iter().any(|t| has_unresolved_type_params(self, t)) {

             return func_name.to_string();
        }
        if let Some(sty) = &self_ty {
            if has_unresolved_type_params(self, sty) {

                 return func_name.to_string();
            }
        }

        // Derive suffix from concrete_tys, OR from self_ty's specialization args if concrete_tys is empty
        // This ensures method specializations like Ptr<u8>::offset get suffix "_u8"

        let suffix = if !concrete_tys.is_empty() {
            concrete_tys.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_")
        } else if let Some(Type::Concrete(_, args)) = &self_ty {
            args.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_")
        } else {
            String::new()
        };
        let mangled = if suffix.is_empty() { func_name.to_string() } else { format!("{}_{}", func_name, suffix) };
        
        if let Some(existing) = self.specializations().get(&(func_name.to_string(), concrete_tys.clone())) {
            let s_res: String = existing.clone();
            return s_res;
        }
        self.specializations_mut().insert((func_name.to_string(), concrete_tys.clone()), mangled.clone());
        
        let file = &self.config.file;
        let found = if let Some(st) = &self_ty {
             // Method lookup
             let (st_base, method_name) = if let Some((base, method)) = func_name.rsplit_once("__") {
                 (base.to_string(), method.to_string())
             } else {
                 ("".to_string(), func_name.to_string())
             };
             
             // If st_base is a specialized name, resolve it to template name
             let template_name = if let Type::Struct(name) = st {
                 self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Enum(name) = st {
                 self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else {
                 st_base
             };
             // Use TraitRegistry for method lookup
             self.trait_registry().find_method_by_name(&template_name, &method_name, st)
        } else {
             // Function lookup
             file.items.iter().find_map(|item| {
                 if let crate::grammar::Item::Fn(f) = item {
                     if f.name == func_name { return Some((f.clone(), None, self.imports().clone())); }
                 }
                 None
             })
        };

        if let Some((func, s_ty, imports)) = found {
            // Validate trait constraints before specialization
            let _ = validate_trait_constraints(self, &func.generics, &concrete_tys);

            // Scan specialized function for new dependencies (e.g. return types, local vars)
            // This prevents "Frozen Emission" panics by discovering deps during Expansion phase.
            let spec_map;
            {
                let old_imports = self.imports().clone();
                *self.imports_mut() = imports.clone();
                
                let old_map = self.current_type_map().clone();
                let old_args = self.current_generic_args().clone();
                let old_self = self.current_self_ty().clone();
                let mut old_const_vals = Vec::new();
                
                *self.current_generic_args_mut() = concrete_tys.clone();
                *self.current_self_ty_mut() = s_ty.clone();

                // Map Generics
                if let Some(st) = &s_ty {
                    // Extract concrete args from Type::Concrete for struct generics
                    let (template_name, struct_concrete_args) = if let Type::Struct(name) = st {
                        let tname = self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone());
                        (tname, vec![])
                    } else if let Type::Enum(name) = st {
                        let tname = self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone());
                        (tname, vec![])
                    } else if let Type::Concrete(name, args) = st {
                        // The args here are the concrete types for the struct generics

                        (name.clone(), args.clone())
                    } else if let Type::Pointer { element, .. } = st {
                        let canonical_element = crate::codegen::type_bridge::resolve_codegen_type(self, element);
                        ("std__core__ptr__Ptr".to_string(), vec![canonical_element])
                    } else {
                        ("".to_string(), vec![])
                    };
                    
                    if !template_name.is_empty() {
                         let gen_params = if let Some(s) = self.struct_templates().get(&template_name) {
                             s.generics.clone()
                         } else if let Some(e) = self.enum_templates().get(&template_name) {
                             e.generics.clone()
                         } else { None };
                          

                          // Use struct_concrete_args when available, fallback to concrete_tys
                          let args_to_map = if struct_concrete_args.is_empty() { &concrete_tys[..] } else { &struct_concrete_args[..] };

                          self.map_generics(&gen_params, args_to_map, &template_name, &mut old_const_vals);
                    }
                } else {
                    // Global Fn
                    if !concrete_tys.is_empty() {
                         self.map_generics(&func.generics, &concrete_tys, &func.name.to_string(), &mut old_const_vals);
                    }
                }
                
                // Method-level generics (e.g., mmap<T> on File struct)
                // CRITICAL: func.generics.params includes BOTH impl-level and method-level params.
                // Only method-level ones must be mapped (skip struct_generic_count from func.generics).
                if let Some(fn_generics) = &func.generics {
                    // Use the CALLER's self_ty for correct struct_generic_count
                    let struct_generic_count = self_ty.as_ref()
                        .and_then(|t| match t {
                            Type::Struct(name) | Type::Concrete(name, _) => {
                                self.struct_templates().get(name)
                                    .and_then(|s| s.generics.as_ref())
                                    .map(|g| g.params.len())
                                    .or_else(|| self.enum_templates().get(name)
                                        .and_then(|e| e.generics.as_ref())
                                        .map(|g| g.params.len()))
                            }
                            Type::Pointer { .. } => Some(1),
                            _ => None
                        })
                        .unwrap_or(0);
                    
                    let method_args: Vec<Type> = concrete_tys.iter().skip(struct_generic_count).cloned().collect();

                    if !method_args.is_empty() {
                        // Create method-only generics by skipping impl-level params
                        let method_only_generics = crate::grammar::Generics {
                            params: fn_generics.params.iter().skip(struct_generic_count).cloned().collect(),
                        };
                        self.map_generics(&Some(method_only_generics), &method_args, &func.name.to_string(), &mut old_const_vals);
                    }
                }
                
                // Scan!

                // Scan for new dependencies discovered during specialization
                let _ = self.scan_types_in_fn_lctx(&func);
                
                // Capture the specialized map before restoring context
                spec_map = self.current_type_map().clone();

                *self.imports_mut() = old_imports;
                *self.current_type_map_mut() = old_map;
                *self.current_generic_args_mut() = old_args;
                *self.current_self_ty_mut() = old_self;
                
                // Restore consts
                for (name, old_val) in old_const_vals.into_iter().rev() {
                    if let Some(v) = old_val {
                        self.evaluator.constant_table.insert(name, v);
                    } else {
                        self.evaluator.constant_table.remove(&name);
                    }
                }
            }

            self.enqueue_monomorphization_task(func_name, &mangled, func.clone(), concrete_tys.clone(), s_ty.clone(), imports.clone(), spec_map);
        };

        mangled
    }
    pub fn specialize_template(&mut self, base_name: &str, concrete_tys: &[Type], is_enum: bool) -> Result<TypeKey, String> {
        // Canonicalize concrete_tys before constructing the TypeKey.
        // Without this, Struct("Node") produces "Box_Node" while Struct("main__Node") produces
        // "Box_main__Node", creating duplicate specializations. By canonicalizing here, all
        // specializations consistently use FQN names.
        let concrete_tys: Vec<Type> = concrete_tys.iter().map(|ty| {
            if let Type::Struct(name) = ty {
                if !name.contains("__") {
                    let suffix = format!("__{}", name);
                    if let Some(canonical) = self.struct_templates().keys()
                        .find(|k| k.ends_with(&suffix))
                        .cloned()
                    {
                        return Type::Struct(canonical);
                    }
                    if let Some(canonical) = self.struct_registry().keys()
                        .find(|k| k.name == *name || k.name.ends_with(&suffix))
                        .map(|k| k.mangle())
                    {
                        return Type::Struct(canonical);
                    }
                }
            } else if let Type::Enum(name) = ty {
                if !name.contains("__") {
                    let suffix = format!("__{}", name);
                    if let Some(canonical) = self.enum_templates().keys()
                        .find(|k| k.ends_with(&suffix))
                        .cloned()
                    {
                        return Type::Enum(canonical);
                    }
                    if let Some(canonical) = self.enum_registry().keys()
                        .find(|k| k.name == *name || k.name.ends_with(&suffix))
                        .map(|k| k.mangle())
                    {
                        return Type::Enum(canonical);
                    }
                }
            }
            ty.clone()
        }).collect();
        let concrete_tys = &concrete_tys;
        
        // Construct TypeKey

        let parts: Vec<&str> = base_name.split("__").collect();
        let (path, name) = if parts.len() > 1 {
             (parts[..parts.len()-1].iter().map(|s| s.to_string()).collect::<Vec<_>>(), parts.last().expect("parts.len() > 1").to_string())
        } else {
             (vec![], base_name.to_string())
        };
        let key = TypeKey {
             path,
             name,
             specialization: if concrete_tys.is_empty() { None } else { Some(concrete_tys.to_vec()) },
        };
        
        let mangled = key.mangle();

        // 1. Check Registry (Existence = Done or In Progress)
        let exists = if is_enum {
            self.enum_registry().contains_key(&key)
        } else {
            self.struct_registry().contains_key(&key)
        };

        if exists { return Ok(key); }

        // 1.5. Generic Guard: Do NOT specialize (expand) if args are still generic
        // After substitute_generics, self-referential {I: Struct("I")} → Generic("I")
        let substituted_tys: Vec<Type> = concrete_tys.iter()
            .map(|t| substitute_generics_ctx(self, t))
            .collect();
        if substituted_tys.iter().any(|t| t.has_generics()) {
             return Ok(key);
        }

        // 2. Check Pending Set
        let is_queued = self.monomorphizer().pending_set.contains(&mangled);
        if is_queued { return Ok(key); }

        // 3. Frozen Check (Provenance Safety)
        if self.monomorphizer().is_frozen {
            // WARNING: Late specialization during emission.
            // Allowed via iterative drainage.
        }

        // 4. Self-Identity Guard (If inside the struct being simplified)
        if let Some(Type::Struct(self_name)) = self.current_self_ty() {
            if *self_name == mangled { return Ok(key); }
        }
        if let Some(Type::Enum(self_name)) = self.current_self_ty() {
             if *self_name == mangled { return Ok(key); }
        }

        // 5. Protected Name Check
        if Type::is_protected_name(&mangled) {
             return Ok(key); 
        }

        // 6. Atomic Registration (Placeholder)
        // Insert empty info to prevent recursive re-entry if registry lookup happens (redundant with pending_set but safe)
        if is_enum {
             let reg = self.enum_registry_mut();
             reg.insert(key.clone(), EnumInfo {
                 name: mangled.clone(), variants: Vec::new(), max_payload_size: 0,
                 template_name: if concrete_tys.is_empty() { None } else { Some(base_name.to_string()) },
                 specialization_args: concrete_tys.to_vec(),
             });
        } else {
             let reg = self.struct_registry_mut();
             reg.insert(key.clone(), StructInfo {
                 name: mangled.clone(), fields: HashMap::new(), field_order: Vec::new(), field_alignments: Vec::new(),
                 template_name: if concrete_tys.is_empty() { None } else { Some(base_name.to_string()) },
                 specialization_args: concrete_tys.to_vec(),
             });
        }

        // 7. Recursive expansion: process immediately to ensure
        // dependencies are sized before dependents
        {
            self.monomorphizer_mut().pending_set.insert(mangled.clone());
        }

        // EXPAND
        if is_enum {
             let res = self.expand_enum_structure(base_name, concrete_tys);
             match res {
                 Ok(info) => { self.enum_registry_mut().insert(key.clone(), info); }
                 Err(e) => {
                     self.enum_registry_mut().remove(&key);
                     self.monomorphizer_mut().pending_set.remove(&mangled);
                     return Err(e);
                 }
             }
        } else {
             let res = self.expand_template_structure(base_name, concrete_tys);
             match res {
                 Ok(info) => { 
                     self.struct_registry_mut().insert(key.clone(), info); 
                 }
                 Err(e) => {
                     self.struct_registry_mut().remove(&key);
                     self.monomorphizer_mut().pending_set.remove(&mangled);
                     return Err(e);
                 }
             }
        };

        // HOISTING (Immediate)
        let full_ty = if is_enum { crate::types::Type::Enum(mangled.clone()) } else { crate::types::Type::Struct(mangled.clone()) };
        if let Ok(mlir_def) = full_ty.to_mlir_storage_type(self) {
             if mlir_def.contains(", (") || mlir_def.contains(", ()") {
                let dummy_name = format!("__typedef_{}", mangled);
                let d = self.decl_out_mut();
                d.push_str(&format!("  llvm.mlir.global private @{}() : {} {{\n", dummy_name, mlir_def));
                d.push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_def));
                d.push_str(&format!("    llvm.return %0 : {}\n", mlir_def));
                d.push_str("  }\n");
             }
        }

        self.monomorphizer_mut().pending_set.remove(&mangled);

        Ok(key)
    }

    pub fn drain_work_queue(&mut self) {
        while let Some(task) = self.monomorphizer_mut().work_queue.pop_front() {
            // Setup Context for Self-Resolution
            let old_self = self.current_self_ty().clone();
            let self_type = if task.is_enum { Type::Enum(task.mangled_name.clone()) } else { Type::Struct(task.mangled_name.clone()) };
            *self.current_self_ty_mut() = Some(self_type);

            // Construct Key for Registry Access
            let base_name = &task.template_name;
            let parts: Vec<&str> = base_name.split("__").collect();
            let (path, name) = if parts.len() > 1 {
                 (parts[..parts.len()-1].iter().map(|s| s.to_string()).collect::<Vec<_>>(), parts.last().expect("parts.len() > 1").to_string())
            } else {
                 (vec![], base_name.to_string())
            };
            let key = TypeKey {
                 path,
                 name,
                 specialization: Some(task.args.clone()),
            };

            // EXPAND (No Registry Borrow Here, only Read Templates + Request Spec)
            if task.is_enum {
                if let Ok(info) = self.expand_enum_structure(&task.template_name, &task.args) {
                    // Commit to Registry
                    if let Some(entry) = self.enum_registry_mut().get_mut(&key) {
                        *entry = info;
                    }
                }
            } else if let Ok(info) = self.expand_template_structure(&task.template_name, &task.args) {
                // Commit to Registry
                if let Some(entry) = self.struct_registry_mut().get_mut(&key) {
                    *entry = info;
                }
            };

            // Restore Context
            *self.current_self_ty_mut() = old_self;

            // Mark as Done (Removing from pending_set is optional if registry is checked first, but good for cleanup)
            self.monomorphizer_mut().pending_set.remove(&task.mangled_name);

            // Emit the struct/enum definition into decl_out immediately after
            // specialization so the type is defined before any function body uses it.
            let full_ty = if task.is_enum { crate::types::Type::Enum(task.mangled_name.clone()) } else { crate::types::Type::Struct(task.mangled_name.clone()) };
            
            // Generate the full body definition string (e.g. !llvm.struct<"Vec_u8", (...)>)
            // to_mlir_storage_type triggers the registry lookup and body formatting.
            if let Ok(mlir_def) = full_ty.to_mlir_storage_type(self) {
                // Only hoist if the returned string contains a body definition (i.e. has fields or explicitly empty body).
                // If it returns an opaque reference (e.g. !llvm.struct<"Foo">), it means it was already emitted elsewhere.
                if mlir_def.contains(", (") || mlir_def.contains(", ()") {
                    let dummy_name = format!("__typedef_{}", task.mangled_name);
                    let d = self.decl_out_mut();
                    d.push_str(&format!("  llvm.mlir.global private @{}() : {} {{\n", dummy_name, mlir_def));
                    d.push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_def));
                    d.push_str(&format!("    llvm.return %0 : {}\n", mlir_def));
                    d.push_str("  }\n");
                }
            }
        }
        
        // Finalize (Freeze)
        self.monomorphizer_mut().is_frozen = true;
    }

    pub fn map_generics(&mut self, generics: &Option<crate::grammar::Generics>, args: &[Type], template_name: &str, old_const_vals: &mut Vec<(String, Option<ConstValue>)>) {

         if let Some(gen) = generics {
             for (i, param) in gen.params.iter().enumerate() {
                 if let Some(concrete) = args.get(i) {
                     let c_t: Type = concrete.clone();
                     let name = match param {
                         crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                         crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                     };
                     if Type::is_protected_name(&name) {
                           panic!("Shadowing Guard: Generic parameter '{}' covers a protected type name in template '{}'", name, template_name);
                      }
                     self.current_type_map_mut().insert(name.clone(), c_t.clone());

                     
                     // Const Generic Injection
                     if let Type::Struct(val_str) = &c_t {
                         if let Ok(int_val) = val_str.parse::<i64>() {
                             let old = self.evaluator.constant_table.insert(name.clone(), ConstValue::Integer(int_val));
                             old_const_vals.push((name, old));
                         }
                     }
                 }
             }
         }
    }

    /// Performs the structural expansion of a template by mapping generic
    /// parameters to concrete arguments and resolving field types.
    /// This is side-effect free w.r.t the struct registry.
    pub fn expand_template_structure(&mut self,
        template_name: &str,
        args: &[Type],
    ) -> Result<StructInfo, String> {
        // 1. Transactional Read: Extract Template Data
        // generics and fields are cloned to free struct_templates for the next level of recursion.
        let templates = self.struct_templates();
        let template = match templates.get(template_name) {
            Some(t) => t.clone(),
            None => return Err(format!("Template '{}' not found in registry.", template_name)),
        };
        let generics = template.generics.clone();
        let fields = template.fields.clone();

        // Fix: Context Swap to Template Definition Scope to prevent Key Drift
        // This makes sure that field resolution (e.g. "GlobalSlabAlloc") happens in the std lib context, NOT the user context.
        let mut _import_guard = None;
        if let Some(registry) = self.config.registry {
             let parts: Vec<&str> = template_name.split("__").collect();
             if parts.len() > 1 {
                 for (pkg_name, mod_info) in &registry.modules {
                      let pkg_mangled = pkg_name.replace(".", "__");
                      let prefix = format!("{}__", pkg_mangled);
                      if template_name.starts_with(&prefix) {
                           let mut combined_imports = mod_info.imports.clone();
                           // Synthesize self-imports ONLY for non-generic types
                           // Generic types (like Vec<T>, SlabCache<SIZE>) should be resolved
                           // via their categorical export metadata which preserves generic_params.
                           {
                                let pkg_prefix_ident = format!("{}__", pkg_mangled);
                                
                                // Only add non-generic struct templates as simple aliases
                                for (s_name, s_def) in &mod_info.struct_templates {
                                     // Skip generic templates - they need explicit instantiation
                                     let has_generics = s_def.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false);
                                     if has_generics {
                                         continue;
                                     }
                                     
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                                
                                // Concrete (non-template) structs can be aliased directly
                                for s_name in mod_info.structs.keys() {
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                           }
                           // Direct import swap (ImportContextGuard expects CodegenContext)
                           let old_imports = std::mem::replace(&mut *self.imports_mut(), combined_imports);
                           _import_guard = Some(old_imports);
                           break; 
                      }
                 }
             }

        }

        // 2. Validate Argument Count
        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
            // Instead of hard error, return placeholder for deferred expansion
            // This handles cases like Vec<T> inside String definition - the T will be
            // substituted later when the actual specialization is requested with concrete args.
            // Only log for debugging, don't fail compilation.

            // Restore imports if they were swapped for template definition scope
            if let Some(old_imports) = _import_guard {
                *self.imports_mut() = old_imports;
            }
            
            // Return a stub StructInfo with the template name - indicates "unspecialized"
            return Ok(StructInfo {
                name: template_name.to_string(),
                fields: std::collections::HashMap::new(),
                field_order: vec![],
                field_alignments: vec![],
                template_name: Some(template_name.to_string()),
                specialization_args: vec![],
            });
        }



        // 3. State Snapshot: Prepare new type mapping
        let old_map = self.current_type_map().clone();
        let old_generic_args = self.current_generic_args().clone();

        let mut type_map = old_map.clone();
        
        if let Some(gen) = &generics {
            for (param, arg) in gen.params.iter().zip(args.iter()) {
                 let name = match param {
                     crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                     crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                 };
                 type_map.insert(name, arg.clone());
            }
        }

        // 4. Transactional Update: Set the recursion context
        {
            *self.current_type_map_mut() = type_map;
            *self.current_generic_args_mut() = args.to_vec();
        }

        // 5. Recursive Discovery: Map fields in the new context
        let mut resolved_fields = HashMap::new();
        let mut field_order = Vec::new();
        let mut field_alignments = Vec::new();

        for (i, field) in fields.iter().enumerate() {
            // resolve_type is recursive and might access struct_templates/current_type_map
            let mut field_ty = resolve_type(self, &field.ty);

            // Handle @packed attribute
            if field.attributes.iter().any(|a| a.name == "packed") {
                 if let Type::Array(inner, len, _) = field_ty {
                      field_ty = Type::Array(inner, len, true);
                 }
            }
            
            let align = crate::grammar::attr::extract_align(&field.attributes);

            resolved_fields.insert(field.name.to_string(), (i, field_ty.clone()));
            field_order.push(field_ty);
            field_alignments.push(align);
        }
        
        // 6. Transactional Restore: Roll back the context
        {
            *self.current_type_map_mut() = old_map;
            *self.current_generic_args_mut() = old_generic_args;
        }
        // Restore imports that were swapped for template definition scope.
        // Without this, the caller's import context is permanently clobbered
        // with the template's module imports (e.g., Slice's 1-import context
        // overwrites main's 21-import context).
        if let Some(old_imports) = _import_guard {
            *self.imports_mut() = old_imports;
        }

        // Phase B: API Surface Discovery (Eager Method Registration)
        let methods = self.find_methods_for_template(template_name);
        for method_name in methods {
             // Skip generic methods. They require inference/turbofish at call site.
             // Registry stores full mangled name in 'name' field with empty path for Struct types.
             let key = crate::types::TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
             
             if let Some((func, _, _)) = self.trait_registry().get_legacy(&key, &method_name) {
                 if let Some(g) = &func.generics {
                     if !g.params.is_empty() {
                         continue; 
                     }
                 }
             } 

             let full_name = format!("{}__{}", template_name, method_name);
             let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
             let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }


        // 7. Return Metadata
        Ok(StructInfo {
            name: self.specialize_template(template_name, args, false)?.mangle(),
            fields: resolved_fields,
            field_order,
            field_alignments,
            template_name: Some(template_name.to_string()),
            specialization_args: args.to_vec(),
        })
    }

    pub fn expand_enum_structure(&mut self,
        template_name: &str,
        args: &[Type],
    ) -> Result<EnumInfo, String> {
         // 1. Transactional Read: Extract Enum Template Data
        let (generics, variants) = {
            let templates = self.enum_templates();
            let template = templates.get(template_name)
                .cloned()
                .ok_or_else(|| format!("Enum Template '{}' not found", template_name))?;
            (template.generics.clone(), template.variants.clone())
        };

        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
             return Err(format!("Generic mismatch for enum {}", template_name));
        }

        // 3. State Snapshot
        let old_map = self.current_type_map().clone();
        let old_generic_args = self.current_generic_args().clone();

        let mut type_map = old_map.clone();
        if let Some(gen) = &generics {
            for (param, arg) in gen.params.iter().zip(args.iter()) {
                 let name = match param {
                     crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                     crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                 };
                 type_map.insert(name, arg.clone());
            }
        }

        // 4. Transactional Update: Set recursion context
        {
            *self.current_type_map_mut() = type_map;
            *self.current_generic_args_mut() = args.to_vec();
        }
        
        let mut resolved_variants = Vec::new();
        let mut max_payload_size = 0;
        
        // 5. Recursive Discovery
        for (idx, v) in variants.iter().enumerate() {
             let p_ty = v.ty.as_ref().map(|sy| crate::codegen::type_bridge::resolve_type(self, sy));
             if let Some(ref ty) = p_ty {
                 let size = ty.size_of(self.struct_registry());
                 if size > max_payload_size { max_payload_size = size; }
             }
             resolved_variants.push((v.name.to_string(), p_ty, idx as i32));
        }

        // 6. Transactional Restore
        {
            *self.current_type_map_mut() = old_map;
            *self.current_generic_args_mut() = old_generic_args;
        }

        // Phase B: API Surface Discovery
        let methods = self.find_methods_for_template(template_name);
        for method_name in methods {
             // Skip generic methods. They require inference/turbofish at call site.
             // Registry stores full mangled name in 'name' field with empty path for Struct types.
             let key = crate::types::TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
             

             if let Some((func, _, _)) = self.trait_registry().get_legacy(&key, &method_name) {
                 if let Some(g) = &func.generics {
                     if !g.params.is_empty() {

                         continue; 
                     }
                 }
             }

             let full_name = format!("{}__{}", template_name, method_name);
             let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
             let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }


        Ok(EnumInfo {
            name: self.specialize_template(template_name, args, true)?.mangle(),
            variants: resolved_variants,
            max_payload_size,
            template_name: Some(template_name.to_string()),
            specialization_args: args.to_vec(),
        })
    }

}


pub use crate::codegen::types::zero_attr::zero_attr;
pub use crate::codegen::types::emit::{emit_const, emit_global_def};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::context::CodegenContext;
    use crate::registry::EnumInfo;
    use crate::grammar::SaltFile;

    #[test]
    fn test_enum_payload_packing() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let name = "PackingEnum".to_string();
        let variants = vec![
            ("A".to_string(), Some(Type::U8), 0),
            ("B".to_string(), Some(Type::Array(Box::new(Type::F64), 8, false)), 1),
        ];

        let info = EnumInfo {
            name: name.clone(),
            variants,
            max_payload_size: 64,
            template_name: None,
            specialization_args: vec![],
        };
        let key = TypeKey { path: vec![], name: name.clone(), specialization: None };
        ctx.enum_registry_mut().insert(key, info);

        let ty = Type::Enum(name);
        let mlir = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();
        // After enum type resolution fix: registered enums return their type alias
        // The inline struct definition with payload is emitted separately in type definitions
        assert_eq!(mlir, "!struct_PackingEnum", "Registered enum should use type alias");
    }

    // =========================================================================
    // TDD: Usize (MLIR index) ↔ I64 type conversion
    // =========================================================================
    // Bug context: The compiler generates MLIR `index` for `usize` params but
    // tracks them as `I64` in local_vars, causing `as i64` casts to be no-ops.
    // These tests ensure the conversion functions correctly emit arith.index_cast.

    #[test]
    fn test_usize_and_i64_are_distinct_types() {
        // CRITICAL: Type::Usize and Type::I64 must NOT be equal.
        // If they were, emit_cast's `if ty == target_ty` check would skip
        // the arith.index_cast, leaving index-typed values in i64 operations.
        assert_ne!(Type::Usize, Type::I64,
            "Type::Usize and Type::I64 must be distinct types");
        assert_ne!(Type::Usize, Type::U64,
            "Type::Usize and Type::U64 must be distinct types");
    }

    #[test]
    fn test_promote_numeric_usize_to_i64_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%arg_len", &Type::Usize, &Type::I64));

        assert!(result.is_ok(), "promote_numeric(Usize, I64) should succeed");
        assert!(out.contains("arith.index_cast"),
            "Usize→I64 must emit arith.index_cast, got: {}", out);
        assert!(out.contains("index to i64"),
            "Cast should be 'index to i64', got: {}", out);
    }

    #[test]
    fn test_promote_numeric_i64_to_usize_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%val", &Type::I64, &Type::Usize));

        assert!(result.is_ok(), "promote_numeric(I64, Usize) should succeed");
        assert!(out.contains("arith.index_cast"),
            "I64→Usize must emit arith.index_cast, got: {}", out);
        assert!(out.contains("i64 to index"),
            "Cast should be 'i64 to index', got: {}", out);
    }

    #[test]
    fn test_cast_numeric_usize_to_i64_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| cast_numeric(lctx, &mut out, "%arg_len", &Type::Usize, &Type::I64));

        assert!(result.is_ok(), "cast_numeric(Usize, I64) should succeed");
        assert!(out.contains("arith.index_cast"),
            "cast_numeric(Usize, I64) must emit arith.index_cast, got: {}", out);
    }

    #[test]
    fn test_usize_identity_does_not_emit_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%val", &Type::Usize, &Type::Usize));

        assert!(result.is_ok(), "promote_numeric(Usize, Usize) should succeed");
        assert!(out.is_empty(),
            "Usize→Usize should be identity (no MLIR emitted), got: {}", out);
    }

    // =========================================================================
    // TDD: Atomic<T> Type Emission — The Slab Memory Leak Root Cause
    // =========================================================================
    // Bug: Atomic<i32> globals emitted as `!llvm.ptr` with `null` init instead
    // of `i32` with `0 : i32` init. This causes LLVM Translation to reject the
    // MLIR with: "Global variable initializer type does not match global variable type!"
    //
    // Call graph layers to fix:
    //   Layer 0: to_mlir_type_simple(Atomic<T>) → T's MLIR type  [already works]
    //   Layer 1: zero_attr(Atomic<T>) → recurse to inner T
    //   Layer 2: to_mlir_storage_type_simple(Atomic<T>) → T's storage type
    //   Layer 3: emit_global_def sees Atomic<T> → unwraps to T for init_val

    // --- Layer 0: to_mlir_type_simple (already correct, assert for safety) ---
    #[test]
    fn test_atomic_i32_mlir_type_simple() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.to_mlir_type_simple(), "i32",
            "Atomic<i32> MLIR type should be 'i32', not '!llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_mlir_type_simple() {
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.to_mlir_type_simple(), "i64",
            "Atomic<u64> MLIR type should be 'i64'");
    }

    // --- Layer 1: zero_attr should recurse into inner type ---
    #[test]
    fn test_atomic_i32_zero_attr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ty = Type::Atomic(Box::new(Type::I32));
        let result = ctx.with_lowering_ctx(|lctx| zero_attr(lctx, &ty));
        assert!(result.is_ok(), "zero_attr(Atomic<i32>) should succeed");
        assert_eq!(result.unwrap(), "0 : i32",
            "zero_attr(Atomic<i32>) must be '0 : i32', not 'null : !llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_zero_attr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ty = Type::Atomic(Box::new(Type::U64));
        let result = ctx.with_lowering_ctx(|lctx| zero_attr(lctx, &ty));
        assert!(result.is_ok(), "zero_attr(Atomic<u64>) should succeed");
        assert_eq!(result.unwrap(), "0 : i64",
            "zero_attr(Atomic<u64>) must be '0 : i64', not 'null : !llvm.ptr'");
    }

    // --- Layer 2: to_mlir_storage_type_simple should unwrap to inner type ---
    #[test]
    fn test_atomic_i32_storage_type_simple() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.to_mlir_storage_type_simple(), "i32",
            "Atomic<i32> storage type should be 'i32', not '!llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_storage_type_simple() {
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.to_mlir_storage_type_simple(), "i64",
            "Atomic<u64> storage type should be 'i64'");
    }

    // --- Layer 3: k_is_ptr_type should NOT match Atomic ---
    #[test]
    fn test_atomic_is_not_ptr_type() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert!(!ty.k_is_ptr_type(),
            "Atomic<i32> is NOT a pointer type — it is a scalar wrapper");
    }

    // --- Layer 4: size_of should reflect inner type, not pointer ---
    #[test]
    fn test_atomic_i32_size_of() {
        let reg = std::collections::HashMap::new();
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.size_of(&reg), 4,
            "Atomic<i32> should be 4 bytes, not 8 (pointer size)");
    }

    #[test]
    fn test_atomic_u64_size_of() {
        let reg = std::collections::HashMap::new();
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.size_of(&reg), 8,
            "Atomic<u64> should be 8 bytes");
    }
}
