//! Free-function emit helpers for MLIR code generation.
//!
//! These are pure formatters that append MLIR instructions to an output buffer.
//! They were extracted from `CodegenContext` methods to avoid opaque borrows
//! when used with `LoweringContext` (view struct pattern).
//!
//! Usage: `use crate::codegen::emit_helpers::*;`

use crate::codegen::context::CodegenConfig;

// =========================================================================
// MLIR Builder Pattern Helpers (Pure Formatters)
// =========================================================================

pub fn emit_binop(out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
    out.push_str(&format!("    {} = {} {}, {} : {}\n", res, op, lhs, rhs, ty));
}

/// Emit binary operation with fast-math attributes for vectorization.
pub fn emit_binop_fast(out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
    out.push_str(&format!("    {} = {} {}, {} {{fastmath = #arith.fastmath<reassoc, contract>}} : {}\n", 
        res, op, lhs, rhs, ty));
}

pub fn emit_const_int(out: &mut String, res: &str, val: i64, ty: &str) {
    out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val, ty));
}

pub fn emit_const_float(out: &mut String, res: &str, val: f64, ty: &str) {
    let val_str = if val == 0.0 {
        "0.0".to_string()
    } else {
        format!("{:.17e}", val)
    };
    out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val_str, ty));
}

pub fn emit_load(out: &mut String, res: &str, ptr: &str, ty: &str) {
    out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, ptr, ty));
}

pub fn emit_load_scoped(out: &mut String, config: &CodegenConfig, res: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
    if !config.emit_alias_scopes {
        emit_load(out, res, ptr, ty);
        return;
    }
    out.push_str(&format!("    {} = llvm.load {} {{ alias_scopes = [{}], noalias = [{}] }} : !llvm.ptr -> {}\n", res, ptr, scope, noalias, ty));
}

pub fn emit_store(out: &mut String, val: &str, ptr: &str, ty: &str) {
    out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", val, ptr, ty));
}

pub fn emit_store_scoped(out: &mut String, config: &CodegenConfig, val: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
    if !config.emit_alias_scopes {
        emit_store(out, val, ptr, ty);
        return;
    }
    out.push_str(&format!("    llvm.store {}, {} {{ alias_scopes = [{}], noalias = [{}] }} : {}, !llvm.ptr\n", val, ptr, scope, noalias, ty));
}

pub fn emit_alloca(alloca_out: &mut String, res: &str, ty: &str) {
    alloca_out.push_str(&format!("    {} = llvm.alloca %c1_i64 x {} : (i64) -> !llvm.ptr\n", res, ty));
}

pub fn emit_gep_field(out: &mut String, res: &str, base: &str, idx: usize, struct_ty: &str) {
    out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n", res, base, idx, struct_ty));
}

pub fn emit_gep(out: &mut String, res: &str, base: &str, idx_var: &str, elem_ty: &str) {
    out.push_str(&format!("    {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", res, base, idx_var, elem_ty));
}

pub fn emit_extractvalue(out: &mut String, res: &str, val: &str, idx: usize, ty: &str) {
    out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", res, val, idx, ty));
}

pub fn emit_extractvalue_logical(out: &mut String, next_id: &mut dyn FnMut() -> usize, res: &str, val: &str, idx: usize, ty: &str, field_ty: &crate::types::Type) -> Result<(), String> {
    if *field_ty == crate::types::Type::Bool {
        let extract_res = format!("%b_extract_{}", next_id());
        out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", extract_res, val, idx, ty));
        emit_trunc(out, res, &extract_res, "i8", "i1");
    } else {
        emit_extractvalue(out, res, val, idx, ty);
    }
    Ok(())
}

pub fn emit_insertvalue(out: &mut String, res: &str, elem: &str, val: &str, idx: usize, ty: &str) {
    out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, elem, val, idx, ty));
}

pub fn emit_insertvalue_logical(out: &mut String, next_id: &mut dyn FnMut() -> usize, res: &str, elem: &str, val: &str, idx: usize, ty: &str, field_ty: &crate::types::Type) -> Result<(), String> {
    if *field_ty == crate::types::Type::Bool {
         let zext_res = format!("%b_zext_ins_{}", next_id());
         emit_cast(out, &zext_res, "arith.extui", elem, "i1", "i8");
         out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, zext_res, val, idx, ty));
    } else {
         emit_insertvalue(out, res, elem, val, idx, ty);
    }
    Ok(())
}

