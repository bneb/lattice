use std::collections::HashMap;
use syn;

use crate::codegen::context::{LoweringContext, LocalKind};
use crate::types::Type;
use crate::codegen::expr::{emit_expr, promote_numeric, get_path_turbofish_args, emit_method_call};
use crate::codegen::type_bridge::resolve_codegen_type;

pub fn try_emit_special_method(
    ctx: &mut LoweringContext,
    out: &mut String,
    m: &syn::ExprMethodCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    expected_ty: Option<&Type>,
    cached_receiver_val: &Option<String>,
    cached_receiver_ty: &Type,
) -> Result<Option<(String, Type)>, String> {
    let _method_name = m.method.to_string();
    let mut cached_receiver_ty = cached_receiver_ty.clone();

    let path_turbofish_args = get_path_turbofish_args(&m.receiver);
    if !path_turbofish_args.is_empty() {
        // Convert syn::Type args to our Type representation
        let concrete_args: Vec<Type> = path_turbofish_args.iter()
            .filter_map(|syn_ty: &syn::Type| {
                crate::grammar::SynType::from_std(syn_ty.clone())
                    .ok()
                    .and_then(|st| crate::types::Type::from_syn(&st))
                    .map(|ty| resolve_codegen_type(ctx, &ty))
            })
            .collect();
        
        if !concrete_args.is_empty() {
            // Update receiver type to include concrete turbofish args
            match &cached_receiver_ty {
                Type::Struct(name) | Type::Concrete(name, _) => {
                    // Replace with Type::Concrete containing the actual turbofish args
                    cached_receiver_ty = Type::Concrete(name.clone(), concrete_args.clone());
                }
                _ => {}
            }
        }
    }
    
    
    // VEC::AS_PTR INTERCEPT - Return native !llvm.ptr (not RawPtr struct)
    // This hoists the inttoptr conversion OUTSIDE loops for vectorization.
    // Instead of returning RawPtr{inner: i64}, we return the !llvm.ptr directly.
    let method_name = m.method.to_string();

    // Verify receiver validity for unsafe methods
    // If calling unsafe methods on Ptr<T> (read, write, offset, etc.),
    // the receiver must be Valid. Safe methods (addr, is_null) are exempt.
    if let Type::Pointer { .. } = &cached_receiver_ty {
        let is_safe = matches!(method_name.as_str(), "addr" | "is_null" | "from_addr" | "empty" | "new");
        if !is_safe {
             // Enforce validity on tracked variables
             if let syn::Expr::Path(path) = &*m.receiver {
                 if let Some(ident) = path.path.get_ident() {
                      let is_dynamic = *ctx.is_dynamic_check_block() || ctx.emission.in_dynamic_check_fn;
                    if !is_dynamic {
                          ctx.pointer_tracker.check_deref(&ident.to_string())?;
                      }
                 }
             }
        }
    }
    
    // Clean Break: Removed NativePtr method interception (get/set/at/offset)
    // These are now handled by standard library intrinsics or unified syntax.

    // Matrix multiplication: receiver.matmul(other)
    // Called via A @ B syntax (preprocessor converts to A.matmul(B))
    // Supports:
    //   - Type::Tensor (rank 2 @ rank 2) -> linalg.matmul
    //   - Type::Tensor (rank 2 @ rank 1) -> linalg.matvec
    //   - Type::Pointer with Tensor element -> extracts shape from Tensor
    if method_name == "matmul" {
        return emit_matmul_method(ctx, out, m, local_vars, cached_receiver_val, &cached_receiver_ty);
    }

    // Universal Function Call Syntax
    // Syntax: receiver.method(_, arg2, arg3) where _ is replaced by receiver pointer
    // This enables fluent chains: (w1 @ input).add_bias(_, HIDDEN, b1).relu(_, HIDDEN)
    // 
    // KEY FIX: We use the ALREADY-EMITTED receiver SSA value (cached_receiver_val)
    // instead of re-emitting the receiver expression, which would cause double
    // evaluation for chained method calls.
    let has_placeholder = m.args.iter().any(|arg| {
        matches!(arg, syn::Expr::Infer(_))
    });
    
    if has_placeholder {
        if let Some(res) = emit_ufcs_method(ctx, out, m, local_vars, expected_ty, cached_receiver_val, &cached_receiver_ty, &method_name)? {
            return Ok(Some(res));
        }
    }
    // NOTE: as_ptr() is handled by normal monomorphized method dispatch.
    // The @inline as_ptr method generates correct MLIR with fully-qualified type aliases.
    


    // RAWPTR TRANSPARENT INTRINSIC INTERCEPT (Native Ptr + GEP for Vectorization)
    // Uses !llvm.ptr + llvm.getelementptr instead of i64 + inttoptr to preserve
    // pointer provenance and enable LLVM loop vectorization.
    let method_name = m.method.to_string();
    if method_name == "read_at" || method_name == "write_at" {
        if let Some(res) = emit_rawptr_method(ctx, out, m, local_vars, cached_receiver_val, &cached_receiver_ty, &method_name)? {
            return Ok(Some(res));
        }
    }
    // Vec accessor intercept
    if method_name == "get_unchecked" || method_name == "set_unchecked" {
        if let Some(res) = emit_vec_unchecked_method(ctx, out, m, local_vars, cached_receiver_val, &cached_receiver_ty, &method_name)? {
            return Ok(Some(res));
        }
    }
    // Use cached receiver type for Tensor and other type-based dispatch
    // (Memoization already done at function entry)
    let receiver_ty = cached_receiver_ty.clone();

    if let Type::Tensor(..) = &receiver_ty {
        if let Some(res) = emit_tensor_method(ctx, out, m, local_vars, cached_receiver_val, &cached_receiver_ty)? {
            return Ok(Some(res));
        }
    }
    // 1. Use receiver TYPE for method mangling (critical for allocator recursion fix)
    // If receiver is a struct/concrete type, use its type name (e.g., GlobalSlabAlloc)

    // If not a special method, return Ok(None) so dispatch continues
    Ok(None)
}


