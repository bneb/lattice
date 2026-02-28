pub mod utils;
 
use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use self::utils::*;
use crate::codegen::type_bridge::*;
use std::collections::{BTreeMap, HashMap};
use syn::spanned::Spanned;
use crate::common::mangling::Mangler;
pub mod aggregate_eq;
pub mod while_loop;
pub mod resolver;
pub mod tensor_ops;
use aggregate_eq::emit_aggregate_eq;
use while_loop::emit_while;
// use crate::codegen::verification::SymbolicContext;

pub(crate) mod binary_ops;
pub(crate) mod literals;
pub(crate) mod calls;
pub(crate) mod control_flow;
pub(crate) mod memory;

// Re-exports from submodules
use binary_ops::{emit_binary, emit_logic, emit_assign, emit_compound_assign, emit_unary, emit_cast};
use literals::{emit_lit, emit_path, emit_array, emit_tuple, emit_repeat, emit_struct, emit_enum_constructor};
pub(crate) use calls::{emit_call, emit_method_call, emit_tensor_constructor, resolve_call_path};
use control_flow::{emit_if_expr, emit_block_expr, emit_match, emit_if_as_select};
use memory::{emit_field, emit_index};
pub(crate) use memory::{translate_to_z3, translate_bool_to_z3};

/// [SOVEREIGN WRITER PROTOCOL] Parse __target_fstring__!(target, "content") macro arguments
/// Returns (target_expression, fstring_content)
fn parse_target_fstring_args(tokens_str: &str) -> Result<(String, String), String> {
    // Format: target , "content"
    // Find the comma separating target from content
    let comma_pos = tokens_str.find(',')
        .ok_or_else(|| format!("Invalid __target_fstring__ syntax: missing comma in '{}'", tokens_str))?;
    
    let target = tokens_str[..comma_pos].trim().to_string();
    let content_part = tokens_str[comma_pos + 1..].trim();
    
    // Content is wrapped in quotes - remove them
    let content = if content_part.starts_with('"') && content_part.ends_with('"') {
        content_part[1..content_part.len()-1].to_string()
    } else {
        return Err(format!("Invalid __target_fstring__ content: expected quoted string, got '{}'", content_part));
    };
    
    Ok((target, content))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LValueKind {
    Ptr, // Generic Pointer
    SSA, // The address itself (SSA register)
    Local, // Stack Variable
    Global(String), // Global Variable with mangled name for LVN
    Bit(String), // Bit Offset SSA for Packed Arrays
    Tensor { memref: String, indices: Vec<String>, elem_ty: Box<crate::types::Type>, shape: Vec<usize> }, // Tensor indexed access
}

/// [ESCAPE ANALYSIS V5.1] Recursively walk an expression to mark all
/// malloc-tracked allocations as escaped (safely returned to caller).
///
/// This closes the "Chain of Custody" gap: when a return expression contains
/// casts, pointer arithmetic, or nested expressions over malloc'd variables,
/// we must recursively discover and mark all source allocations.
pub fn mark_expression_escaped(ctx: &mut LoweringContext, expr: &syn::Expr) {
    match expr {
        // Direct variable: `return p;`
        syn::Expr::Path(p) => {
            if p.path.segments.len() == 1 {
                let var_name = p.path.segments[0].ident.to_string();
                let alloc_id = format!("malloc:{}", var_name);
                ctx.malloc_tracker.mark_escaped(&alloc_id);
                ctx.malloc_tracker.mark_escaped(&var_name);
            }
        }
        // Cast: `return p as Ptr<T>;` — recurse into inner expression
        syn::Expr::Cast(c) => {
            mark_expression_escaped(ctx, &c.expr);
        }
        // Binary op (pointer arithmetic): `return p + offset;`
        // Conservative: mark both sides as escaped
        syn::Expr::Binary(b) => {
            mark_expression_escaped(ctx, &b.left);
            mark_expression_escaped(ctx, &b.right);
        }
        // Method call: `return result.unwrap();` — check receiver
        syn::Expr::MethodCall(m) => {
            mark_expression_escaped(ctx, &m.receiver);
        }
        // Paren: `return (p as Ptr<T>);`
        syn::Expr::Paren(p) => {
            mark_expression_escaped(ctx, &p.expr);
        }
        // Struct construction handled via __pending_struct sentinel
        _ => {}
    }
    // Always drain inline struct sentinel (harmless no-op if empty)
    ctx.malloc_tracker.mark_escaped("__pending_struct");
}

pub fn emit_expr(ctx: &mut LoweringContext, out: &mut String, expr: &syn::Expr, local_vars: &mut HashMap<String, (Type, LocalKind)>, expected_ty: Option<&Type>) -> Result<(String, Type), String> {
    if let Some(segments) = get_path_from_expr(expr) {
        if let Some((pkg, item)) = resolve_package_prefix_ctx(ctx, &segments) {
            let mangled_name = if item.is_empty() { pkg } else { format!("{}__{}", pkg, item) };
            
            // 1. Constant Lookup
             if let Some(val) = ctx.evaluator.constant_table.get(&mangled_name).cloned() {
                  // [PHASE 4a] Only inline scalar constants. Complex/String values
                  // should never appear here (filtered at insertion), but guard anyway.
                   if let Some((ty, val_str, is_float)) = match val {
                      crate::evaluator::ConstValue::Integer(i) => Some((Type::I64, i.to_string(), false)),
                      crate::evaluator::ConstValue::Bool(b) => Some((Type::Bool, if b { "1" } else { "0" }.to_string(), false)),
                      crate::evaluator::ConstValue::Float(f) => Some((Type::F64, f.to_string(), true)),
                      _ => None,  // Complex/String: fall through to global load
                  } {
                      let tmp_val = format!("%const_resolved_{}", ctx.next_id());
                      let mlir_ty = ty.to_mlir_type(ctx)?;
                      if is_float {
                          ctx.emit_const_float(out, &tmp_val, val_str.parse().unwrap_or(0.0), &mlir_ty);
                      } else {
                          ctx.emit_const_int(out, &tmp_val, val_str.parse().unwrap_or(0), &mlir_ty);
                      }
                      return Ok((tmp_val, ty));
                  }
             }

            // 2. Global Lookup - ONLY for actual values, NOT function symbols
            // [GLOBAL-LVN FIX] Skip function types - they should be handled by emit_call, not loaded as values
            if let Some(target_ty) = ctx.resolve_global(&mangled_name) {
                // Function types are symbols, not loadable values - skip to let emit_path handle
                if !matches!(target_ty, crate::types::Type::Fn(..)) {
                    // [PILLAR 2: Global LVN] Check cache FIRST
                    // Within a basic block, we reuse the cached value instead of reloading
                    {
                        let cache = &ctx.emission;
                        if let Some(cached_val) = cache.global_lvn.get_cached(&mangled_name) {
                            return Ok((cached_val.clone(), target_ty.clone()));
                        }
                    }
                    
                    // Not cached - do actual load
                    let ptr = format!("%global_resolved_ptr_{}", ctx.next_id());
                    ctx.emit_addressof(out, &ptr, &mangled_name)?;
                    let _mlir_ty = target_ty.to_mlir_storage_type(ctx)?;
                    let val_loaded = format!("%global_resolved_val_{}", ctx.next_id());
                    ctx.emit_load_logical(out, &val_loaded, &ptr, &target_ty)?;
                    
                    // [PILLAR 2: Global LVN] Cache the loaded value for future reuse
                    ctx.emission.global_lvn.cache_value(mangled_name.clone(), val_loaded.clone());
                    
                    return Ok((val_loaded, target_ty.clone()));
                    

                }
                // Function types fall through to emit_path for proper call handling
            }

            // 3. Namespace Lookahead Guard: Stop recursion into packages
            // DISABLED: Prematurely catches imports like `OutOfMemory` that need suffix resolution in emit_path
            /*
            if item.is_empty() {
                return Err(format!("Package or module '{}' used as value", segments.join(".")));
            }
            */
        }
    }

    // V25.0-V25.2 Domain Isolation: Pointer contamination is now prevented upstream
    
    match expr {
        syn::Expr::Lit(lit) => emit_lit(ctx, out, lit, expected_ty),
        syn::Expr::Path(p) => emit_path(ctx, out, p, local_vars, expected_ty),
        syn::Expr::Assign(a) => emit_assign(ctx, out, a, local_vars),
        syn::Expr::Block(b) => emit_block_expr(ctx, out, &b.block, local_vars, expected_ty),
        syn::Expr::Binary(b) => emit_binary(ctx, out, b, local_vars, expected_ty),
        syn::Expr::Field(f) => emit_field(ctx, out, f, local_vars),
        syn::Expr::Struct(s) => emit_struct(ctx, out, s, local_vars),
        syn::Expr::Call(c) => emit_call(ctx, out, c, local_vars, expected_ty),
        syn::Expr::If(i) => emit_if_expr(ctx, out, i, local_vars, expected_ty),
        syn::Expr::Unary(u) => emit_unary(ctx, out, u, local_vars, expected_ty),
        syn::Expr::Cast(c) => emit_cast(ctx, out, c, local_vars, expected_ty),
        syn::Expr::Index(i) => emit_index(ctx, out, i, local_vars, expected_ty),
        syn::Expr::MethodCall(m) => emit_method_call(ctx, out, m, local_vars, expected_ty),
        syn::Expr::Paren(p) => emit_expr(ctx, out, &p.expr, local_vars, expected_ty),
        syn::Expr::Reference(r) => {
             let (ptr, ty, kind) = emit_lvalue(ctx, out, &r.expr, local_vars)?;
             
             if let LValueKind::Bit(_) = kind {
                 return Err("Cannot take reference to packed array element (bit)".to_string());
             }

             // [FIELD BORROW] &ptr.field returns Ptr<FieldType> — zero-copy field address.
             // Only applies when the base expression is actually a Ptr<T> type.
             // Stack-spilled struct locals also use GEP (LValueKind::Ptr) but should
             // produce &FieldType (Reference), not Ptr<FieldType>.
             if let syn::Expr::Field(f) = &*r.expr {
                 // Check the actual type of the base expression (the part before .field)
                 let base_ty = emit_expr(ctx, &mut String::new(), &f.base, local_vars, None)
                     .map(|(_, t)| t);
                 if let Ok(Type::Pointer { .. }) = base_ty {
                     return Ok((ptr, Type::Pointer {
                         element: Box::new(ty),
                         provenance: crate::types::Provenance::Naked,
                         is_mutable: r.mutability.is_some(),
                     }));
                 }
             }

             Ok((ptr, Type::Reference(Box::new(ty), r.mutability.is_some())))
         }
        syn::Expr::Return(r) => {
            if let Some(e) = &r.expr {
                let expected_ret = ctx.current_ret_ty().clone();
                let (val_raw, ty) = emit_expr(ctx, out, e, local_vars, expected_ret.as_ref())?;

                // [ESCAPE ANALYSIS V5.1] Recursive escape marking.
                // Walks the expression tree to find all malloc-tracked sources,
                // handling casts, pointer arithmetic, method calls, etc.
                mark_expression_escaped(ctx, e);

                // [ARENA ESCAPE ANALYSIS] Law I: The Return Rule.
                // return x is valid iff depth(x) <= 1.
                // A pointer from a local arena (depth >= 2) cannot escape.
                if let Some(var_name) = extract_return_var_name_from_expr(e) {
                    if let Err(msg) = ctx.arena_escape_tracker.check_return_escape(&var_name) {
                        return Err(msg);
                    }
                }

                let mut val = val_raw;
                if ty == Type::Unit {
                    // [ESCAPE ANALYSIS V5.1] Still emit cleanup before returning
                    ctx.transfer_ownership(&val)?;
                    crate::codegen::stmt::emit_cleanup_for_return(ctx, out, local_vars)?;
                    out.push_str("    func.return\n");
                    return Ok(("%unreachable".to_string(), Type::Never));
                }
                if let Some(expected) = &expected_ret {
                    val = crate::codegen::type_bridge::promote_numeric(ctx, out, &val, &ty, expected)?;
                }
                let mlir_ty = if let Some(exp_ty) = &expected_ret {
                    let e_ty: Type = (*exp_ty).clone();
                    e_ty.to_mlir_type(ctx)?
                } else {
                    ty.to_mlir_type(ctx)?
                };
                
                // [V1.1] RAII-Lite: Transfer ownership of returned value and emit cleanup
                ctx.transfer_ownership(&val)?;
                crate::codegen::stmt::emit_cleanup_for_return(ctx, out, local_vars)?;
                
                // [v0.9.2 POSTCONDITION PIVOT] Z3 verification of ensures clauses at return site
                let ensures = ctx.current_ensures().clone();
                if !ensures.is_empty() {
                    let fn_name = ctx.current_fn_name().clone();
                    let file_items = &ctx.config.file.items;
                    let (requires, param_names) = file_items.iter()
                        .filter_map(|item| {
                            if let crate::grammar::Item::Fn(f) = item {
                                if f.name.to_string() == fn_name || fn_name.ends_with(&f.name.to_string()) {
                                    let params: Vec<String> = f.args.iter().map(|a| a.name.to_string()).collect();
                                    return Some((f.requires.clone(), params));
                                }
                            }
                            None
                        })
                        .next()
                        .unwrap_or((vec![], vec![]));

                    match crate::codegen::verification::VerificationEngine::verify_postcondition(
                        ctx, &ensures, &requires, e, &param_names, local_vars, &fn_name,
                    ) {
                        Ok(true) => {
                            out.push_str(&format!("    // z3_postcondition_verified: ensures proven for '{}'\n", fn_name));
                        }
                        Ok(false) => {}
                        Err(err) => {
                            return Err(err);
                        }
                    }
                }

                let loc = ctx.loc_tag(r.span());
                out.push_str(&format!("    func.return {} : {}{}\n", val, mlir_ty, loc));
            } else {
                // [V1.1] RAII-Lite: Emit cleanup before void return
                crate::codegen::stmt::emit_cleanup_for_return(ctx, out, local_vars)?;
                
                let loc = ctx.loc_tag(r.span());
                out.push_str(&format!("    func.return{}\n", loc));
            }
            Ok(("%unreachable".to_string(), Type::Never))
        }
        syn::Expr::Repeat(r) => emit_repeat(ctx, out, r, local_vars, expected_ty),
        syn::Expr::Array(a) => emit_array(ctx, out, a, local_vars, expected_ty),
        syn::Expr::Tuple(t) => emit_tuple(ctx, out, t, local_vars),
        syn::Expr::Match(m) => emit_match(ctx, out, m, local_vars, expected_ty),
        syn::Expr::Try(t) => {
            let (val, ty) = emit_expr(ctx, out, &t.expr, local_vars, None)?;
            // [UNIFIED] Structural Result<T> detection via enum registry
            if let Some(enum_info) = ctx.is_result_enum(&ty) {
                let result_mlir = ty.to_mlir_type(ctx)?;
                // Extract the tag (discriminant) — always at index 0
                let tag = format!("%try_tag_{}", ctx.next_id());
                ctx.emit_extractvalue(out, &tag, &val, 0, &result_mlir);
                
                // Compare tag against Err discriminant (Err = 1 in Ok=0/Err=1 layout)
                let is_err = format!("%try_is_err_{}", ctx.next_id());
                let err_disc = format!("%try_err_disc_{}", ctx.next_id());
                ctx.emit_const_int(out, &err_disc, 1, "i32");
                out.push_str(&format!("    {} = arith.cmpi \"eq\", {}, {} : i32\n", is_err, tag, err_disc));
                
                let label_err = format!("try_err_{}", ctx.next_id());
                let label_ok = format!("try_ok_{}", ctx.next_id());
                ctx.emit_cond_br(out, &is_err, &label_err, &label_ok);
                
                // Err path: early-return with the function's Result type
                ctx.emit_label(out, &label_err);
                
                // Get the function's return type to check if re-wrapping is needed
                let fn_ret_ty = ctx.current_ret_ty().clone();
                let fn_result_mlir = if let Some(ref ret_ty) = fn_ret_ty {
                    ret_ty.to_mlir_type(ctx)?
                } else {
                    result_mlir.clone()
                };
                
                if fn_result_mlir == result_mlir {
                    // Same Result type — return directly (fast path)
                    let loc = ctx.loc_tag(t.span());
                    out.push_str(&format!("    func.return {} : {}{}\n", val, result_mlir, loc));
                } else {
                    // Different Result types (e.g., Result<File> vs Result<i64>)
                    // Extract Status from callee's Err, re-wrap into function's Result type
                    let err_raw = format!("%try_err_raw_{}", ctx.next_id());
                    let ok_ty_callee = match &ty {
                        Type::Concrete(_, args) if !args.is_empty() => args[0].clone(),
                        _ => Type::I64,
                    };
                    let callee_payload_size = std::cmp::max(ctx.size_of(&ok_ty_callee), 8);
                    let callee_raw_ty = format!("!llvm.array<{} x i8>", callee_payload_size);
                    out.push_str(&format!("    {} = llvm.extractvalue {}[1] : {}\n", err_raw, val, result_mlir));
                    // Type-pun raw array → Status
                    let err_buf = format!("%try_err_buf_{}", ctx.next_id());
                    let err_one = format!("%try_err_one_{}", ctx.next_id());
                    ctx.emit_const_int(out, &err_one, 1, "i64");
                    out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", err_buf, err_one, callee_raw_ty));
                    out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", err_raw, err_buf, callee_raw_ty));
                    let status_val = format!("%try_status_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !struct_std__status__Status\n", status_val, err_buf));
                    
                    // Construct new Result<FnOkType>::Err(status)
                    let fn_ok_ty = if let Some(ref ret_ty) = fn_ret_ty {
                        match ret_ty {
                            Type::Concrete(_, args) if !args.is_empty() => args[0].clone(),
                            _ => Type::I64,
                        }
                    } else {
                        Type::I64
                    };
                    let fn_payload_size = std::cmp::max(ctx.size_of(&fn_ok_ty), 8);
                    let fn_raw_ty = format!("!llvm.array<{} x i8>", fn_payload_size);
                    
                    let new_result = format!("%try_rewrap_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", new_result, fn_result_mlir));
                    let err_disc = format!("%try_ewrap_disc_{}", ctx.next_id());
                    ctx.emit_const_int(out, &err_disc, 1, "i32");
                    let with_disc = format!("%try_ewrap_d_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.insertvalue {}, {}[0] : {}\n", with_disc, err_disc, new_result, fn_result_mlir));
                    // Type-pun Status → fn's raw array
                    let wrap_buf = format!("%try_ewrap_buf_{}", ctx.next_id());
                    let wrap_one = format!("%try_ewrap_one_{}", ctx.next_id());
                    ctx.emit_const_int(out, &wrap_one, 1, "i64");
                    out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", wrap_buf, wrap_one, fn_raw_ty));
                    out.push_str(&format!("    llvm.store {}, {} : !struct_std__status__Status, !llvm.ptr\n", status_val, wrap_buf));
                    let wrap_loaded = format!("%try_ewrap_arr_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", wrap_loaded, wrap_buf, fn_raw_ty));
                    let final_result = format!("%try_ewrap_final_{}", ctx.next_id());
                    out.push_str(&format!("    {} = llvm.insertvalue {}, {}[1] : {}\n", final_result, wrap_loaded, with_disc, fn_result_mlir));
                    let loc = ctx.loc_tag(t.span());
                    out.push_str(&format!("    func.return {} : {}{}\n", final_result, fn_result_mlir, loc));
                }
                
                // Ok path: extract the Ok payload (index 1) with type-punning
                ctx.emit_label(out, &label_ok);
                let ok_raw = format!("%try_ok_raw_{}", ctx.next_id());
                // Determine the Ok payload type from the Result's generic args
                let ok_ty = match &ty {
                    Type::Concrete(_, args) if !args.is_empty() => args[0].clone(),
                    _ => Type::I64, // fallback
                };
                let ok_mlir = ok_ty.to_mlir_type(ctx)?;
                // Compute raw union payload size: max(ok_size, status_size=8)
                let ok_size = ctx.size_of(&ok_ty);
                let payload_size = std::cmp::max(ok_size, 8); // Status is (i32, i32) = 8 bytes
                let raw_array_ty = format!("!llvm.array<{} x i8>", payload_size);
                out.push_str(&format!("    {} = llvm.extractvalue {}[1] : {}\n", ok_raw, val, result_mlir));
                // Type-pun through memory: store as raw array, load as typed struct
                let ok_buf = format!("%try_ok_buf_{}", ctx.next_id());
                let one = format!("%try_one_{}", ctx.next_id());
                ctx.emit_const_int(out, &one, 1, "i64");
                out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", ok_buf, one, raw_array_ty));
                out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", ok_raw, ok_buf, raw_array_ty));
                let ok_payload = format!("%try_ok_val_{}", ctx.next_id());
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", ok_payload, ok_buf, ok_mlir));
                
                Ok((ok_payload, ok_ty))
            } else {
                Err(format!("? operator requires Result<T> type, got {:?}", ty))
            }
        }
        syn::Expr::While(w) => emit_while(ctx, out, w, local_vars),
        
        // [V4.0 SCORCHED EARTH] Handle prefixed string macros
        // __fstring__!("...") and __hex__!("...") are generated by preprocessing
        syn::Expr::Macro(m) => {
            let macro_name = m.mac.path.segments.last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            
            // Extract the string content from the macro tokens
            let tokens_str = m.mac.tokens.to_string();
            let content = tokens_str.trim_matches('"').to_string();
            
            match macro_name.as_str() {
                "__fstring__" => {
                    // [V4.0 INJECTED DEPENDENCY STRATEGY]
                    // Force discovery of InterpolatedStringHandler BEFORE expansion.
                    // This ensures the handler methods are in the TraitRegistry.
                    let _ = ctx.ensure_struct_exists("std__string__InterpolatedStringHandler", &[]);
                    // [V5.0] Also discover Formatter for structural formatting support
                    let _ = ctx.ensure_struct_exists("std__core__fmt__Formatter", &[]);
                    
                    // Expand f-string to Salt code using Rust path notation (::)
                    let expanded = ctx.native_fstring_expand(&content);
                    // Parse as Salt expression - path is std::string::InterpolatedStringHandler::new
                    let parsed: syn::Expr = syn::parse_str(&expanded)
                        .map_err(|e| format!("F-string parse error: {} (code: {})", e, 
                            expanded.chars().take(100).collect::<String>()))?;
                    
                    // Emit the parsed expression - this triggers hydration!
                    emit_expr(ctx, out, &parsed, local_vars, expected_ty)
                }
                "__hex__" => {
                    // Expand hex string using native handler  
                    let expanded = ctx.native_hex_expand(&content);
                    // Parse and emit the expanded code
                    let expanded_expr: syn::Expr = syn::parse_str(&expanded)
                        .map_err(|e| format!("Failed to parse expanded hex: {} (code: {})", e, expanded))?;
                    emit_expr(ctx, out, &expanded_expr, local_vars, expected_ty)
                }
                "__target_fstring__" => {
                    // [SOVEREIGN WRITER PROTOCOL] Handle target.f"..." syntax
                    // The macro receives: target, "content"
                    // We decompose into direct write_* calls on the target
                    
                    // Parse the macro arguments: target, "content"
                    let (target_expr, fstring_content) = parse_target_fstring_args(&tokens_str)?;
                    
                    // Generate the expanded code using Sovereign Decomposition
                    let expanded = ctx.native_target_fstring_expand(&target_expr, &fstring_content);
                    
                    // Parse and emit
                    let parsed: syn::Expr = syn::parse_str(&expanded)
                        .map_err(|e| format!("Target f-string parse error: {} (code: {})", e,
                            expanded.chars().take(200).collect::<String>()))?;
                    
                    emit_expr(ctx, out, &parsed, local_vars, expected_ty)
                }
                "__railway__" => {
                    // [PHASE 2] Railway operator: __railway__!(expr, fn[, extra_args])
                    // Expands to: match expr { Result::Ok(v) => fn(v[, extra_args]), Result::Err(e) => Result::Err(e) }
                    let parts: Vec<&str> = tokens_str.splitn(2, ',').collect();
                    if parts.len() < 2 {
                        return Err(format!("__railway__! requires at least 2 args: expr, fn. Got: {}", tokens_str));
                    }
                    
                    let expr_str = parts[0].trim();
                    let rest = parts[1].trim();
                    
                    // rest might be "fn_name" or "fn_name, arg1, arg2"
                    let fn_and_args: Vec<&str> = rest.splitn(2, ',').collect();
                    let fn_name = fn_and_args[0].trim();
                    let extra_args = if fn_and_args.len() > 1 {
                        format!(", {}", fn_and_args[1].trim())
                    } else {
                        String::new()
                    };
                    
                    // Build the match expression
                    let expanded = format!(
                        "match {} {{ Result::Ok(__railway_v) => {}(__railway_v{}), Result::Err(__railway_e) => Result::Err(__railway_e) }}",
                        expr_str, fn_name, extra_args
                    );
                    
                    let parsed: syn::Expr = syn::parse_str(&expanded)
                        .map_err(|e| format!("Railway expansion parse error: {} (code: {})", e,
                            expanded.chars().take(200).collect::<String>()))?;
                    
                    emit_expr(ctx, out, &parsed, local_vars, expected_ty)
                }
                "__force_unwrap__" => {
                    // [SOVEREIGN] Force-unwrap operator (~)
                    // Performance: happy path is one i32 cmp + branch + extractvalue — zero call overhead
                    // Error path: exit(status.code) for Result, exit(1) for Option
                    let inner_str = tokens_str.trim();
                    let inner_expr: syn::Expr = syn::parse_str(inner_str)
                        .map_err(|e| format!("force_unwrap parse error: {} (input: {})", e, inner_str))?;
                    let (val, ty) = emit_expr(ctx, out, &inner_expr, local_vars, None)?;
                    
                    if let Some(_enum_info) = ctx.is_result_enum(&ty) {
                        // Result<T>: tag check + extract Ok or exit(status.code)
                        let result_mlir = ty.to_mlir_type(ctx)?;
                        let tag = format!("%fu_tag_{}", ctx.next_id());
                        ctx.emit_extractvalue(out, &tag, &val, 0, &result_mlir);
                        
                        let is_err = format!("%fu_is_err_{}", ctx.next_id());
                        let err_disc = format!("%fu_err_disc_{}", ctx.next_id());
                        ctx.emit_const_int(out, &err_disc, 1, "i32");
                        out.push_str(&format!("    {} = arith.cmpi \"eq\", {}, {} : i32\n", is_err, tag, err_disc));
                        
                        let label_err = format!("fu_err_{}", ctx.next_id());
                        let label_ok = format!("fu_ok_{}", ctx.next_id());
                        ctx.emit_cond_br(out, &is_err, &label_err, &label_ok);
                        
                        // Err path: extract Status.code from payload and exit with it
                        ctx.emit_label(out, &label_err);
                        // Err payload is array<N x i8>; type-pun to Status via memory
                        let ok_ty_for_size = match &ty {
                            Type::Concrete(_, args) if !args.is_empty() => args[0].clone(),
                            _ => Type::I64,
                        };
                        let payload_size = std::cmp::max(ctx.size_of(&ok_ty_for_size), 8);
                        let raw_array_ty = format!("!llvm.array<{} x i8>", payload_size);
                        let err_raw = format!("%fu_err_raw_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.extractvalue {}[1] : {}\n", err_raw, val, result_mlir));
                        // Type-pun raw array → Status through memory
                        let err_buf = format!("%fu_err_buf_{}", ctx.next_id());
                        let err_one = format!("%fu_err_one_{}", ctx.next_id());
                        ctx.emit_const_int(out, &err_one, 1, "i64");
                        out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", err_buf, err_one, raw_array_ty));
                        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", err_raw, err_buf, raw_array_ty));
                        let err_status = format!("%fu_err_status_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !struct_std__status__Status\n", err_status, err_buf));
                        // Extract code (field 0) from Status
                        let status_code = format!("%fu_status_code_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.extractvalue {}[0] : !struct_std__status__Status\n", status_code, err_status));
                        out.push_str(&format!("    llvm.call @exit({}) : (i32) -> ()\n", status_code));
                        out.push_str("    llvm.unreachable\n");
                        
                        // Ok path: extract payload with type-punning through memory
                        ctx.emit_label(out, &label_ok);
                        let ok_ty = match &ty {
                            Type::Concrete(_, args) if !args.is_empty() => args[0].clone(),
                            _ => Type::I64,
                        };
                        let ok_mlir = ok_ty.to_mlir_type(ctx)?;
                        let ok_raw = format!("%fu_ok_raw_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.extractvalue {}[1] : {}\n", ok_raw, val, result_mlir));
                        let ok_buf = format!("%fu_ok_buf_{}", ctx.next_id());
                        let ok_one = format!("%fu_ok_one_{}", ctx.next_id());
                        ctx.emit_const_int(out, &ok_one, 1, "i64");
                        out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", ok_buf, ok_one, raw_array_ty));
                        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", ok_raw, ok_buf, raw_array_ty));
                        let ok_payload = format!("%fu_ok_val_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", ok_payload, ok_buf, ok_mlir));
                        
                        Ok((ok_payload, ok_ty))
                    } else if ctx.is_option_enum(&ty).is_some() {
                        // Option<T>: tag check + extract Some or exit(1)
                        let option_mlir = ty.to_mlir_type(ctx)?;
                        let tag = format!("%fu_tag_{}", ctx.next_id());
                        ctx.emit_extractvalue(out, &tag, &val, 0, &option_mlir);
                        
                        let is_none = format!("%fu_is_none_{}", ctx.next_id());
                        let none_disc = format!("%fu_none_disc_{}", ctx.next_id());
                        ctx.emit_const_int(out, &none_disc, 1, "i32");
                        out.push_str(&format!("    {} = arith.cmpi \"eq\", {}, {} : i32\n", is_none, tag, none_disc));
                        
                        let label_none = format!("fu_none_{}", ctx.next_id());
                        let label_some = format!("fu_some_{}", ctx.next_id());
                        ctx.emit_cond_br(out, &is_none, &label_none, &label_some);
                        
                        // None path: exit(1)
                        ctx.emit_label(out, &label_none);
                        let exit_code = format!("%fu_exit_code_{}", ctx.next_id());
                        ctx.emit_const_int(out, &exit_code, 1, "i32");
                        out.push_str(&format!("    llvm.call @exit({}) : (i32) -> ()\n", exit_code));
                        out.push_str("    llvm.unreachable\n");
                        
                        // Some path: extract payload with type-punning through memory
                        ctx.emit_label(out, &label_some);
                        let some_ty = match &ty {
                            Type::Concrete(_, args) if !args.is_empty() => args[0].clone(),
                            _ => Type::I64,
                        };
                        let some_mlir = some_ty.to_mlir_type(ctx)?;
                        let some_size = ctx.size_of(&some_ty);
                        let some_raw_ty = format!("!llvm.array<{} x i8>", some_size);
                        let some_raw = format!("%fu_some_raw_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.extractvalue {}[1] : {}\n", some_raw, val, option_mlir));
                        let some_buf = format!("%fu_some_buf_{}", ctx.next_id());
                        let some_one = format!("%fu_some_one_{}", ctx.next_id());
                        ctx.emit_const_int(out, &some_one, 1, "i64");
                        out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", some_buf, some_one, some_raw_ty));
                        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", some_raw, some_buf, some_raw_ty));
                        let some_payload = format!("%fu_some_val_{}", ctx.next_id());
                        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", some_payload, some_buf, some_mlir));
                        
                        Ok((some_payload, some_ty))
                    } else {
                        Err(format!("~ operator requires Result<T> or Option<T> type, got {:?}", ty))
                    }
                }
                "__fstring_append_expr" => {
                    // [V5.0 STRUCTURAL FORMATTING] Type-aware f-string expression dispatch
                    // This macro is emitted by native_fstring_expand for each interpolated expr.
                    // It resolves the expression type WITHOUT emitting code (to avoid double emission),
                    // then generates the right append_* call or a Formatter chain for structs.
                    
                    // Parse: __fstring_append_expr!(__h, expr)
                    let comma_pos = tokens_str.find(',')
                        .ok_or_else(|| format!("__fstring_append_expr! requires handler and expr: {}", tokens_str))?;
                    let handler_name = tokens_str[..comma_pos].trim().to_string();
                    let expr_str = tokens_str[comma_pos + 1..].trim().to_string();
                    
                    // Heuristic type resolution: resolve type from local_vars without emitting code
                    let resolved_ty = resolve_fstring_expr_type(&expr_str, local_vars, ctx);
                    
                    // Generate the correct append call based on resolved type
                    let append_code = match &resolved_ty {
                        Some(Type::I32) => format!("{}.append_i32({})", handler_name, expr_str),
                        Some(Type::I8) | Some(Type::I16) => format!("{}.append_i32({} as i32)", handler_name, expr_str),
                        Some(Type::I64) | Some(Type::Usize) => format!("{}.append_i64({})", handler_name, expr_str),
                        Some(Type::U8) | Some(Type::U16) | Some(Type::U32) | Some(Type::U64) => format!("{}.append_i64({} as i64)", handler_name, expr_str),
                        Some(Type::F32) => format!("{}.append_f64({} as f64)", handler_name, expr_str),
                        Some(Type::F64) => format!("{}.append_f64({})", handler_name, expr_str),
                        Some(Type::Bool) => format!("{}.append_bool({})", handler_name, expr_str),
                        Some(Type::Reference(inner, _)) if matches!(**inner, Type::U8) => {
                            format!("{}.append_str({})", handler_name, expr_str)
                        }
                        Some(Type::Struct(name)) | Some(Type::Concrete(name, _)) => {
                            let name = name.clone();
                            let the_ty = resolved_ty.as_ref().unwrap();
                            // Check if the struct has a fmt() method
                            let type_key = crate::codegen::type_bridge::type_to_type_key(the_ty);
                            if ctx.trait_registry().contains_method(&type_key, "fmt") {
                                // Generate Formatter chain
                                let fmt_id = ctx.next_id();
                                format!(
                                    "{{ let mut __fmt_{id} = std::core::fmt::Formatter::new(); \
                                     ({expr}).fmt(&mut __fmt_{id}); \
                                     {handler}.append_fmt_result(__fmt_{id}.as_ptr(), __fmt_{id}.len()); }}",
                                    id = fmt_id,
                                    expr = expr_str,
                                    handler = handler_name
                                )
                            } else {
                                // No fmt method — fall back to i32 (legacy behavior)
                                eprintln!("WARNING: Struct '{}' has no fmt() method in f-string", name);
                                format!("{}.append_i32({})", handler_name, expr_str)
                            }
                        }
                        _ => {
                            // Could not resolve type — use append_i32 (legacy default)
                            format!("{}.append_i32({})", handler_name, expr_str)
                        }
                    };
                    
                    // Parse and emit the generated append code
                    let append_parsed: syn::Expr = syn::parse_str(&append_code)
                        .map_err(|e| format!("__fstring_append_expr! codegen parse error: {} (code: {})", e, append_code))?;
                    emit_expr(ctx, out, &append_parsed, local_vars, None)
                }
                _ => Err(format!("Unknown macro in expression: {}", macro_name))
            }
        }
        
        _ => Err(format!("Unsupported expression: {:?}", expr)),
    }
}


