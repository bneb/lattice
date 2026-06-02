use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

pub fn emit_memory_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    match name {
        "reinterpret_cast" => {
            if let Some(target) = expected_ty {
                if let Some(arg) = args.first() {
                    // Provenance-aware codegen (base + offset)
                    if let Type::Reference(_inner_ty, _) = target {
                        if let syn::Expr::Binary(bin_expr) = arg {
                            if matches!(bin_expr.op, syn::BinOp::Add(_)) {
                                if let syn::Expr::Path(path) = &*bin_expr.left {
                                    if path.path.segments.len() == 1 {
                                        let (base_val, base_ty) = emit_expr(ctx, out, &bin_expr.left, local_vars, None)?;
                                        if matches!(base_ty, Type::U64) {
                                            let (offset_val, _) = emit_expr(ctx, out, &bin_expr.right, local_vars, Some(&Type::U64))?;
                                            let base_ptr = format!("%prov_base_ptr_{}", ctx.next_id());
                                            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", base_ptr, base_val));
                                            let res = format!("%prov_gep_{}", ctx.next_id());
                                            out.push_str(&format!("    {} = llvm.getelementptr inbounds {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, i8\n",
                                                res, base_ptr, offset_val));
                                            ctx.emission.ephemeral_refs.insert(res.clone());
                                            return Ok(Some((res, target.clone())));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Standard path
                    let (val, ty) = emit_expr(ctx, out, arg, local_vars, None)?;
                    if ty.k_is_ptr_type() && target.k_is_ptr_type() {
                        if matches!(target, Type::Reference(_, _)) {
                            ctx.emission.ephemeral_refs.insert(val.clone());
                        }
                        return Ok(Some((val, target.clone())));
                    }

                    let target_mlir = target.to_mlir_type(ctx)?;
                    let ty_mlir = ty.to_mlir_type(ctx)?;
                    let res = format!("%cast_{}", ctx.next_id());

                    if target_mlir == ty_mlir {
                         return Ok(Some((val, target.clone())));
                    }

                    if ty_mlir == "!llvm.ptr" && target_mlir.starts_with("i") {
                         out.push_str(&format!("    {} = llvm.ptrtoint {} : {} to {}\n", res, val, ty_mlir, target_mlir));
                    } else if ty_mlir.starts_with("i") && target_mlir == "!llvm.ptr" {
                         out.push_str(&format!("    {} = llvm.inttoptr {} : {} to {}\n", res, val, ty_mlir, target_mlir));
                    } else if ty_mlir.starts_with("!llvm.struct") || ty_mlir.starts_with("!struct_") ||
                              target_mlir.starts_with("!llvm.struct") || target_mlir.starts_with("!struct_") {
                         let tmp_ptr = format!("%cast_ptr_{}", ctx.next_id());
                         let one_id = ctx.next_id();
                         out.push_str(&format!("    %cast_one_{} = arith.constant 1 : i64\n", one_id));
                         out.push_str(&format!("    {} = llvm.alloca %cast_one_{} x {} : (i64) -> !llvm.ptr\n", tmp_ptr, one_id, ty_mlir));
                         out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", val, tmp_ptr, ty_mlir));
                         out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, tmp_ptr, target_mlir));
                    } else {
                         out.push_str(&format!("    {} = llvm.bitcast {} : {} to {}\n", res, val, ty_mlir, target_mlir));
                    }
                    if matches!(target, Type::Reference(_, _)) {
                        ctx.emission.ephemeral_refs.insert(res.clone());
                    }
                    return Ok(Some((res, target.clone())));
                }
            }
            Err("reinterpret_cast intrinsic requires expected type context".to_string())
        }
        "size_of" | "intrin__size_of" | "std__core__mem__intrin__size_of" => {
            let size = expected_ty.map(|t| ctx.size_of(t)).unwrap_or(8);
            let res = format!("%size_of_{}", ctx.next_id());
            ctx.emit_const_int(out, &res, size as i64, "i64");
            Ok(Some((res, Type::I64)))
        }
        "align_of" | "intrin__align_of" | "std__core__mem__intrin__align_of" => {
            let align = expected_ty.map(|t| ctx.align_of(t)).unwrap_or(8);
            let res = format!("%align_of_{}", ctx.next_id());
            ctx.emit_const_int(out, &res, align as i64, "i64");
            Ok(Some((res, Type::I64)))
        }
        "ref_to_addr" | "intrin__ref_to_addr" | "std__core__ptr__intrin__ref_to_addr" => {
            if args.len() != 1 {
                return Err("ref_to_addr expects 1 argument".to_string());
            }
            let (arg_val, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let res = format!("%ref_addr_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, arg_val));
            Ok(Some((res, Type::I64)))
        }
        "std__core__slab_alloc__intrin__zeroed" | "zeroed" | "intrin__zeroed" | "std__core__mem__intrin__zeroed" => {
             if args.len() == 2 {
                 let (ptr, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
                 let (size, _) = emit_expr(ctx, out, &args[1], local_vars, None)?;
                 let val = format!("%zero_{}", ctx.next_id());
                 ctx.emit_const_int(out, &val, 0, "i8");
                 out.push_str(&format!("    \"llvm.intr.memset\"({}, {}, {}, {}) : (!llvm.ptr, i8, i64, i1) -> ()\n", 
                     ptr, val, size, "false"));
                 Ok(Some(("".to_string(), Type::Unit)))
             } else {
                // Zero-initialize value of expected type
                if let Some(expected) = expected_ty {
                    let mlir_ty = expected.to_mlir_type(ctx)?;
                    let res = format!("%zeroed_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.mlir.zero : {}\n", res, mlir_ty));
                    return Ok(Some((res, expected.clone())));
                }
                Err("zeroed<T>() requires type inference context".to_string())
             }
        }
        "memset" | "intrin__memset" | "std__core__mem__memset" => {
            if args.len() == 3 {
                let (ptr_val, ptr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
                let (val_arg, val_ty) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I8))?;
                let (len_val, len_ty) = emit_expr(ctx, out, &args[2], local_vars, None)?;
                
                let ptr_llvm = if ptr_ty.to_mlir_storage_type(ctx)? == "!llvm.ptr" {
                    ptr_val
                } else {
                    let p = format!("%memset_ptr_{}", ctx.next_id());
                    ctx.emit_inttoptr(out, &p, &ptr_val, "i64");
                    p
                };
                
                let val_i8 = {
                    let val_mlir = val_ty.to_mlir_type(ctx)?;
                    if val_mlir != "i8" {
                        let trunc = format!("%memset_val_i8_{}", ctx.next_id());
                        out.push_str(&format!("    {} = arith.trunci {} : {} to i8\n", trunc, val_arg, val_mlir));
                        trunc
                    } else {
                        val_arg
                    }
                };
                
                let len_mlir = len_ty.to_mlir_type(ctx)?;
                let len_i64 = if len_mlir != "i64" {
                    let ext = format!("%memset_len_ext_{}", ctx.next_id());
                    out.push_str(&format!("    {} = arith.extsi {} : {} to i64\n", ext, len_val, len_mlir));
                    ext
                } else {
                    len_val
                };
                
                out.push_str(&format!("    \"llvm.intr.memset\"({}, {}, {}) <{{isVolatile = false}}> : (!llvm.ptr, i8, i64) -> ()\n",
                    ptr_llvm, val_i8, len_i64));
                Ok(Some(("".to_string(), Type::Unit)))
            } else {
                Err("memset(ptr, value, len) requires 3 arguments".to_string())
            }
        }
        "memcpy" | "intrin__memcpy" | "std__core__mem__memcpy" => {
            if args.len() == 3 {
                let (dst_val, dst_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
                let (src_val, src_ty) = emit_expr(ctx, out, &args[1], local_vars, None)?;
                let (len_val, len_ty) = emit_expr(ctx, out, &args[2], local_vars, None)?;
                
                let dst_ptr = if dst_ty.to_mlir_storage_type(ctx)? == "!llvm.ptr" {
                    dst_val
                } else {
                    let p = format!("%memcpy_dst_ptr_{}", ctx.next_id());
                    ctx.emit_inttoptr(out, &p, &dst_val, "i64");
                    p
                };
                let src_ptr = if src_ty.to_mlir_storage_type(ctx)? == "!llvm.ptr" {
                    src_val
                } else {
                    let p = format!("%memcpy_src_ptr_{}", ctx.next_id());
                    ctx.emit_inttoptr(out, &p, &src_val, "i64");
                    p
                };
                
                let len_mlir = len_ty.to_mlir_type(ctx)?;
                let len_i64 = if len_mlir != "i64" {
                    let ext = format!("%memcpy_len_ext_{}", ctx.next_id());
                    out.push_str(&format!("    {} = arith.extsi {} : {} to i64\n", ext, len_val, len_mlir));
                    ext
                } else {
                    len_val
                };
                
                out.push_str(&format!("    \"llvm.intr.memcpy\"({}, {}, {}) <{{isVolatile = false}}> : (!llvm.ptr, !llvm.ptr, i64) -> ()\n",
                    dst_ptr, src_ptr, len_i64));
                Ok(Some(("".to_string(), Type::Unit)))
            } else {
                Err("memcpy(dst, src, len) requires 3 arguments".to_string())
            }
        }
        "unreachable" | "intrin__unreachable" => {
            let ret_ty = expected_ty.cloned().unwrap_or(Type::Unit);
            if ret_ty != Type::Unit {
                let mlir_ty = ret_ty.to_mlir_type(ctx)?;
                out.push_str(&format!("    %unreachable = llvm.mlir.undef : {}\n", mlir_ty));
            }
            out.push_str("    llvm.unreachable\n");
            Ok(Some(("%unreachable".to_string(), ret_ty)))
        }
        name if name.contains("ptr_is_null") || name == "is_null" => {
            if args.is_empty() {
                return Err("Intrinsic 'ptr_is_null' expects 1 argument: (ptr)".to_string());
            }
            let (mut ptr, mut ptr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            
            if let Type::Reference(inner, _) = &ptr_ty {
                let load_val = format!("%loaded_ptr_{}", ctx.next_id());
                ctx.emit_load_logical(out, &load_val, &ptr, inner)?;
                ptr = load_val;
                ptr_ty = (**inner).clone();
            }
            
            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let is_aggregate = matches!(ptr_ty, Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _));
            let loaded_ptr = if is_aggregate {
                let load_val = format!("%loaded_struct_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", load_val, ptr, struct_ty));
                load_val
            } else {
                ptr.clone()
            };
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                loaded_ptr
            } else if struct_ty == "i64" {
                let raw = format!("%raw_null_ptr_{}", ctx.next_id());
                ctx.emit_inttoptr(out, &raw, &loaded_ptr, "i64");
                raw
            } else {
                let val_i64 = format!("%ptr_val_null_{}", ctx.next_id());
                ctx.emit_extractvalue(out, &val_i64, &loaded_ptr, 0, &struct_ty);
                let raw = format!("%raw_null_ptr_{}", ctx.next_id());
                ctx.emit_inttoptr(out, &raw, &val_i64, "i64");
                raw
            };
            
            let null_ptr = format!("%null_ptr_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.mlir.zero : !llvm.ptr\n", null_ptr));
            
            let res = format!("%is_null_res_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.icmp \"eq\" {}, {} : !llvm.ptr\n", res, raw_ptr, null_ptr));
            
            Ok(Some((res, Type::Bool)))
        }
        name if name.contains("ptr_offset") => {
            if args.len() != 2 {
                return Err("Intrinsic 'ptr_offset' expects 2 arguments: (ptr, count)".to_string());
            }
            let (mut ptr, mut ptr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            
            if let Type::Reference(inner, _) = &ptr_ty {
                let load_val = format!("%loaded_ptr_{}", ctx.next_id());
                ctx.emit_load_logical(out, &load_val, &ptr, inner)?;
                ptr = load_val;
                ptr_ty = (**inner).clone();
            }
            
            let (count, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
            
            let elem_ty = if let Type::Concrete(name, args) = &ptr_ty {
                if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                    args[0].to_mlir_type(ctx)?
                } else {
                    "i8".to_string()
                }
            } else if let Type::Struct(name) = &ptr_ty {
                let inner_name = if let Some(suffix) = name.strip_suffix("_Ptr") {
                    Some(suffix.to_string())
                } else if name.contains("Ptr_") {
                    name.rsplit_once("Ptr_").map(|(_, inner)| inner.to_string())
                } else {
                    None
                };
                
                if let Some(inner) = inner_name {
                    match inner.as_str() {
                        "u8" | "i8" => "i8".to_string(),
                        "u16" | "i16" => "i16".to_string(),
                        "u32" | "i32" => "i32".to_string(),
                        "u64" | "i64" => "i64".to_string(),
                        "f32" => "f32".to_string(),
                        "f64" => "f64".to_string(),
                        struct_name => {
                            let inner_ty = Type::Struct(struct_name.to_string());
                            inner_ty.to_mlir_storage_type(ctx).unwrap_or_else(|_| "i8".to_string())
                        }
                    }
                } else {
                    "i8".to_string()
                }
            } else if let Type::Pointer { element, .. } = &ptr_ty {
                element.to_mlir_type(ctx)?
            } else {
                "i8".to_string()
            };
            
            let res = format!("%ptr_offset_{}", ctx.next_id());
            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let is_aggregate = matches!(ptr_ty, Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _));
            let loaded_ptr = if is_aggregate {
                let load_val = format!("%loaded_struct_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", load_val, ptr, struct_ty));
                load_val
            } else {
                ptr.clone()
            };
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                loaded_ptr
            } else {
                let val_i64 = if struct_ty == "i64" {
                    loaded_ptr
                } else {
                    let val_i64 = format!("%ptr_val_{}", ctx.next_id());
                    ctx.emit_extractvalue(out, &val_i64, &loaded_ptr, 0, &struct_ty);
                    val_i64
                };
                
                let raw = format!("%raw_ptr_{}", ctx.next_id());
                ctx.emit_inttoptr(out, &raw, &val_i64, "i64");
                raw
            };
            
            let gep_ptr = format!("%gep_ptr_{}", ctx.next_id());
            ctx.emit_gep(out, &gep_ptr, &raw_ptr, &count, &elem_ty);
            
            if struct_ty == "!llvm.ptr" {
                return Ok(Some((gep_ptr, ptr_ty)));
            }

            let new_addr = format!("%new_addr_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", new_addr, gep_ptr));
            
            if struct_ty == "i64" {
                 return Ok(Some((new_addr, ptr_ty)));
            } else {
                out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", res, struct_ty));
                let res_final = format!("%res_final_{}", ctx.next_id());
                ctx.emit_insertvalue(out, &res_final, &new_addr, &res, 0, &struct_ty);
                return Ok(Some((res_final, ptr_ty)));
            }
        }
        name if name.contains("ptr_read") => {
            if args.is_empty() {
                return Err("Intrinsic 'ptr_read' / 'ptr_read_at' expects 1-2 arguments: (ptr) or (ptr, index)".to_string());
            }
            let (mut ptr, mut ptr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            
            if let Type::Reference(inner, _) = &ptr_ty {
                let load_val = format!("%loaded_ptr_{}", ctx.next_id());
                ctx.emit_load_logical(out, &load_val, &ptr, inner)?;
                ptr = load_val;
                ptr_ty = (**inner).clone();
            }
            
            let index_val = if args.len() >= 2 {
                let (idx, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
                Some(idx)
            } else {
                None
            };
            
            let inner_ty = if let Type::Concrete(name, args) = &ptr_ty {
                if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                    args[0].clone()
                } else { return Err(format!("ptr_read expected Ptr<T>, got {:?}", ptr_ty)); }
            } else if let Type::Struct(name) = &ptr_ty {
                if name.ends_with("_u8") { Type::U8 }
                else if name.ends_with("_i64") { Type::I64 }
                else { return Err(format!("ptr_read expected Ptr<T>, got Struct {}", name)); }
            } else if let Type::Pointer { element, .. } = &ptr_ty {
                (**element).clone()
            } else {
                return Err("ptr_read expects a pointer type".to_string());
            };
            
            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let is_aggregate = matches!(ptr_ty, Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _));
            let loaded_ptr = if is_aggregate {
                let load_val = format!("%loaded_struct_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", load_val, ptr, struct_ty));
                load_val
            } else {
                ptr.clone()
            };
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                 loaded_ptr
            } else {
                let val_i64 = if struct_ty == "i64" {
                    loaded_ptr
                } else {
                    let val = format!("%ptr_val_r_{}", ctx.next_id());
                    ctx.emit_extractvalue(out, &val, &loaded_ptr, 0, &struct_ty);
                    val
                };
                
                let raw = format!("%raw_ptr_r_{}", ctx.next_id());
                ctx.emit_inttoptr(out, &raw, &val_i64, "i64");
                raw
            };
            
            let load_ptr = if let Some(idx) = index_val {
                let elem_ty = inner_ty.to_mlir_type(ctx)?;
                let gep = format!("%ptr_read_gep_{}", ctx.next_id());
                ctx.emit_gep(out, &gep, &raw_ptr, &idx, &elem_ty);
                gep
            } else {
                raw_ptr
            };
            
            let res = format!("%ptr_read_{}", ctx.next_id());
            ctx.emit_load_logical(out, &res, &load_ptr, &inner_ty)?;
            return Ok(Some((res, inner_ty)));
        }
        name if name.contains("ptr_write") => {
            if args.len() != 2 {
                return Err("Intrinsic 'ptr_write' expects 2 arguments: (ptr, value)".to_string());
            }
            let (mut ptr, mut ptr_ty) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            
            if let Type::Reference(inner, _) = &ptr_ty {
                let load_val = format!("%loaded_ptr_{}", ctx.next_id());
                ctx.emit_load_logical(out, &load_val, &ptr, inner)?;
                ptr = load_val;
                ptr_ty = (**inner).clone();
            }
            
            let inner_ty = if let Type::Concrete(name, args) = &ptr_ty {
                if (name.ends_with("Ptr") || name.contains("Ptr")) && !args.is_empty() {
                    args[0].clone()
                } else { return Err(format!("ptr_write expected Ptr<T>, got {:?}", ptr_ty)); }
            } else if let Type::Struct(name) = &ptr_ty {
                if name.ends_with("_u8") { Type::U8 }
                else if name.ends_with("_i64") { Type::I64 }
                else { return Err(format!("ptr_write expected Ptr<T>, got Struct {}", name)); }
            } else if let Type::Pointer { element, .. } = &ptr_ty {
                (**element).clone()
            } else {
                return Err("ptr_write expects a pointer type".to_string());
            };
            
            let (val, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&inner_ty))?;
            
            let struct_ty = ptr_ty.to_mlir_storage_type(ctx)?;
            
            let is_aggregate = matches!(ptr_ty, Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _));
            let loaded_ptr = if is_aggregate {
                let load_val = format!("%loaded_struct_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", load_val, ptr, struct_ty));
                load_val
            } else {
                ptr.clone()
            };
            
            let raw_ptr = if struct_ty == "!llvm.ptr" {
                 loaded_ptr
            } else {
                let val_i64 = if struct_ty == "i64" {
                    loaded_ptr
                } else {
                    let val = format!("%ptr_val_w_{}", ctx.next_id());
                    ctx.emit_extractvalue(out, &val, &loaded_ptr, 0, &struct_ty);
                    val
                };
                
                let raw = format!("%raw_ptr_w_{}", ctx.next_id());
                ctx.emit_inttoptr(out, &raw, &val_i64, "i64");
                raw
            };
            
            ctx.emit_store_logical(out, &val, &raw_ptr, &inner_ty)?;
            return Ok(Some(("%unit".to_string(), Type::Unit)));
        }
        name if name.contains("from_ref") => {
             if let Some(arg) = args.first() {
                 let (val_var, _) = emit_expr(ctx, out, arg, local_vars, None)?;
                 
                 let addr_var = format!("%addr_{}", ctx.next_id());
                 out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", addr_var, val_var));
                 
                 if let Some(expected) = expected_ty {
                      let struct_ty = expected.to_mlir_storage_type(ctx)?;
                      if struct_ty == "i64" {
                           return Ok(Some((addr_var, expected.clone())));
                      } else {
                          let res = format!("%from_ref_{}", ctx.next_id());
                          out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", res, struct_ty));
                          let res_final = format!("%res_final_{}", ctx.next_id());
                          ctx.emit_insertvalue(out, &res_final, &addr_var, &res, 0, &struct_ty);
                          return Ok(Some((res_final, expected.clone())));
                      }
                 }
             }
             Err("from_ref requires expected type context".to_string())
        }
        _ => Ok(None),
    }
}
