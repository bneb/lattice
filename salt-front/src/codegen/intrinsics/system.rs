use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

fn emit_prefetch_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    _expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    if args.len() != 4 {
        return Err("intrin_prefetch expects 4 arguments: (addr, rw, locality, cache_type)".to_string());
    }
    let (ptr, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::I64))?;

    let extract_int = |e: &syn::Expr| -> Result<i32, String> {
        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(ref i), .. }) = e {
            i.base10_parse::<i32>().map_err(|e| e.to_string())
        } else {
            Err("prefetch arguments (rw, locality, cache_type) must be integer literals".to_string())
        }
    };

    let rw = extract_int(&args[1])?;
    let hint = extract_int(&args[2])?;
    let cache = extract_int(&args[3])?;

    let ptr_converted = format!("%prefetch_ptr_{}", ctx.next_id());
    out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", ptr_converted, ptr));
    out.push_str(&format!("    \"llvm.intr.prefetch\"({}) <{{rw = {} : i32, hint = {} : i32, cache = {} : i32}}> : (!llvm.ptr) -> ()\n",
        ptr_converted, rw, hint, cache));

    let res = format!("%prefetch_res_{}", ctx.next_id());
    out.push_str(&format!("    {} = arith.constant 0 : i64\n", res));
    Ok(Some((res, Type::I64)))
}

fn emit_macos_syscall(
    ctx: &mut LoweringContext,
    out: &mut String,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    _expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    if args.len() != 4 {
        return Err("intrin::macos_syscall expects 4 arguments: (syscall_num, fd, ptr, len)".to_string());
    }
    let (syscall_num, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::I64))?;
    let (fd, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
    let (ptr, ptr_ty) = emit_expr(ctx, out, &args[2], local_vars, None)?;
    let (len, _) = emit_expr(ctx, out, &args[3], local_vars, None)?;

    let ptr_i64 = if matches!(ptr_ty, Type::Struct(_) | Type::Concrete(_, _)) {
        let ptr_ty_mlir = ptr_ty.to_mlir_storage_type(ctx)?;
        let extracted = format!("%ptr_raw_{}", ctx.next_id());
        if ptr_ty_mlir.contains("struct") || ptr_ty_mlir.starts_with("!struct_") || ptr_ty_mlir.starts_with("!llvm.struct") {
            ctx.emit_extractvalue(out, &extracted, &ptr, 0, &ptr_ty_mlir);
            extracted
        } else {
            ptr.clone()
        }
    } else if matches!(ptr_ty, Type::Reference(_, _)) {
        let addr = format!("%ptr_addr_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", addr, ptr));
        addr
    } else {
        ptr.clone()
    };

    let res = format!("%sys_res_{}", ctx.next_id());
    out.push_str(&format!("    {} = \"llvm.intr.aarch64.syscall\"({}, {}, {}, {}) : (i64, i64, i64, i64) -> i64\n",
        res, syscall_num, fd, ptr_i64, len));
    Ok(Some((res, Type::I64)))
}