fn emit_matmul_method(
    ctx: &mut LoweringContext,
    out: &mut String,
    m: &syn::ExprMethodCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    cached_receiver_val: &Option<String>,
    cached_receiver_ty: &Type,
) -> Result<Option<(String, Type)>, String> {

        if m.args.len() != 1 {
            return Err("matmul requires exactly one argument: A.matmul(B)".to_string());
        }
        
        // Get receiver (matrix A)
        let (a_val, a_ty) = if let Some(ref val) = cached_receiver_val {
            (val.clone(), cached_receiver_ty.clone())
        } else {
            emit_expr(ctx, out, &m.receiver, local_vars, None)?
        };
        
        // Get argument (matrix/vector B)
        let (b_val, b_ty) = emit_expr(ctx, out, &m.args[0], local_vars, None)?;
        
        // Helper to extract shape from Type (Tensor or Ptr<Tensor>)
        fn extract_shape(ty: &Type) -> Option<(Type, Vec<usize>)> {
            match ty {
                Type::Tensor(elem, shape) => Some((*elem.clone(), shape.clone())),
                Type::Pointer { element, .. } => {
                    if let Type::Tensor(inner, shape) = element.as_ref() {
                        Some((*inner.clone(), shape.clone()))
                    } else {
                        None
                    }
                }
                _ => None
            }
        }
        
        // Extract shapes from operands
        let (a_elem, a_shape) = extract_shape(&a_ty)
            .ok_or_else(|| format!("matmul requires Tensor or Ptr<Tensor> for A, got {:?}", a_ty))?;
        let (_b_elem, b_shape) = extract_shape(&b_ty)
            .ok_or_else(|| format!("matmul requires Tensor or Ptr<Tensor> for B, got {:?}", b_ty))?;
        
        // Validate and determine operation type
        let a_rank = a_shape.len();
        let b_rank = b_shape.len();
        
        if a_rank != 2 {
            return Err(format!("matmul requires rank-2 matrix for A, got rank {}", a_rank));
        }
        
        let m_dim = a_shape[0];
        let k_dim = a_shape[1];
        let elem_mlir = a_elem.to_mlir_type(ctx)?;
        
        // Matrix-Vector: A[M,K] @ B[K] = C[M]
        // linalg.matvec with JIT memref casting for M4 optimization
        if b_rank == 1 {
            if b_shape[0] != k_dim {
                return Err(format!("matvec dimension mismatch: A is {}x{}, B is {}", 
                    m_dim, k_dim, b_shape[0]));
            }
            
            // Define memref types for structured linalg ops
            let a_memref_ty = format!("memref<{}x{}x{}>", m_dim, k_dim, elem_mlir);
            let b_memref_ty = format!("memref<{}x{}>", k_dim, elem_mlir);
            let c_memref_ty = format!("memref<{}x{}>", m_dim, elem_mlir);
            
            // JIT MemRef Casting
            // If the value is already a memref (Tensor type directly), use it as-is.
            // If it's behind a pointer (Ptr<Tensor>), cast from !llvm.ptr → memref.

            let a_memref = if matches!(a_ty, Type::Tensor(..)) {
                a_val.clone()
            } else {
                let view = format!("%a_view_{}", ctx.next_id());
                out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : !llvm.ptr to {}\n",
                    view, a_val, a_memref_ty));
                view
            };

            let b_memref = if matches!(b_ty, Type::Tensor(..)) {
                b_val.clone()
            } else {
                let view = format!("%b_view_{}", ctx.next_id());
                out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : !llvm.ptr to {}\n",
                    view, b_val, b_memref_ty));
                view
            };
            
            // Allocate output: memref<Mxf32> (for result vector)
            let c_memref = format!("%c_buf_{}", ctx.next_id());
            out.push_str(&format!("    {} = memref.alloc() : {}\n", c_memref, c_memref_ty));
            
            // Zero-initialize output
            let zero = format!("%mv_zero_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 0.0 : {}\n", zero, elem_mlir));
            out.push_str(&format!("    linalg.fill ins({} : {}) outs({} : {})\n", 
                zero, elem_mlir, c_memref, c_memref_ty));
            
            // i,k loop nest for matvec C[i] += A[i,k] * B[k]
            // Explicit loops (not linalg.matvec) so clang can auto-vectorize
            // the inner k-loop with sequential B[k] access.
            let z_idx = format!("%mv_z_{}", ctx.next_id());
            let o_idx = format!("%mv_1_{}", ctx.next_id());
            let m_idx = format!("%mv_m_{}", ctx.next_id());
            let k_idx = format!("%mv_k_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 0 : index\n", z_idx));
            out.push_str(&format!("    {} = arith.constant 1 : index\n", o_idx));
            out.push_str(&format!("    {} = arith.constant {} : index\n", m_idx, m_dim));
            out.push_str(&format!("    {} = arith.constant {} : index\n", k_idx, k_dim));

            let i_iv = format!("%mv_i_{}", ctx.next_id());
            let k_iv = format!("%mv_k_{}", ctx.next_id());
            let a_ik = format!("%mv_ak_{}", ctx.next_id());
            let b_k = format!("%mv_bk_{}", ctx.next_id());
            let m = format!("%mv_m_{}", ctx.next_id());
            let c_val = format!("%mv_c_{}", ctx.next_id());
            let c_new = format!("%mv_cn_{}", ctx.next_id());

            out.push_str(&format!("    scf.for {} = {} to {} step {} {{\n", i_iv, z_idx, m_idx, o_idx));
            out.push_str(&format!("      scf.for {} = {} to {} step {} {{\n", k_iv, z_idx, k_idx, o_idx));
            out.push_str(&format!("        {} = memref.load {}[{}, {}] : {}\n", a_ik, a_memref, i_iv, k_iv, a_memref_ty));
            out.push_str(&format!("        {} = memref.load {}[{}] : {}\n", b_k, b_memref, k_iv, b_memref_ty));
            out.push_str(&format!("        {} = arith.mulf {}, {} : {}\n", m, a_ik, b_k, elem_mlir));
            out.push_str(&format!("        {} = memref.load {}[{}] : {}\n", c_val, c_memref, i_iv, c_memref_ty));
            out.push_str(&format!("        {} = arith.addf {}, {} : {}\n", c_new, c_val, m, elem_mlir));
            out.push_str(&format!("        memref.store {}, {}[{}] : {}\n", c_new, c_memref, i_iv, c_memref_ty));
            out.push_str("      }\n");
            out.push_str("    }\n");
            
            // Extract raw pointer from memref for fluent chaining (.add_bias().relu())
            let c_idx = format!("%c_idx_{}", ctx.next_id());
            let c_i64 = format!("%c_i64_{}", ctx.next_id());
            let c_ptr = format!("%matvec_result_{}", ctx.next_id());
            out.push_str(&format!("    {} = memref.extract_aligned_pointer_as_index {} : {} -> index\n", 
                c_idx, c_memref, c_memref_ty));
            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", c_i64, c_idx));
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", c_ptr, c_i64));
            
            // Return Ptr<Tensor<f32, {1, M}>> for chaining with add_bias, relu
            let result_shape = vec![m_dim];
            let result_ty = Type::Tensor(Box::new(a_elem.clone()), result_shape.clone());
            let ptr_ty = Type::Pointer {
                element: Box::new(result_ty),
                provenance: crate::types::Provenance::Stack,
                is_mutable: true,
            };
            
            return Ok(Some((c_ptr, ptr_ty)));
        }
        
        // Matrix-Matrix: A[M,K] @ B[K,N] = C[M,N]
        // linalg.matmul with JIT memref casting
        if b_rank == 2 {
            let n_dim = b_shape[1];
            
            if b_shape[0] != k_dim {
                return Err(format!("matmul dimension mismatch: A is {}x{}, B is {}x{}", 
                    m_dim, k_dim, b_shape[0], b_shape[1]));
            }
            
            // Define memref types
            let a_memref_ty = format!("memref<{}x{}x{}>", m_dim, k_dim, elem_mlir);
            let b_memref_ty = format!("memref<{}x{}x{}>", k_dim, n_dim, elem_mlir);
            let c_memref_ty = format!("memref<{}x{}x{}>", m_dim, n_dim, elem_mlir);
            
            // JIT memref casting: skip if already memref, cast only Ptr<Tensor>
            let a_memref = if matches!(a_ty, Type::Tensor(..)) { a_val.clone() } else {
                let v = format!("%a_view_{}", ctx.next_id());
                out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : !llvm.ptr to {}\n", v, a_val, a_memref_ty)); v
            };
            let b_memref = if matches!(b_ty, Type::Tensor(..)) { b_val.clone() } else {
                let v = format!("%b_view_{}", ctx.next_id());
                out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : !llvm.ptr to {}\n", v, b_val, b_memref_ty)); v
            };
            
            // Allocate output buffer
            let c_memref = format!("%matmul_buf_{}", ctx.next_id());
            out.push_str(&format!("    {} = memref.alloc() : {}\n", c_memref, c_memref_ty));
            
            // Zero-initialize output
            let zero = format!("%mm_zero_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 0.0 : {}\n", zero, elem_mlir));
            out.push_str(&format!("    linalg.fill ins({} : {}) outs({} : {})\n", 
                zero, elem_mlir, c_memref, c_memref_ty));
            
            // Cache-tiled matmul: ii, kk tile loops over L1-sized blocks.
            // Inner j-loop has constant bound 0..N → clang auto-vectorizes.
            let (ii, i_max, kk, k_max, z_idx, n_idx, o_idx) =
                emit_matmul_tile_loops(ctx, out, m_dim, k_dim, n_dim, &elem_mlir);

            let i_iv = format!("%t_i_{}", ctx.next_id());
            let k_iv = format!("%t_k_{}", ctx.next_id());
            let j_iv = format!("%t_j_{}", ctx.next_id());
            let a_ik = format!("%t_ak_{}", ctx.next_id());
            let b_kj = format!("%t_bk_{}", ctx.next_id());
            let c_ij = format!("%t_ci_{}", ctx.next_id());
            let m = format!("%t_mul_{}", ctx.next_id());
            let s = format!("%t_sum_{}", ctx.next_id());

            out.push_str(&format!("        scf.for {} = {} to {} step {} {{\n", i_iv, ii, i_max, o_idx));
            out.push_str(&format!("          scf.for {} = {} to {} step {} {{\n", k_iv, kk, k_max, o_idx));
            out.push_str(&format!("            {} = memref.load {}[{}, {}] : {}\n", a_ik, a_memref, i_iv, k_iv, a_memref_ty));
            out.push_str(&format!("            scf.for {} = {} to {} step {} {{\n", j_iv, z_idx, n_idx, o_idx));
            out.push_str(&format!("              {} = memref.load {}[{}, {}] : {}\n", b_kj, b_memref, k_iv, j_iv, b_memref_ty));
            out.push_str(&format!("              {} = arith.mulf {}, {} : {}\n", m, a_ik, b_kj, elem_mlir));
            out.push_str(&format!("              {} = memref.load {}[{}, {}] : {}\n", c_ij, c_memref, i_iv, j_iv, c_memref_ty));
            out.push_str(&format!("              {} = arith.addf {}, {} : {}\n", s, c_ij, m, elem_mlir));
            out.push_str(&format!("              memref.store {}, {}[{}, {}] : {}\n", s, c_memref, i_iv, j_iv, c_memref_ty));
            out.push_str("            }\n");  // close j
            out.push_str("          }\n");    // close k
            out.push_str("        }\n");      // close i
            out.push_str("      }\n");        // close kk
            out.push_str("    }\n");          // close ii
            
            // Extract pointer for chaining
            let c_idx = format!("%c_idx_{}", ctx.next_id());
            let c_i64 = format!("%c_i64_{}", ctx.next_id());
            let c_ptr = format!("%matmul_result_{}", ctx.next_id());
            out.push_str(&format!("    {} = memref.extract_aligned_pointer_as_index {} : {} -> index\n", 
                c_idx, c_memref, c_memref_ty));
            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", c_i64, c_idx));
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", c_ptr, c_i64));
            
            let result_shape = vec![m_dim, n_dim];
            let result_ty = Type::Tensor(Box::new(a_elem.clone()), result_shape.clone());
            let ptr_ty = Type::Pointer {
                element: Box::new(result_ty),
                provenance: crate::types::Provenance::Stack,
                is_mutable: true,
            };
            
            return Ok(Some((c_ptr, ptr_ty)));
        }
        
        Err(format!("matmul: unsupported operand ranks: {} @ {}", a_rank, b_rank))
    }

