use std::collections::BTreeMap;
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use super::utils::*;
use crate::codegen::type_bridge::*;
use crate::common::mangling::Mangler;
use super::resolver;
use std::collections::HashMap;
use super::{emit_expr, emit_lvalue, LValueKind, extract_ident_name, infer_generics, unify_types};
use super::literals::emit_enum_constructor;

pub fn emit_call(ctx: &mut LoweringContext, out: &mut String, c: &syn::ExprCall, local_vars: &mut HashMap<String, (Type, LocalKind)>, _expected: Option<&Type>) -> Result<(String, Type), String> {
    
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
    {
        let is_indirect = match &*c.func {
            // Local variable: f(acc, val) where f is a parameter of Type::Fn
            syn::Expr::Path(p) if p.path.segments.len() == 1 => {
                let name = p.path.segments[0].ident.to_string();
                local_vars.get(&name).map(|(ty, _)| matches!(ty, Type::Fn(_, _))).unwrap_or(false)
            },
            // Parenthesized field access: (self.func)(val)
            syn::Expr::Paren(_) => true,
            // Direct field access: self.func(val) — only if field is Fn type
            syn::Expr::Field(_) => true,
            _ => false,
        };

        if is_indirect {
            // Try to evaluate the call target as an expression
            let fn_result = emit_expr(ctx, out, &c.func, local_vars, None);
            if let Ok((fn_ptr_val, fn_ty)) = fn_result {
                if let Type::Fn(param_tys, ret_ty) = &fn_ty {

                    // Emit arguments
                    let mut arg_vals = Vec::new();
                    let mut arg_mlir_tys = Vec::new();
                    for (i, arg_expr) in c.args.iter().enumerate() {
                        let hint = param_tys.get(i);
                        let (mut val, mut ty) = emit_expr(ctx, out, arg_expr, local_vars, hint)?;
                        // Numeric promotion to match parameter types
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

                    if **ret_ty == Type::Unit {
                        out.push_str(&format!("    llvm.call {}({}) : !llvm.ptr, ({}) -> ()\n",
                            fn_ptr_val, args_str, args_tys_str));
                        return Ok(("".to_string(), Type::Unit));
                    } else {
                        let res_val = format!("%indirect_call_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.call {}({}) : !llvm.ptr, ({}) -> {}\n",
                            res_val, fn_ptr_val, args_str, args_tys_str, ret_mlir_ty));
                        return Ok((res_val, *ret_ty.clone()));
                    }
                }
            }
            // If evaluation failed or type wasn't Fn, fall through to resolver
        }
    }

    let mut resolver = resolver::CallSiteResolver::new(ctx);
    let resolved_call = resolver.resolve_call(c, local_vars, _expected)?;

    match resolved_call {
        resolver::CallKind::Intrinsic(name, explicit_generics) => {
            let args_vec: Vec<syn::Expr> = c.args.iter().cloned().collect();
            
            // [SOVEREIGN V3] Intrinsic Return Type Lookup
            // If no explicit generic or expected type is provided, check if specific function exists (e.g. extern decl)
            // This is critical for explicit allocators like tensor_alloc_weights() -> Tensor<...>
            let lookup_ret_ty = if explicit_generics.is_empty() && _expected.is_none() {
                 ctx.resolve_global_func(&name).map(|(ty, _)| {
                     if let Type::Fn(_, ret) = ty { *ret } else { Type::Unit }
                 })
            } else { None };

            // For intrinsics like size_of<T>, pass the explicit generic as expected type
            let expected_for_intrinsic = if !explicit_generics.is_empty() {
                Some(&explicit_generics[0])
            } else if let Some(ty) = &lookup_ret_ty {
                Some(ty)
            } else {
                _expected
            };
            match ctx.emit_intrinsic(out, &name, &args_vec, local_vars, expected_for_intrinsic) {
                Ok(Some((val, ty))) => Ok((val, ty)),
                Ok(None) => Err(format!("Intrinsic '{}' not found", name)), // Should check registry?
                Err(e) => Err(format!("Intrinsic '{}' emission failed: {}", name, e)),
            }
        },
        resolver::CallKind::EnumConstructor(res) => {
             let args_vec: Vec<syn::Expr> = c.args.iter().cloned().collect();
             emit_enum_constructor(ctx, out, res, &args_vec, local_vars)
        },
        resolver::CallKind::StructLiteral(struct_name, fields) => {
            // In-Place Struct Initialization: alloca + stores (no function call)
            let struct_ty = Type::Struct(struct_name.clone());
            let mlir_struct_ty = struct_ty.to_mlir_type(ctx)?;
            
            // 1. Allocate stack space for the struct
            let alloca_var = format!("%struct_init_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.alloca %c1_i64 x {} : (i64) -> !llvm.ptr\n", 
                alloca_var, mlir_struct_ty));
            
            // 2. Store each argument into the corresponding field
            let args_vec: Vec<syn::Expr> = c.args.iter().cloned().collect();
            for (i, ((field_name, field_ty), arg_expr)) in fields.iter().zip(args_vec.iter()).enumerate() {
                // Emit the argument value
                let (arg_val, _arg_ty) = emit_expr(ctx, out, arg_expr, local_vars, Some(field_ty))?;
                
                // GEP to the field offset
                let gep_var = format!("%field_ptr_{}", ctx.next_id());
                let field_mlir_ty = field_ty.to_mlir_type(ctx)?;
                out.push_str(&format!("    {} = llvm.getelementptr {} [0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n",
                    gep_var, alloca_var, i, mlir_struct_ty));
                
                // Store the value
                out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", 
                    arg_val, gep_var, field_mlir_ty));
                let _ = field_name; // Used in future for named field init
            }
            
            // 3. Load the struct value from the alloca
            let load_var = format!("%struct_val_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", 
                load_var, alloca_var, mlir_struct_ty));
            
            Ok((load_var, struct_ty))
        },
        resolver::CallKind::TransparentVecAccess { method, element_ty, receiver, args } => {
            // TRANSPARENT VEC ACCESSOR: Direct MLIR emission bypassing function calls
            // This is the performance-critical path for Vec::get_unchecked and Vec::set_unchecked
            
            // 1. Emit the receiver expression (Vec value or pointer) 
            let (vec_val, vec_ty) = emit_expr(ctx, out, &receiver, local_vars, None)?;
            
            // 2. Extract the raw data pointer from Vec<T, A>.data
            // Vec layout: { data: Ptr<T>, len: i64, cap: i64, allocator: A }
            // Field 0 is `data` which is !llvm.ptr — single extractvalue.
            let (base_ptr_val, _) = {
                let vec_mlir_ty = vec_ty.to_mlir_type(ctx)?;
                
                // Extract data field (index 0) from Vec — this is !llvm.ptr
                let data_ptr = format!("%vec_data_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", 
                    data_ptr, vec_val, vec_mlir_ty));
                
                (data_ptr, Type::I64)
            };
            
            // 3. Calculate element address: base + (index * stride)
            // Emit the index expression
            let index_expr = args.get(0).ok_or("get_unchecked/set_unchecked requires index argument")?;
            let (index_val, _) = emit_expr(ctx, out, index_expr, local_vars, Some(&Type::I64))?;
            
            // Calculate stride (size of element type)
            let stride = ctx.size_of(&element_ty) as i64;
            let stride_val = format!("%stride_{}", ctx.next_id());
            ctx.emit_const_int(out, &stride_val, stride, "i64");
            
            // offset = index * stride
            let offset_val = format!("%offset_{}", ctx.next_id());
            ctx.emit_binop(out, &offset_val, "arith.muli", &index_val, &stride_val, "i64");
            
            // final_addr = base + offset
            let final_addr = format!("%elem_addr_{}", ctx.next_id());
            ctx.emit_binop(out, &final_addr, "arith.addi", &base_ptr_val, &offset_val, "i64");
            
            // 4. Convert i64 address to !llvm.ptr
            let elem_ptr = format!("%elem_ptr_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", 
                elem_ptr, final_addr));
            
            // 5. Emit load or store
            let elem_mlir_ty = element_ty.to_mlir_type(ctx)?;
            
            if method == "get_unchecked" {
                // Emit llvm.load
                let result_val = format!("%vec_get_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", 
                    result_val, elem_ptr, elem_mlir_ty));
                Ok((result_val, element_ty))
            } else {
                // set_unchecked: Emit llvm.store
                let value_expr = args.get(1).ok_or("set_unchecked requires value argument")?;
                let (value_val, _) = emit_expr(ctx, out, value_expr, local_vars, Some(&element_ty))?;
                
                out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", 
                    value_val, elem_ptr, elem_mlir_ty));
                Ok(("".to_string(), Type::Unit))
            }
        },
        resolver::CallKind::Function(mangled_name, ret_ty, arg_tys, lazy_task) => {
             // LAZY REVOLUTION: The Active Resolution Agent

             if !ctx.is_function_defined(&mangled_name) {
                 if let Some(task) = &lazy_task {
                     // [KERNEL FIX] In lib mode, cross-module functions should NOT be hydrated —
                     // they're compiled separately in their own .o file. Emit external declarations
                     // instead, preventing failures from missing module globals (e.g., GLOBAL_SCHED).
                     //
                     // [FORWARD REFERENCE FIX] @no_mangle functions have bare mangled names
                     // (e.g., "sched_yield") that don't start with the package prefix
                     // (e.g., "kernel__core__syscall__"). Without this fix, they are
                     // misclassified as cross-module and only get forward declarations,
                     // silently dropping their bodies from the MLIR output.
                     let is_cross_module = ctx.config.lib_mode && {
                         let current_pkg = &ctx.current_package;
                         if let Some(pkg) = current_pkg.as_ref() {
                             let pkg_prefix = pkg.name.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("__");
                             let name_mismatches_prefix = !mangled_name.starts_with(&format!("{}__", pkg_prefix));
                             if name_mismatches_prefix {
                                 // Before declaring cross-module, check if this function is
                                 // actually defined in the current file with @no_mangle.
                                 // If so, it's a local function with a bare name — NOT cross-module.
                                 let is_local_no_mangle = ctx.config.file.items.iter().any(|item| {
                                     if let crate::grammar::Item::Fn(f) = item {
                                         let is_nm = f.attributes.iter().any(|a| a.name == "no_mangle");
                                         is_nm && f.name.to_string() == mangled_name
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
                         // Foreign module function — just declare it, don't hydrate
                         ctx.ensure_external_declaration(&mangled_name, &arg_tys, &ret_ty)?;
                     } else {
                         // Same module — hydrate the body
                         ctx.hydrate_specialization(*task.clone())?;
                     }
                 } else {
                     // Fallback for Externs/Globals not requiring specialization
                     ctx.ensure_external_declaration(&mangled_name, &arg_tys, &ret_ty)?;
                 }
             }

             let args_vec: Vec<syn::Expr> = c.args.iter().cloned().collect();

             // Extract Verification Data from Task
             let requires = lazy_task.as_ref().map(|t| t.func.requires.clone()).unwrap_or_default();
             let param_names: Vec<String> = lazy_task.as_ref()
                 .map(|t| t.func.args.iter().map(|a| a.name.to_string()).collect())
                 .unwrap_or_default();

             // 2. Emit Arguments & Capture for Verification
             let mut args_vals = Vec::new();
             let mut inferred_tys = Vec::new();
             let _arg_tys_ref = &arg_tys;
             let use_fallback_inference = arg_tys.is_empty() && !c.args.is_empty();

             // [SOVEREIGN V4.0] Verify Preconditions at Call Site
             // translate_to_z3 is pure Z3 — no MLIR emitted. Safe to call before arg emission.
             if !requires.is_empty() {
                 if let Err(e) = crate::codegen::verification::VerificationEngine::verify(ctx, &requires, &param_names, &args_vec, local_vars) {
                     eprintln!("Verification Error: {}", e);
                     return Err(e);
                 }
             }

             // Emit Args
             for (i, arg_expr) in args_vec.iter().enumerate() {
                 // [SOVEREIGN V25.0]: Domain-Isolated Argument Evaluation
                 // We pass None as the hint to prevent "Type Osmosis" (Pointer hints bleeding into indices).
                 // This ensures the Pointer base and the Usize index never share a type-hint context.
                 let (mut val, mut ty) = emit_expr(ctx, out, arg_expr, local_vars, None)?;
                 
                 // PILLAR 1: Verified Metal Alignment
                 // We perform an explicit, isolated promotion to the parameter's target type.
                 if let Some(target) = arg_tys.get(i) {
                     // Auto-spill for Owned types if needed (Linear Type Bridging)
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
                     // V25.0: Explicit Numeric Promotion (handles Usize -> I64, F64 -> F32, etc.)
                     // This correctly FAILS for "Usize -> Pointer" because there's no hint to trick it.
                     if !ty.structural_eq(target) {
                         val = promote_numeric(ctx, out, &val, &ty, target)?;
                     }
                     ty = target.clone();
                 }

                 
                 args_vals.push(val);
                 if use_fallback_inference {
                     inferred_tys.push(ty);
                 }
             }

             let final_arg_tys = if use_fallback_inference { inferred_tys } else { arg_tys };

             // 3. Emit Low-Level Call
             let call_name = mangled_name;
             
             ctx.ensure_func_declared(&call_name, &final_arg_tys, &ret_ty)?;

             let mut args_tys_code = Vec::new();
             let args_str = args_vals.join(", ");
             for t in &final_arg_tys {
                 args_tys_code.push(t.to_mlir_type(ctx)?);
             }
             let args_tys_str = args_tys_code.join(", ");
             
             let res_val = if ret_ty != Type::Unit {
                 format!("%call_{}_{}", call_name, ctx.next_id())
             } else {
                 "".to_string() 
             };

             // [SOVEREIGN V4.1] LLVM Intrinsic Interception
             // Intercept memcpy calls and emit LLVM intrinsic for vectorized store optimization.
             // The llvm.intr.memcpy allows LLVM to merge small constant stores into SIMD instructions.
             if call_name == "memcpy" && args_vals.len() == 3 {
                 // Convert i64 addresses to !llvm.ptr for the intrinsic
                 let dest_ptr = format!("%memcpy_dest_{}", ctx.next_id());
                 let src_ptr = format!("%memcpy_src_{}", ctx.next_id());
                 out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", dest_ptr, args_vals[0]));
                 out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", src_ptr, args_vals[1]));
                 
                 // Emit the LLVM memcpy intrinsic (isVolatile = false for optimization)
                 out.push_str(&format!("    \"llvm.intr.memcpy\"({}, {}, {}) <{{isVolatile = false}}> : (!llvm.ptr, !llvm.ptr, i64) -> ()\n", 
                     dest_ptr, src_ptr, args_vals[2]));
                 
                 // Return the destination address as the result (matching C memcpy semantics)
                 if !res_val.is_empty() {
                     out.push_str(&format!("    {} = arith.constant 0 : i64\n", res_val));
                 }
             } else if call_name == "free" && !args_vals.is_empty() {
                 // [SOVEREIGN V5.0] Z3 Ownership Tracking: Deallocator Interception
                 // When free(var) is called, extract the source variable name from the
                 // argument expression and mark the corresponding malloc allocation as released.
                 if let Some(first_arg) = args_vec.first() {
                     let var_name = extract_ident_name(first_arg);
                     if let Some(var_name) = var_name {
                         let alloc_id = format!("malloc:{}", var_name);
                         // Mark the allocation as released in the Z3 tracker.
                         // If the allocation was never tracked (e.g., freeing a foreign pointer),
                         // mark_released silently allows it (existing behavior).
                         if let Err(e) = ctx.ownership_tracker.mark_released(
                             &alloc_id,
                             &ctx.z3_solver
                         ) {
                             return Err(e);
                         }
                         // [DAG MallocTracker] Also mark freed in the standalone tracker
                         ctx.malloc_tracker.free(&alloc_id);
                     }
                 }
                 // Emit the actual free() call
                 out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", call_name, args_str, args_tys_str));
             } else if res_val.is_empty() {
                 out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", call_name, args_str, args_tys_str));
             } else {
                 out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", res_val, call_name, args_str, args_tys_str, ret_ty.to_mlir_type(ctx)?));
             }

             // [SOVEREIGN V5.0] Z3 Ownership Tracking: Allocator Interception
             // When malloc() is called, store a pending allocation marker so that
             // the let-binding in stmt.rs can register it with the Z3 tracker.
             if call_name == "malloc" && !res_val.is_empty() {
                 // Store the pending malloc result. The let-binding handler in stmt.rs
                 // will pick this up and register the source variable name with the
                 // Z3 ownership tracker via malloc_tracker.
                 *ctx.pending_malloc_result = Some(res_val.clone());
             }

             // [SALT MEMORY MODEL] Pointer State Interception
             // Detect constructors that produce known pointer states:
             // - Ptr::empty() → Empty state
             // - Box::new()   → Valid state  
             // - Arena::alloc() → Valid state
             // - from_addr(0) → Empty state (detected by name + arg value)
             if call_name.contains("__empty") && call_name.contains("Ptr") {
                 *ctx.pending_pointer_state = 
                     Some(crate::codegen::verification::PointerState::Empty);
             } else if call_name.contains("__new") && call_name.contains("Box") {
                 *ctx.pending_pointer_state = 
                     Some(crate::codegen::verification::PointerState::Valid);
             } else if (call_name.contains("__alloc") || call_name.contains("__place")) && call_name.contains("Arena") {
                 *ctx.pending_pointer_state = 
                     Some(crate::codegen::verification::PointerState::Valid);
             }
             
             let mut final_res = res_val;
             let mut final_ret_ty = ret_ty.clone();

             // [SOVEREIGN V3] Tensor Dehydration: Removed (Type::Tensor is !llvm.ptr now)

             // Post-Call Promotion (e.g. if we expected something else)
             if let Some(exp) = _expected {
                 if exp.is_numeric() && final_ret_ty.is_numeric() {
                     if let Ok(promoted) = promote_numeric(ctx, out, &final_res, &final_ret_ty, exp) {
                         final_res = promoted;
                         final_ret_ty = exp.clone();
                     }
                 }
             }

             Ok((final_res, final_ret_ty))
        }
    }
}

#[allow(dead_code)]
pub(crate) fn resolve_call_path(ctx: &mut LoweringContext, func_expr: &syn::Expr) -> Result<Option<(String, Vec<Type>)>, String> {
     if let Some(segments) = get_path_from_expr(func_expr) {
        let mut g_args: Vec<Type> = Vec::new();
        if let syn::Expr::Path(p) = func_expr {
             for seg in &p.path.segments {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    for arg in &args.args {
                        match arg {
                            syn::GenericArgument::Type(ty) => {
                                g_args.push(resolve_type(ctx, &crate::grammar::SynType::from_std(ty.clone()).unwrap()));
                            }
                            syn::GenericArgument::Const(expr) => {
                                 if let Ok(crate::evaluator::ConstValue::Integer(val)) = ctx.evaluator.eval_expr(expr) {
                                     g_args.push(crate::types::Type::Struct(val.to_string()));
                                 } else {
                                     g_args.push(crate::types::Type::Struct("0".to_string()));
                                 }
                            }
                            _ => {}
                        }
                    }
                }
             }
        }
        
        let name = if let Some((pkg, item)) = resolve_package_prefix_ctx(ctx, &segments) {
             if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) }
        } else {
             // Imports resolution fallback
             if segments.len() >= 2 {
                 // ... (Keep existing logic or rely on resolve_package_prefix being robust?)
                 // resolve_package_prefix handles exact alias and tail match.
                 // If it returned None, we join with __.
                 Mangler::mangle(&segments)
             } else {
                 Mangler::mangle(&segments)
             }
        };
        
        Ok(Some((name, g_args)))
     } else {
         Ok(None)
     }
}

