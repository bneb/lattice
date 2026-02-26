use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use super::utils::*;
use crate::codegen::type_bridge::*;
use std::collections::HashMap;
use super::{emit_expr, emit_lvalue, LValueKind, extract_field_assign_receiver};
use super::aggregate_eq::emit_aggregate_eq;

pub fn emit_binary(ctx: &mut LoweringContext, out: &mut String, b: &syn::ExprBinary, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected: Option<&Type>) -> Result<(String, Type), String> {
    if matches!(b.op, syn::BinOp::AddAssign(_) | syn::BinOp::SubAssign(_) | syn::BinOp::MulAssign(_) | syn::BinOp::DivAssign(_) | syn::BinOp::RemAssign(_) | syn::BinOp::BitAndAssign(_) | syn::BinOp::BitOrAssign(_) | syn::BinOp::BitXorAssign(_) | syn::BinOp::ShlAssign(_) | syn::BinOp::ShrAssign(_)) {
        return emit_compound_assign(ctx, out, b, local_vars);
    }

    let is_cmp = matches!(b.op, syn::BinOp::Eq(_) | syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_) | syn::BinOp::Ne(_));
    let is_logic = matches!(b.op, syn::BinOp::And(_) | syn::BinOp::Or(_));
    
    // Determine hint for LHS
    // [V25.1] Domain Isolation: Never pass Pointer hints to arithmetic operands
    // Pointer types contaminate index expressions (Type Osmosis)
    let lhs_expected = if is_logic {
        Some(&Type::Bool)
    } else if is_cmp {
        None // Comparisons don't hint operands with Bool
    } else if let Some(exp) = expected {
        // Strip Pointer types - they don't apply to arithmetic operands
        if exp.k_is_ptr_type() { None } else { Some(exp) }
    } else {
        None
    };

    let (lhs_val, lhs_ty) = emit_expr(ctx, out, &b.left, local_vars, lhs_expected)?;
    
    if is_logic {
         return emit_logic(ctx, out, b, (lhs_val, lhs_ty), local_vars);
    }

    // Determine hint for RHS
    // [V25.1.1] Domain Isolation: Strip Pointer from RHS hint
    // For pointer arithmetic (Ptr + int), RHS must be integer - not contaminated with Pointer
    let rhs_expected = if lhs_ty.k_is_ptr_type() { None } else { Some(&lhs_ty) };
    
    let (rhs_val, rhs_ty) = emit_expr(ctx, out, &b.right, local_vars, rhs_expected)?;

    // Validation: Bitwise ops require integers
    if matches!(b.op, syn::BinOp::BitAnd(_) | syn::BinOp::BitOr(_) | syn::BinOp::BitXor(_) | syn::BinOp::Shl(_) | syn::BinOp::Shr(_)) {
        if !lhs_ty.is_integer() || (!rhs_ty.is_integer() && !matches!(b.op, syn::BinOp::Shl(_) | syn::BinOp::Shr(_))) {
             // Allow shift by distinct integer type, but others must match or be integer
             // Actually strict mode: RHS of shift is also integer.
             if !rhs_ty.is_integer() {
                 return Err(format!("Bitwise operator requires integer operands, found {:?} and {:?}", lhs_ty, rhs_ty));
             }
        }
        if !lhs_ty.is_integer() || !rhs_ty.is_integer() {
             return Err(format!("Bitwise operator requires integer operands, found {:?} and {:?}", lhs_ty, rhs_ty));
        }
    }

    // [SOVEREIGN V5.2] Type-Aware Pointer Addition
    // Enables: let next = ptr + offset; (Native GEP Lowering)
    if matches!(b.op, syn::BinOp::Add(_)) {
        // Check Pointer/Reference + Integer pattern
        let lhs_is_ptr = matches!(lhs_ty, Type::Pointer { .. });
        let rhs_is_ptr = matches!(rhs_ty, Type::Pointer { .. });
        
        if lhs_is_ptr && rhs_ty.is_integer() {
            // Extract the element type
            let elem_ty = match &lhs_ty {

                Type::Pointer { element, .. } => *element.clone(),
                _ => return Err(format!("Cannot offset non-pointer type {:?}", lhs_ty)),
            };
            
            // Convert rhs to i64 for GEP
            let idx_i64 = promote_numeric(ctx, out, &rhs_val, &rhs_ty, &Type::I64)?;
            
            // Emit type-aware GEP
            let elem_mlir = elem_ty.to_mlir_type(ctx)?;
            let res = format!("%gep_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n",
                res, lhs_val, idx_i64, elem_mlir));
            
            // [V7.3] Propagate alias scope from base pointer to GEP result
            ctx.control_flow.propagate_scope_provenance(&lhs_val, &res);
            
            return Ok((res, lhs_ty.clone()));
        }
        
        // Also support Integer + Pointer (commutative)
        if lhs_ty.is_integer() && rhs_is_ptr {
            let elem_ty = match &rhs_ty {

                Type::Pointer { element, .. } => *element.clone(),
                _ => return Err(format!("Cannot offset non-pointer type {:?}", rhs_ty)),
            };
            
            let idx_i64 = promote_numeric(ctx, out, &lhs_val, &lhs_ty, &Type::I64)?;
            let elem_mlir = elem_ty.to_mlir_type(ctx)?;
            let res = format!("%gep_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n",
                res, rhs_val, idx_i64, elem_mlir));
            
            // [V7.3] Propagate alias scope from base pointer to GEP result
            ctx.control_flow.propagate_scope_provenance(&rhs_val, &res);
            
            return Ok((res, rhs_ty.clone()));
        }
    }

    // Validation: Arithmetic ops require numerics
    if matches!(b.op, syn::BinOp::Add(_) | syn::BinOp::Sub(_) | syn::BinOp::Mul(_) | syn::BinOp::Div(_) | syn::BinOp::Rem(_)) {
        if (!lhs_ty.is_numeric() && !matches!(lhs_ty, Type::Tensor(..))) || (!rhs_ty.is_numeric() && !matches!(rhs_ty, Type::Tensor(..))) {
             return Err(format!("Arithmetic operator requires numeric or tensor operands, found {:?} and {:?}", lhs_ty, rhs_ty));
        }
    }

    let common_ty = {
        let is_shift = matches!(b.op, syn::BinOp::Shl(_) | syn::BinOp::Shr(_));
        if is_shift {
            // For shifts, result type is LHS type. RHS (amount) must be cast to LHS width for LLVM.
            lhs_ty.clone()
        } else if matches!(lhs_ty, Type::Tensor(..)) && matches!(rhs_ty, Type::Tensor(..)) {
            // Tensor-Tensor operations
             if let (Type::Tensor(inner1, shape1), Type::Tensor(inner2, shape2)) = (&lhs_ty, &rhs_ty) {
                if matches!(b.op, syn::BinOp::Add(_)) {
                    if shape1 != shape2 {
                        return Err(format!("Tensor shape mismatch in Add: {:?} and {:?}", shape1, shape2));
                    }
                    if inner1 != inner2 {
                        return Err(format!("Tensor element type mismatch in Add: {:?} and {:?}", inner1, inner2));
                    }

                    // Use salt_add from Code Red bridge
                    let count: u64 = shape1.iter().map(|d| *d as u64).product();
                    let size_bytes = count * 4; // F32 only for now
                    
                    if !inner1.is_float() {
                         return Err(format!("salt_add only supports F32, found {:?}", inner1));
                    }

                    ctx.ensure_external_declaration("alloc", &[Type::U64], &Type::U64)?;
                    let size_val = format!("%sz_add_{}", ctx.next_id());
                    out.push_str(&format!("    {} = arith.constant {} : i64\n", size_val, size_bytes));
                    let ptr_u64 = format!("%ptr_add_u64_{}", ctx.next_id());
                    out.push_str(&format!("    {} = func.call @alloc({}) : (i64) -> i64\n", ptr_u64, size_val));
                    
                    let res_ptr = format!("%res_ptr_add_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", res_ptr, ptr_u64));

                    // void salt_add(float *a, float *b, float *c, uint64_t count)
                    ctx.ensure_external_declaration("salt_add", 
                        &[Type::Reference(Box::new(Type::F32), false), Type::Reference(Box::new(Type::F32), false), Type::Reference(Box::new(Type::F32), false), Type::U64], 
                        &Type::Unit)?;

                    let count_val = format!("%count_{}", ctx.next_id());
                    out.push_str(&format!("    {} = arith.constant {} : i64\n", count_val, count));

                    out.push_str(&format!("    func.call @salt_add({}, {}, {}, {}) : (!llvm.ptr, !llvm.ptr, !llvm.ptr, i64) -> ()\n",
                        lhs_val, rhs_val, res_ptr, count_val));

                    return Ok((res_ptr, lhs_ty.clone()));
                } else if matches!(b.op, syn::BinOp::Mul(_)) {
                    // Check if it's a matmul (2D x 2D)
                    if shape1.len() == 2 && shape2.len() == 2 && shape1[1] == shape2[0] {
                        let res_shape = vec![shape1[0], shape2[1]];
                        let _res_ty = Type::Tensor(inner1.clone(), res_shape.clone());

                        let m = shape1[0] as i64;
                        let k = shape1[1] as i64;
                        let n = shape2[1] as i64;

                        // [SOVEREIGN V5] Pure FFI Strategy: Direct C Call
                        // Avoids MLIR MemRef descriptor complexity entirely.
                        // salt_matmul(lhs, rhs, res, m, k, n) in ml_bridge.c
                        
                        // 1. Calculate Sizes & Allocate Result
                        let res_bytes = m * n * 4;
                        let res_sz = format!("%sz_res_{}", ctx.next_id());
                        out.push_str(&format!("    {} = arith.constant {} : i64\n", res_sz, res_bytes));
                        
                        let res_alloc_i64 = format!("%res_alloc_i64_{}", ctx.next_id());
                        out.push_str(&format!("    {} = func.call @alloc({}) : (i64) -> i64\n", res_alloc_i64, res_sz));
                        let res_raw_ptr = format!("%res_raw_ptr_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", res_raw_ptr, res_alloc_i64));
                        
                        // 2. Prepare Dimension Constants
                        let m_const = format!("%m_const_{}", ctx.next_id());
                        let k_const = format!("%k_const_{}", ctx.next_id());
                        let n_const = format!("%n_const_{}", ctx.next_id());
                        out.push_str(&format!("    {} = arith.constant {} : i64\n", m_const, m));
                        out.push_str(&format!("    {} = arith.constant {} : i64\n", k_const, k));
                        out.push_str(&format!("    {} = arith.constant {} : i64\n", n_const, n));
                        
                        // 3. Call salt_matmul(lhs, rhs, res, m, k, n)
                        // Emit extern declaration directly (salt_matmul is in ml_bridge.c)
                        if !ctx.is_function_defined("salt_matmul") {
                            ctx.definitions_buffer_mut().push_str("  func.func private @salt_matmul(!llvm.ptr, !llvm.ptr, !llvm.ptr, i64, i64, i64) -> ()\n");
                            ctx.external_decls_mut().insert("salt_matmul".to_string());
                        }
                        
                        out.push_str(&format!("    func.call @salt_matmul({}, {}, {}, {}, {}, {}) : (!llvm.ptr, !llvm.ptr, !llvm.ptr, i64, i64, i64) -> ()\n",
                            lhs_val, rhs_val, res_raw_ptr, m_const, k_const, n_const));
                        
                        // 4. Return Result Pointer
                        let res_ty = Type::Tensor(inner1.clone(), vec![shape1[0], shape2[1]]);
                        return Ok((res_raw_ptr, res_ty));     
                        
                    } 
                    // Support Matrix-Vector (2D x 1D) -> 1D
                    else if shape1.len() == 2 && shape2.len() == 1 && shape1[1] == shape2[0] {
                        let res_shape = vec![shape1[0]]; // [M]
                        let res_ty = Type::Tensor(inner1.clone(), res_shape.clone());
                        
                        let m = shape1[0] as i64;
                        let k = shape1[1] as i64;
                        
                        // [SOVEREIGN V5.1] Pure FFI Strategy: Direct C Call
                        // salt_matvec(lhs, rhs, res, m, k) in ml_bridge.c
                        
                        // 1. Calculate Sizes & Allocate Result
                        let res_bytes = m * 4;
                        let res_sz = format!("%sz_res_{}", ctx.next_id());
                        out.push_str(&format!("    {} = arith.constant {} : i64\n", res_sz, res_bytes));
                        
                        let res_alloc_i64 = format!("%res_alloc_i64_{}", ctx.next_id());
                        out.push_str(&format!("    {} = func.call @alloc({}) : (i64) -> i64\n", res_alloc_i64, res_sz));
                        let res_raw_ptr = format!("%res_raw_ptr_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", res_raw_ptr, res_alloc_i64));
                        
                        // 2. Prepare Dimension Constants
                        let m_const = format!("%m_const_{}", ctx.next_id());
                        let k_const = format!("%k_const_{}", ctx.next_id());
                        out.push_str(&format!("    {} = arith.constant {} : i64\n", m_const, m));
                        out.push_str(&format!("    {} = arith.constant {} : i64\n", k_const, k));
                        
                        // 3. Call salt_matvec(lhs, rhs, res, m, k)
                        if !ctx.is_function_defined("salt_matvec") {
                            ctx.definitions_buffer_mut().push_str("  func.func private @salt_matvec(!llvm.ptr, !llvm.ptr, !llvm.ptr, i64, i64) -> ()\n");
                            ctx.external_decls_mut().insert("salt_matvec".to_string());
                        }
                        
                        out.push_str(&format!("    func.call @salt_matvec({}, {}, {}, {}, {}) : (!llvm.ptr, !llvm.ptr, !llvm.ptr, i64, i64) -> ()\n",
                            lhs_val, rhs_val, res_raw_ptr, m_const, k_const));
                        
                        // 4. Return Result Pointer
                        return Ok((res_raw_ptr, res_ty));
                    }

                    // Else: Fallthrough not supported yet
                }
             }
            lhs_ty.clone()
        } else {
            let op_max = if lhs_ty.is_numeric() && rhs_ty.is_numeric() {
                // [V25.3] Special case: Usize (MLIR index) mixed with I64/U64
                // MLIR's index type is semantically different from i64 - always promote to i64
                match (&lhs_ty, &rhs_ty) {
                    (Type::Usize, Type::I64) | (Type::Usize, Type::U64) => rhs_ty.clone(),
                    (Type::I64, Type::Usize) | (Type::U64, Type::Usize) => lhs_ty.clone(),
                    _ => {
                        if lhs_ty.size_of(&*ctx.struct_registry()) >= rhs_ty.size_of(&*ctx.struct_registry()) { lhs_ty.clone() } else { rhs_ty.clone() }
                    }
                }
            } else {
                lhs_ty.clone()
            };

            if let Some(exp) = expected {
                if is_cmp {
                    op_max
                } else if exp.is_numeric() && op_max.is_numeric() {
                    if exp.size_of(&*ctx.struct_registry()) >= op_max.size_of(&*ctx.struct_registry()) { exp.clone() } else { op_max }
                } else {
                    exp.clone()
                }
            } else {
                op_max
            }
        }
    };

    let lhs_prom = crate::codegen::type_bridge::promote_numeric(ctx, out, &lhs_val, &lhs_ty, &common_ty)?;
    
    // For shifts, we force-cast the RHS (amount) to match LHS width (LLVM requirement).
    // This allows implicit narrowing of shift amount (e.g. i32 << 4_i64).
    let rhs_prom = if matches!(b.op, syn::BinOp::Shl(_) | syn::BinOp::Shr(_)) {
        crate::codegen::type_bridge::cast_numeric(ctx, out, &rhs_val, &rhs_ty, &common_ty)?
    } else {
        crate::codegen::type_bridge::promote_numeric(ctx, out, &rhs_val, &rhs_ty, &common_ty)?
    };
    
    let res = format!("%bin_{}", ctx.next_id());
    let op = get_arith_op(&b.op, &common_ty);
    let mlir_ty = common_ty.to_mlir_type(ctx)?;

    if op.contains("cmp") {
        if matches!(common_ty, Type::Struct(_) | Type::Concrete(..) | Type::Tuple(_) | Type::Array(..) | Type::Enum(_)) {
            // [SOVEREIGN V7.0] Check for trait-based eq before structural comparison
            // Handles both Type::Struct("name") and Type::Concrete("name", []) —
            // String values resolve as Concrete("std__string__String", [])
            let struct_name_opt = match &common_ty {
                Type::Struct(name) => Some(name.clone()),
                Type::Concrete(name, _) => Some(name.clone()),
                _ => None,
            };
            if let Some(struct_name) = struct_name_opt {
                let is_eq = matches!(b.op, syn::BinOp::Eq(_));
                let is_ne = matches!(b.op, syn::BinOp::Ne(_));
                if is_eq || is_ne {
                    // Resolve struct name through type system (handles unqualified -> qualified)
                    // e.g. "String" -> "std__string__String" via imports
                    let resolved_ty = crate::codegen::type_bridge::resolve_codegen_type(ctx, &common_ty);
                    let resolved_name = resolved_ty.mangle_suffix();
                    let eq_method_name = format!("{}__{}", resolved_name, "eq");
                    // Fallback: also check unqualified name for same-package types
                    let eq_method_name_unqual = format!("{}__{}", struct_name, "eq");
                    let has_trait_eq = ctx.generic_impls().contains_key(&eq_method_name)
                        || ctx.generic_impls().contains_key(&eq_method_name_unqual);
                    // Use whichever key was found
                    let eq_method_name = if ctx.generic_impls().contains_key(&eq_method_name) {
                        eq_method_name
                    } else {
                        eq_method_name_unqual
                    };
                    
                    if has_trait_eq {
                        
                        // Trigger hydration of the eq method body on-demand
                        // CRITICAL: Pass self_ty so request_specialization uses TraitRegistry
                        // method-lookup path (not function-lookup which can't find impl methods)
                        let _ = ctx.request_specialization(&eq_method_name, vec![], Some(resolved_ty.clone()));
                        
                        // Auto-deref: the trait method expects &T (ptr), but we have T (value).
                        // Allocate both operands on the stack to get pointers.
                        let struct_mlir = common_ty.to_mlir_storage_type(ctx)?;
                        let lhs_ptr = format!("%eq_lhs_ptr_{}", ctx.next_id());
                        let rhs_ptr = format!("%eq_rhs_ptr_{}", ctx.next_id());
                        let c1 = format!("%eq_c1_{}", ctx.next_id());
                        out.push_str(&format!("    {} = arith.constant 1 : i64\n", c1));
                        out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", lhs_ptr, c1, struct_mlir));
                        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", lhs_prom, lhs_ptr, struct_mlir));
                        let c1b = format!("%eq_c1b_{}", ctx.next_id());
                        out.push_str(&format!("    {} = arith.constant 1 : i64\n", c1b));
                        out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", rhs_ptr, c1b, struct_mlir));
                        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", rhs_prom, rhs_ptr, struct_mlir));
                        
                        let call_res = format!("%eq_call_{}", ctx.next_id());
                        let args_str = format!("{}, {}", lhs_ptr, rhs_ptr);
                        // Emit call directly with the fully-qualified eq method name.
                        // Do NOT use ctx.emit_call — it re-mangles the name under the caller's scope.
                        out.push_str(&format!("    {} = func.call @{}({}) : (!llvm.ptr, !llvm.ptr) -> i1\n", call_res, eq_method_name, args_str));
                        
                        if is_ne {
                            let inv_res = format!("%ne_res_{}", ctx.next_id());
                            let true_val = format!("%true_{}", ctx.next_id());
                            ctx.emit_const_int(out, &true_val, 1, "i1");
                            out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", inv_res, call_res, true_val));
                            return Ok((inv_res, Type::Bool));
                        } else {
                            return Ok((call_res, Type::Bool));
                        }
                    }
                }
            }
            // Fallback: structural field-by-field comparison
             let res = emit_aggregate_eq(ctx, out, &b.op, &lhs_prom, &rhs_prom, &common_ty)?;
             return Ok((res, Type::Bool));
        }

        if let Type::Reference(inner, _) = &common_ty {
            // [SOVEREIGN V7.0] General Trait Dispatch for Equality
            // For Reference types, check if the inner type requires trait-based eq.
            // CRITICAL: Primitives (i64, i32, u8, etc.) MUST skip trait dispatch and
            // fall through to the hardware comparison path (arith.cmpi). Dispatching
            // to trait eq for primitives causes mangle_fn_name in emit_call to
            // re-mangle "i64__eq" under the caller's package context, producing
            // incorrect function names like "std__collections__hash_map__i64__eq".
            let is_eq = matches!(b.op, syn::BinOp::Eq(_));
            let is_ne = matches!(b.op, syn::BinOp::Ne(_));
            
            // Only dispatch to trait eq for non-primitive types (Struct, String, etc.)
            // Primitives use the efficient load+compare path below
            if (is_eq || is_ne) && !inner.is_numeric() && !inner.is_integer() && !matches!(**inner, Type::Bool | Type::I8 | Type::U8) {
                let mangle_name = inner.mangle_suffix();
                let eq_method_name = format!("{}__{}", mangle_name, "eq");
                let has_trait_eq = ctx.generic_impls().contains_key(&eq_method_name);
                
                if has_trait_eq {
                    // Trigger hydration of the eq method body on-demand
                    // CRITICAL: Pass self_ty so request_specialization uses TraitRegistry
                    // method-lookup path (not function-lookup which can't find impl methods)
                    let _ = ctx.request_specialization(&eq_method_name, vec![], Some((**inner).clone()));
                    
                    let call_res = format!("%eq_call_{}", ctx.next_id());
                    let args_str = format!("{}, {}", lhs_prom, rhs_prom);
                    
                    // Emit call directly with the fully-qualified eq method name.
                    // Do NOT re-mangle via mangle_fn_name — the eq_method_name is
                    // already correctly qualified (e.g., "main__Point__eq"), and
                    // re-mangling would prepend the caller's scope (HashMap).
                    if let Some(r) = Some(&*call_res) {
                        out.push_str(&format!("    {} = func.call @{}({}) : (!llvm.ptr, !llvm.ptr) -> i1\n", r, eq_method_name, args_str));
                    }
                    
                    if is_ne {
                        let inv_res = format!("%ne_res_{}", ctx.next_id());
                        let true_val = format!("%true_{}", ctx.next_id());
                        ctx.emit_const_int(out, &true_val, 1, "i1");
                        out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", inv_res, call_res, true_val));
                        return Ok((inv_res, Type::Bool));
                    } else {
                        return Ok((call_res, Type::Bool));
                    }
                }
            }

            let inner_mlir = inner.to_mlir_type(ctx)?;
            let inner_op = get_arith_op(&b.op, inner);
            let inner_pred = get_comparison_pred(&b.op, inner);
            
            // Load values from both references
            let lhs_loaded = format!("%deref_lhs_{}", ctx.next_id());
            let rhs_loaded = format!("%deref_rhs_{}", ctx.next_id());
            ctx.emit_load(out, &lhs_loaded, &lhs_prom, &inner_mlir);
            ctx.emit_load(out, &rhs_loaded, &rhs_prom, &inner_mlir);
            
            // Compare loaded values
            ctx.emit_cmp(out, &res, &inner_op, &inner_pred, &lhs_loaded, &rhs_loaded, &inner_mlir);
            return Ok((res, Type::Bool));
        }

        let pred = get_comparison_pred(&b.op, &common_ty);
        ctx.emit_cmp(out, &res, &op, &pred, &lhs_prom, &rhs_prom, &mlir_ty);
        Ok((res, Type::Bool))
    } else {
        // [V7.5/V8] Use fast-math for floating-point ops in reduction context or @fast_math functions
        let is_fp = matches!(common_ty, Type::F32 | Type::F64);
        let in_fast = {
            let emission = &ctx.emission;
            emission.in_fast_math_reduction || emission.in_fast_math_fn
        };
        if is_fp && in_fast {
            ctx.emit_binop_fast(out, &res, &op, &lhs_prom, &rhs_prom, &mlir_ty);
        } else {
            ctx.emit_binop(out, &res, &op, &lhs_prom, &rhs_prom, &mlir_ty);
        }
        Ok((res.to_string(), common_ty))
    }
}