/// Tile size for cache blocking: ~32KB f64, ~64KB f32 per tile — fits L1.
fn matmul_tile_size(elem_mlir: &str) -> usize {
    match elem_mlir { "f64" | "i64" | "u64" => 64, _ => 128 }
}

/// Emit cache-tile wrapper loops for matrix multiplication.
/// Tiles the i and k dimensions; j stays untiled for constant-bound
/// auto-vectorization. Returns (ii, i_max, kk, k_max, z_idx, n_idx, o_idx)
/// — tile IVs, inner bounds, and the j-loop bounds (always 0..N).
fn emit_matmul_tile_loops(
    ctx: &mut LoweringContext, out: &mut String,
    m: usize, k: usize, n: usize, elem_mlir: &str,
) -> (String, String, String, String, String, String, String) {
    let tile = matmul_tile_size(elem_mlir);
    let z = format!("%tz_{}", ctx.next_id());
    let o = format!("%to_{}", ctx.next_id());
    let mc = format!("%tm_{}", ctx.next_id());
    let kc = format!("%tk_{}", ctx.next_id());
    let nc = format!("%tn_{}", ctx.next_id());
    let ti = format!("%tTi_{}", ctx.next_id());
    let tk = format!("%tTk_{}", ctx.next_id());
    out.push_str(&format!(
        "    {z} = arith.constant 0 : index\n    {o} = arith.constant 1 : index\n    {mc} = arith.constant {m} : index\n    {kc} = arith.constant {k} : index\n    {nc} = arith.constant {n} : index\n    {ti} = arith.constant {tile} : index\n    {tk} = arith.constant {tile} : index\n",
    ));
    // Tile i and k dimensions; j stays untiled for constant-bound vectorization
    let ii = format!("%tii_{}", ctx.next_id());
    let i_ub = format!("%tiub_{}", ctx.next_id());
    let i_max = format!("%timx_{}", ctx.next_id());
    let kk = format!("%tkk_{}", ctx.next_id());
    let k_ub = format!("%tkub_{}", ctx.next_id());
    let k_max = format!("%tkmx_{}", ctx.next_id());
    out.push_str(&format!(
        "    scf.for {ii} = {z} to {mc} step {ti} {{\n      {i_ub} = arith.addi {ii}, {ti} : index\n      {i_max} = arith.minsi {i_ub}, {mc} : index\n      scf.for {kk} = {z} to {kc} step {tk} {{\n        {k_ub} = arith.addi {kk}, {tk} : index\n        {k_max} = arith.minsi {k_ub}, {kc} : index\n",
    ));
    (ii, i_max, kk, k_max, z, nc, o)
}