// Extracted helpers

/// [SOVEREIGN V5.0] Extract the source-level identifier name from a syn::Expr.
/// Used by the malloc/free tracking system to identify which variable is being freed.
/// Returns None if the expression is not a simple identifier (e.g., it's a complex expression).
pub(crate) fn extract_ident_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) => {
            // Simple variable: `buf` -> Some("buf")
            if p.path.segments.len() == 1 {
                Some(p.path.segments[0].ident.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// [ARENA ESCAPE ANALYSIS] Extract the root receiver variable from a field access chain.
/// For `ctx.saved_ptr`, returns Some("ctx"). For `a.b.c`, returns Some("a").
/// Returns None if the LHS is not a field-access expression.
fn extract_field_assign_receiver(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Field(f) => {
            // Walk to the root of the field chain
            let mut base = &*f.base;
            loop {
                match base {
                    syn::Expr::Field(inner) => base = &*inner.base,
                    syn::Expr::Path(p) => {
                        return p.path.get_ident().map(|id| id.to_string());
                    }
                    _ => return None,
                }
            }
        }
        _ => None,
    }
}

/// [ARENA ESCAPE ANALYSIS] Extract the simple variable name from a return expression.
/// For `n`, returns Some("n"). For `n as Ptr<T>`, also returns Some("n").
fn extract_return_var_name_from_expr(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) => p.path.get_ident().map(|id| id.to_string()),
        syn::Expr::Cast(c) => extract_return_var_name_from_expr(&c.expr),
        syn::Expr::Paren(p) => extract_return_var_name_from_expr(&p.expr),
        _ => None,
    }
}