pub fn emit_logic(ctx: &mut LoweringContext, out: &mut String, b: &syn::ExprBinary, lhs: (String, Type), local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(String, Type), String> {
    let (lhs_val, lhs_ty) = lhs;
    if lhs_ty != Type::Bool {
        return Err(format!("Logical operator requires boolean operands, found {:?}", lhs_ty));
    }
    
    let is_and = matches!(b.op, syn::BinOp::And(_));
    let next_block = format!("logic_rhs_{}", ctx.next_id());
    let merge_block = format!("logic_merge_{}", ctx.next_id());
    let res_ptr = format!("%logic_res_ptr_{}", ctx.next_id());
    
    ctx.emit_alloca(out, &res_ptr, "i1");
    
    // For AND: if lhs is true, go to next_block (eval RHS). If false, store false and go to merge.
    // For OR: if lhs is true, store true and go to merge. If false, go to next_block (eval RHS).
    if is_and {
        out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}\n", lhs_val, next_block, merge_block));
        
        // Short-circuit (False) block is actually the merge block? 
        // No, we need to store the value.
        // Wait, if it's false, we go to merge, but we need to have stored false.
    } else {
        out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}\n", lhs_val, merge_block, next_block));
    }
    
    // Pre-store the short-circuit value before the branch?
    // In MLIR, we can't easily jump to a block and have a value unless we use phi or store.
    // Let's use store.
    
    // Actually, let's do it properly with 3 blocks: RHS, SHORT, MERGE.
    let short_block = format!("logic_short_{}", ctx.next_id());

    // Redo branching
    out.truncate(out.len() - (format!("    cf.cond_br {}, ^{}, ^{}\n", lhs_val, next_block, merge_block).len())); // Simple way to "undo" last push if I was more careful
    // Better: just don't push until I know.
    
    let (true_dest, false_dest) = if is_and {
        (next_block.clone(), short_block.clone())
    } else {
        (short_block.clone(), next_block.clone())
    };
    
    out.push_str(&format!("    cf.cond_br {}, ^{}, ^{}\n", lhs_val, true_dest, false_dest));
    
    // Short-circuit block
    out.push_str(&format!("  ^{}:\n", short_block));
    let short_val = if is_and { 0 } else { 1 };
    let c_res = format!("%c_{}_{}", short_val, ctx.next_id());
    ctx.emit_const_int(out, &c_res, short_val as i64, "i1");
    out.push_str(&format!("    llvm.store {}, {} : i1, !llvm.ptr\n", c_res, res_ptr));
    out.push_str(&format!("    cf.br ^{}\n", merge_block));
    
    // RHS block
    out.push_str(&format!("  ^{}:\n", next_block));
    let (rhs_val, rhs_ty) = emit_expr(ctx, out, &b.right, local_vars, Some(&Type::Bool))?;
    if rhs_ty != Type::Bool {
        return Err(format!("Logical operator requires boolean operands, found {:?}", rhs_ty));
    }
    out.push_str(&format!("    llvm.store {}, {} : i1, !llvm.ptr\n", rhs_val, res_ptr));
    out.push_str(&format!("    cf.br ^{}\n", merge_block));
    
    // Merge block
    out.push_str(&format!("  ^{}:\n", merge_block));
    let final_res = format!("%logic_final_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> i1\n", final_res, res_ptr));
    
    Ok((final_res, Type::Bool))
}