#[allow(clippy::too_many_arguments)] // REASON: all 8 params independently meaningful; bundling would obscure intent
fn emit_ufcs_method(
    ctx: &mut LoweringContext,
    out: &mut String,
    m: &syn::ExprMethodCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    expected_ty: Option<&Type>,
    cached_receiver_val: &Option<String>,
    cached_receiver_ty: &Type,
    method_name: &str,
) -> Result<Option<(String, Type)>, String> {

        // Get the receiver value - must be pre-emitted for chaining to work
        let (receiver_val, receiver_ty) = if let Some(ref val) = cached_receiver_val {
            (val.clone(), cached_receiver_ty.clone())
        } else {
            // Emit receiver ONCE - this is critical for chain propagation
            emit_expr(ctx, out, &m.receiver, local_vars, None)?
        };
        

        
        // Build args for intrinsic: substitute _ with receiver
        // We need to emit the non-placeholder args first
        let mut emitted_args: Vec<(String, Type)> = Vec::new();
        for arg in m.args.iter() {
            if matches!(arg, syn::Expr::Infer(_)) {
                // Inject the pre-emitted receiver
                emitted_args.push((receiver_val.clone(), receiver_ty.clone()));
            } else {
                // Emit this argument normally
                let (arg_val, arg_ty) = emit_expr(ctx, out, arg, local_vars, None)?;
                emitted_args.push((arg_val, arg_ty));
            }
        }
        
        // Try intrinsic dispatch with pre-emitted values
        // We need a special path that takes already-emitted SSA values
        match method_name {
            "add_bias" => {
                // add_bias(dst, size, bias) - in-place addition
                if emitted_args.len() != 3 {
                    return Err("add_bias expects 3 arguments: (dst, size, bias_ptr)".to_string());
                }
                let dst_ptr = &emitted_args[0].0;
                let size_val = &emitted_args[1].0;
                let bias_ptr = &emitted_args[2].0;
                
                // Emit SCF loop for add_bias
                let lb = format!("%ab_lb_{}", ctx.next_id());
                let ub = format!("%ab_ub_{}", ctx.next_id());
                let step = format!("%ab_step_{}", ctx.next_id());
                
                out.push_str(&format!("    {} = arith.constant 0 : index\n", lb));
                out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", ub, size_val));
                out.push_str(&format!("    {} = arith.constant 1 : index\n", step));
                
                let iv = format!("%ab_iv_{}", ctx.next_id());
                out.push_str(&format!("    scf.for {} = {} to {} step {} {{\n", iv, lb, ub, step));
                
                let dst_gep = format!("%ab_dst_gep_{}", ctx.next_id());
                let bias_gep = format!("%ab_bias_gep_{}", ctx.next_id());
                let iv_i64 = format!("%ab_iv_i64_{}", ctx.next_id());
                
                out.push_str(&format!("      {} = arith.index_cast {} : index to i64\n", iv_i64, iv));
                out.push_str(&format!("      {} = llvm.getelementptr inbounds {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", dst_gep, dst_ptr, iv_i64));
                out.push_str(&format!("      {} = llvm.getelementptr inbounds {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", bias_gep, bias_ptr, iv_i64));
                
                let dst_val = format!("%ab_dst_val_{}", ctx.next_id());
                let bias_val = format!("%ab_bias_val_{}", ctx.next_id());
                let sum_val = format!("%ab_sum_{}", ctx.next_id());
                
                out.push_str(&format!("      {} = llvm.load {} : !llvm.ptr -> f32\n", dst_val, dst_gep));
                out.push_str(&format!("      {} = llvm.load {} : !llvm.ptr -> f32\n", bias_val, bias_gep));
                out.push_str(&format!("      {} = arith.addf {}, {} : f32\n", sum_val, dst_val, bias_val));
                out.push_str(&format!("      llvm.store {}, {} : f32, !llvm.ptr\n", sum_val, dst_gep));
                
                out.push_str("    }\n");
                
                // Return receiver for chaining
                Ok(Some((receiver_val, receiver_ty)))
            },
            "relu" => {
                // relu(dst, size) - in-place ReLU
                if emitted_args.is_empty() {
                    return Err("relu expects at least 1 argument: (dst, size)".to_string());
                }
                let dst_ptr = &emitted_args[0].0;
                let size_val = if emitted_args.len() >= 2 { &emitted_args[1].0 } else { return Err("relu needs size".to_string()); };
                
                // Emit SCF loop for relu
                let lb = format!("%relu_lb_{}", ctx.next_id());
                let ub = format!("%relu_ub_{}", ctx.next_id());
                let step = format!("%relu_step_{}", ctx.next_id());
                let zero = format!("%relu_zero_{}", ctx.next_id());
                
                out.push_str(&format!("    {} = arith.constant 0 : index\n", lb));
                out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", ub, size_val));
                out.push_str(&format!("    {} = arith.constant 1 : index\n", step));
                out.push_str(&format!("    {} = arith.constant 0.0 : f32\n", zero));
                
                let iv = format!("%relu_iv_{}", ctx.next_id());
                out.push_str(&format!("    scf.for {} = {} to {} step {} {{\n", iv, lb, ub, step));
                
                let dst_gep = format!("%relu_gep_{}", ctx.next_id());
                let iv_i64 = format!("%relu_iv_i64_{}", ctx.next_id());
                
                out.push_str(&format!("      {} = arith.index_cast {} : index to i64\n", iv_i64, iv));
                out.push_str(&format!("      {} = llvm.getelementptr inbounds {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", dst_gep, dst_ptr, iv_i64));
                
                let val = format!("%relu_val_{}", ctx.next_id());
                let res = format!("%relu_res_{}", ctx.next_id());
                
                out.push_str(&format!("      {} = llvm.load {} : !llvm.ptr -> f32\n", val, dst_gep));
                out.push_str(&format!("      {} = arith.maxnumf {}, {} : f32\n", res, val, zero));
                out.push_str(&format!("      llvm.store {}, {} : f32, !llvm.ptr\n", res, dst_gep));
                
                out.push_str("    }\n");
                
                // Return receiver for chaining
                Ok(Some((receiver_val, receiver_ty)))
            },
            "copy_from" => {
                // copy_from(dst, size, src) - copy src to dst
                if emitted_args.len() != 3 {
                    return Err("copy_from expects 3 arguments: (dst, size, src)".to_string());
                }
                let dst_ptr = &emitted_args[0].0;
                let size_val = &emitted_args[1].0;
                let src_ptr = &emitted_args[2].0;
                
                // Emit SCF loop for copy
                let lb = format!("%copy_lb_{}", ctx.next_id());
                let ub = format!("%copy_ub_{}", ctx.next_id());
                let step = format!("%copy_step_{}", ctx.next_id());
                
                out.push_str(&format!("    {} = arith.constant 0 : index\n", lb));
                out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", ub, size_val));
                out.push_str(&format!("    {} = arith.constant 1 : index\n", step));
                
                let iv = format!("%copy_iv_{}", ctx.next_id());
                out.push_str(&format!("    scf.for {} = {} to {} step {} {{\n", iv, lb, ub, step));
                
                let src_gep = format!("%copy_src_gep_{}", ctx.next_id());
                let dst_gep = format!("%copy_dst_gep_{}", ctx.next_id());
                let iv_i64 = format!("%copy_iv_i64_{}", ctx.next_id());
                
                out.push_str(&format!("      {} = arith.index_cast {} : index to i64\n", iv_i64, iv));
                out.push_str(&format!("      {} = llvm.getelementptr inbounds {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", src_gep, src_ptr, iv_i64));
                out.push_str(&format!("      {} = llvm.getelementptr inbounds {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, f32\n", dst_gep, dst_ptr, iv_i64));
                
                let val = format!("%copy_val_{}", ctx.next_id());
                
                out.push_str(&format!("      {} = llvm.load {} : !llvm.ptr -> f32\n", val, src_gep));
                out.push_str(&format!("      llvm.store {}, {} : f32, !llvm.ptr\n", val, dst_gep));
                
                out.push_str("    }\n");
                
                // Return receiver for chaining
                Ok(Some((receiver_val, receiver_ty)))
            },
            _ => {
                // Universal _ forwarding for ANY method.
                // Inject receiver SSA value as a synthetic local, replace Expr::Infer
                // nodes with Expr::Path referencing it, then recurse through normal dispatch.
                let placeholder_name = format!("__placeholder_{}", ctx.next_id());
                local_vars.insert(
                    placeholder_name.clone(),
                    (receiver_ty.clone(), crate::codegen::context::LocalKind::SSA(receiver_val.clone())),
                );
                
                // Reconstruct args: replace each Expr::Infer with Path to the placeholder
                let mut new_args = syn::punctuated::Punctuated::new();
                for arg in m.args.iter() {
                    if matches!(arg, syn::Expr::Infer(_)) {
                        let ident = syn::Ident::new(&placeholder_name, proc_macro2::Span::call_site());
                        let path = syn::ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: syn::Path::from(ident),
                        };
                        new_args.push(syn::Expr::Path(path));
                    } else {
                        new_args.push(arg.clone());
                    }
                }
                
                // Build a modified ExprMethodCall with placeholders resolved
                let mut modified = m.clone();
                modified.args = new_args;
                
                // Recurse through normal dispatch (no longer has Infer nodes)
                emit_method_call(ctx, out, &modified, local_vars, expected_ty).map(Some)
            }
        }
        // All match arms return — this is never reached
}