pub fn emit_cmp(out: &mut String, res: &str, cmp_op: &str, pred: &str, lhs: &str, rhs: &str, ty: &str) {
    let comma = if cmp_op == "llvm.icmp" || cmp_op == "llvm.fcmp" { "" } else { "," };
    out.push_str(&format!("    {} = {} \"{}\"{} {}, {} : {}\n", res, cmp_op, pred, comma, lhs, rhs, ty));
}

pub fn emit_cast(out: &mut String, res: &str, op: &str, val: &str, from_ty: &str, to_ty: &str) {
    out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, val, from_ty, to_ty));
}

pub fn emit_trunc(out: &mut String, res: &str, val: &str, from_ty: &str, to_ty: &str) {
    out.push_str(&format!("    {} = arith.trunci {} : {} to {}\n", res, val, from_ty, to_ty));
}

pub fn emit_br(out: &mut String, label: &str) {
    out.push_str(&format!("    llvm.br ^{}\n", label));
}

pub fn emit_cond_br(out: &mut String, cond: &str, true_label: &str, false_label: &str) {
    out.push_str(&format!("    llvm.cond_br {}, ^{}, ^{}\n", cond, true_label, false_label));
}

pub fn emit_label(out: &mut String, label: &str) {
    out.push_str(&format!("  ^{}:\n", label));
}

pub fn emit_return(out: &mut String, val: &str, ty: &str) {
    out.push_str(&format!("    llvm.return {} : {}\n", val, ty));
}

pub fn emit_return_void(out: &mut String) {
    out.push_str("    llvm.return\n");
}

pub fn emit_load_exclusive(out: &mut String, res: &str, ptr: &str, ty: &str) {
    out.push_str(&format!("    {} = \"llvm.load\"({}) {{salt.access = \"exclusive\"}} : (!llvm.ptr) -> {}\n", res, ptr, ty));
}

pub fn emit_load_atomic(out: &mut String, next_id: &mut dyn FnMut() -> usize, res: &str, ptr: &str, ty: &str) {
    let zero = format!("%atomic_zero_{}", next_id());
    out.push_str(&format!("    {} = arith.constant 0 : {}\n", zero, ty));
    out.push_str(&format!("    {} = llvm.atomicrmw _or {}, {} seq_cst : !llvm.ptr, {}\n", res, ptr, zero, ty));
}

pub fn emit_store_atomic(out: &mut String, next_id: &mut dyn FnMut() -> usize, val: &str, ptr: &str, ty: &str) {
    let discard = format!("%atomic_discard_{}", next_id());
    out.push_str(&format!("    {} = llvm.atomicrmw xchg {}, {} seq_cst : !llvm.ptr, {}\n", discard, ptr, val, ty));
}

pub fn emit_atomicrmw(out: &mut String, res: &str, op: &str, ptr: &str, val: &str, ty: &str) {
    out.push_str(&format!("    {} = llvm.atomicrmw {} {}, {} seq_cst : !llvm.ptr, {}\n", res, op, ptr, val, ty));
}

pub fn emit_inttoptr(out: &mut String, res: &str, val: &str, from_ty: &str) {
    out.push_str(&format!("    {} = llvm.inttoptr {} : {} to !llvm.ptr\n", res, val, from_ty));
}

pub fn emit_verify(out: &mut String, next_id: &mut dyn FnMut() -> usize, cond: &str, _msg: &str) {
    let true_const = format!("%verify_true_{}", next_id());
    let violated = format!("%verify_violated_{}", next_id());
    out.push_str(&format!("    {} = arith.constant true\n", true_const));
    out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", violated, cond, true_const));
    out.push_str(&format!("    scf.if {} {{\n", violated));
    out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
    out.push_str("      scf.yield\n");
    out.push_str("    }\n");
}

pub fn emit_noalias_metadata(next_id: &mut dyn FnMut() -> usize, region_name: &str) -> (String, String) {
    let id = next_id();
    let scope_domain = format!("@alias_domain_{}", id);
    let scope_id = format!("@alias_scope_{}_{}", region_name, id);
    (scope_id, scope_domain)
}