pub fn emit_method_call(ctx: &mut LoweringContext, out: &mut String, m: &syn::ExprMethodCall, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected_ty: Option<&Type>) -> Result<(String, Type), String> {
    
    // [OWNERSHIP TRACKING] When .free() or .drop() is called on a variable,
    // mark it as released in the Z3 ownership tracker so verify_leak_free passes.
    // Also remove from cleanup stack to prevent double-free in RAII cleanup.
    let method_name = m.method.to_string();
    if method_name == "free" || method_name == "drop" {
        if let syn::Expr::Path(p) = &*m.receiver {
            if let Some(ident) = p.path.get_ident() {
                let var_name = ident.to_string();
                let _ = ctx.ownership_tracker.mark_released(
                    &var_name,
                    &ctx.z3_solver
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
    
    // [RECEIVER MEMOIZATION - CRITICAL FIX]
    // Emit the receiver expression EXACTLY ONCE at the top of emit_method_call.
    // This prevents chained calls like obj.methodA().methodB() from re-evaluating methodA() 
    // multiple times (which caused 4x mmap calls for a single .mmap().unwrap() chain).
    // All subsequent type checks and dispatch logic MUST use these cached values.
    let (cached_receiver_val, cached_receiver_ty): (Option<String>, Type) = 
        if let syn::Expr::Path(p) = &*m.receiver {
            // For simple path expressions (local variables, globals), use emit_lvalue to get address
            // without loading aggregate types. This is more efficient for struct receivers.
            if let Some(ident) = p.path.get_ident() {
                let var_name = ident.to_string();
                if let Some((ty, kind)) = local_vars.get(&var_name) {
                    // Local variable - we have it in scope, don't need to emit
                    match kind {
                        crate::codegen::context::LocalKind::Ptr(ptr) => {
                            // For pointer-backed locals, we have the address already
                            (Some(ptr.clone()), Type::Reference(Box::new(ty.clone()), false))
                        },
                        crate::codegen::context::LocalKind::SSA(val) => {
                            (Some(val.clone()), ty.clone())
                        },
                    }
                } else {
                    // Not a local - try emit_expr once
                    match emit_expr(ctx, out, &m.receiver, local_vars, None) {
                        Ok((val, ty)) => (Some(val), ty),
                        Err(_) => {
                            // Static type resolution fallback
                            let ty = resolve_type(ctx, &crate::grammar::SynType::from_std(
                                syn::Type::Path(syn::TypePath { qself: None, path: p.path.clone() })
                            ).unwrap());
                            (None, ty)
                        }
                    }
                }
            } else {
                // Multi-segment path - emit once
                match emit_expr(ctx, out, &m.receiver, local_vars, None) {
                    Ok((val, ty)) => (Some(val), ty),
                    Err(_) => {
                        let ty = resolve_type(ctx, &crate::grammar::SynType::from_std(
                            syn::Type::Path(syn::TypePath { qself: None, path: p.path.clone() })
                        ).unwrap());
                        (None, ty)
                    }
                }
            }
        } else {
            // Complex expression (method call, field access, etc.) - emit exactly once
            match emit_expr(ctx, out, &m.receiver, local_vars, None) {
                Ok((val, ty)) => (Some(val), ty),
                Err(_) => (None, Type::Unit),
            }
        };
    // [SOVEREIGN FIX] Substitute generics in cached receiver type at the source
    // This ensures ALL uses of cached_receiver_ty throughout emit_method_call 
    // will have concrete types (e.g., Ptr<u8> instead of Ptr<T>)
    let mut cached_receiver_ty = cached_receiver_ty.substitute(&ctx.current_type_map());
    // [CANONICAL RESOLUTION] Canonicalize receiver type to prevent raw Struct("Node")
    // from producing !struct_Box_Node instead of !struct_Box_main__Node in MLIR.
    cached_receiver_ty = resolve_codegen_type(ctx, &cached_receiver_ty);
    
    // [GRAYDON FIX] For static method calls with turbofish (e.g., HashMap::<i64, i64>::with_capacity),
    // extract the turbofish args from the path and inject them into the receiver type.
    // This is critical because get_path_from_expr discards the turbofish arguments.
    let path_turbofish_args = get_path_turbofish_args(&m.receiver);
    if !path_turbofish_args.is_empty() {
        // Convert syn::Type args to our Type representation
        let concrete_args: Vec<Type> = path_turbofish_args.iter()
            .filter_map(|syn_ty| {
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

    // [POINTER SAFETY CONTRACT] Verify receiver validity for unsafe methods
    // If calling unsafe methods on Ptr<T> (read, write, offset, etc.),
    // the receiver must be Valid. Safe methods (addr, is_null) are exempt.
    if let Type::Pointer { .. } = &cached_receiver_ty {
        let is_safe = matches!(method_name.as_str(), "addr" | "is_null" | "from_addr" | "empty" | "new");
        if !is_safe {
             // Enforce validity on tracked variables
             if let syn::Expr::Path(path) = &*m.receiver {
                 if let Some(ident) = path.path.get_ident() {
                      // Check state
                      ctx.pointer_tracker.check_deref(&ident.to_string())?;
                 }
             }
        }
    }
    
    // [SOVEREIGN V1.0] Clean Break: Removed NativePtr method interception (get/set/at/offset)
    // These are now handled by standard library intrinsics or unified syntax.

    // [SOVEREIGN FLUENT-MATH] Matrix multiplication: receiver.matmul(other)
    // Called via A @ B syntax (preprocessor converts to A.matmul(B))
    // Supports:
    //   - Type::Tensor (rank 2 @ rank 2) -> linalg.matmul
    //   - Type::Tensor (rank 2 @ rank 1) -> linalg.matvec
    //   - Type::Pointer with Tensor element -> extracts shape from Tensor
    if method_name == "matmul" {
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
        // [SOVEREIGN PHASE 3] linalg.matvec with JIT memref casting for M4 optimization
        if b_rank == 1 {
            if b_shape[0] != k_dim {
                return Err(format!("matvec dimension mismatch: A is {}x{}, B is {}", 
                    m_dim, k_dim, b_shape[0]));
            }
            
            // Define memref types for structured linalg ops
            let a_memref_ty = format!("memref<{}x{}x{}>", m_dim, k_dim, elem_mlir);
            let b_memref_ty = format!("memref<{}x{}>", k_dim, elem_mlir);
            let c_memref_ty = format!("memref<{}x{}>", m_dim, elem_mlir);
            
            // [MECHANICAL SYMPATHY] JIT MemRef Casting
            // Zero-cost metadata wrap: !llvm.ptr → memref via unrealized_conversion_cast
            // This tells MLIR the exact strides/sizes for tiling optimization
            
            // Cast A: !llvm.ptr → memref<MxKxf32>
            let a_memref = format!("%a_view_{}", ctx.next_id());
            out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : !llvm.ptr to {}\n", 
                a_memref, a_val, a_memref_ty));
            
            // Cast B: !llvm.ptr → memref<Kxf32>  
            let b_memref = format!("%b_view_{}", ctx.next_id());
            out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : !llvm.ptr to {}\n",
                b_memref, b_val, b_memref_ty));
            
            // Allocate output: memref<Mxf32> (for result vector)
            let c_memref = format!("%c_buf_{}", ctx.next_id());
            out.push_str(&format!("    {} = memref.alloc() : {}\n", c_memref, c_memref_ty));
            
            // Zero-initialize output
            let zero = format!("%mv_zero_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 0.0 : {}\n", zero, elem_mlir));
            out.push_str(&format!("    linalg.fill ins({} : {}) outs({} : {})\n", 
                zero, elem_mlir, c_memref, c_memref_ty));
            
            // [CORE] Emit linalg.matvec - enables register blocking, software pipelining, AMX
            out.push_str(&format!("    linalg.matvec ins({} : {}, {} : {}) outs({} : {})\n",
                a_memref, a_memref_ty, b_memref, b_memref_ty, c_memref, c_memref_ty));
            
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
            
            return Ok((c_ptr, ptr_ty));
        }
        
        // Matrix-Matrix: A[M,K] @ B[K,N] = C[M,N]
        // [SOVEREIGN PHASE 3] linalg.matmul with JIT memref casting
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
            
            // JIT memref casting for matrix operands
            let a_memref = format!("%a_view_{}", ctx.next_id());
            out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : !llvm.ptr to {}\n", 
                a_memref, a_val, a_memref_ty));
            
            let b_memref = format!("%b_view_{}", ctx.next_id());
            out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : !llvm.ptr to {}\n",
                b_memref, b_val, b_memref_ty));
            
            // Allocate output buffer
            let c_memref = format!("%matmul_buf_{}", ctx.next_id());
            out.push_str(&format!("    {} = memref.alloc() : {}\n", c_memref, c_memref_ty));
            
            // Zero-initialize output
            let zero = format!("%mm_zero_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 0.0 : {}\n", zero, elem_mlir));
            out.push_str(&format!("    linalg.fill ins({} : {}) outs({} : {})\n", 
                zero, elem_mlir, c_memref, c_memref_ty));
            
            // Emit linalg.matmul - enables AMX on Apple Silicon
            out.push_str(&format!("    linalg.matmul ins({} : {}, {} : {}) outs({} : {})\n",
                a_memref, a_memref_ty, b_memref, b_memref_ty, c_memref, c_memref_ty));
            
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
            
            return Ok((c_ptr, ptr_ty));
        }
        
        return Err(format!("matmul: unsupported operand ranks: {} @ {}", a_rank, b_rank));
    }
    
    // [SOVEREIGN FLUENT-MATH / UFCS] Universal Function Call Syntax
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
        match method_name.as_str() {
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
                return Ok((receiver_val, receiver_ty));
            },
            "relu" => {
                // relu(dst, size) - in-place ReLU
                if emitted_args.len() < 1 {
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
                return Ok((receiver_val, receiver_ty));
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
                return Ok((receiver_val, receiver_ty));
            },
            _ => {
                // [GENERALIZED PLACEHOLDER] Universal _ forwarding for ANY method.
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
                return emit_method_call(ctx, out, &modified, local_vars, expected_ty);
            }
        }
    }

    // NOTE: as_ptr() is handled by normal monomorphized method dispatch.
    // The @inline as_ptr method generates correct MLIR with fully-qualified type aliases.
    


    // RAWPTR TRANSPARENT INTRINSIC INTERCEPT (Native Ptr + GEP for Vectorization)
    // Uses !llvm.ptr + llvm.getelementptr instead of i64 + inttoptr to preserve
    // pointer provenance and enable LLVM loop vectorization.
    let method_name = m.method.to_string();
    if method_name == "read_at" || method_name == "write_at" {
        // Use cached receiver (MEMOIZATION FIX - no duplicate emission)
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
                    return Ok((result_val, element_ty));
                } else {
                    // write_at: Emit llvm.store
                    let value_expr = m.args.get(1).ok_or("write_at requires value argument")?;
                    let (value_val, _) = emit_expr(ctx, out, value_expr, local_vars, Some(&element_ty))?;
                    
                    out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n",
                        value_val, elem_ptr, elem_mlir_ty));
                    return Ok(("".to_string(), Type::Unit));
                }
            }
        }
    }
    
    // TRANSPARENT VEC ACCESSOR INTERCEPT (Zero-Overhead Path)
    if method_name == "get_unchecked" || method_name == "set_unchecked" {
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
                            return Ok((result_val, elem_ty));
                        } else {
                            let value_expr = m.args.get(1).ok_or("set_unchecked requires value argument")?;
                            let (value_val, _) = emit_expr(ctx, out, value_expr, local_vars, Some(&elem_ty))?;
                            
                            out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", 
                                value_val, elem_ptr, elem_mlir_ty));
                            return Ok(("".to_string(), Type::Unit));
                        }
                    }
                }
            }
        }
        
        // Fallback: use cached receiver for non-local receivers (MEMOIZATION FIX)
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

                
                // [PHASE 5 FIX] Extract data pointer directly from Vec field 0.
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
                    return Ok((result_val, elem_ty));
                } else {
                    let value_expr = m.args.get(1).ok_or("set_unchecked requires value argument")?;
                    let (value_val, _) = emit_expr(ctx, out, value_expr, local_vars, Some(&elem_ty))?;
                    
                    out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", 
                        value_val, elem_ptr, elem_mlir_ty));
                    return Ok(("".to_string(), Type::Unit));
                }
            }
        }
    }
    
    // Use cached receiver type for Tensor and other type-based dispatch
    // (Memoization already done at function entry)
    let receiver_ty = cached_receiver_ty.clone();

    if let Type::Tensor(inner, _shape) = &receiver_ty {
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
            return Ok((res, receiver_ty.clone()));
        } else if method == "fill" {
            if let Some(arg_expr) = m.args.first() {
                let (val, ty) = emit_expr(ctx, out, arg_expr, local_vars, Some(&inner))?;
                let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                let res = format!("%fill_{}", ctx.next_id());
                let mlir_ty = receiver_ty.to_mlir_type(ctx)?;
                let elem_mlir = inner.to_mlir_storage_type(ctx)?;
                let empty_res = format!("%empty_{}", ctx.next_id());
                out.push_str(&format!("    {} = tensor.empty() : {}\n", empty_res, mlir_ty));
                out.push_str(&format!("    {} = linalg.fill ins({} : {}) outs({} : {}) -> {}\n", 
                    res, val_prom, elem_mlir, empty_res, mlir_ty, mlir_ty)); 
                return Ok((res, receiver_ty.clone()));
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
             // Use cached receiver value (MEMOIZATION FIX - no duplicate emission)
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
             return Ok((res, *inner.clone()));
        }
    }

    // 1. Use receiver TYPE for method mangling (critical for allocator recursion fix)
    // If receiver is a struct/concrete type, use its type name (e.g., GlobalSlabAlloc)
    // instead of the variable name (e.g., GLOBAL_ALLOC) to construct the method name.
    let method = m.method.to_string();
    let type_based_pkg = match &receiver_ty {
        Type::Struct(name) => Some(name.clone()),
        Type::Concrete(name, _args) => {
            // For generic types like Ptr<u8>, we need the base name for method lookup
            // but will add the specialization suffix separately
            Some(name.clone())
        },
        // [SOVEREIGN FIX] Handle Type::Pointer for Ptr<T> method calls with fully-qualified name
        Type::Pointer { .. } => Some("std__core__ptr__Ptr".to_string()),
        _ => None,
    };

    // [MODULE NAMESPACE GUARD] If receiver has no value (cached_receiver_val is None),
    // it was resolved purely as a type/module from imports (e.g. `serial` → Concrete("kernel__drivers__serial")).
    // In this case, check if it's actually a known struct or enum in the registries.
    // If not, it's a module namespace — clear type_based_pkg so we don't inject &self.
    // NOTE: We intentionally do NOT check generic_impls here because it stores
    // module-level functions (e.g. kernel__drivers__serial__init) not just struct methods.
    let type_based_pkg = if type_based_pkg.is_some() && cached_receiver_val.is_none() {
        let name = type_based_pkg.as_ref().unwrap();
        let is_known_type = ctx.struct_registry().values().any(|info| &info.name == name)
            || ctx.enum_registry().values().any(|info| &info.name == name);
        if is_known_type {
            type_based_pkg
        } else {

            None
        }
    } else {
        type_based_pkg
    };
    
    // Extract generic type arguments for method specialization suffix
    let receiver_generic_suffix = match &receiver_ty {
        Type::Concrete(_, args) if !args.is_empty() => {
            Some(args.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_"))
        },
        // [SOVEREIGN FIX] Extract element type from Type::Pointer for method specialization suffix
        Type::Pointer { element, .. } => {
            Some(element.mangle_suffix())
        },
        _ => None,
    };

    // 2. Check for Static Package Call (Namespace Lookahead)
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
                 let receiver_concrete_args: Vec<Type> = match &receiver_ty {
                     Type::Concrete(_, args) => args.clone(),
                     Type::Reference(inner, _) => match inner.as_ref() {
                         Type::Concrete(_, args) => args.clone(),
                         _ => vec![],
                     },
                     _ => vec![],
                 };

                 
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
                                let mut params: Vec<Type> = func_args.iter()
                                     .filter_map(|arg| arg.ty.as_ref().and_then(|t| Type::from_syn(t)))
                                     .collect();
                                let mut args_for_infer = emitted_tys.clone();
                                
                                // Infer from Return Type expectation
                                if let Some(ret_def) = &func_ret_type {
                                     if let Some(exp) = expected_ty {
                                          if let Some(ret_ty_gen) = Type::from_syn(ret_def) {
                                               params.push(ret_ty_gen);
                                               args_for_infer.push(exp.clone());
                                          }
                                     }
                                }

                                let concrete = infer_generics(&params, &args_for_infer, &generics);
                                mangled = ctx.request_specialization(&original_mangled, concrete.clone(), None);

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
                                     Type::from_syn(rt).unwrap_or(Type::Unit)
                                } else { Type::Unit };
                                let ret_ty_subst = ret_ty_base.substitute(&subst_map);

                                let args_subst = func_args.iter().filter_map(|arg| {
                                     arg.ty.as_ref().and_then(|t| Type::from_syn(t)).map(|ty| ty.substitute(&subst_map))
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
                  

                  
                  // [V4.0 SOVEREIGN] Use TraitRegistry for method lookup  
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
                      
                      if let Type::Concrete(_, args) = &receiver_ty {
                          if let Some(generics) = &func_def.generics {
                              for (i, param) in generics.params.iter().enumerate() {
                                  if let crate::grammar::GenericParam::Type { name, .. } = param {
                                      if let Some(arg) = args.get(i) {
                                          subst_map.insert(name.to_string(), arg.clone());
                                      }
                                  }
                              }
                          }
                          // Also add T -> arg[0] mapping for simple cases
                          if args.len() == 1 {
                              subst_map.insert("T".to_string(), args[0].clone());
                          }
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
                              Some((task.func.ret_type.clone(), task.func.args.clone(), task.type_map.clone()))
                          } else { None }
                      });
                      let pending_sig = if let Some((ret_type, func_args, type_map)) = pending_task_data {
                          let ret_ty = if let Some(rt) = &ret_type {
                              crate::codegen::type_bridge::resolve_type(ctx, rt).substitute(&type_map)
                          } else { Type::Unit };
                          let args: Vec<Type> = func_args.iter().filter_map(|arg| {
                              arg.ty.as_ref().map(|t| {
                                  crate::codegen::type_bridge::resolve_type(ctx, t).substitute(&type_map)
                              })
                          }).collect();
                          Some((ret_ty, args))
                      } else { None };
                      
                      if let Some((ret_ty, args)) = pending_sig {

                          (ret_ty, args)
                      } else {
                          // Fallback: Check for free function in package
                          let short_mangled = format!("{}__{}", pkg, method);

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
                  let short_mangled = format!("{}__{}", pkg, method); // pkg is from resolve_package_prefix, likely the module path?
                  // Wait, pkg variable from line 2296 is just the base package.
                  // If resolve_package_prefix returns (pkg, item). item is struct name.
                  // So pkg is the module.
                  // So short_mangled = pkg + "__" + method.

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
                                      return Ok((res, *inner));
                                  } else if method == "load" {
                                      let res = format!("%atomic_load_{}", ctx.next_id());
                                      ctx.emit_load_atomic(out, &res, &receiver_addr, &mlir_ty);
                                      return Ok((res, *inner));
                                  } else if method == "store" {
                                      let (val, ty) = emit_expr(ctx, out, &m.args[0], local_vars, Some(&inner))?;
                                      let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                                      ctx.emit_store_atomic(out, &val_prom, &receiver_addr, &mlir_ty);
                                      return Ok(("%unit".to_string(), Type::Unit));
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
                 // Methods expect a reference to self - take address if needed
                 let recv_ref = if matches!(recv_ty, Type::Reference(_, _)) {
                     recv_val
                 } else {
                     // For globals like GLOBAL_ALLOC, we need to load its address
                     if let syn::Expr::Path(p) = &*m.receiver {
                         let name = p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("__");
                         if let Some((canonical, _)) = resolve_package_prefix_ctx(ctx, &[name.clone()]) {
                             let full_name = if canonical.is_empty() { name } else { canonical };
                             let ptr_var = format!("%recv_ptr_{}", ctx.next_id());
                             out.push_str(&format!("    {} = llvm.mlir.addressof @{} : !llvm.ptr\n", ptr_var, full_name));
                             ptr_var
                         } else {
                             recv_val
                         }
                     } else {
                         recv_val
                     }
                 };
                 final_args.push(recv_ref);
                 // Use the resolved signature type for self if available, otherwise fallback to Reference
                 let self_arg_ty = if !expected_arg_tys.is_empty() {
                     expected_arg_tys[0].clone()
                 } else {
                     Type::Reference(Box::new(receiver_ty.clone()), true)
                 };
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
             return Ok((res, ret_ty));
        }
    }

    let method_name = m.method.to_string();

    // 1. Try Intrinsic (e.g. popcount)
    let mut intrinsic_args = vec![*m.receiver.clone()];
    intrinsic_args.extend(m.args.iter().cloned());
    if let Ok(Some(res)) = ctx.emit_intrinsic(out, &method_name, &intrinsic_args, local_vars, expected_ty) {
         return Ok(res);
    }

    // Special handling for Atomic intrinsics (fetch_add, fetch_sub, load, store)
    // These intercept method calls on Atomic<T> and lower them directly to LLVM
    // atomic instructions, enabling lock-free data structures (Treiber stacks, etc.)
    if let Ok((receiver_addr, receiver_ty, _kind)) = emit_lvalue(ctx, out, &m.receiver, local_vars) {
        if let Type::Atomic(inner) = receiver_ty {
            let mlir_ty = inner.to_mlir_type(ctx)?;
            if method_name == "fetch_add" {
                 let (val, ty) = emit_expr(ctx, out, &m.args[0], local_vars, Some(&inner))?;
                 let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                 let res = format!("%atomic_res_{}", ctx.next_id());
                 ctx.emit_atomicrmw(out, &res, "add", &receiver_addr, &val_prom, &mlir_ty);
                 return Ok((res, *inner));
            } else if method_name == "fetch_sub" {
                 let (val, ty) = emit_expr(ctx, out, &m.args[0], local_vars, Some(&inner))?;
                 let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                 let res = format!("%atomic_res_{}", ctx.next_id());
                 ctx.emit_atomicrmw(out, &res, "sub", &receiver_addr, &val_prom, &mlir_ty);
                 return Ok((res, *inner));
            } else if method_name == "load" {
                 let res = format!("%atomic_load_{}", ctx.next_id());
                 ctx.emit_load_atomic(out, &res, &receiver_addr, &mlir_ty);
                 return Ok((res, *inner));
            } else if method_name == "store" {
                 let (val, ty) = emit_expr(ctx, out, &m.args[0], local_vars, Some(&inner))?;
                 let val_prom = promote_numeric(ctx, out, &val, &ty, &inner)?;
                 ctx.emit_store_atomic(out, &val_prom, &receiver_addr, &mlir_ty);
                 return Ok(("%unit".to_string(), Type::Unit));
            }
        }
    }

    // [ABI FIX] Use emit_lvalue for receivers to get address directly without loading 1KB+ structs
    // This is the core fix for the "Fat Receiver" bug - we pass pointers, not values
    let (receiver_ptr, receiver_ty) = if let Ok((addr, raw_ty, _kind)) = emit_lvalue(ctx, out, &m.receiver, local_vars) {
        // [SOVEREIGN FIX] Apply current type_map to resolve generics in lvalue types
        let ty = raw_ty.substitute(&ctx.current_type_map());

        // Success: we have the address of the receiver
        // Determine if this is an aggregate type that should be passed by reference
        // [SOVEREIGN FIX] Recursively check through Type::Owned, Type::Reference wrappers
        fn is_aggregate_type(ty: &Type) -> bool {
            match ty {
                Type::Struct(_) | Type::Concrete(_, _) | Type::Array(_, _, _) => true,
                Type::Owned(inner) => is_aggregate_type(inner),
                Type::Reference(inner, _) => is_aggregate_type(inner),
                _ => false,
            }
        }
        let is_aggregate = is_aggregate_type(&ty);
        if is_aggregate {

            // Return the pointer directly - wrap type in Reference to signal pointer semantics
            // This ensures downstream coercion logic (lines 2740+) knows we have a pointer
            (addr, Type::Reference(Box::new(ty), false))
        } else {
            // For non-aggregates (primitives), load as usual

            let val = format!("%recv_load_{}", ctx.next_id());
            let mlir_ty = ty.to_mlir_storage_type(ctx)?;
            ctx.emit_load(out, &val, &addr, &mlir_ty);
            (val, ty)
        }
    } else {
        // Fallback: emit_lvalue failed, use cached receiver (MEMOIZATION FIX - no duplicate emission)
        // For computed receivers like function results, we already emitted above
        if let Some(ref val) = cached_receiver_val {

            // [SOVEREIGN FIX] Apply substitution to cached receiver type too
            (val.clone(), cached_receiver_ty.substitute(&ctx.current_type_map()))
        } else {
            return Err(format!("Method call '{}' requires a receiver value", method_name));
        }
    };
    let receiver_val = receiver_ptr.clone();
    // Extract inner type for method resolution (strip the Reference wrapper we added)
    let raw_lookup_ty = if let Type::Reference(inner, _) = &receiver_ty { *inner.clone() } else { receiver_ty.clone() };
    // SOVEREIGN FIX: Apply current type_map substitution to resolve generics like T -> F32
    // This ensures that method calls inside generic functions use concrete types
    let current_map = ctx.current_type_map().clone();

    let method_lookup_ty = resolve_codegen_type(ctx, &raw_lookup_ty.substitute(&current_map));

    // [SOVEREIGN FIX] Use fully-qualified TEMPLATE name for target_name (without specialization suffix)
    // The specialization suffix is added separately via request_specialization
    // Example: Type::Pointer { element: U8 } -> "std__core__ptr__Ptr" (not "std__core__ptr__Ptr_u8")
    let target_name = match &method_lookup_ty {
        Type::Pointer { .. } => "std__core__ptr__Ptr".to_string(),
        Type::Struct(name) => name.clone(),
        Type::Concrete(base, _) => base.clone(),  // Use base template name without args
        _ => method_lookup_ty.mangle_suffix(),
    };

    // Helper closure for lookup
    let lookup_recursive = |ty: &Type| -> Option<((crate::grammar::SaltFn, Option<Type>, Vec<crate::grammar::ImportDecl>), Type)> {
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
        
        // 1. Extract Concrete Args from Receiver
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
        // [SOVEREIGN FIX] Handle Type::Pointer for Ptr<T> method specialization
        // Extract element type as concrete_ty and use std__core__ptr__Ptr as template
        } else if let Type::Pointer { element, .. } = &peeled_ty {
             // [CANONICAL RESOLUTION] Canonicalize element type before it enters specialization.
             // Without this, Struct("Node") flows in as T, creating Ptr__addr_Node
             // instead of the canonical Ptr__addr_main__Node.
             let canonical_element = resolve_codegen_type(ctx, &(**element));
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
                  // [VERIFIED METAL] Phase 5: Use centralized template lookup
                  if let Some(template_name) = ctx.find_struct_template_by_name(&t_name) {
                      if let Some(template) = ctx.struct_templates().get(&template_name) {
                          gen_params = template.generics.as_ref().map(|g| g.params.clone());
                      }
                  }
                  // Check Enums
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

        // [STATIC METHOD GUARD] Static methods (e.g., Arena::new) have no self parameter.
        // Detect this before indexing func.args[0].
        // Parser (grammar.rs:1389) creates self args with name = Ident::new("self", ...) 
        // and ty = Some(Self/&Self). Static methods have regular named first args.
        let is_static_method = func.args.is_empty() || func.args[0].name != "self";

        let (self_arg_ty_raw, is_bare_self, signature_arg_tys_raw) = if !is_static_method {
            let self_arg = &func.args[0];
            // [ABI FIX] Detect if self is by-value or by-reference by checking the RESOLVED type
            let ty_raw = if let Some(t) = &self_arg.ty { 
                resolve_type(ctx, t)
            } else { 
                method_lookup_ty.clone()
            };
            let bare = !matches!(&ty_raw, Type::Reference(..));
            let sig_tys = func.args.iter().skip(1).map(|a| resolve_type(ctx, a.ty.as_ref().unwrap())).collect::<Vec<_>>();
            (Some(ty_raw), bare, sig_tys)
        } else {
            // Static method: all args are regular parameters
            let sig_tys = func.args.iter().map(|a| resolve_type(ctx, a.ty.as_ref().unwrap())).collect::<Vec<_>>();
            (None, false, sig_tys)
        };
        let signature_ret_raw_unsubst = if let Some(rt) = &func.ret_type { resolve_type(ctx, rt) } else { Type::Unit };
        
        // SOVEREIGN FIX V9.10: Build method-level generic mapping
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
                        let syn_ty = crate::grammar::SynType::from_std(ty_arg.clone()).unwrap();
                        let ty = crate::types::Type::from_syn(&syn_ty).unwrap();
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
                Err(e) => {

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
        // [SOVEREIGN FIX] Use method_lookup_ty (substituted) instead of receiver_ty (unsubstituted)
        // This prevents Generic Wall Breach: T must be resolved to concrete type before request_specialization
        let mut final_receiver_ty = method_lookup_ty.clone();
        
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
        

        
        // [SOVEREIGN FIX] Check if receiver is ALREADY a pointer (from alloca/local variable/GEP)
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
        
        let mut actual_target_name = target_name.clone();

            
            // Add method-level generic arguments if present (from sync ExprMethodCall.turbofish?)
            if let Some(tf) = &m.turbofish {
                for arg in &tf.args {
                    if let syn::GenericArgument::Type(ty_arg) = arg {
                         concrete_tys.push(resolve_codegen_type(ctx, &crate::types::Type::from_syn(&crate::grammar::SynType::from_std(ty_arg.clone()).unwrap()).unwrap()));
                    }
                }
            }
            
            // If concrete_tys is empty (no explicit args), try to infer from context map if template matches
            if concrete_tys.is_empty() {
                 if let Some(t_name) = &template_name_opt {
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

            // [PHASE 4.1 BIDIR BRIDGE] If method has its own generics that were inferred
            // (not from turbofish, but from bidirectional inference), inject them into concrete_tys
            // so the specialization path generates the correct mangled name.
            // Example: mmap<T> with inferred T=f32 => concrete_tys = [f32]
            // [SOVEREIGN V4.1] Method-Level Generic Injection (Robust)
            if let Some(fn_generics) = &func.generics {
                let turbofish_count = if let Some(tf) = &m.turbofish { tf.args.len() } else { 0 };
                
                // CRITICAL: func.generics.params may contain EITHER:
                //   (a) Method-only params [F2, T] (from resolve_method using raw func)
                //   (b) Merged impl+method params [I, F, F2, T] (from trait_registry merged func)
                // We must ONLY append method-level generics (not struct-level ones already in concrete_tys).
                // Build a set of struct-level generic names to filter against.
                let struct_generic_names: std::collections::HashSet<String> = {
                    let mut names = std::collections::HashSet::new();
                    if let Some(t_name) = &template_name_opt {
                        let gen_params = {
                            let templates = ctx.struct_templates();
                            if let Some(s) = templates.get(t_name) {
                                s.generics.as_ref().map(|g| g.params.clone())
                            } else {
                                drop(templates);
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



                // Append only METHOD-level generics: those not in struct_generic_names and not from turbofish
                let mut turbofish_remaining = turbofish_count;
                for param in fn_generics.params.iter() {
                     let name = match param {
                         crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                         crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                     };
                     
                     // Skip struct-level generics (already in concrete_tys from struct specialization)
                     if struct_generic_names.contains(&name) {
                         continue;
                     }
                     
                     // Skip turbofish-provided method generics
                     if turbofish_remaining > 0 {
                         turbofish_remaining -= 1;
                         continue;
                     }
                     
                     if let Some(resolved) = method_generic_map.get(&name) {

                          concrete_tys.push(resolved.clone());
                     }
                }
            }

            let is_specialized = !concrete_tys.is_empty();
            if is_specialized {
                // Fix: Use the Specialized Struct Name as the base (e.g. Ptr_u8) 
                // instead of the Template Name (Ptr).
                // This aligns with how emit_fn names the definition (Ptr_u8__method).
                // We must also remove the Struct Generics from concrete_tys to avoid doubling them in the suffix.
                
                let _suffix_args = concrete_tys.clone();
                let _base_prefix = target_name.clone();
                
                let mut handled = false;

                if let Some(t_name) = &template_name_opt {
                     // Check if peeled_ty is effectively specialized
                     let specialized_mangled_raw = ctx.get_mangled(&peeled_ty).to_string();
                     // Strip !struct_ prefix — get_mangled returns MLIR type form but we need it for function name
                     let specialized_mangled = specialized_mangled_raw.strip_prefix("!struct_")
                         .unwrap_or(&specialized_mangled_raw).to_string();
                     
                     // We want to force the name to "Ptr_u8__method", but keep concrete_tys for logic.
                     if specialized_mangled != *t_name {
                          // [SOVEREIGN FIX] Use fully-qualified specialized name for Type::Pointer
                          // get_mangled returns short name "Ptr_u8", but we need "std__core__ptr__Ptr_u8"
                          // CRITICAL: Method name format must be template__method_suffix (e.g., Ptr__addr_u8)
                          // NOT template_suffix__method (e.g., Ptr_u8__addr)
                          let (_base_prefix, override_name) = if let Type::Pointer { element, .. } = &peeled_ty {
                              let suffix = element.mangle_suffix();
                              // template__method_suffix format matches from_addr pattern
                              ("std__core__ptr__Ptr".to_string(), format!("std__core__ptr__Ptr__{}_{}",method_name, suffix))
                          } else {
                              (specialized_mangled.clone(), format!("{}__{}",specialized_mangled, method_name))
                          };
                          
                          // Use explicit request

                          actual_target_name = ctx.request_explicit_specialization(
                              &format!("{}__{}", t_name, method_name), // Lookup Key (Template__Method)
                              &override_name, // Override ID
                              concrete_tys.clone(), // Context Args (cloned to be safe if reused, strict move below)
                              Some(method_lookup_ty.clone()) // [ABI FIX] Use inner type, not Reference-wrapped
                          );
                          handled = true;
                     }
                }
                
                if !handled {
                    let base_prefix = template_name_opt.as_ref().unwrap_or(&target_name);
                    actual_target_name = ctx.request_specialization(&format!("{}__{}", base_prefix, method_name), concrete_tys, Some(method_lookup_ty.clone())); // [ABI FIX]
                }
            }

        
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
        
        if ctx.external_decls().contains(&mangled_method) {
            if res.is_empty() {
                out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled_method, args_str, mlir_arg_tys));
            } else {
                out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", res, mangled_method, args_str, mlir_arg_tys, ret_ty.to_mlir_type(ctx)?));
            }
        } else {
            if res.is_empty() {
                out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled_method, args_str, mlir_arg_tys));
            } else {
                out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", res, mangled_method, args_str, mlir_arg_tys, ret_ty.to_mlir_type(ctx)?));
            }
        }
        
        Ok((res, ret_ty))
    } else {
        Err(format!("Method {} not found on type {}", method_name, target_name))
    }
}

/// Emit a Tensor constructor: Tensor<T>(value, [d1, d2, ...])
/// Allocates a contiguous memref and fills with the initial value.
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
            resolve_type(ctx, &crate::grammar::SynType::from_std(ty.clone()).unwrap())
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