fn emit_rawptr_method(
    ctx: &mut LoweringContext,
    out: &mut String,
    m: &syn::ExprMethodCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    cached_receiver_val: &Option<String>,
    cached_receiver_ty: &Type,
    method_name: &str,
) -> Result<Option<(String, Type)>, String> {

        // Use cached receiver no duplicate emission
        if let Some(ref recv_val) = cached_receiver_val {
            let recv_ty = cached_receiver_ty.clone();
            // Check if receiver is NativePtr (native !llvm.ptr) or RawPtr<T> struct
            let (base_ptr, element_ty) = match &recv_ty {

                // RawPtr<T> struct: extract i64 and convert to ptr
                Type::Concrete(name, args) if name.contains("RawPtr") && !args.is_empty() => {

                    let rawptr_mlir_ty = recv_ty.to_mlir_type(ctx)?;
                    let base_addr_i64 = format!("%rawptr_inner_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n",
                        base_addr_i64, recv_val, rawptr_mlir_ty));
                    let ptr_val = format!("%base_ptr_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n",
                        ptr_val, base_addr_i64));
                    (ptr_val, args[0].clone())
                },
                Type::Struct(name) if name.contains("RawPtr") => {
                    let suffix = name.rsplit('_').next().unwrap_or("i64");
                    let elem_ty = match suffix {
                        "i32" => Type::I32, "i64" => Type::I64, "u8" => Type::U8,
                        "u32" => Type::U32, "u64" => Type::U64, "f32" => Type::F32, "f64" => Type::F64,
                        _ => Type::I64,
                    };

                    let rawptr_mlir_ty = recv_ty.to_mlir_type(ctx)?;
                    let base_addr_i64 = format!("%rawptr_inner_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n",
                        base_addr_i64, recv_val, rawptr_mlir_ty));
                    let ptr_val = format!("%base_ptr_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n",
                        ptr_val, base_addr_i64));
                    (ptr_val, elem_ty)
                },
                _ => {
                    // Not a pointer type, fall through to normal method resolution
                    ("".to_string(), Type::Unit)
                }
            };
            
            if !base_ptr.is_empty() {
                // Get the index argument
                let index_expr = m.args.get(0).ok_or("read_at/write_at requires index argument")?;
                let (index_val, _) = emit_expr(ctx, out, index_expr, local_vars, Some(&Type::I64))?;
                
                // Use llvm.getelementptr for indexed access (enables vectorization!)
                // GEP with inbounds tells LLVM this is a valid array access
                let elem_mlir_ty = element_ty.to_mlir_type(ctx)?;
                let elem_ptr = format!("%elem_gep_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.getelementptr inbounds {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n",
                    elem_ptr, base_ptr, index_val, elem_mlir_ty));
                
                if method_name == "read_at" {
                    // Emit llvm.load
                    let result_val = format!("%rawptr_read_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n",
                        result_val, elem_ptr, elem_mlir_ty));
                    return Ok(Some((result_val, element_ty)));
                } else {
                    // write_at: Emit llvm.store
                    let value_expr = m.args.get(1).ok_or("write_at requires value argument")?;
                    let (value_val, _) = emit_expr(ctx, out, value_expr, local_vars, Some(&element_ty))?;
                    
                    out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n",
                        value_val, elem_ptr, elem_mlir_ty));
                    return Ok(Some(("".to_string(), Type::Unit)));
                }
            }
        }
        Ok(None)
}