pub fn emit_system_intrinsic(
    ctx: &mut LoweringContext,
    out: &mut String,
    name: &str,
    args: &[syn::Expr],
    local_vars: &mut HashMap<String, (Type, LocalKind)>,
    expected_ty: Option<&Type>,
) -> Result<Option<(String, Type)>, String> {
    match name {
        "read_tls_deadline" | "keuos__read_tls_deadline" => {
            if !args.is_empty() {
                return Err("read_tls_deadline() takes no arguments".to_string());
            }
            let res = format!("%deadline_{}", ctx.next_id());
            out.push_str(&format!(
                "    {} = \"llvm.intr.read.register\"() {{\
                    name = \"x19\"\
                }} : () -> i64\n",
                res
            ));
            Ok(Some((res, Type::I64)))
        }
        "m4_wfe" | "keuos__wfe" => {
            if !args.is_empty() {
                return Err("Intrinsic 'm4_wfe' expects 0 arguments".to_string());
            }
            out.push_str("    \"llvm.intr.aarch64.hint\"() {hint = 2 : i32} : () -> ()\n");
            Ok(Some(("".to_string(), Type::Unit)))
        }
        "m4_sev" | "keuos__sev" => {
            if !args.is_empty() {
                return Err("Intrinsic 'm4_sev' expects 0 arguments".to_string());
            }
            out.push_str("    \"llvm.intr.aarch64.hint\"() {hint = 4 : i32} : () -> ()\n");
            Ok(Some(("".to_string(), Type::Unit)))
        }
        "m4_dmb_ish" | "keuos__dmb_ish" => {
            if !args.is_empty() {
                return Err("Intrinsic 'm4_dmb_ish' expects 0 arguments".to_string());
            }
            out.push_str("    \"llvm.fence\"() {syncscope = \"\", ordering = 5 : i64} : () -> ()\n");
            Ok(Some(("".to_string(), Type::Unit)))
        }
        "trap" => {
            if !args.is_empty() {
                return Err("Intrinsic 'trap' expects 0 arguments".to_string());
            }
            out.push_str("    \"llvm.intr.trap\"() : () -> ()\n");
            Ok(Some(("".to_string(), Type::Unit)))
        }
        "fn_addr" => {
            if args.len() != 1 {
                return Err("Intrinsic 'fn_addr' expects 1 argument (function pointer)".to_string());
            }
            let (ptr_var, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let res = format!("%fn_addr_{}", ctx.next_id());
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, ptr_var));
            Ok(Some((res, Type::U64)))
        }
        "intrin_prefetch" | "std__simd__intrin_prefetch" => {
            emit_prefetch_intrinsic(ctx, out, args, local_vars, expected_ty)
        }
        "intrin_expect" | "std__simd__intrin_expect" => {
            if args.len() != 2 {
                return Err("intrin_expect expects 2 arguments: (val, expected)".to_string());
            }
            let (val, _) = emit_expr(ctx, out, &args[0], local_vars, Some(&Type::I64))?;
            let (expected, _) = emit_expr(ctx, out, &args[1], local_vars, Some(&Type::I64))?;
            let res = format!("%expect_{}", ctx.next_id());
            out.push_str(&format!("    {} = \"llvm.intr.expect\"({}, {}) : (i64, i64) -> i64\n",
                res, val, expected));
            Ok(Some((res, Type::I64)))
        }
        "yield_check" | "salt_yield_check" | "std__thread__yield_now" => {
            ctx.emit_lto_hook(out, "__salt_yield_check", args, local_vars, expected_ty)
        }
        "target__has_feature" => {
            if let Some(syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. })) = args.first() {
                    let feature = s.value();
                    let has = match feature.as_str() {
                        "neon" | "aarch64" | "m4" => true,
                        "avx" | "x86_64" => false,
                        _ => false,
                    };
                    let res = format!("%has_feat_{}", ctx.next_id());
                    ctx.emit_const_int(out, &res, if has { 1 } else { 0 }, "i1");
                    return Ok(Some((res, Type::Bool)));
            }
            Err("target::has_feature expects string literal".to_string())
        }
        name if name.contains("macos_syscall") => {
            emit_macos_syscall(ctx, out, args, local_vars, expected_ty)
        }
        "pulse_io_submit" | "keuos__io_submit" => {
            if args.len() != 2 { return Err("pulse_io_submit expects 2 args: (ring, batch)".to_string()); }
            let (ring, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let (batch, _) = emit_expr(ctx, out, &args[1], local_vars, None)?;

            ctx.entity_registry_mut().register_hook("salt_kqueue_submit");
            let res = format!("%io_res_{}", ctx.next_id());
            out.push_str(&format!("    {} = func.call @salt_kqueue_submit({}, {}) : (!llvm.ptr, !llvm.ptr) -> i64\n",
                res, ring, batch));
            Ok(Some((res, Type::I64)))
        }
        "pulse_io_reap" | "keuos__io_reap" => {
            if args.len() != 3 { return Err("pulse_io_reap expects 3 args: (ring, batch, timeout)".to_string()); }
            let (ring, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;
            let (batch, _) = emit_expr(ctx, out, &args[1], local_vars, None)?;
            let (timeout, _) = emit_expr(ctx, out, &args[2], local_vars, None)?;

            ctx.entity_registry_mut().register_hook("salt_kqueue_reap");
            let res = format!("%io_res_{}", ctx.next_id());
            out.push_str(&format!("    {} = func.call @salt_kqueue_reap({}, {}, {}) : (!llvm.ptr, !llvm.ptr, i64) -> i64\n",
                res, ring, batch, timeout));
            Ok(Some((res, Type::I64)))
        }
        "pulse_io_teardown" | "keuos__io_teardown" => {
            if args.len() != 1 { return Err("pulse_io_teardown expects 1 arg: (ring)".to_string()); }
            let (ring, _) = emit_expr(ctx, out, &args[0], local_vars, None)?;

            ctx.entity_registry_mut().register_hook("salt_kqueue_teardown");
            out.push_str(&format!("    func.call @salt_kqueue_teardown({}) : (!llvm.ptr) -> ()\n", ring));
            Ok(Some(("".to_string(), Type::Unit)))
        }
        "spin_loop_hint" | "std__sync__spin_loop_hint" => {
            if !args.is_empty() {
                return Err("spin_loop_hint() takes no arguments".to_string());
            }
            out.push_str("    \"llvm.inline_asm\"() <{asm_string = \"pause\", constraints = \"\", asm_dialect = 0 : i64}> {has_side_effects} : () -> ()\n");
            Ok(Some(("%unit".to_string(), Type::Unit)))
        }
        _ => Ok(None),
    }
}