pub fn unify_types(t1: &Type, t2: &Type) -> Result<Type, String> {
    if t1 == t2 {
        Ok(t1.clone())
    } else if matches!(t1, Type::Never) {
        Ok(t2.clone())
    } else if matches!(t2, Type::Never) {
        Ok(t1.clone())
    } else {
        Err(format!("Type mismatch in branches: found {:?} and {:?}", t1, t2))
    }
}

pub fn emit_lvalue(ctx: &mut LoweringContext, out: &mut String, expr: &syn::Expr, local_vars: &mut HashMap<String, (Type, LocalKind)>) -> Result<(String, Type, LValueKind), String> {
    // 0. Namespace Lookahead (Canonical Path Flattening)
    if let Some(segments) = get_path_from_expr(expr) {
        if let Some((pkg, item)) = resolve_package_prefix_ctx(ctx, &segments) {
            let mangled_name = if item.is_empty() { pkg.clone() } else { format!("{}__{}", pkg, item) };
            if let Some(ty) = ctx.resolve_global(&mangled_name) {
                ctx.ensure_global_declared(&mangled_name, &ty)?;
                let addr = format!("%addr_glob_{}", ctx.next_id());
                ctx.emit_addressof(out, &addr, &mangled_name)?;
                return Ok((addr, ty, LValueKind::Global(mangled_name.clone())));
            }
            
            // [FIX] Check if the base (pkg) is a global. If so, this is a field access on a global.
            // e.g. GLOBAL_SCHED.yield_pending -> pkg="...__GLOBAL_SCHED", item="yield_pending"
            // We should fall through to standard field handling.
            if !item.is_empty() && ctx.resolve_global(&pkg).is_some() {
                // Determine it's a field access on a global - fall through to match expr
            } else if item.is_empty() {
                return Err(format!("Package or module '{}' used as L-Value", segments.join(".")));
            } else {
                 // Not a global field access, and not a full global.
                 // If it starts with an import, it's an undefined static.
                 let first = &segments[0];
                 if ctx.imports().iter().any(|imp| {
                    if let Some(alias) = &imp.alias { alias == first }
                    else if let Some(f) = imp.name.first() { 
                        let f_str: String = f.to_string();
                        f_str == *first 
                    }
                    else { false }
                 }) {
                    return Err(format!("Undefined global or static '{}' in package/module path '{}'", segments.last().unwrap(), segments.join(".")));
                 }
            }
        } else {
            // No package prefix resolved. Check for partial import match (Error Case)
            let first = &segments[0];
            if ctx.imports().iter().any(|imp| {
                if let Some(alias) = &imp.alias { alias == first }
                else if let Some(f) = imp.name.first() { 
                    let f_str: String = f.to_string();
                    f_str == *first 
                }
                else { false }
            }) {
                return Err(format!("Undefined global or static '{}' in package/module path '{}'", segments.last().unwrap(), segments.join(".")));
            }
        }
    }

    match expr {
        syn::Expr::Path(p) => {
            let segments: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
            let name = Mangler::mangle(&segments);
            let first = &segments[0];
            
            // 1. Check Local Scope First (Shadowing)
            if let Some((ty, kind)) = local_vars.get(first).cloned() {
                if segments.len() > 1 { return Err(format!("Cannot access field/namespace of local variable {} using path syntax", first)); }
                match kind {
                    LocalKind::Ptr(ptr_name) => return Ok((ptr_name, ty, LValueKind::Local)),
                    LocalKind::SSA(ssa_name) => {
                        if matches!(ty, Type::Tensor(..)) {
                            return Ok((ssa_name, ty, LValueKind::SSA));
                        }
                        // [SSA PROMOTION] Check if this is an ephemeral ref (e.g. from reinterpret_cast)
                        // If so, it's already a pointer - just return it directly without spilling
                        let is_ephemeral_ref = ctx.emission.ephemeral_refs.contains(&ssa_name);
                        if is_ephemeral_ref {
                            // Ephemeral refs ARE pointers, return as LValueKind::Ptr without spilling
                            return Ok((ssa_name, ty, LValueKind::Ptr));
                        }
                        // [REFERENCE FIX] Reference types (e.g., &self) are already pointers to the 
                        // underlying struct. Do NOT spill them - return the pointer value directly.
                        // Spilling a pointer creates a pointer-to-pointer which breaks field access.
                        if matches!(ty, Type::Reference(_, _)) {
                            return Ok((ssa_name, ty, LValueKind::Ptr));
                        }
                        // [NOTE] Ptr<T> spills look wasteful in MLIR but are eliminated by
                        // LLVM's mem2reg during `clang -O3`. Attempting to skip the spill here
                        // breaks emit_assign's type dispatch (LValueKind::SSA + Pointer type
                        // causes it to peel to element type instead of keeping the Pointer).
                        // The correct optimization target is MLIR-level passes, not codegen.
                        let tmp_alloca = format!("%spill_{}_{}", first, ctx.next_id());
                        let mlir_ty = ty.to_mlir_type(ctx)?;
                        ctx.emit_alloca(out, &tmp_alloca, &mlir_ty);
                        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", ssa_name, tmp_alloca, mlir_ty));
                        return Ok((tmp_alloca, ty, LValueKind::Ptr));
                    }
                }
            }

            // 2. Check Package/Global Resolution
            if let Some((pkg, item)) = resolve_package_prefix_ctx(ctx, &segments) {
                let mangled_name = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) };
                if let Some(ty) = ctx.resolve_global(&mangled_name) {
                    let addr = format!("%addr_glob_{}", ctx.next_id());
                    ctx.emit_addressof(out, &addr, &mangled_name)?;
                    return Ok((addr, ty, LValueKind::Global(mangled_name.clone())));
                }
                return Err(format!("Package or module '{}' used as L-Value", segments.join(".")));
            }
                let mut mangled = name.clone();
                if let Some(pkg) = &ctx.config.file.package {
                    let pkg_mangled = Mangler::mangle(&pkg.name.iter().map(|id: &syn::Ident| id.to_string()).collect::<Vec<_>>());
                    let local_mangled = format!("{}__{}", pkg_mangled, mangled);
                    if ctx.globals().contains_key(&local_mangled) {
                        mangled = local_mangled;
                    }
                }

                if let Some(ty) = ctx.resolve_global(&mangled) {
                    ctx.ensure_global_declared(&mangled, &ty)?;
                    let addr = format!("%addr_glob_{}", ctx.next_id());
                    ctx.emit_addressof(out, &addr, &mangled)?;
                    Ok((addr, ty, LValueKind::Global(mangled.clone())))
                } else {
                     Err(format!("Undefined variable or global: {}", name))
                }
        }
        syn::Expr::Index(i) => {
             let (base_ptr, base_ty, base_kind) = emit_lvalue(ctx, out, &i.expr, local_vars)?;
             
             // TENSOR SPECIAL CASE: Handle tuple indices before generic index evaluation
             if let Type::Tensor(ref elem_ty, ref shape) = base_ty {
                 // [FIX] If the tensor variable is stored in a stack slot, load the pointer first
                 let tensor_ptr = match base_kind {
                     LValueKind::Local | LValueKind::Ptr => {
                         // base_ptr is a stack slot holding the tensor pointer - load it
                         let loaded = format!("%tensor_lv_loaded_{}", ctx.next_id());
                         if ctx.config.emit_alias_scopes {
                             out.push_str(&format!("    {} = llvm.load {} {{ alias_scopes = [#scope_local], noalias = [#scope_global] }} : !llvm.ptr -> !llvm.ptr\n", loaded, base_ptr));
                         } else {
                             ctx.emit_load(out, &loaded, &base_ptr, "!llvm.ptr");
                         }
                         loaded
                     }
                     LValueKind::SSA => {
                         // SSA value IS the tensor pointer
                         base_ptr.clone()
                     }
                     LValueKind::Global(_) => {
                         // Global - load the pointer from global address
                         let loaded = format!("%tensor_lv_global_loaded_{}", ctx.next_id());
                         if ctx.config.emit_alias_scopes {
                             out.push_str(&format!("    {} = llvm.load {} {{ alias_scopes = [#scope_global], noalias = [#scope_local] }} : !llvm.ptr -> !llvm.ptr\n", loaded, base_ptr));
                         } else {
                             ctx.emit_load(out, &loaded, &base_ptr, "!llvm.ptr");
                         }
                         loaded
                     }
                     _ => base_ptr.clone() // Tensor/Bit kinds shouldn't happen here
                 };

                 // TENSOR LVALUE: Handle multi-dimensional indexed assignment
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
                             let idx = format!("%idx_lv_{}", ctx.next_id());
                             out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", idx, i64_val));
                             idx
                         };
                         v.push(idx_index);
                     }
                     v
                 } else {
                     // Single index
                     let (idx_val, idx_ty) = emit_expr(ctx, out, index_expr, local_vars, Some(&Type::Usize))?;
                     let idx_index = if idx_ty == Type::Usize {
                         idx_val
                     } else {
                         let i64_val = promote_numeric(ctx, out, &idx_val, &idx_ty, &Type::I64)?;
                         let idx = format!("%idx_lv_{}", ctx.next_id());
                         out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", idx, i64_val));
                         idx
                     };
                     vec![idx_index]
                 };
                 
                 return Ok((tensor_ptr.clone(), *elem_ty.clone(), LValueKind::Tensor { 
                     memref: tensor_ptr, 
                     indices, 
                     elem_ty: Box::new(*elem_ty.clone()), 
                     shape: shape.clone() 
                 }));
             }
             
             // [SOVEREIGN V2.0]: First-Class Pointer Indexing for LValue
             // Handle Ptr<T> BEFORE generic index evaluation (which uses Usize hint)
             // This enables ptr[i] = value on the left-hand side of assignment
             if let Type::Pointer { ref element, .. } = base_ty {
                 
                 // For Ptr<T>, get the pointer value from local variable
                 let loaded_ptr = if matches!(base_kind, LValueKind::SSA) {
                     base_ptr.clone()
                 } else {
                     // Load the pointer value from stack slot
                     let res = format!("%ptr_lvalue_loaded_{}", ctx.next_id());
                     ctx.emit_load(out, &res, &base_ptr, "!llvm.ptr");
                     res
                 };

                 // [ZERO-TRUST INDEX EVALUATION]
                 // Pass None instead of I64 hint to sever Context Contamination.
                 // This prevents emit_expr from trying to match the outer Pointer context.

                 let (raw_idx_val, raw_idx_ty) = emit_expr(ctx, out, &i.index, local_vars, None)?;

                 
                 // [MANUAL INTEGER ALIGNMENT]
                 // Now that we have the raw value, force it to I64 for the GEP.
                 let idx_final = if raw_idx_ty == Type::I64 {
                     raw_idx_val
                 } else {
                     // This will correctly promote Usize -> I64 (Index Cast), NOT Usize -> Pointer.
                     promote_numeric(ctx, out, &raw_idx_val, &raw_idx_ty, &Type::I64)?
                 };

                 let ptr = format!("%ptr_elem_{}", ctx.next_id());
                 let elem_mlir = element.to_mlir_storage_type(ctx)?;
                 ctx.emit_gep(out, &ptr, &loaded_ptr, &idx_final, &elem_mlir);
                 
                 // Return element type (F32), NOT the Pointer base
                 return Ok((ptr, (**element).clone(), LValueKind::Ptr));
             }
             
             // For non-Tensor, non-Pointer types: evaluate single index
             let (idx_val, idx_ty) = emit_expr(ctx, out, &i.index, local_vars, Some(&Type::Usize))?;
             
             // Ensure index is sign-extended to i64 for GEP
             let idx_prom = promote_numeric(ctx, out, &idx_val, &idx_ty, &Type::I64)?;
             
             match base_ty {
                 Type::Array(ref elem_ty, _, packed) => {
                     let array_ty = base_ty.to_mlir_type(ctx)?;
                     
                     if packed {
                          let c64 = format!("%c64_{}", ctx.next_id());
                          ctx.emit_const_int(out, &c64, 64, "i64");
                          let word_idx = format!("%word_idx_{}", ctx.next_id());
                          ctx.emit_binop(out, &word_idx, "arith.divui", &idx_prom, &c64, "i64");
                          let bit_off = format!("%bit_off_{}", ctx.next_id());
                          ctx.emit_binop(out, &bit_off, "arith.remui", &idx_prom, &c64, "i64");

                          let ptr = format!("%word_ptr_{}", ctx.next_id());
                          out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", 
                              ptr, base_ptr, word_idx, array_ty));
                          
                          Ok((ptr, Type::Bool, LValueKind::Bit(bit_off)))
                     } else {
                         let ptr = format!("%array_elem_ptr_{}", ctx.next_id());
                         out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", 
                             ptr, base_ptr, idx_prom, array_ty));
                         
                         Ok((ptr, *elem_ty.clone(), base_kind)) // Clone needed if ref
                     }
                 },
                 Type::Window(ref elem_ty, _) | Type::Owned(ref elem_ty) => {
                      let kind = if matches!(base_ty, Type::Owned(..)) { LValueKind::Local } else { LValueKind::Global(String::new()) };
                      
                      let loaded_ptr = if matches!(base_kind, LValueKind::SSA) {
                          base_ptr
                      } else {
                          let res = format!("%loaded_ptr_{}", ctx.next_id());
                          let scopes = match base_kind {
                              LValueKind::Local => Some(("#scope_local", "#scope_global")),
                              LValueKind::Global(_) => Some(("#scope_global", "#scope_local")),
                              _ => None,
                          };
                          // Load the pointer from the stack slot/global
                          // The type of the variable is Window/Owned, effectively a pointer.
                          // emit_load_logical will load the pointer value.
                          ctx.emit_load_logical_with_scope(out, &res, &base_ptr, &base_ty, scopes)?;
                          res
                      };

                      let ptr = format!("%elem_ptr_{}", ctx.next_id());
                      let elem_mlir = elem_ty.to_mlir_type(ctx)?;
                      ctx.emit_gep(out, &ptr, &loaded_ptr, &idx_prom, &elem_mlir);
                      
                      Ok((ptr, *elem_ty.clone(), kind))
                 },
                 Type::Reference(ref elem_ty, _) => {
                      // Reference can point to Stack or Global/Heap.
                      let loaded_ptr = if matches!(base_kind, LValueKind::SSA) {
                          base_ptr
                      } else {
                          let res = format!("%ref_loaded_ptr_{}", ctx.next_id());
                          let scopes = match base_kind {
                              LValueKind::Local => Some(("#scope_local", "#scope_global")),
                              LValueKind::Global(_) => Some(("#scope_global", "#scope_local")),
                              _ => None,
                          };
                          ctx.emit_load_logical_with_scope(out, &res, &base_ptr, &base_ty, scopes)?;
                          res
                      };

                      let ptr = format!("%elem_ptr_{}", ctx.next_id());
                      let elem_mlir = elem_ty.to_mlir_type(ctx)?;
                      ctx.emit_gep(out, &ptr, &loaded_ptr, &idx_prom, &elem_mlir);
                      
                      Ok((ptr, *elem_ty.clone(), LValueKind::Ptr))
                 },
                 // Note: Tensor and Pointer are handled above before the match, so won't reach here
                 
                 // [SOVEREIGN V9.0] Struct/Concrete indexing via `data: Ptr<T>` field
                 // Enables: slice[i] = val, slice[i] += expr
                 // Resolves Slice<T>[i] by loading the `data` field (Ptr<T>) and GEP with index.
                 Type::Struct(_) | Type::Concrete(..) => {
                     // Resolve the struct to find its fields
                     let resolved = crate::codegen::type_bridge::resolve_codegen_type(ctx, &base_ty);
                     let struct_name = match &resolved {
                         Type::Struct(n) => n.clone(),
                         Type::Concrete(n, args) => {
                             let suffix = args.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_");
                             format!("{}_{}", n, suffix)
                         }
                         _ => return Err(format!("Index operator not supported on type {:?}", base_ty)),
                     };
                     
                     if let Some(struct_info) = ctx.find_struct_by_name(&struct_name) {
                         // Look for `data` field of type Ptr<T>
                         if let Some((field_idx, field_ty)) = struct_info.fields.get("data") {
                             if let Type::Pointer { element, .. } = field_ty {
                                 let element = (**element).clone();
                                 let field_idx = *field_idx;
                                 
                                 // Load the base struct pointer if needed
                                 let struct_ptr = if matches!(base_kind, LValueKind::SSA) {
                                     base_ptr.clone()
                                 } else {
                                     let res = format!("%struct_loaded_{}", ctx.next_id());
                                     ctx.emit_load(out, &res, &base_ptr, "!llvm.ptr");
                                     res
                                 };
                                 
                                 // GEP to the `data` field
                                 let storage_ty = resolved.to_mlir_storage_type(ctx)?;
                                 let data_field_ptr = format!("%data_field_ptr_{}", ctx.next_id());
                                 out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n",
                                     data_field_ptr, struct_ptr, field_idx, storage_ty));
                                 
                                 // Load the Ptr<T> value from the data field
                                 let data_ptr = format!("%data_ptr_{}", ctx.next_id());
                                 ctx.emit_load(out, &data_ptr, &data_field_ptr, "!llvm.ptr");
                                 
                                 // GEP with the index into the data pointer
                                 let elem_ptr = format!("%slice_elem_ptr_{}", ctx.next_id());
                                 let elem_mlir = element.to_mlir_storage_type(ctx)?;
                                 ctx.emit_gep(out, &elem_ptr, &data_ptr, &idx_prom, &elem_mlir);
                                 
                                 return Ok((elem_ptr, element, LValueKind::Ptr));
                             }
                         }
                     }
                     
                     Err(format!("Index operator not supported on type {:?} (no 'data: Ptr<T>' field found)", base_ty))
                 }

                 _ => Err(format!("Index operator not supported on type {:?}", base_ty))
             }
        },
        syn::Expr::Field(f) => {
             let (base_addr, base_ty, kind) = emit_lvalue(ctx, out, &f.base, local_vars)?;
             
             if base_ty.k_is_ptr_type() {
                 if let syn::Member::Named(id) = &f.member {
                     // Ptr struct has { inner: &T, size: i64 } - handle inner field access
                     if id.to_string() == "inner" {
                         return Ok((base_addr, crate::types::Type::I64, kind));
                     }
                 }
             }
             
             match base_ty {
                  crate::types::Type::Struct(ref sn) | crate::types::Type::Concrete(ref sn, _) => {
                       // FIX: Force separate specialization for Concrete types
                       let sn_resolved = if let crate::types::Type::Concrete(base, args) = &base_ty {
                           ctx.ensure_struct_exists(base, args)?
                       } else {
                           sn.clone()
                       };
                       let sn = &sn_resolved;

                       let field_name = if let syn::Member::Named(id) = &f.member { id.to_string() } else { return Err("Named field expected".to_string()); };
                       if ctx.enum_registry().values().any(|i| i.name == *sn) {
                           return Err(format!("Field access '{}' on Enum '{}' not supported (use match)", field_name, sn));
                       }
                        let mut info_opt = ctx.struct_registry().values().find(|i| i.name == *sn).cloned();
                        if info_opt.is_none() {
                             // [VERIFIED METAL] Phase 5: Use centralized struct lookup
                             info_opt = ctx.find_struct_by_name(sn);
                        }
                        let info = info_opt.expect(&format!("Struct info missing for '{}' (available: {:?})", sn, ctx.struct_registry().iter().map(|(k, v)| (k.name.clone(), v.fields.len())).collect::<Vec<_>>()));

                       if let Some((idx, raw_field_ty)) = info.fields.get(&field_name) {
                            // [SOVEREIGN V4.0] CHAINED RESOLUTION FIX: Build local specialization map
                            let mut local_spec_map = ctx.current_type_map().clone();
                            if !info.specialization_args.is_empty() {
                                if let Some(ref template_name) = info.template_name {
                                    if let Some(template_def) = ctx.struct_templates().get(template_name).cloned() {
                                        if let Some(ref generics) = template_def.generics {
                                            for (i, param) in generics.params.iter().enumerate() {
                                                if let crate::grammar::GenericParam::Type { name: param_name, .. } = param {
                                                    if i < info.specialization_args.len() {
                                                        local_spec_map.insert(param_name.to_string(), info.specialization_args[i].clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            let field_ty = raw_field_ty.substitute(&local_spec_map);
                            let gep_var = format!("%gep_f_{}", ctx.next_id());
                           // [SOVEREIGN FIX] Use to_mlir_type for consistent struct alias naming
                           let mlir_struct = Type::Struct(info.name.clone()).to_mlir_type(ctx)?;

                           // SCALAR WRAPPER OPTIMIZATION
                           if mlir_struct == "i64" {
                               return Ok((base_addr, field_ty.clone(), LValueKind::Ptr));
                           }

                           let phys_idx = ctx.get_physical_index(&info.field_order, *idx);
                           ctx.emit_gep_field(out, &gep_var, &base_addr, phys_idx, &mlir_struct);
                           let f_ty: crate::types::Type = field_ty.clone();
                           Ok((gep_var, f_ty, LValueKind::Ptr))
                       } else { Err(format!("Field not found {} (info.fields has {:?})", field_name, info.fields.keys().collect::<Vec<_>>())) }
                  }
                  crate::types::Type::Owned(ref inner) => {
                      let inner_resolved = if let crate::types::Type::Concrete(base, args) = &**inner {
                           crate::types::Type::Struct(ctx.ensure_struct_exists(base, args)?)
                      } else {
                           *inner.clone()
                      };


                      match inner_resolved {
                           crate::types::Type::Struct(ref sn) | crate::types::Type::Concrete(ref sn, _) => {
                                let field_name = if let syn::Member::Named(id) = &f.member { id.to_string() } else { return Err("Named field expected".to_string()); };
                                let mut info_opt = ctx.struct_registry().values().find(|i| i.name == *sn).cloned();
                                if info_opt.is_none() {
                                     // Fallback
                                     let suffix = format!("__{}", sn);
                                     for info in ctx.struct_registry().values() {
                                         if info.name.ends_with(&suffix) {
                                             info_opt = Some(info.clone());
                                             break;
                                         }
                                     }
                                }
                                let info = info_opt.expect("Struct info missing");

                                if let Some((idx, raw_field_ty)) = info.fields.get(&field_name) {
                                     // [SOVEREIGN V4.0] CHAINED RESOLUTION FIX: Build local specialization map
                                     let mut local_spec_map = ctx.current_type_map().clone();
                                     if !info.specialization_args.is_empty() {
                                         if let Some(ref template_name) = info.template_name {
                                             if let Some(template_def) = ctx.struct_templates().get(template_name).cloned() {
                                                 if let Some(ref generics) = template_def.generics {
                                                     for (i, param) in generics.params.iter().enumerate() {
                                                         if let crate::grammar::GenericParam::Type { name: param_name, .. } = param {
                                                             if i < info.specialization_args.len() {
                                                                 local_spec_map.insert(param_name.to_string(), info.specialization_args[i].clone());
                                                             }
                                                         }
                                                     }
                                                 }
                                             }
                                         }
                                     }
                                     let field_ty = raw_field_ty.substitute(&local_spec_map);
                                     let loaded_ptr = if kind == LValueKind::SSA { base_addr } else {
                                         let res = format!("%loaded_ptr_{}", ctx.next_id());
                                         ctx.emit_load(out, &res, &base_addr, "!llvm.ptr");
                                         res
                                     };
                                     let gep_var = format!("%gep_f_{}", ctx.next_id());
                                     let mlir_struct = inner.to_mlir_type(ctx)?;

                                     let phys_idx = ctx.get_physical_index(&info.field_order, *idx);
                                     ctx.emit_gep_field(out, &gep_var, &loaded_ptr, phys_idx, &mlir_struct);
                                     let f_ty: crate::types::Type = field_ty.clone();
                                     Ok((gep_var, f_ty, LValueKind::Ptr))
                                } else { Err(format!("Field not found {}", field_name)) }
                           }
                           crate::types::Type::Tuple(ref elems) => {
                               let idx = if let syn::Member::Unnamed(idx) = &f.member { idx.index as usize } else { return Err("Tuple access requires index".to_string()); };
                               if idx >= elems.len() { return Err(format!("Tuple index out of bounds: {} >= {}", idx, elems.len())); }
                               let field_ty = &elems[idx];
                               let loaded_ptr = if kind == LValueKind::SSA { base_addr } else {
                                   let res = format!("%loaded_ptr_{}", ctx.next_id());
                                   ctx.emit_load(out, &res, &base_addr, "!llvm.ptr");
                                   res
                               };
                               let gep_var = format!("%gep_tuple_{}", ctx.next_id());
                               let mlir_tuple = inner.to_mlir_type(ctx)?;
                               ctx.emit_gep_field(out, &gep_var, &loaded_ptr, idx, &mlir_tuple);
                               Ok((gep_var, field_ty.clone(), LValueKind::Ptr))
                           }
                           _ => return Err(format!("Field access not supported on type {:?}", inner_resolved)),
                      }
                  }
                  crate::types::Type::Tuple(ref elems) => {
                       let idx = if let syn::Member::Unnamed(idx) = &f.member { idx.index as usize } else { return Err("Tuple access requires index".to_string()); };
                       if idx >= elems.len() { return Err(format!("Tuple index out of bounds: {} >= {}", idx, elems.len())); }
                       let field_ty = &elems[idx];
                       let gep_var = format!("%gep_tuple_{}", ctx.next_id());
                       let mlir_tuple = base_ty.to_mlir_type(ctx)?;
                       ctx.emit_gep_field(out, &gep_var, &base_addr, idx, &mlir_tuple);
                       Ok((gep_var, field_ty.clone(), LValueKind::Ptr))
                  }
                  crate::types::Type::Reference(ref inner, _) => {
                      // [SOVEREIGN FIX] For reference types (like &mut self), the base_addr
                      // IS the address of the struct. We should NOT load from it - that would
                      // treat the struct bytes as a pointer, causing segfaults.
                      // The reference type means base_addr points to the struct directly.
                      let loaded_ptr = base_addr.clone();
                      
                      let inner_resolved = if let crate::types::Type::Concrete(base, args) = &**inner {
                          crate::types::Type::Struct(ctx.ensure_struct_exists(base, args)?)
                      } else {
                          *inner.clone()
                      };

                      match inner_resolved {
                           crate::types::Type::Struct(ref sn) => {
                               let field_name = if let syn::Member::Named(id) = &f.member { id.to_string() } else { return Err("Named field expected".to_string()); };
                               let mut info_opt = ctx.struct_registry().values().find(|i| i.name == *sn).cloned();
                           if info_opt.is_none() {
                                // Fallback: Suffix Search - but prefer SHORTEST match to avoid
                                // matching NodePtr_main__TrieNode when looking for TrieNode
                                let suffix = format!("__{}", sn);
                                let mut best_match: Option<crate::registry::StructInfo> = None;
                                let mut best_len = usize::MAX;
                                for info in ctx.struct_registry().values() {
                                    if info.name.ends_with(&suffix) && info.name.len() < best_len {
                                        best_len = info.name.len();
                                        best_match = Some(info.clone());
                                    }
                                }
                                info_opt = best_match;
                           }
                           // DEBUG: Log all available struct names

                           let info = info_opt.expect(&format!("Struct info missing for '{}' (available in registry: {:?})", sn, ctx.struct_registry().iter().map(|(k, v)| (&k.name, v.fields.len())).collect::<Vec<_>>()));

                               if let Some((idx, field_ty)) = info.fields.get(&field_name) {
                                   let gep_var = format!("%gep_f_{}", ctx.next_id());
                                   // [SOVEREIGN FIX] Use to_mlir_type for consistent struct alias naming
                                   // This ensures fully-qualified names are used
                                   let struct_mlir_ty = Type::Struct(info.name.clone()).to_mlir_type(ctx)?;
                                   let phys_idx = ctx.get_physical_index(&info.field_order, *idx);
                                   ctx.emit_gep_field(out, &gep_var, &loaded_ptr, phys_idx, &struct_mlir_ty);
                                   let f_ty: crate::types::Type = field_ty.clone();
                                   Ok((gep_var, f_ty, LValueKind::Ptr))
                               } else { Err(format!("Field not found {} (struct '{}' has {} fields: {:?})", field_name, sn, info.fields.len(), info.fields.keys().collect::<Vec<_>>())) }
                           }
                           crate::types::Type::Tuple(ref elems) => {
                               let idx = if let syn::Member::Unnamed(idx) = &f.member { idx.index as usize } else { return Err("Tuple access requires index".to_string()); };
                               if idx >= elems.len() { return Err(format!("Tuple index out of bounds: {} >= {}", idx, elems.len())); }
                               let field_ty = &elems[idx];
                               let gep_var = format!("%gep_tuple_{}", ctx.next_id());
                               let mlir_tuple = inner.to_mlir_type(ctx)?;
                               ctx.emit_gep_field(out, &gep_var, &loaded_ptr, idx, &mlir_tuple);
                               Ok((gep_var, field_ty.clone(), LValueKind::Ptr))
                           }
                           _ => Err(format!("Cannot access field {:?} on reference to inner type {:?}", f.member, inner))
                       }
                   }
                   crate::types::Type::Pointer { ref element, .. } => {
                       // [PTR L-VALUE FIX] For Ptr<T> types, load the raw pointer value,
                       // then GEP into the pointed-to struct's field.
                       // This mirrors the Type::Reference arm but loads the pointer first.
                       let loaded_ptr = if matches!(kind, LValueKind::SSA) {
                           base_addr.clone()
                       } else {
                           let res = format!("%ptr_field_loaded_{}", ctx.next_id());
                           ctx.emit_load(out, &res, &base_addr, "!llvm.ptr");
                           res
                       };
                       
                       let inner_resolved = if let crate::types::Type::Concrete(base, args) = &**element {
                           crate::types::Type::Struct(ctx.ensure_struct_exists(base, args)?)
                       } else {
                           *element.clone()
                       };

                       match inner_resolved {
                            crate::types::Type::Struct(ref sn) => {
                                let field_name = if let syn::Member::Named(id) = &f.member { id.to_string() } else { return Err("Named field expected".to_string()); };
                                let mut info_opt = ctx.struct_registry().values().find(|i| i.name == *sn).cloned();
                                if info_opt.is_none() {
                                     info_opt = ctx.find_struct_by_name(sn);
                                }
                                if info_opt.is_none() {
                                     let suffix = format!("__{}", sn);
                                     let mut best_match: Option<crate::registry::StructInfo> = None;
                                     let mut best_len = usize::MAX;
                                     for info in ctx.struct_registry().values() {
                                         if info.name.ends_with(&suffix) && info.name.len() < best_len {
                                             best_len = info.name.len();
                                             best_match = Some(info.clone());
                                         }
                                     }
                                     info_opt = best_match;
                                }
                                let info = info_opt.expect(&format!("Struct info missing for '{}' in Ptr<T> field access", sn));

                                if let Some((idx, raw_field_ty)) = info.fields.get(&field_name) {
                                    // Build local specialization map for generic structs
                                    let mut local_spec_map = ctx.current_type_map().clone();
                                    if !info.specialization_args.is_empty() {
                                        if let Some(ref template_name) = info.template_name {
                                            if let Some(template_def) = ctx.struct_templates().get(template_name).cloned() {
                                                if let Some(ref generics) = template_def.generics {
                                                    for (i, param) in generics.params.iter().enumerate() {
                                                        if let crate::grammar::GenericParam::Type { name: param_name, .. } = param {
                                                            if i < info.specialization_args.len() {
                                                                local_spec_map.insert(param_name.to_string(), info.specialization_args[i].clone());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    let field_ty = raw_field_ty.substitute(&local_spec_map);
                                    let gep_var = format!("%gep_f_{}", ctx.next_id());
                                    let struct_mlir_ty = Type::Struct(info.name.clone()).to_mlir_type(ctx)?;
                                    let phys_idx = ctx.get_physical_index(&info.field_order, *idx);
                                    ctx.emit_gep_field(out, &gep_var, &loaded_ptr, phys_idx, &struct_mlir_ty);
                                    let f_ty: crate::types::Type = field_ty.clone();
                                    Ok((gep_var, f_ty, LValueKind::Ptr))
                                } else { Err(format!("Field not found {} on Ptr<{}> (fields: {:?})", field_name, sn, info.fields.keys().collect::<Vec<_>>())) }
                            }
                            _ => Err(format!("Cannot access field {:?} on Ptr<{:?}> (element type is not a struct)", f.member, element))
                       }
                   }
                   _ => Err(format!("Cannot access field {:?} on base type {:?}", f.member, base_ty))
              }
         }
         syn::Expr::Unary(u) => {
              if let syn::UnOp::Deref(_) = u.op {
                  let (val, ty) = emit_expr(ctx, out, &u.expr, local_vars, None)?;
                   match ty {
                       crate::types::Type::Reference(inner, _) | crate::types::Type::Owned(inner) => Ok((val, *inner, LValueKind::SSA)),
                       crate::types::Type::Pointer { element, .. } => Ok((val, *element, LValueKind::SSA)),
                       _ => Err(format!("Cannot dereference type {:?}", ty))
                  }
              } else {
                 Err("Only Deref unary supported in LValue".to_string())
             }
        }
        syn::Expr::Cast(c) => {
             let target_ty = resolve_type(ctx, &crate::grammar::SynType::from_std(*c.ty.clone()).unwrap());
             match target_ty {
                 Type::Reference(..) | Type::Owned(..) => {
                     let (val, ty) = emit_expr(ctx, out, &c.expr, local_vars, None)?;
                     let ptr_val = promote_numeric(ctx, out, &val, &ty, &target_ty)?;
                     Ok((ptr_val, target_ty.clone(), LValueKind::SSA))
                 }
                 _ => Err(format!("Cast to non-pointer type {:?} cannot be used as L-Value", target_ty))
             }
        }
        syn::Expr::Paren(p) => emit_lvalue(ctx, out, &p.expr, local_vars),
        // Handle method calls that return references - they can be used as lvalues
        syn::Expr::MethodCall(m) => {
            let (val, ty) = emit_method_call(ctx, out, m, local_vars, None)?;

            // If the method returns a reference, the value IS the pointer
            match ty {
                Type::Reference(inner, _) => {

                    Ok((val, *inner, LValueKind::SSA))
                }
                _ => Err(format!("Method {} returns {:?} which is not a reference type (cannot be used as L-Value)", m.method, ty))
            }
        }
        _ => Err(format!("Expression {:?} is not a valid L-Value (addressable memory location)", expr))
    }
}

// =========================================================================
// Z3 Symbolic Translation
// =========================================================================


/*
pub fn translate_bool_to_z3<'a, 'ctx>(
    ctx: &mut LoweringContext<'a, 'ctx>, 
    expr: &syn::Expr, 
    local_vars: &HashMap<String, (Type, LocalKind)>,
    sym_ctx: &SymbolicContext<'a>
) -> Result<z3::ast::Bool<'a>, String> {
    use z3::ast::Ast;
    match expr {
        syn::Expr::Binary(b) => {
            match b.op {
                syn::BinOp::Eq(_) | syn::BinOp::Ne(_) | syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_) => {
                    let lhs = translate_to_z3(ctx, &b.left, local_vars, sym_ctx)?;
                    let rhs = translate_to_z3(ctx, &b.right, local_vars, sym_ctx)?;
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
                    Ok(z3::ast::Bool::and(ctx.z3_ctx, &[&bl, &br]))
                }
                syn::BinOp::Or(_) => {
                    let bl = translate_bool_to_z3(ctx, &b.left, local_vars, sym_ctx)?;
                    let br = translate_bool_to_z3(ctx, &b.right, local_vars, sym_ctx)?;
                    Ok(z3::ast::Bool::or(ctx.z3_ctx, &[&bl, &br]))
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
            Ok(z3::ast::Bool::from_bool(ctx.z3_ctx, b.value))
        }
        _ => Err("Unsupported symbolic boolean expression".to_string()),
    }
}
*/


/// Structural unification for bidirectional type inference.
/// Walks two types in parallel, extracting bindings for generic placeholders.
/// Handles both Type::Generic("T") and Type::Struct("T") (single-char uppercase names).
pub(crate) fn unify_types_recursive(template: &Type, concrete: &Type, map: &mut std::collections::BTreeMap<String, Type>) {
    match (template, concrete) {
        // Type::Generic("T") — explicit generic marker
        (Type::Generic(name), _) => {
            if !map.contains_key(name) {
                map.insert(name.clone(), concrete.clone());
            }
        }
        // Recurse into Pointer { element }
        (Type::Pointer { element: e1, .. }, Type::Pointer { element: e2, .. }) => {
            unify_types_recursive(e1, e2, map);
        }
        // Recurse into Concrete args  
        (Type::Concrete(n1, args1), Type::Concrete(n2, args2)) if args1.len() == args2.len() => {
            // Allow matching even if base names differ slightly (qualified vs unqualified)
            // e.g., "std__core__result__Result" vs "std__core__result__Result"
            if n1 == n2 || n1.ends_with(&format!("__{}", n2)) || n2.ends_with(&format!("__{}", n1)) {
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    unify_types_recursive(a1, a2, map);
                }
            }
        }
        // Recurse into Reference
        (Type::Reference(inner1, _), Type::Reference(inner2, _)) => {
            unify_types_recursive(inner1, inner2, map);
        }
        // Recurse into Array
        (Type::Array(inner1, _, _), Type::Array(inner2, _, _)) => {
            unify_types_recursive(inner1, inner2, map);
        }
        _ => {} // Base case: no unification possible
    }
}

/// Infer phantom generics from resolved Fn return types.
/// Example: Map<I, F, T> where F = Fn(i64)->i64 => T = i64
/// This handles generics that don't appear in struct fields but represent
/// the output type of a function generic.
pub fn infer_phantom_generics(
    declared_generics: &[String],
    map: &mut BTreeMap<String, Type>,
) {
    let unresolved: Vec<String> = declared_generics.iter()
        .filter(|g| !map.contains_key(*g))
        .cloned()
        .collect();

    if unresolved.is_empty() { return; }

    // Collect return types from all resolved Fn types
    let fn_return_types: Vec<Type> = map.values()
        .filter_map(|ty| {
            if let Type::Fn(_, ret) = ty {
                Some((**ret).clone())
            } else {
                None
            }
        })
        .collect();

    // If there's exactly one unresolved generic and at least one Fn return type,
    // use the first Fn's return type as the phantom generic
    if unresolved.len() == 1 && fn_return_types.len() == 1 {
        map.insert(unresolved[0].clone(), fn_return_types[0].clone());
    }
}

pub(crate) fn infer_generics(
    params: &[Type], 
    args: &[Type],   
    generics: &crate::grammar::Generics
) -> Vec<Type> {
    let mut mapping: std::collections::BTreeMap<String, Type> = std::collections::BTreeMap::new();
    
    // Helper to unify types recursively
    fn unify(p: &Type, a: &Type, map: &mut std::collections::BTreeMap<String, Type>) {
        match (p, a) {
            (Type::Generic(name), _) => {
                if !map.contains_key(name) {
                    map.insert(name.clone(), a.clone());
                }
            },
            (Type::Reference(p_in, _), Type::Reference(a_in, _)) | 
            (Type::Owned(p_in), Type::Owned(a_in)) |
            (Type::Atomic(p_in), Type::Atomic(a_in)) |
            (Type::Array(p_in, _, _), Type::Array(a_in, _, _)) => unify(p_in, a_in, map),
            (Type::Concrete(n1, args1), Type::Concrete(n2, args2)) => { 
                 if n1 == n2 && args1.len() == args2.len() {
                      for (p_arg, a_arg) in args1.iter().zip(args2.iter()) {
                           unify(p_arg, a_arg, map);
                      }
                 }
            },
            (Type::Struct(_n1), Type::Struct(_n2)) => {
                 // Structs have no args to unify
            },
             _ => {}
        }
    }

    for (p, a) in params.iter().zip(args.iter()) {
        unify(p, a, &mut mapping);
    }
    
    let mut res = Vec::new();
    for param in &generics.params {
        let name_str = match param {
             crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
             crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
        };
        
        if let Some(ty) = mapping.get(&name_str) {
            res.push(ty.clone());
        } else {
             res.push(Type::Unit);
        }
    }
    res
}

/// [V5.0 STRUCTURAL FORMATTING] Heuristically resolve the type of an f-string expression
/// without emitting any MLIR code. This is used by __fstring_append_expr to determine
/// the correct append_* method to call.
///
/// Strategy:
/// 1. Simple identifiers: look up in local_vars (handles most cases)
/// 2. Expressions like `a + b`: try to infer from operand types
/// 3. Field accesses like `p.x`: try to resolve struct field types  
/// 4. Everything else: return None (falls back to append_i32)
fn resolve_fstring_expr_type(
    expr_str: &str,
    local_vars: &HashMap<String, (Type, LocalKind)>,
    ctx: &mut LoweringContext,
) -> Option<Type> {
    let trimmed = expr_str.trim();
    
    // 1. Simple identifier — direct lookup in local_vars
    if trimmed.chars().all(|c| c.is_alphanumeric() || c == '_') && !trimmed.is_empty() {
        if let Some((ty, _)) = local_vars.get(trimmed) {
            // For Ptr-backed locals, unwrap the reference
            let actual_ty = match ty {
                Type::Reference(inner, _) => (**inner).clone(),
                _ => ty.clone(),
            };
            return Some(actual_ty);
        }
    }
    
    // 2. Check for binary expressions (a + b, a * b, etc.)
    //    If both sides are integers, result is integer (promote to i64 for safety)
    for op in &[" + ", " - ", " * ", " / ", " % "] {
        if let Some(pos) = trimmed.find(op) {
            let lhs = &trimmed[..pos];
            let rhs = &trimmed[pos + op.len()..];
            let lhs_ty = resolve_fstring_expr_type(lhs, local_vars, ctx);
            let rhs_ty = resolve_fstring_expr_type(rhs, local_vars, ctx);
            match (&lhs_ty, &rhs_ty) {
                (Some(l), Some(r)) if l.is_integer() && r.is_integer() => {
                    // Promote to the wider type
                    if matches!(l, Type::I64) || matches!(r, Type::I64) 
                        || matches!(l, Type::Usize) || matches!(r, Type::Usize) {
                        return Some(Type::I64);
                    }
                    return Some(Type::I32);
                }
                (Some(l), Some(r)) if l.is_float() || r.is_float() => {
                    return Some(Type::F64);
                }
                _ => {}
            }
        }
    }
    
    // 3. Field access: expr.field — try to resolve from struct definition
    if let Some(dot_pos) = trimmed.rfind('.') {
        let base = &trimmed[..dot_pos];
        let _field = &trimmed[dot_pos + 1..];
        // If base is a known variable, and it's a struct, look up field type
        if let Some(base_ty) = resolve_fstring_expr_type(base, local_vars, ctx) {
            if let Type::Struct(name) | Type::Concrete(name, _) = &base_ty {
                // Try to get field info from struct_templates
                if let Ok(fields) = ctx.get_struct_fields(name) {
                    for (fname, fty) in &fields {
                        if fname == _field {
                            return Some(fty.clone());
                        }
                    }
                }
            }
        }
    }
    
    // 4. Integer literal detection
    if trimmed.parse::<i64>().is_ok() {
        // Check if it fits in i32
        if let Ok(_) = trimmed.parse::<i32>() {
            return Some(Type::I32);
        }
        return Some(Type::I64);
    }
    
    // 5. Float literal detection
    if trimmed.contains('.') && trimmed.parse::<f64>().is_ok() {
        return Some(Type::F64);
    }
    
    // Could not resolve — caller will use append_i32 fallback
    None
}