fn emit_vec_unchecked_method(
    ctx: &mut LoweringContext,
    out: &mut String,
    m: &syn::ExprMethodCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    cached_receiver_val: &Option<String>,
    cached_receiver_ty: &Type,
    method_name: &str,
) -> Result<Option<(String, Type)>, String> {

        // Check if receiver is a local variable pointing to a Vec on the stack
        if let syn::Expr::Path(p) = &*m.receiver {
            if let Some(ident) = p.path.get_ident() {
                let var_name = ident.to_string();
                if let Some((vec_ty, kind)) = local_vars.get(&var_name) {
                    // Check if it's Vec<T> 
                    let inner_ty = match vec_ty {
                        Type::Reference(inner, _) => inner.as_ref().clone(),
                        _ => vec_ty.clone(),
                    };
                    
                    // Extract element type from Vec<T>
                    let element_ty = match &inner_ty {
                        Type::Concrete(name, args) if name.contains("Vec") && !args.is_empty() => {
                            Some(args[0].clone())
                        },
                        Type::Struct(name) if name.contains("Vec_") => {
                            let suffix = name.rsplit('_').next().unwrap_or("i64");
                            Some(match suffix {
                                "i32" => Type::I32,
                                "i64" => Type::I64,
                                "u8" => Type::U8,
                                "u32" => Type::U32,
                                "u64" => Type::U64,
                                "f32" => Type::F32,
                                "f64" => Type::F64,
                                _ => Type::I64,
                            })
                        },
                        _ => None,
                    };
                    
                    if let Some(elem_ty) = element_ty {
                        // Get the stack slot pointer for this local variable
                        let slot_ptr = match kind {
                            crate::codegen::context::LocalKind::Ptr(ptr) => ptr.clone(),
                            crate::codegen::context::LocalKind::SSA(_) => format!("%local_{}", var_name),
                        };
                        

                        
                        // Vec layout: { data: Ptr<T>, len: i64, cap: i64, allocator: A }
                        // Field 0 (data) is at offset 0, so loading i64 from slot gives the ptr addr
                        
                        // The Vec->buf->ptr->inner path has all field indices = 0
                        // So the inner i64 address is at offset 0 from the stack slot
                        // Instead of 3 GEPs, load directly from stack slot as i64
                        // Mark as invariant (value never changes) and with alias scopes to 
                        // indicate this local load doesn't alias with heap stores
                        let base_addr = format!("%base_addr_{}", ctx.next_id());
                        if ctx.config.emit_alias_scopes {
                            out.push_str(&format!("    {} = llvm.load {} {{ invariant, alias_scopes = [#scope_local], noalias = [#scope_global] }} : !llvm.ptr -> i64\n",
                                base_addr, slot_ptr));
                        } else {
                            ctx.emit_load(out, &base_addr, &slot_ptr, "i64");
                        }
                        
                        // Calculate element address: base + (index * stride)
                        let index_expr = m.args.get(0).ok_or("get_unchecked/set_unchecked requires index argument")?;
                        let (index_val, _) = emit_expr(ctx, out, index_expr, local_vars, Some(&Type::I64))?;
                        
                        // Calculate stride and offset
                        let stride = ctx.size_of(&elem_ty) as i64;
                        let stride_val = format!("%stride_{}", ctx.next_id());
                        ctx.emit_const_int(out, &stride_val, stride, "i64");
                        
                        let offset_val = format!("%offset_{}", ctx.next_id());
                        ctx.emit_binop(out, &offset_val, "arith.muli", &index_val, &stride_val, "i64");
                        
                        let final_addr = format!("%elem_addr_{}", ctx.next_id());
                        ctx.emit_binop(out, &final_addr, "arith.addi", &base_addr, &offset_val, "i64");
                        
                        // Convert i64 address to !llvm.ptr
                        let elem_ptr = format!("%elem_ptr_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", 
                            elem_ptr, final_addr));
                        
                        let elem_mlir_ty = elem_ty.to_mlir_type(ctx)?;
                        
                        if method_name == "get_unchecked" {
                            let result_val = format!("%vec_get_{}", ctx.next_id());
                            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", 
                                result_val, elem_ptr, elem_mlir_ty));
                            return Ok(Some((result_val, elem_ty)));
                        } else {
                            let value_expr = m.args.get(1).ok_or("set_unchecked requires value argument")?;
                            let (value_val, _) = emit_expr(ctx, out, value_expr, local_vars, Some(&elem_ty))?;
                            
                            out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", 
                                value_val, elem_ptr, elem_mlir_ty));
                            return Ok(Some(("".to_string(), Type::Unit)));
                        }
                    }
                }
            }
        }
        
        // Fallback: use cached receiver for non-local receivers (no duplicate emission)
        if let Some(ref vec_val) = cached_receiver_val {
            let vec_ty = cached_receiver_ty.clone();
            let inner_ty = match &vec_ty {
                Type::Reference(inner, _) => inner.as_ref().clone(),
                _ => vec_ty.clone(),
            };
            
            let element_ty = match &inner_ty {
                Type::Concrete(name, args) if name.contains("Vec") && !args.is_empty() => {
                    Some(args[0].clone())
                },
                Type::Struct(name) if name.contains("Vec_") => {
                    let suffix = name.rsplit('_').next().unwrap_or("i64");
                    Some(match suffix {
                        "i32" => Type::I32,
                        "i64" => Type::I64,
                        "u8" => Type::U8,
                        "u32" => Type::U32,
                        "u64" => Type::U64,
                        "f32" => Type::F32,
                        "f64" => Type::F64,
                        _ => Type::I64,
                    })
                },
                _ => None,
            };
            
            if let Some(elem_ty) = element_ty {

                
                // Extract data pointer directly from Vec field 0.
                // Vec layout: { data: Ptr<T>, len: i64, cap: i64, allocator: A }
                let vec_mlir_ty = vec_ty.to_mlir_type(ctx)?;
                let data_ptr = format!("%vec_data_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", 
                    data_ptr, vec_val, vec_mlir_ty));
                
                let index_expr = m.args.get(0).ok_or("get_unchecked/set_unchecked requires index argument")?;
                let (index_val, _) = emit_expr(ctx, out, index_expr, local_vars, Some(&Type::I64))?;
                
                // Use native GEP for element access (preserves pointer provenance)
                let elem_mlir_ty = elem_ty.to_mlir_type(ctx)?;
                let elem_ptr = format!("%elem_ptr_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", 
                    elem_ptr, data_ptr, index_val, elem_mlir_ty));
                
                if method_name == "get_unchecked" {
                    let result_val = format!("%vec_get_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", 
                        result_val, elem_ptr, elem_mlir_ty));
                    return Ok(Some((result_val, elem_ty)));
                } else {
                    let value_expr = m.args.get(1).ok_or("set_unchecked requires value argument")?;
                    let (value_val, _) = emit_expr(ctx, out, value_expr, local_vars, Some(&elem_ty))?;
                    
                    out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", 
                        value_val, elem_ptr, elem_mlir_ty));
                    return Ok(Some(("".to_string(), Type::Unit)));
                }
            }
        }
        Ok(None)
}