pub fn emit_assign(ctx: &mut LoweringContext, out: &mut String, a: &syn::ExprAssign, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(String, Type), String> {

    let (ptr, raw_ptr_ty, kind) = emit_lvalue(ctx, out, &a.left, local_vars)?;
    // [SOVEREIGN FIX] Substitute generics in lvalue type (T -> u8 etc.)
    let ptr_ty = raw_ptr_ty.substitute(&ctx.current_type_map());

    
    // [SOVEREIGN V2.1]: Conditional Hint Purification
    // For indexed pointer access (ptr[i] = val), the LHS type is Pointer<element>
    // and we need to peel to get the element type.
    // For struct field access (self.ptr = val), the field IS the type to assign.
    // We only peel if the assignment target is a pointer being dereferenced,
    // NOT if the field itself is a Pointer type we're assigning to.
    let element_ty = match (&kind, &ptr_ty) {
        // For indexed access via Tensor, the type is already the element
        (LValueKind::Tensor { .. }, _) => ptr_ty.clone(),
        // For regular struct field ptr access, use the field type directly
        (LValueKind::Ptr, Type::Pointer { .. }) => ptr_ty.clone(),
        // [SOVEREIGN FIX] For local variable assignments to Pointer-typed vars
        // (e.g., `curr = n.next` where curr: Ptr<ListNode>), use the type directly.
        // We are storing a Pointer value into the alloca, NOT dereferencing.
        (LValueKind::Local, _) => ptr_ty.clone(),
        // [PMM FIX] Global variables are storage locations, NOT dereferences.
        // `FREE_LIST_HEAD = current as !llvm.ptr` stores a pointer VALUE into the global.
        // We must NOT peel the Pointer type — the store type must be !llvm.ptr, not i8.
        (LValueKind::Global(_), _) => ptr_ty.clone(),
        // For other pointer cases where we're dereferencing, peel
        (_, Type::Pointer { ref element, .. }) => (**element).clone(),
        _ => ptr_ty.clone(),
    };
    
    // Evaluate RHS with the appropriate hint
    let (rhs_val, rhs_ty) = emit_expr(ctx, out, &a.right, local_vars, Some(&element_ty))?;

    // [POINTER STATE DRAIN] Consume any pending pointer state set by the RHS.
    // Without this, Ptr::empty() in `node.children[i] = Ptr::empty()` leaks
    // its Empty state to the next let-binding, potentially in a later function.
    // If the LHS is a simple named variable, apply the state to it.
    {
        let pending_state = ctx.pending_pointer_state.take();
        if let Some(state) = pending_state {
            // Extract LHS variable name for simple assignments like `curr = child`
            if let syn::Expr::Path(p) = &*a.left {
                if let Some(ident) = p.path.get_ident() {
                    let var_name = ident.to_string();
                    match state {
                        crate::codegen::verification::PointerState::Empty => {
                            ctx.pointer_tracker.mark_empty(&var_name);
                        }
                        crate::codegen::verification::PointerState::Valid => {
                            ctx.pointer_tracker.mark_valid(&var_name);
                        }
                        crate::codegen::verification::PointerState::Optional => {
                            ctx.pointer_tracker.mark_optional(&var_name);
                        }
                    }
                }
            }
            // For non-path LHS (array index, field access), just drain — don't track
        }
    }

    // [ESCAPE ANALYSIS] Field-assign escape: `self.field = malloc_var`
    // When a malloc'd value is stored into a struct field (via &mut self),
    // it escapes the current function scope. Mark it as escaped.
    // Also peels through Expr::Cast to handle `self.data = ptr as Ptr<u8>` patterns.
    {
        let mut rhs_expr: &syn::Expr = &*a.right;
        while let syn::Expr::Cast(c) = rhs_expr {
            rhs_expr = &c.expr;
        }
        if let syn::Expr::Path(p) = rhs_expr {
            if p.path.segments.len() == 1 {
                let rhs_var = p.path.segments[0].ident.to_string();
                let alloc_id = format!("malloc:{}", rhs_var);
                ctx.malloc_tracker.mark_escaped(&alloc_id);
                // Also try the var name itself (for cast aliases)
                ctx.malloc_tracker.mark_escaped(&rhs_var);
            }
        }
    }

    // [ARENA ESCAPE ANALYSIS] Law II: The Assignment Rule.
    // `a = b` is valid iff depth(b) <= depth(a).
    // Cannot store a short-lived arena pointer into a long-lived container.
    if ctx.arena_escape_tracker.is_active() {
        if let syn::Expr::Path(rhs_p) = &*a.right {
            if let Some(rhs_ident) = rhs_p.path.get_ident() {
                let rhs_var = rhs_ident.to_string();
                // Extract LHS receiver for field access: ctx.saved_ptr → "ctx"
                let lhs_var = extract_field_assign_receiver(&a.left);
                if let Some(lhs_var) = lhs_var {
                    if let Err(msg) = ctx.arena_escape_tracker.check_store_escape(&rhs_var, &lhs_var) {
                        return Err(msg);
                    }
                }
            }
        }
    }

    
    let rhs_prom = promote_numeric(ctx, out, &rhs_val, &rhs_ty, &element_ty)?;
    
    let _scopes = match kind {
        LValueKind::Local => Some(("#scope_local", "#scope_global")),
        LValueKind::Global(_) => Some(("#scope_global", "#scope_local")),
        _ => None,
    };
    
    let scopes = match kind {
        LValueKind::Local => Some(("#scope_local", "#scope_global")),
        LValueKind::Global(_) => Some(("#scope_global", "#scope_local")),
        _ => None,
    };

    if let LValueKind::Bit(offset_val) = kind {
          // BIT PACKED STORE
          // ptr points to the i64 word.
          // We need RMW loop: Load word, mask bit, OR new bit, Store word.
          let _word_ty = Type::I64;
          let word_mlir = "i64";
          
          let prev_word = format!("%prev_word_{}", ctx.next_id());
          ctx.emit_load(out, &prev_word, &ptr, word_mlir);
          
          // Prepare mask: ~(1 << offset)
          let one = format!("%one_{}", ctx.next_id());
          ctx.emit_const_int(out, &one, 1, "i64");
          let sh_mask = format!("%sh_mask_{}", ctx.next_id());
          ctx.emit_binop(out, &sh_mask, "arith.shli", &one, &offset_val, "i64");
          
          let minus_one = format!("%c_minus_1_{}", ctx.next_id());
          ctx.emit_const_int(out, &minus_one, -1, "i64");
          
          let neg_mask = format!("%neg_mask_{}", ctx.next_id());
          ctx.emit_binop(out, &neg_mask, "arith.xori", &sh_mask, &minus_one, "i64");
          
          // Mask out old bit
          let cleared = format!("%cleared_{}", ctx.next_id());
          ctx.emit_binop(out, &cleared, "arith.andi", &prev_word, &neg_mask, "i64");
          
          // Prepare new bit
          let bit_val = if rhs_ty == Type::Bool {
               let zext = format!("%bit_zext_{}", ctx.next_id());
               ctx.emit_cast(out, &zext, "arith.extui", &rhs_prom, "i1", "i64");
               zext
          } else {
               rhs_prom
          };
          
          let shifted_bit = format!("%shifted_bit_{}", ctx.next_id());
          ctx.emit_binop(out, &shifted_bit, "arith.shli", &bit_val, &offset_val, "i64");
          
          // OR together
          let new_word = format!("%new_word_{}", ctx.next_id());
          ctx.emit_binop(out, &new_word, "arith.ori", &cleared, &shifted_bit, "i64");
          
          // Store
          ctx.emit_store(out, &new_word, &ptr, word_mlir);
          return Ok(("%unit".to_string(), Type::Unit));
    }
    
    // TENSOR STORE [V7.9]: Use memref.store for indexed Tensor assignment
    if let LValueKind::Tensor { memref, indices, elem_ty, shape } = kind {
        let elem_mlir = elem_ty.to_mlir_storage_type(ctx)?;
        
        // Build memref type string
        let shape_str = shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
        let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
        
        // Build index list string
        let indices_str = indices.join(", ");
        
        // Emit memref.store
        out.push_str(&format!("    memref.store {}, {}[{}] : {}\n", 
           rhs_prom, memref, indices_str, memref_ty));

        return Ok(("%unit".to_string(), Type::Unit));
    }
    
    ctx.emit_store_logical_with_scope(out, &rhs_prom, &ptr, &element_ty, scopes)?;
    
    // [PILLAR 2: Global LVN] Update cache with the stored value
    // Subsequent reads of this global will use the cached SSA value
    if let LValueKind::Global(ref global_name) = kind {
        if !global_name.is_empty() {
            ctx.emission.global_lvn.cache_value(global_name.clone(), rhs_prom.clone());
        }
    }
    
    Ok((rhs_prom.clone(), Type::Unit))
}

pub fn emit_compound_assign(ctx: &mut LoweringContext, out: &mut String, b: &syn::ExprBinary, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(String, Type), String> {
    let (ptr, ptr_ty, kind) = emit_lvalue(ctx, out, &b.left, local_vars)?;
    
    let _scopes = match kind {
        LValueKind::Local => Some(("#scope_local", "#scope_global")),
        LValueKind::Global(_) => Some(("#scope_global", "#scope_local")),
        _ => None,
    };

    let scopes = match kind {
        LValueKind::Local => Some(("#scope_local", "#scope_global")),
        LValueKind::Global(_) => Some(("#scope_global", "#scope_local")),
        _ => None,
    };

    if let LValueKind::Bit(offset_val) = kind {
         // BIT COMPOUND ASSIGN
         let word_mlir = "i64";
         let prev_word = format!("%prev_word_{}", ctx.next_id());
         ctx.emit_load(out, &prev_word, &ptr, word_mlir);

         // Extract LHS (Bit)
         let one = format!("%one_{}", ctx.next_id());
         ctx.emit_const_int(out, &one, 1, "i64");
         let shifted_down = format!("%shifted_down_{}", ctx.next_id());
         ctx.emit_binop(out, &shifted_down, "arith.shrui", &prev_word, &offset_val, "i64");
         let lhs_val_i1 = format!("%lhs_bit_{}", ctx.next_id());
         ctx.emit_cast(out, &lhs_val_i1, "arith.trunci", &shifted_down, "i64", "i1");
         
         // Evaluate RHS
         let (rhs_val, rhs_ty) = emit_expr(ctx, out, &b.right, local_vars, None)?;
         let rhs_prom = promote_numeric(ctx, out, &rhs_val, &rhs_ty, &Type::Bool)?; // Assuming bool op

         // Perform Op
         let op_res = format!("%op_res_{}", ctx.next_id());
         
         // Map Op to arith
         let bin_op = match b.op {
            syn::BinOp::BitAndAssign(_) => "arith.andi",
            syn::BinOp::BitOrAssign(_) => "arith.ori",
            syn::BinOp::BitXorAssign(_) => "arith.xori",
             _ => return Err(format!("Unsupported compound assign on bool/bit: {:?}", b.op))
         };
         ctx.emit_binop(out, &op_res, bin_op, &lhs_val_i1, &rhs_prom, "i1");

         // Write Back (RMW)
         let sh_mask = format!("%sh_mask_{}", ctx.next_id());
         ctx.emit_binop(out, &sh_mask, "arith.shli", &one, &offset_val, "i64");
         
         let minus_one = format!("%c_minus_1_cmp_{}", ctx.next_id());
         ctx.emit_const_int(out, &minus_one, -1, "i64");
         
         let neg_mask = format!("%neg_mask_{}", ctx.next_id());
         ctx.emit_binop(out, &neg_mask, "arith.xori", &sh_mask, &minus_one, "i64");
         
         let cleared = format!("%cleared_{}", ctx.next_id());
         ctx.emit_binop(out, &cleared, "arith.andi", &prev_word, &neg_mask, "i64");
         
         let bit_val_ext = format!("%bit_val_ext_{}", ctx.next_id());
         ctx.emit_cast(out, &bit_val_ext, "arith.extui", &op_res, "i1", "i64");
         let shifted_bit = format!("%shifted_bit_{}", ctx.next_id());
         ctx.emit_binop(out, &shifted_bit, "arith.shli", &bit_val_ext, &offset_val, "i64");
         
         let new_word = format!("%new_word_{}", ctx.next_id());
         ctx.emit_binop(out, &new_word, "arith.ori", &cleared, &shifted_bit, "i64");
         
         ctx.emit_store(out, &new_word, &ptr, word_mlir);
         return Ok(("%unit".to_string(), Type::Unit));
    }

    // TENSOR COMPOUND ASSIGN [V7.9]: Use memref.load + op + memref.store
    if let LValueKind::Tensor { ref memref, ref indices, ref elem_ty, ref shape } = kind {
        let elem_mlir = elem_ty.to_mlir_storage_type(ctx)?;
        
        // Build memref type string
        let shape_str = shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
        let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
        
        // Build index list string
        let indices_str = indices.join(", ");
        
        // 1. Load current value using memref.load
        let load_tmp = format!("%tensor_load_cmp_{}", ctx.next_id());
        out.push_str(&format!("    {} = memref.load {}[{}] : {}\n", 
            load_tmp, memref, indices_str, memref_ty));
        
        // 2. Evaluate RHS and Promote
        let (rhs_val, rhs_ty) = emit_expr(ctx, out, &b.right, local_vars, Some(elem_ty))?;
        let rhs_prom = promote_numeric(ctx, out, &rhs_val, &rhs_ty, elem_ty)?;
        
        // 3. Compute Result
        let bin_op = if elem_ty.is_float() {
            match b.op {
                syn::BinOp::AddAssign(_) => "arith.addf",
                syn::BinOp::SubAssign(_) => "arith.subf",
                syn::BinOp::MulAssign(_) => "arith.mulf",
                syn::BinOp::DivAssign(_) => "arith.divf",
                _ => return Err(format!("Unsupported Tensor float assign op: {:?}", b.op))
            }
        } else {
            match b.op {
                syn::BinOp::AddAssign(_) => "arith.addi",
                syn::BinOp::SubAssign(_) => "arith.subi",
                syn::BinOp::MulAssign(_) => "arith.muli",
                syn::BinOp::DivAssign(_) => "arith.divsi",
                _ => return Err(format!("Unsupported Tensor int assign op: {:?}", b.op))
            }
        };
        
        let op_res = format!("%tensor_op_{}", ctx.next_id());
        ctx.emit_binop(out, &op_res, bin_op, &load_tmp, &rhs_prom, &elem_mlir);
        
        // 4. Store using memref.store
        out.push_str(&format!("    memref.store {}, {}[{}] : {}\n", 
            op_res, memref, indices_str, memref_ty));
        
        return Ok(("%unit".to_string(), Type::Unit));
    }

    let load_tmp = format!("%load_cmp_{}", ctx.next_id());
    ctx.emit_load_logical_with_scope(out, &load_tmp, &ptr, &ptr_ty, scopes)?;
    
    let lhs_val = load_tmp;
    
    let (rhs_val, rhs_ty) = emit_expr(ctx, out, &b.right, local_vars, None)?;
    
    let common_ty = ptr_ty.clone(); 
    let rhs_prom = promote_numeric(ctx, out, &rhs_val, &rhs_ty, &common_ty)?;
    
    let _op_res = format!("%cmp_res_{}", ctx.next_id());
    let bin_op = if matches!(ptr_ty, Type::F32 | Type::F64) {
        match b.op {
            syn::BinOp::AddAssign(_) => "arith.addf",
            syn::BinOp::SubAssign(_) => "arith.subf",
            syn::BinOp::MulAssign(_) => "arith.mulf",
            syn::BinOp::DivAssign(_) => "arith.divf",
            syn::BinOp::RemAssign(_) => "arith.remf",
            _ => return Err(format!("Unsupported float assign op: {:?}", b.op))
        }
    } else {
        match b.op {
            syn::BinOp::AddAssign(_) => "arith.addi",
            syn::BinOp::SubAssign(_) => "arith.subi",
            syn::BinOp::MulAssign(_) => "arith.muli",
            syn::BinOp::DivAssign(_) => "arith.divsi",
            syn::BinOp::RemAssign(_) => "arith.remsi",
            syn::BinOp::BitAndAssign(_) => "arith.andi",
            syn::BinOp::BitOrAssign(_) => "arith.ori",
            syn::BinOp::BitXorAssign(_) => "arith.xori",
            syn::BinOp::ShlAssign(_) => "arith.shli",
            syn::BinOp::ShrAssign(_) => "arith.shrsi",
            _ => return Err(format!("Unsupported assign op: {:?}", b.op))
        }
    };
    
    // Standardize store
    let _mlir_storage_ty = ptr_ty.to_mlir_storage_type(ctx)?;
    let binop_ty = ptr_ty.to_mlir_type(ctx)?; 

    let op_res = format!("%cmp_res_{}", ctx.next_id());
    ctx.emit_binop(out, &op_res, bin_op, &lhs_val, &rhs_prom, &binop_ty);

    ctx.emit_store_logical_with_scope(out, &op_res, &ptr, &ptr_ty, scopes)?;
    
    // [PILLAR 2: Global LVN] Update cache with the computed result
    if let LValueKind::Global(ref global_name) = kind {
        if !global_name.is_empty() {
            ctx.emission.global_lvn.cache_value(global_name.clone(), op_res.clone());
        }
    }
    
    Ok(("%unit".to_string(), Type::Unit))
}

pub fn emit_unary(ctx: &mut LoweringContext, out: &mut String, u: &syn::ExprUnary, local_vars: &mut HashMap<String, (Type, LocalKind)>, _expected: Option<&Type>) -> Result<(String, Type), String> {
    let (val, ty) = emit_expr(ctx, out, &u.expr, local_vars, None)?;
    let res = format!("%unary_{}", ctx.next_id());
    let mlir_ty = ty.to_mlir_type(ctx)?;
    
    match &u.op {
        syn::UnOp::Not(_) => {
            if ty == Type::Bool {
                let mask = format!("%not_mask_{}", ctx.next_id());
                ctx.emit_const_int(out, &mask, 1, "i1");
                let res = format!("%not_res_{}", ctx.next_id());
                ctx.emit_binop(out, &res, "arith.xori", &val, &mask, "i1");
                Ok((res, Type::Bool))
            } else if ty.is_integer() {
                 let val_neg_1 = format!("%u_all_ones_{}", ctx.next_id());
                 let mlir_ty = ty.to_mlir_type(ctx)?;
                 out.push_str(&format!("    {} = arith.constant -1 : {}\n", val_neg_1, mlir_ty));
                 let res = format!("%unary_{}", ctx.next_id());
                 ctx.emit_binop(out, &res, "arith.xori", &val, &val_neg_1, &mlir_ty);
                 Ok((res, ty))
            } else {
                Err("Not operator only supported for bool or integer".to_string())
            }
        }
        syn::UnOp::Neg(_) => {
            if ty == Type::Bool {
                 return Err("Cannot apply unary minus to boolean type. Use '!' for logical not.".to_string());
            }
            if matches!(ty, Type::F32 | Type::F64) {
                 out.push_str(&format!("    {} = arith.negf {} : {}\n", res, val, mlir_ty));
            } else {
                 // For integers, 0 - val
                 // Ensure %c0 is available? Usually emitted at start of func. 
                 // If not, we should emit it locally? 
                 // Best practice: emit a local zero constant just in case.
                 let zero_const = format!("%zero_{}", ctx.next_id());
                 out.push_str(&format!("    {} = arith.constant 0 : {}\n", zero_const, mlir_ty));
                 ctx.emit_binop(out, &res, "arith.subi", &zero_const, &val, &mlir_ty);
            }
            Ok((res, ty))
        }
        syn::UnOp::Deref(_) => {
            // [POINTER SAFETY] Check if pointer is safe to dereference (Valid)
            if let syn::Expr::Path(expr_path) = &*u.expr {
                if let Some(ident) = expr_path.path.get_ident() {
                    ctx.pointer_tracker.check_deref(&ident.to_string())?;
                }
            }

            let inner_ty = match ty {
                Type::Reference(inner, _) => *inner,
                Type::Pointer { element, .. } => *element, // [V12.4] Support deref of Ptr<T> directly
                _ => return Err(format!("Cannot dereference non-pointer type: {:?}", ty)),
            };
            
            let inner_mlir = inner_ty.to_mlir_storage_type(ctx)?;
            let raw_res = format!("%deref_raw_{}", ctx.next_id());
            
            // [V7.3] Check if this pointer originates from a registered argument scope
            // If so, emit load with fine-grained alias metadata
            if ctx.config.emit_alias_scopes {
                let cf = &ctx.control_flow;
                if let Some(scope_id) = cf.get_pointer_scope(&val) {
                    // Get other scopes for noalias list
                    let other_scopes = cf.get_other_arg_scopes(scope_id);
                    drop(cf);
                    
                    let alias_scope = format!("#scope_arg_{}", scope_id);
                    let noalias_str = if other_scopes.is_empty() {
                        String::new()
                    } else {
                        let noalias_list: Vec<_> = other_scopes.iter().map(|id| format!("#scope_arg_{}", id)).collect();
                        format!(", noalias_scopes = [{}]", noalias_list.join(", "))
                    };
                    out.push_str(&format!("    {} = llvm.load {} {{ alias_scopes = [{}]{} }} : !llvm.ptr -> {}\n",
                        raw_res, val, alias_scope, noalias_str, inner_mlir));
                } else {
                    drop(cf);
                    // Fallback to regular load with local/global scope
                    out.push_str(&format!("    {} = llvm.load {} {{ alias_scopes = [#scope_local], noalias = [#scope_global] }} : !llvm.ptr -> {}\n",
                        raw_res, val, inner_mlir));
                }
            } else {
                // Alias scopes disabled — plain load
                ctx.emit_load(out, &raw_res, &val, &inner_mlir);
            }
            
            let final_res = if inner_ty == Type::Bool {
                let trunc = format!("%b_trunc_deref_{}", ctx.next_id());
                ctx.emit_trunc(out, &trunc, &raw_res, "i8", "i1");
                trunc
            } else {
                raw_res
            };
            Ok((final_res, inner_ty))
        }
        _ => Err(format!("Unsupported unary operator: {:?}", u.op))
    }
}

pub fn emit_cast(ctx: &mut LoweringContext, out: &mut String, c: &syn::ExprCast, local_vars: &mut HashMap<String, (Type, LocalKind)>, _expected: Option<&Type>) -> Result<(String, Type), String> {
    let raw_target_ty = resolve_type(ctx, &crate::grammar::SynType::from_std(*c.ty.clone()).unwrap());
    // [SOVEREIGN FIX] Apply current type_map to resolve generics like T -> u8 in cast expressions
    let target_ty = raw_target_ty.substitute(&ctx.current_type_map());

    // [STRING→STRINGVIEW CAST] "hello" as StringView → synthesize { ptr, len } at compile time
    // Detects string literal source + StringView target type, emits struct construction.
    {
        let is_stringview_target = match &target_ty {
            Type::Struct(name) | Type::Concrete(name, _) => name.contains("StringView"),
            _ => false,
        };
        if is_stringview_target {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &*c.expr {
                let val = s.value();
                let str_len = val.len();

                // Reuse or create the string global (same dedup logic as normal string literals)
                let existing = ctx.string_literals().iter()
                    .find(|(_, content, _)| *content == val)
                    .map(|(name, _, _)| name.clone());
                let global_id = if let Some(existing_id) = existing {
                    existing_id
                } else {
                    let new_id = format!("str_{}", ctx.next_id());
                    ctx.string_literals_mut().push((new_id.clone(), val.clone(), str_len));
                    new_id
                };

                // Get pointer to the string global
                let ptr_var = format!("%sv_ptr_{}", ctx.next_id());
                ctx.emit_addressof(out, &ptr_var, &global_id)?;

                // Emit compile-time length constant
                let len_var = format!("%sv_len_{}", ctx.next_id());
                out.push_str(&format!("    {} = arith.constant {} : i64\n", len_var, str_len));

                // Build StringView struct: { ptr: !llvm.ptr, len: i64 }
                let mlir_ty = target_ty.to_mlir_type(ctx)?;
                let undef = format!("%sv_undef_{}", ctx.next_id());
                let with_ptr = format!("%sv_wptr_{}", ctx.next_id());
                let result = format!("%sv_result_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", undef, mlir_ty));
                out.push_str(&format!("    {} = llvm.insertvalue {}, {}[0] : {}\n", with_ptr, ptr_var, undef, mlir_ty));
                out.push_str(&format!("    {} = llvm.insertvalue {}, {}[1] : {}\n", result, len_var, with_ptr, mlir_ty));

                return Ok((result, target_ty));
            }
        }
    }

    // [STACK ARRAY] Array → Pointer cast: return alloca address directly.
    // Stack arrays are allocated via `llvm.alloca ... x !llvm.array<N x T>` → `!llvm.ptr`.
    // We must NOT load the array value (which is `!llvm.array<N x i8>`), but instead
    // return the alloca pointer itself (which is already `!llvm.ptr`).
    if matches!(target_ty, Type::Pointer { .. }) {
        // Check if inner expression is a variable of Array type
        if let syn::Expr::Path(p) = &*c.expr {
            if p.path.segments.len() == 1 {
                let var_name = p.path.segments[0].ident.to_string();
                if let Some((ref var_ty, ref kind)) = local_vars.get(&var_name).cloned() {
                    if matches!(var_ty, Type::Array(..)) {
                        // Return the alloca address — it's already !llvm.ptr
                        let ptr = match kind {
                            LocalKind::Ptr(ptr) => ptr.clone(),
                            LocalKind::SSA(ssa) => ssa.clone(),
                        };
                        return Ok((ptr, target_ty));
                    }
                }
            }
        }
    }

    // [PTR CAST] Pointer-to-Pointer and Integer-to-Pointer casts
    // With LLVM opaque pointers, Ptr<A> → Ptr<B> is a type-level no-op.
    // Integer → Ptr<T> uses llvm.inttoptr.
    if matches!(target_ty, Type::Pointer { .. }) {
        let (val, ty) = emit_expr(ctx, out, &c.expr, local_vars, None)?;
        if matches!(ty, Type::Pointer { .. }) {
            // Ptr<A> → Ptr<B>: same !llvm.ptr SSA value, just change the type
            return Ok((val, target_ty));
        }
        if matches!(ty, Type::U64 | Type::I64 | Type::Usize) {
            // [SIP SAFETY GATE] Reject integer-to-pointer in sip_mode (Mode B SIPs)
            // Allowing this would let SIPs write to arbitrary kernel memory.
            // Kernel code (lib_mode without sip_mode) is NOT gated.
            if ctx.config.sip_mode {
                return Err("SIP safety violation: integer-to-pointer cast is not allowed in Mode B SIPs. \
                    Raw pointer creation bypasses compiler verification.".to_string());
            }
            let res = format!("%inttoptr_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", res, val));
            return Ok((res, target_ty));
        }
    }

    // [KERNEL CAST] Integer/Reference → Reference (&T / &mut T)
    // Enables kernel patterns like: *(addr as &mut u8) = value
    // References lower to !llvm.ptr, so this is semantically identical to Pointer casts.
    if matches!(target_ty, Type::Reference(_, _)) {
        let (val, ty) = emit_expr(ctx, out, &c.expr, local_vars, None)?;
        if matches!(ty, Type::Reference(_, _) | Type::Pointer { .. }) {
            // Ref→Ref or Ptr→Ref: same !llvm.ptr SSA value, just change the type
            return Ok((val, target_ty));
        }
        if matches!(ty, Type::U64 | Type::I64 | Type::Usize) {
            // [SIP SAFETY GATE] Reject integer-to-reference in sip_mode (Mode B SIPs)
            if ctx.config.sip_mode {
                return Err("SIP safety violation: integer-to-pointer cast is not allowed in Mode B SIPs. \
                    Raw pointer creation bypasses compiler verification.".to_string());
            }
            let res = format!("%inttoptr_ref_{}", ctx.next_id());
            let int_val = if ty == Type::Usize {
                let temp = format!("%idx_to_i64_{}", ctx.next_id());
                out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", temp, val));
                temp
            } else {
                val
            };
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", res, int_val));
            return Ok((res, target_ty));
        }
    }

    // [V25.5] Propagate target type as expected hint to inner expression
    // This allows (i * 8) as u64 to infer i64 context, fixing Usize+I64 mixing in loop arithmetic
    let (val, ty) = emit_expr(ctx, out, &c.expr, local_vars, Some(&target_ty))?;

    if ty == target_ty {
        return Ok((val, target_ty));
    }

    let res = if ty == Type::Bool && target_ty.is_numeric() {
        let target_mlir = target_ty.to_mlir_type(ctx)?;
        let res = format!("%cast_bool_{}", ctx.next_id());
        out.push_str(&format!("    {} = arith.extui {} : i1 to {}\n", res, val, target_mlir));
        res
    } else if ty.is_numeric() && target_ty == Type::Bool {
        let res = format!("%cast_to_bool_{}", ctx.next_id());
        let zero_const = format!("%zero_cmp_{}", ctx.next_id());
        let mlir_ty = ty.to_mlir_type(ctx)?;
        out.push_str(&format!("    {} = arith.constant 0 : {}\n", zero_const, mlir_ty));
        out.push_str(&format!("    {} = arith.cmpi \"ne\", {}, {} : {}\n", res, val, zero_const, mlir_ty));
        res
    } else if (matches!(ty, Type::Reference(_, _) | Type::Concrete(..)) && matches!(target_ty, Type::Reference(_, _) | Type::Concrete(..))) {
         // Check if concrete types are Ptr
         let is_ty_ptr = matches!(ty, Type::Reference(_, _)) || if let Type::Concrete(ref n, _) = ty { n.contains("Ptr") } else { false };
         let is_target_ptr = matches!(target_ty, Type::Reference(_, _)) || if let Type::Concrete(ref n, _) = target_ty { n.contains("Ptr") } else { false };
         
         if is_ty_ptr && is_target_ptr {
             // Pointer Cast (Bitcast)
             let target_mlir = target_ty.to_mlir_type(ctx)?;
             let ty_mlir = ty.to_mlir_type(ctx)?;
             
             if (ty_mlir.starts_with("!llvm.struct") || ty_mlir.starts_with("!struct_")) && (target_mlir.starts_with("!llvm.struct") || target_mlir.starts_with("!struct_")) {

                  // Struct to Struct Cast (Ptr<A> -> Ptr<B>)
                  let is_thin_ptr = match ty {
                      Type::Concrete(ref n, _) => n.ends_with("Ptr") || n.contains("std__core__ptr__Ptr"),
                      Type::Struct(ref n) => n.contains("std__core__ptr__Ptr") || n.contains("Ptr_") || n.ends_with("Ptr_u8") || n.ends_with("Ptr_i64"),
                      _ => false
                  };

                  
                  let inner_ptr = format!("%inner_ptr_{}", ctx.next_id());
                  out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", inner_ptr, val, ty_mlir));
                  
                  let init_struct = format!("%init_cast_{}", ctx.next_id());
                  out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", init_struct, target_mlir));
                  
                  let step1 = format!("%step1_{}", ctx.next_id());
                  out.push_str(&format!("    {} = llvm.insertvalue {}, {}[0] : {}\n", step1, inner_ptr, init_struct, target_mlir));
                  
                  if !is_thin_ptr {
                       let size_val = format!("%size_val_{}", ctx.next_id());
                       out.push_str(&format!("    {} = llvm.extractvalue {}[1] : {}\n", size_val, val, ty_mlir));
                       let res = format!("%cast_res_{}", ctx.next_id());
                       out.push_str(&format!("    {} = llvm.insertvalue {}, {}[1] : {}\n", res, size_val, step1, target_mlir));
                       res
                  } else {
                       step1
                  }

             } else {
                 let res = format!("%cast_ptr_{}", ctx.next_id());
                 out.push_str(&format!("    {} = llvm.bitcast {} : {} to {}\n", res, val, ty_mlir, target_mlir));
                 res
             }
         } else {
             cast_numeric(ctx, out, &val, &ty, &target_ty)?
         }
    } else {
        cast_numeric(ctx, out, &val, &ty, &target_ty)?
    };
    Ok((res, target_ty.clone()))
}
