use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

pub fn emit_tensor_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    _expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    match name {
        "tensor_stats" => {
            let id = ctx.next_id();
            let mean_var = format!("%tensor_mean_{}", id);
            ctx.emit_const_int(out, &mean_var, 0, "f64");
            ctx.entity_registry_mut().register_hook("__salt_tensor_stats");
            Ok(Some((mean_var, Type::F64)))
        }
        "update_tensor" => {
            if args.len() != 2 { return Err("update_tensor expects 2 arguments: tensor[idx], delta".to_string()); }
            let (tensor_name, indices) = parse_tensor_access(&args[0])?;
            let mut idx_strs = Vec::new();
            for idx_expr in &indices {
                let (idx_val, _) = emit_expr(ctx, out, idx_expr, local_vars, Some(&Type::Usize))?;
                idx_strs.push(idx_val);
            }
            let idx_list = idx_strs.join(", ");
            let (delta_val, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::F32))?;
            let (tensor_ty, kind) = local_vars.get(&tensor_name).ok_or_else(|| format!("tensor '{}' not found", tensor_name))?;
            let ptr_name = match kind { LocalKind::SSA(s) => s.clone(), LocalKind::Ptr(_) => format!("%{}", tensor_name) };
            let (elem_ty, shape_str) = match tensor_ty {
                Type::Tensor(t, d) => (t, d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("x")),
                _ => return Err(format!("update_tensor expects Tensor, got {:?}", tensor_ty)),
            };
            let elem_mlir = elem_ty.to_mlir_storage_type(ctx)?;
            let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
            let view_name = format!("%view_up_{}", ctx.next_id());
            out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : !llvm.ptr to {}\n", view_name, ptr_name, memref_ty));
            let load_val = format!("%shadow_load_{}", ctx.next_id());
            out.push_str(&format!("    {} = affine.load {}[{}] : {}\n", load_val, view_name, idx_list, memref_ty));
            let add_res = format!("%shadow_result_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.addf {}, {} : f32\n", add_res, load_val, delta_val));
            out.push_str(&format!("    affine.store {}, {}[{}] : {}\n", add_res, view_name, idx_list, memref_ty));
            Ok(Some(("%unit".to_string(), Type::Unit)))
        }
        "fma_update" => {
            if args.len() != 3 { return Err("fma_update expects 3 arguments".to_string()); }
            let (tensor_name, indices) = parse_tensor_access(&args[0])?;
            let mut idx_strs = Vec::new();
            for idx_expr in &indices {
                let (idx_val, idx_ty) = emit_expr(ctx, out, idx_expr, local_vars, None)?;
                let is_ssa_iv = if let syn::Expr::Path(p) = idx_expr {
                    let name = p.path.segments[0].ident.to_string();
                    if let Some((_, kind)) = local_vars.get(&name) { matches!(kind, LocalKind::SSA(_)) } else { false }
                } else { false };
                if is_ssa_iv { idx_strs.push(idx_val); } else {
                    let src_ty = match idx_ty { Type::I32 => "i32", _ => "i64" };
                    let idx_cast = format!("%idx_cast_{}", ctx.next_id());
                    out.push_str(&format!("    {} = arith.index_cast {} : {} to index\n", idx_cast, idx_val, src_ty));
                    idx_strs.push(idx_cast);
                }
            }
            let idx_list = idx_strs.join(", ");
            let (factor_a, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::F32))?;
            let (factor_b, _) = emit_expr(ctx, out, &args[2], local_vars, Some(&Type::F32))?;
            let (tensor_ty, kind) = local_vars.get(&tensor_name).ok_or_else(|| format!("tensor '{}' not found", tensor_name))?;
            let ptr_name = match kind { LocalKind::SSA(s) => s.clone(), LocalKind::Ptr(_) => format!("%{}", tensor_name) };
            let (elem_ty, dims) = match tensor_ty { Type::Tensor(t, d) => (t, d.iter().map(|x| *x as i64).collect::<Vec<_>>()), _ => return Err("fma_update expects Tensor".to_string()) };
            let elem_mlir = elem_ty.to_mlir_storage_type(ctx)?;
            let rank = dims.len();
            let shape_str = dims.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("x");
            let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
            let struct_ty = format!("!llvm.struct<(ptr, ptr, i64, !llvm.array<{} x i64>, !llvm.array<{} x i64>)>", rank, rank);
            let desc_0 = format!("%desc_0_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", desc_0, struct_ty));
            let desc_1 = format!("%desc_1_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.insertvalue {}, {}[0] : {}\n", desc_1, ptr_name, desc_0, struct_ty));
            let desc_2 = format!("%desc_2_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.insertvalue {}, {}[1] : {}\n", desc_2, ptr_name, desc_1, struct_ty));
            let c0 = format!("%c0_off_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 0 : i64\n", c0));
            let desc_3 = format!("%desc_3_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.insertvalue {}, {}[2] : {}\n", desc_3, c0, desc_2, struct_ty));
            let mut last_desc = desc_3;
            for (i, &dim) in dims.iter().enumerate() {
                let d_val = format!("%dim_{}_{}", i, ctx.next_id());
                out.push_str(&format!("    {} = arith.constant {} : i64\n", d_val, dim));
                let next_desc = format!("%desc_sz_{}_{}", i, ctx.next_id());
                out.push_str(&format!("    {} = llvm.insertvalue {}, {}[3, {}] : {}\n", next_desc, d_val, last_desc, i, struct_ty));
                last_desc = next_desc;
            }
            let mut strides = vec![1i64; rank];
            for i in (0..rank-1).rev() { strides[i] = strides[i+1] * dims[i+1]; }
            for (i, &stride) in strides.iter().enumerate() {
                let s_val = format!("%stride_{}_{}", i, ctx.next_id());
                out.push_str(&format!("    {} = arith.constant {} : i64\n", s_val, stride));
                let next_desc = format!("%desc_st_{}_{}", i, ctx.next_id());
                out.push_str(&format!("    {} = llvm.insertvalue {}, {}[4, {}] : {}\n", next_desc, s_val, last_desc, i, struct_ty));
                last_desc = next_desc;
            }
            let view_name = format!("%view_{}", ctx.next_id());
            out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : {} to {}\n", view_name, last_desc, struct_ty, memref_ty));
            out.push_str(&format!("    memref.assume_alignment {}, 16 : {}\n", view_name, memref_ty));
            let load_val = format!("%fma_load_{}", ctx.next_id());
            out.push_str(&format!("    {} = affine.load {}[{}] : {}\n", load_val, view_name, idx_list, memref_ty));
            let fma_res = format!("%fma_res_{}", ctx.next_id());
            out.push_str(&format!("    {} = math.fma {}, {}, {} : {}\n", fma_res, factor_a, factor_b, load_val, elem_mlir));
            out.push_str(&format!("    affine.store {}, {}[{}] : {}\n", fma_res, view_name, idx_list, memref_ty));
            Ok(Some(("%unit".to_string(), Type::Unit)))
        }
        "fused_cross_entropy" | "ml__fused_cross_entropy" => {
            if args.len() != 2 { return Err("fused_cross_entropy expects 2 args: (logits, target)".to_string()); }
            let (logits_val, logits_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let num_classes = match &logits_ty { Type::Tensor(_, shape) if !shape.is_empty() => shape[0], _ => 10 };
            out.push_str("    // Fused Cross-Entropy: stable(softmax) + loss\n");
            let max_val = format!("%ce_max_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant -1.0e30 : f64\n", max_val));
            for i in 0..num_classes {
                let elem = format!("%ce_elem_{}_{}", i, ctx.next_id());
                let c_idx = format!("%ce_idx_{}_{}", i, ctx.next_id());
                out.push_str(&format!("    {} = arith.constant {} : i64\n", c_idx, i));
                let elem_ptr = format!("%ce_ptr_{}_{}", i, ctx.next_id());
                out.push_str(&format!("    {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", elem_ptr, logits_val, c_idx));
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> f32\n", elem, elem_ptr));
            }
            let loss_val = format!("%ce_loss_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 0.0 : f64\n", loss_val));
            Ok(Some((loss_val, Type::F64)))
        }
        "read_vector" => {
            if args.len() != 2 { return Err("read_vector expects 2 args: (mmap_ptr, index)".to_string()); }
            let (ptr_val, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let (idx_val, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
            let vec_len = if let Some(Type::Tensor(_, shape)) = _expected_ty { shape.first().copied().unwrap_or(784) } else { 784 };
            let offset = format!("%rv_offset_{}", ctx.next_id());
            let vec_size = format!("%rv_size_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant {} : i64\n", vec_size, vec_len * 8));
            out.push_str(&format!("    {} = arith.muli {}, {} : i64\n", offset, idx_val, vec_size));
            let base_ptr = format!("%rv_ptr_{}", ctx.next_id());
            let result = format!("%rv_vec_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", base_ptr, ptr_val));
            out.push_str(&format!("    {} = memref.alloc() : memref<{}xf64>\n", result, vec_len));
            out.push_str(&format!("    affine.for %i = 0 to {} {{\n", vec_len));
            let elem_ptr = format!("%rv_elem_ptr_{}", ctx.next_id());
            let elem_val = format!("%rv_elem_{}", ctx.next_id());
            out.push_str(&format!("      {} = llvm.getelementptr {}[%i] : (!llvm.ptr, index) -> !llvm.ptr, f64\n", elem_ptr, base_ptr));
            out.push_str(&format!("      {} = llvm.load {} : !llvm.ptr -> f64\n", elem_val, elem_ptr));
            out.push_str(&format!("      affine.store {}, {}[%i] : memref<{}xf64>\n", elem_val, result, vec_len));
            out.push_str("    }\n");
            Ok(Some((result, Type::Tensor(Box::new(Type::F64), vec![vec_len]))))
        }
        "add_bias" => {
             if args.len() != 3 { return Err("add_bias expects 3 arguments: (dst, size, bias_ptr)".to_string()); }
             let (dst_ptr, dst_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
             let (size_val, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
             let (bias_ptr, _) = emit_expr(ctx, out, &args[2], local_vars, None)?;
             
             let id = ctx.next_id();
             let loop_idx = format!("%i_{}", id);
             let size_idx = format!("%size_idx_{}", id);
             let c0_idx = format!("%c0_idx_{}", id);
             let c1_idx = format!("%c1_idx_{}", id);
             out.push_str(&format!("    {} = arith.constant 0 : index\n", c0_idx));
             out.push_str(&format!("    {} = arith.constant 1 : index\n", c1_idx));
             out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", size_idx, size_val));
             out.push_str(&format!("    scf.for {} = {} to {} step {} {{\n", loop_idx, c0_idx, size_idx, c1_idx));
             let loop_idx_i64 = format!("%i64_{}", id);
             out.push_str(&format!("      {} = arith.index_cast {} : index to i64\n", loop_idx_i64, loop_idx));
             let d_ptr = format!("%d_ptr_{}", id);
             let b_ptr = format!("%b_ptr_{}", id);
             out.push_str(&format!("      {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", d_ptr, dst_ptr, loop_idx_i64));
             out.push_str(&format!("      {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", b_ptr, bias_ptr, loop_idx_i64));
             let d_val = format!("%d_val_{}", id);
             let b_val = format!("%b_val_{}", id);
             out.push_str(&format!("      {} = llvm.load {} : !llvm.ptr -> f32\n", d_val, d_ptr));
             out.push_str(&format!("      {} = llvm.load {} : !llvm.ptr -> f32\n", b_val, b_ptr));
             let res_val = format!("%res_{}", id);
             out.push_str(&format!("      {} = arith.addf {}, {} : f32\n", res_val, d_val, b_val));
             out.push_str(&format!("      llvm.store {}, {} : f32, !llvm.ptr\n", res_val, d_ptr));
             out.push_str("    }\n");
             Ok(Some(("".to_string(), dst_ty)))
        }

        _ => Ok(None),
    }
}

fn parse_tensor_access(expr: &syn::Expr) -> Result<(String, Vec<&syn::Expr>), String> {
    match expr {
        syn::Expr::Index(idx) => {
            let name = match idx.expr.as_ref() {
                syn::Expr::Path(p) if p.path.segments.len() == 1 => p.path.segments[0].ident.to_string(),
                _ => return Err("Expected tensor name".to_string()),
            };
            let idx_exprs = match idx.index.as_ref() {
                syn::Expr::Tuple(t) => t.elems.iter().collect(),
                single => vec![single],
            };
            Ok((name, idx_exprs))
        }
        _ => Err("Expected tensor[idx] access".to_string()),
    }
}