fn emit_tensor_method(
    ctx: &mut LoweringContext,
    out: &mut String,
    m: &syn::ExprMethodCall,
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    cached_receiver_val: &Option<String>,
    receiver_ty: &Type,
) -> Result<Option<(String, Type)>, String> {
    let Type::Tensor(inner, _shape) = receiver_ty else { return Ok(None); };

        let method = m.method.to_string();
        if method == "zeros" {
            let res = format!("%zeros_{}", ctx.next_id());
            let mlir_ty = receiver_ty.to_mlir_type(ctx)?;
            let elem_mlir = inner.to_mlir_storage_type(ctx)?;
            let zero_reg = format!("%c0_{}", ctx.next_id());
            if inner.as_ref().is_float() {
                ctx.emit_const_float(out, &zero_reg, 0.0, &elem_mlir);
            } else {
                ctx.emit_const_int(out, &zero_reg, 0, &elem_mlir);
            }
            let empty_res = format!("%empty_{}", ctx.next_id());
            out.push_str(&format!("    {} = tensor.empty() : {}\n", empty_res, mlir_ty));
            out.push_str(&format!("    {} = linalg.fill ins({} : {}) outs({} : {}) -> {}\n", 
                res, zero_reg, elem_mlir, empty_res, mlir_ty, mlir_ty)); 
            return Ok(Some((res, receiver_ty.clone())));
        } else if method == "fill" {
            if let Some(arg_expr) = m.args.first() {
                let (val, ty) = emit_expr(ctx, out, arg_expr, local_vars, Some(inner))?;
                let val_prom = promote_numeric(ctx, out, &val, &ty, inner)?;
                let res = format!("%fill_{}", ctx.next_id());
                let mlir_ty = receiver_ty.to_mlir_type(ctx)?;
                let elem_mlir = inner.to_mlir_storage_type(ctx)?;
                let empty_res = format!("%empty_{}", ctx.next_id());
                out.push_str(&format!("    {} = tensor.empty() : {}\n", empty_res, mlir_ty));
                out.push_str(&format!("    {} = linalg.fill ins({} : {}) outs({} : {}) -> {}\n", 
                    res, val_prom, elem_mlir, empty_res, mlir_ty, mlir_ty)); 
                return Ok(Some((res, receiver_ty.clone())));
            }
        } else if method == "sum" {
             let res = format!("%sum_{}", ctx.next_id());
             let elem_mlir = inner.to_mlir_storage_type(ctx)?;
             let acc = format!("%acc_{}", ctx.next_id());
             if inner.as_ref().is_float() {
                 ctx.emit_const_float(out, &acc, 0.0, &elem_mlir);
             } else {
                 ctx.emit_const_int(out, &acc, 0, &elem_mlir);
             }
             let _recv_val = format!("%recv_{}", ctx.next_id());
             // Use cached receiver value no duplicate emission
             let rv = cached_receiver_val.clone().ok_or("Tensor sum requires a receiver value")?;
             out.push_str(&format!("    {} = linalg.reduce ins({} : {}) outs({} : {}) \n    ({{ ^bb0(%arg0: {}, %arg1: {}): \n", 
                 res, rv, receiver_ty.to_mlir_type(ctx)?, acc, elem_mlir, elem_mlir, elem_mlir));
             let red_res = format!("%red_res_{}", ctx.next_id());
             if inner.as_ref().is_float() {
                 out.push_str(&format!("        {} = arith.addf %arg0, %arg1 : {}\n", red_res, elem_mlir));
             } else {
                 out.push_str(&format!("        {} = arith.addi %arg0, %arg1 : {}\n", red_res, elem_mlir));
             }
             out.push_str(&format!("        linalg.yield {} : {}\n    }}) : {} -> {}\n", 
                 red_res, elem_mlir, receiver_ty.to_mlir_type(ctx)?, elem_mlir));
             return Ok(Some((res, *inner.clone())));
        }
        Ok(None)
}
