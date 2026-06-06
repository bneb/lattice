use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use super::utils::*;
use crate::codegen::type_bridge::*;
use crate::common::mangling::Mangler;
use super::resolver;
use std::collections::HashMap;
use super::{emit_expr, extract_ident_name};
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

                    let mut res_val = String::new();
                    if **ret_ty == Type::Unit {
                        out.push_str(&format!("    llvm.call {}({}) : !llvm.ptr, ({}) -> ()\n",
                            fn_ptr_val, args_str, args_tys_str));
                    } else {
                        res_val = format!("%indirect_call_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.call {}({}) : !llvm.ptr, ({}) -> {}\n",
                            res_val, fn_ptr_val, args_str, args_tys_str, ret_mlir_ty));
                    }

                    ctx.emission.global_lvn.clear();
                    return Ok((res_val, *ret_ty.clone()));
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
                                         let is_nm = f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" );
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

                 // [SOVEREIGN PHASE 3] Strict Affine Memory Safety
                 // Mark variable as consumed if it is passed by value (target is affine, not a reference)
                 if ty.is_affine() {
                     if let Some(var_name) = crate::codegen::expr::extract_ident_name(arg_expr) {
                         ctx.consumed_vars_mut().insert(var_name);
                     }
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
                 // Convert i64 addresses to !llvm.ptr for the intrinsic if necessary
                 let is_ptr = |t: &Type| {
                     match t {
                         Type::Struct(name) => name.contains("Ptr"),
                         _ => false,
                     }
                 };
                 let dest_ptr = if is_ptr(&final_arg_tys[0]) {
                     args_vals[0].clone()
                 } else {
                     let p = format!("%memcpy_dest_{}", ctx.next_id());
                     out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", p, args_vals[0]));
                     p
                 };
                 
                 let src_ptr = if is_ptr(&final_arg_tys[1]) {
                     args_vals[1].clone()
                 } else {
                     let p = format!("%memcpy_src_{}", ctx.next_id());
                     out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", p, args_vals[1]));
                     p
                 };
                 
                 let size_val = if is_ptr(&final_arg_tys[2]) {
                     let s = format!("%memcpy_size_{}", ctx.next_id());
                     out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", s, args_vals[2]));
                     s
                 } else {
                     args_vals[2].clone()
                 };

                 // Emit the LLVM memcpy intrinsic (isVolatile = false for optimization)
                 out.push_str(&format!("    \"llvm.intr.memcpy\"({}, {}, {}) <{{isVolatile = false}}> : (!llvm.ptr, !llvm.ptr, i64) -> ()\n", 
                     dest_ptr, src_ptr, size_val));
                 
                 // Return the destination address as the result (matching C memcpy semantics)
                 let ret_val = if ret_ty != Type::Unit {
                     cast_numeric(ctx, out, &args_vals[0], &final_arg_tys[0], &ret_ty)?
                 } else {
                     "".to_string()
                 };
                 
                 return Ok((ret_val, ret_ty.clone()));
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
                         
                         // [SALT MEMORY MODEL] Mark pointer state as Freed
                         ctx.pointer_tracker.mark_freed(&var_name);
                     }
                 }
                 // Emit the actual free() call
                 out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", call_name, args_str, args_tys_str));
             } else if res_val.is_empty() {
                 out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", call_name, args_str, args_tys_str));
             } else {
                 out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", res_val, call_name, args_str, args_tys_str, ret_ty.to_mlir_type(ctx)?));
             }
             
             // [COMPILER BUG FIX]: Function calls may mutate global variables.
             // We MUST invalidate the Global Local Value Numbering cache!
             ctx.emission.global_lvn.clear();

             // [SOVEREIGN V5.0] Z3 Ownership Tracking: Allocator Interception
             // When malloc() is called, store a pending allocation marker so that
             // the let-binding in stmt.rs can register it with the Z3 tracker.
             if call_name == "malloc" && !res_val.is_empty() {
                 // Store the pending malloc result. The let-binding handler in stmt.rs
                 // will pick this up and register the source variable name with the
                 // Z3 ownership tracker via malloc_tracker.
                 *ctx.pending_malloc_result = Some(res_val.clone());
             }

             // [ESCAPE ANALYSIS V5.2] Mark malloc'd pointers as escaped when passed
             // as function arguments. A pointer passed to another function has its
             // ownership shared/transferred — it's not a leak.
             // This fixes the Basalt WASM pattern:
             //   let tokens = malloc(n * 8);
             //   ingest_prompt(es, tokens, n);  // tokens escapes via argument
             for (i, arg_expr) in c.args.iter().enumerate() {
                 super::mark_expression_escaped(ctx, arg_expr);
                 
                 // [SALT MEMORY MODEL] Conservative Aliasing
                 // Any pointer passed to a function might be freed or mutated.
                 // Mark its state as Optional (unknown) to require a re-check.
                 if call_name != "free" {
                     if let Some(Type::Pointer { .. }) = final_arg_tys.get(i) {
                         if let Some(var_name) = super::extract_ident_name(arg_expr) {
                             ctx.pointer_tracker.mark_optional(&var_name);
                         }
                     }
                 }
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
             } else if call_name == "malloc" || call_name.ends_with("__malloc") {
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
                                let syn_ty = crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?;
                                g_args.push(resolve_type(ctx, &syn_ty));
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
    
    // [ESCAPE ANALYSIS V5.2] Mark malloc'd pointers as escaped when passed
    // as function arguments to method calls.
    for arg_expr in m.args.iter() {
        super::mark_expression_escaped(ctx, arg_expr);
    }

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
                            let is_aggregate = is_aggregate_type(&ty);
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
        
    // [SOVEREIGN FIX] Substitute generics in cached receiver type at the source
    let mut cached_receiver_ty = cached_receiver_ty.substitute(&ctx.current_type_map());
    // [CANONICAL RESOLUTION] Canonicalize receiver type to prevent raw Struct("Node")
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
