//! HIR-to-MLIR Emitter (Phase 11)
//!
//! Lowers HIR `Item`s produced by `lower_async_fn_cfg()` into MLIR strings.
//! The HIR-to-HIR transformation already decomposed the coroutine into standard
//! synchronous constructs (structs, loops, ifs, assigns). This emitter just
//! sees standard nodes — it never needs to know what a "coroutine" is.
//!
//! ## Zero-Cost Erasure
//! `StmtKind::Assume`, `ExprKind::Requires`, and `ExprKind::Ensures` compile
//! to exactly zero bytes of machine code. They exist purely for the frontend
//! verification engine (Z3) and are erased at the MLIR boundary.
//!
//! ## Poll<T> ABI
//! `Poll<T>` is lowered as `!llvm.struct<(i32, T)>`:
//!   - Field 0: i32 discriminant (0 = Pending, 1 = Ready)
//!   - Field 1: T payload (undefined when Pending)

use std::collections::HashMap;
use crate::hir::items::{Item, ItemKind, Struct, Field};
use crate::hir::stmt::{Stmt, StmtKind};
use crate::hir::expr::{Expr, ExprKind, Literal, BinOp, UnOp, Block};
use crate::types::Type;

/// Incrementing counter for unique SSA names within an emission session.
/// Also carries struct field maps for GEP index resolution.
struct HirEmitCtx {
    counter: usize,
    break_labels: Vec<String>,
    continue_labels: Vec<String>,
    /// Map from struct name → ordered list of field names.
    /// Used to resolve field name → index for `llvm.getelementptr`.
    struct_fields: HashMap<String, Vec<String>>,
}

impl HirEmitCtx {
    fn new() -> Self {
        HirEmitCtx {
            counter: 0,
            break_labels: vec![],
            continue_labels: vec![],
            struct_fields: HashMap::new(),
        }
    }

    fn next_id(&mut self) -> usize {
        let id = self.counter;
        self.counter += 1;
        id
    }

    /// Register a struct definition so field indices can be resolved later.
    fn register_struct(&mut self, name: &str, fields: &[Field]) {
        let names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
        self.struct_fields.insert(name.to_string(), names);
    }

    /// Resolve a field name to its index within a struct.
    /// Falls back to 0 if the struct or field is not found (shouldn't happen
    /// after proper registration).
    fn resolve_field_index(&self, struct_name: &str, field_name: &str) -> usize {
        if let Some(fields) = self.struct_fields.get(struct_name) {
            fields.iter().position(|f| f == field_name).unwrap_or(0)
        } else {
            // Fallback: __state is always 0
            0
        }
    }

    /// Given a base type (which may be a Reference to a Struct), extract the
    /// struct name for field lookup.
    fn extract_struct_name(ty: &Type) -> Option<String> {
        match ty {
            Type::Struct(name) => Some(name.clone()),
            Type::Reference(inner, _) => Self::extract_struct_name(inner),
            _ => None,
        }
    }
}

// ============================================================================
// Type Resolution
// ============================================================================

/// Resolve an HIR Type to its MLIR type string.
///
/// Special handling for `Poll<T>` which lowers to `!llvm.struct<(i32, T)>`.
/// For primitive types, delegates to `to_mlir_type_simple()`.
pub fn resolve_hir_type(ty: &Type) -> String {
    match ty {
        Type::Concrete(name, args) if name == "Poll" && !args.is_empty() => {
            let inner = resolve_hir_type(&args[0]);
            format!("!llvm.struct<(i32, {})>", inner)
        }
        Type::Reference(inner, _mutable) => {
            let _ = inner; // suppress unused
            "!llvm.ptr".to_string()
        }
        Type::Struct(name) => {
            // Named struct reference — use the type alias defined by emit_hir_struct_def
            format!("!struct_{}", name)
        }
        _ => ty.to_mlir_type_simple(),
    }
}

// ============================================================================
// Item Emission
// ============================================================================

/// Top-level entry point: emit all HIR Items (struct def + step fn) as MLIR.
pub fn emit_hir_items(items: &[Item]) -> Result<String, String> {
    let mut ctx = HirEmitCtx::new();
    let mut out = String::new();

    // First pass: register all struct definitions for field resolution
    for item in items {
        if let ItemKind::Struct(s) = &item.kind {
            ctx.register_struct(&item.name, &s.fields);
        }
    }

    // Second pass: emit MLIR
    for item in items {
        match &item.kind {
            ItemKind::Struct(s) => {
                out.push_str(&emit_hir_struct_def(&item.name, s));
            }
            ItemKind::Fn(f) => {
                let fn_mlir = emit_hir_fn(&mut ctx, &item.name, f)?;
                out.push_str(&fn_mlir);
            }
            _ => {} // Other item kinds not produced by async lowering
        }
    }
    Ok(out)
}

/// Emit an HIR struct as an MLIR type alias.
///
/// Example: `!struct___AsyncState_foo = !llvm.struct<"__AsyncState_foo", (i64, i32)>`
fn emit_hir_struct_def(name: &str, s: &Struct) -> String {
    let fields: Vec<String> = s.fields.iter()
        .map(|f| resolve_hir_type(&f.ty))
        .collect();
    format!(
        "!struct_{} = !llvm.struct<\"{}\", ({})>\n",
        name,
        name,
        fields.join(", "),
    )
}

/// Emit an HIR function as MLIR `func.func`.
fn emit_hir_fn(ctx: &mut HirEmitCtx, name: &str, f: &crate::hir::items::Fn) -> Result<String, String> {
    let mut out = String::new();

    // Build argument list
    let args: Vec<String> = f.inputs.iter()
        .map(|p| format!("%arg_{}: {}", p.name, resolve_hir_type(&p.ty)))
        .collect();

    let ret_ty = resolve_hir_type(&f.output);

    out.push_str(&format!(
        "  func.func private @{}({}) -> {} {{\n",
        name,
        args.join(", "),
        ret_ty,
    ));

    // Emit body
    if let Some(body) = &f.body {
        emit_hir_block(ctx, &mut out, body, 2)?;
    }

    out.push_str("  }\n\n");
    Ok(out)
}

// ============================================================================
// Statement Emission
// ============================================================================

/// Emit a block of HIR statements. Returns true if a terminator was emitted.
fn emit_hir_block(
    ctx: &mut HirEmitCtx,
    out: &mut String,
    block: &Block,
    indent: usize,
) -> Result<bool, String> {
    let mut terminated = false;
    for stmt in &block.stmts {
        if emit_hir_stmt(ctx, out, stmt, indent)? {
            terminated = true;
            break;
        }
    }
    Ok(terminated)
}

/// Emit a single HIR statement. Returns true if this is a terminator (return/break/continue).
fn emit_hir_stmt(
    ctx: &mut HirEmitCtx,
    out: &mut String,
    stmt: &Stmt,
    indent: usize,
) -> Result<bool, String> {
    let pad = "  ".repeat(indent);
    match &stmt.kind {
        // ── Zero-Cost Erasure ──────────────────────────────────────────
        // Assume exists purely for the Z3 verification engine.
        // It compiles to exactly zero bytes of machine code.
        StmtKind::Assume(_) => Ok(false),

        // ── Standard Synchronous Nodes ────────────────────────────────
        StmtKind::Expr(expr) => {
            emit_hir_expr(ctx, out, expr, indent)?;
            Ok(false)
        }
        StmtKind::Semi(expr) => {
            emit_hir_expr(ctx, out, expr, indent)?;
            Ok(false)
        }
        StmtKind::Local(local) => {
            if let Some(init) = &local.init {
                let val = emit_hir_expr(ctx, out, init, indent)?;
                if let crate::hir::stmt::Pattern::Bind { name, var_id, .. } = &local.pat {
                    out.push_str(&format!(
                        "{}// let {} (v{}) = {}\n",
                        pad, name, var_id.0, val,
                    ));
                }
            }
            Ok(false)
        }
        StmtKind::Return(opt_expr) => {
            if let Some(expr) = opt_expr {
                let val = emit_hir_expr(ctx, out, expr, indent)?;
                let ret_ty = resolve_hir_type(&expr.ty);
                out.push_str(&format!("{}func.return {} : {}\n", pad, val, ret_ty));
            } else {
                out.push_str(&format!("{}func.return\n", pad));
            }
            Ok(true)
        }
        StmtKind::Loop(body) => {
            let label_header = format!("loop_header_{}", ctx.next_id());
            let label_exit = format!("loop_exit_{}", ctx.next_id());

            out.push_str(&format!("{}cf.br ^{}\n", pad, label_header));
            out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_header));

            ctx.break_labels.push(label_exit.clone());
            ctx.continue_labels.push(label_header.clone());

            let diverges = emit_hir_block(ctx, out, body, indent)?;

            ctx.break_labels.pop();
            ctx.continue_labels.pop();

            if !diverges {
                out.push_str(&format!("{}cf.br ^{}\n", pad, label_header));
            }

            out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_exit));
            Ok(false)
        }
        StmtKind::While { cond, body } => {
            let label_header = format!("while_header_{}", ctx.next_id());
            let label_body = format!("while_body_{}", ctx.next_id());
            let label_exit = format!("while_exit_{}", ctx.next_id());

            out.push_str(&format!("{}cf.br ^{}\n", pad, label_header));
            out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_header));

            let cond_val = emit_hir_expr(ctx, out, cond, indent)?;
            out.push_str(&format!(
                "{}cf.cond_br {}, ^{}, ^{}\n",
                pad, cond_val, label_body, label_exit,
            ));
            out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_body));

            ctx.break_labels.push(label_exit.clone());
            ctx.continue_labels.push(label_header.clone());

            let diverges = emit_hir_block(ctx, out, body, indent)?;

            ctx.break_labels.pop();
            ctx.continue_labels.pop();

            if !diverges {
                out.push_str(&format!("{}cf.br ^{}\n", pad, label_header));
            }

            out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_exit));
            Ok(false)
        }
        StmtKind::Break => {
            if let Some(label) = ctx.break_labels.last() {
                out.push_str(&format!("{}cf.br ^{}\n", pad, label));
            }
            Ok(true)
        }
        StmtKind::Continue => {
            if let Some(label) = ctx.continue_labels.last() {
                out.push_str(&format!("{}cf.br ^{}\n", pad, label));
            }
            Ok(true)
        }
        StmtKind::For { .. } => {
            // For loops are not produced by the async lowering.
            // If we encounter one, it should have been lowered during HIR processing.
            Err("HIR emitter: unexpected StmtKind::For in async step function".into())
        }
    }
}

// ============================================================================
// Expression Emission
// ============================================================================

/// Emit an HIR expression, returning the SSA name of the result.
fn emit_hir_expr(
    ctx: &mut HirEmitCtx,
    out: &mut String,
    expr: &Expr,
    indent: usize,
) -> Result<String, String> {
    let pad = "  ".repeat(indent);
    match &expr.kind {
        ExprKind::Literal(lit) => emit_hir_literal(ctx, out, lit, &expr.ty, &pad),
        ExprKind::Var(var_id) => Ok(format!("%v{}", var_id.0)),
        ExprKind::UnresolvedIdent(name) => Ok(format!("%{}", name)),
        ExprKind::Binary { op, lhs, rhs } => emit_hir_binary_op(ctx, out, op, lhs, rhs, indent, &pad),
        ExprKind::Unary { op, expr: inner } => emit_hir_unary_op(ctx, out, op, inner, &expr.ty, indent, &pad),
        ExprKind::Field { base, field } => emit_hir_field(ctx, out, base, field, &expr.ty, indent, &pad),
        ExprKind::Assign { lhs, rhs } => emit_hir_assign(ctx, out, lhs, rhs, indent, &pad),
        ExprKind::If { cond, then_branch, else_branch } => emit_hir_if(ctx, out, cond, then_branch, else_branch.as_deref(), indent, &pad),
        ExprKind::Block(block) => {
            emit_hir_block(ctx, out, block, indent)?;
            Ok("%unit".to_string())
        }
        ExprKind::StructLit { name, fields, type_args: _ } => emit_hir_struct_lit(ctx, out, name, fields, &expr.ty, indent, &pad),
        ExprKind::Requires(_) | ExprKind::Ensures(_) => Ok("%unit".to_string()),
        ExprKind::Yield(_) => Err("HIR emitter: unexpected ExprKind::Yield — should have been lowered".into()),
        ExprKind::Path(_) | ExprKind::Call { .. } | ExprKind::Cast { .. } |
        ExprKind::Index { .. } | ExprKind::Ref(_) | ExprKind::MethodCall { .. } |
        ExprKind::While { .. } | ExprKind::Loop(_) | ExprKind::Return(_) |
        ExprKind::Break | ExprKind::Continue => {
            let id = ctx.next_id();
            let result = format!("%expr_{}", id);
            out.push_str(&format!("{}// HIR expr: {:?}\n", pad, std::mem::discriminant(&expr.kind)));
            Ok(result)
        }
    }
}

fn emit_hir_literal(ctx: &mut HirEmitCtx, out: &mut String, lit: &Literal, ty: &Type, pad: &str) -> Result<String, String> {
    let id = ctx.next_id();
    match lit {
        Literal::Int(n) => {
            let resolved_ty = resolve_hir_type(ty);
            let name = format!("%c{}_{}", n, id);
            out.push_str(&format!("{}{} = arith.constant {} : {}\n", pad, name, n, resolved_ty));
            Ok(name)
        }
        Literal::Bool(b) => {
            let name = format!("%b{}_{}", b, id);
            let val = if *b { "true" } else { "false" };
            out.push_str(&format!("{}{} = arith.constant {} : i1\n", pad, name, val));
            Ok(name)
        }
        Literal::Float(f) => {
            let resolved_ty = resolve_hir_type(ty);
            let name = format!("%f_{}", id);
            out.push_str(&format!("{}{} = arith.constant {:e} : {}\n", pad, name, f, resolved_ty));
            Ok(name)
        }
        Literal::String(s) => Ok(format!("\"{}\"", s)),
    }
}

fn emit_hir_binary_op(
    ctx: &mut HirEmitCtx, out: &mut String, op: &BinOp,
    lhs: &Expr, rhs: &Expr, indent: usize, pad: &str
) -> Result<String, String> {
    let lhs_val = emit_hir_expr(ctx, out, lhs, indent)?;
    let rhs_val = emit_hir_expr(ctx, out, rhs, indent)?;
    let id = ctx.next_id();
    let result = format!("%binop_{}", id);
    let lhs_ty = resolve_hir_type(&lhs.ty);

    match op {
        BinOp::Eq => out.push_str(&format!("{}{} = arith.cmpi eq, {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Ne => out.push_str(&format!("{}{} = arith.cmpi ne, {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Lt => out.push_str(&format!("{}{} = arith.cmpi slt, {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Le => out.push_str(&format!("{}{} = arith.cmpi sle, {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Gt => out.push_str(&format!("{}{} = arith.cmpi sgt, {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Ge => out.push_str(&format!("{}{} = arith.cmpi sge, {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Add => out.push_str(&format!("{}{} = arith.addi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Sub => out.push_str(&format!("{}{} = arith.subi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Mul => out.push_str(&format!("{}{} = arith.muli {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Div => out.push_str(&format!("{}{} = arith.divsi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Rem => out.push_str(&format!("{}{} = arith.remsi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::And | BinOp::BitAnd => out.push_str(&format!("{}{} = arith.andi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Or | BinOp::BitOr => out.push_str(&format!("{}{} = arith.ori {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::BitXor => out.push_str(&format!("{}{} = arith.xori {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Shl => out.push_str(&format!("{}{} = arith.shli {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::Shr => out.push_str(&format!("{}{} = arith.shrsi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::AddAssign => out.push_str(&format!("{}{} = arith.addi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::SubAssign => out.push_str(&format!("{}{} = arith.subi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::MulAssign => out.push_str(&format!("{}{} = arith.muli {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::DivAssign => out.push_str(&format!("{}{} = arith.divsi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
        BinOp::RemAssign => out.push_str(&format!("{}{} = arith.remsi {}, {} : {}\n", pad, result, lhs_val, rhs_val, lhs_ty)),
    }
    Ok(result)
}

fn emit_hir_unary_op(
    ctx: &mut HirEmitCtx, out: &mut String, op: &UnOp,
    inner: &Expr, ty: &Type, indent: usize, pad: &str
) -> Result<String, String> {
    let inner_val = emit_hir_expr(ctx, out, inner, indent)?;
    let id = ctx.next_id();
    let result = format!("%unop_{}", id);
    match op {
        UnOp::Not => {
            let true_const = format!("%true_{}", id);
            out.push_str(&format!("{}{} = arith.constant true : i1\n", pad, true_const));
            out.push_str(&format!("{}{} = arith.xori {}, {} : i1\n", pad, result, inner_val, true_const));
        }
        UnOp::Neg => {
            let resolved_ty = resolve_hir_type(&inner.ty);
            let zero = format!("%zero_{}", id);
            out.push_str(&format!("{}{} = arith.constant 0 : {}\n", pad, zero, resolved_ty));
            out.push_str(&format!("{}{} = arith.subi {}, {} : {}\n", pad, result, zero, inner_val, resolved_ty));
        }
        UnOp::Deref => {
            let resolved_ty = resolve_hir_type(ty);
            out.push_str(&format!("{}{} = llvm.load {} : !llvm.ptr -> {}\n", pad, result, inner_val, resolved_ty));
        }
    }
    Ok(result)
}

fn emit_hir_field(
    ctx: &mut HirEmitCtx, out: &mut String, base: &Expr,
    field: &str, ty: &Type, indent: usize, pad: &str
) -> Result<String, String> {
    let base_val = emit_hir_expr(ctx, out, base, indent)?;
    let id = ctx.next_id();
    let field_ty = resolve_hir_type(ty);

    let struct_name = HirEmitCtx::extract_struct_name(&base.ty);
    let field_idx = if let Some(ref sname) = struct_name {
        ctx.resolve_field_index(sname, field)
    } else if field == "__state" { 0 } else {
        let mut found = 0;
        let mut names: Vec<&String> = ctx.struct_fields.keys().collect();
        names.sort();
        for sname in names {
            if let Some(fields) = ctx.struct_fields.get(sname) {
                if let Some(pos) = fields.iter().position(|f| f == field) { found = pos; break; }
            }
        }
        found
    };

    let struct_ty = if let Some(ref sname) = struct_name {
        format!("!struct_{}", sname)
    } else {
        ctx.struct_fields.keys().min().map(|name| format!("!struct_{}", name)).unwrap_or_else(|| resolve_hir_type(&base.ty))
    };

    let field_ptr = format!("%field_ptr_{}", id);
    let result = format!("%field_{}", id);
    out.push_str(&format!("{}{} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n", pad, field_ptr, base_val, field_idx, struct_ty));
    out.push_str(&format!("{}{} = llvm.load {} : !llvm.ptr -> {}\n", pad, result, field_ptr, field_ty));
    Ok(result)
}

fn emit_hir_assign(
    ctx: &mut HirEmitCtx, out: &mut String, lhs: &Expr,
    rhs: &Expr, indent: usize, pad: &str
) -> Result<String, String> {
    let rhs_val = emit_hir_expr(ctx, out, rhs, indent)?;
    let id = ctx.next_id();

    if let ExprKind::Field { base, field } = &lhs.kind {
        let base_val = emit_hir_expr(ctx, out, base, indent)?;
        let struct_name = HirEmitCtx::extract_struct_name(&base.ty);
        let field_idx = if let Some(ref sname) = struct_name {
            ctx.resolve_field_index(sname, field)
        } else if field == "__state" { 0 } else {
            let mut found = 0;
            let mut keys: Vec<&String> = ctx.struct_fields.keys().collect();
            keys.sort();
            for k in keys {
                if let Some(fields) = ctx.struct_fields.get(k) {
                    if let Some(pos) = fields.iter().position(|f| f == field) { found = pos; break; }
                }
            }
            found
        };

        let struct_ty = if let Some(ref sname) = struct_name {
            format!("!struct_{}", sname)
        } else {
            ctx.struct_fields.keys().min().map(|name| format!("!struct_{}", name)).unwrap_or_else(|| resolve_hir_type(&base.ty))
        };

        let rhs_ty = resolve_hir_type(&rhs.ty);
        let field_ptr = format!("%assign_ptr_{}", id);
        out.push_str(&format!("{}{} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n", pad, field_ptr, base_val, field_idx, struct_ty));
        out.push_str(&format!("{}llvm.store {}, {} : {}, !llvm.ptr\n", pad, rhs_val, field_ptr, rhs_ty));
    }
    Ok(rhs_val)
}

fn emit_hir_if(
    ctx: &mut HirEmitCtx, out: &mut String, cond: &Expr,
    then_branch: &crate::hir::expr::Block, else_branch: Option<&Expr>,
    indent: usize, pad: &str
) -> Result<String, String> {
    let cond_val = emit_hir_expr(ctx, out, cond, indent)?;
    let id = ctx.next_id();
    let label_then = format!("then_{}", id);
    let label_else = format!("else_{}", id);
    let label_merge = format!("merge_{}", id);

    if let Some(else_expr) = else_branch {
        out.push_str(&format!("{}cf.cond_br {}, ^{}, ^{}\n", pad, cond_val, label_then, label_else));
        out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_then));
        let then_diverges = emit_hir_block(ctx, out, then_branch, indent)?;
        if !then_diverges { out.push_str(&format!("{}cf.br ^{}\n", pad, label_merge)); }
        
        out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_else));
        if let ExprKind::Block(else_block) = &else_expr.kind {
            let else_diverges = emit_hir_block(ctx, out, else_block, indent)?;
            if !else_diverges { out.push_str(&format!("{}cf.br ^{}\n", pad, label_merge)); }
        }
        out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_merge));
    } else {
        out.push_str(&format!("{}cf.cond_br {}, ^{}, ^{}\n", pad, cond_val, label_then, label_merge));
        out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_then));
        let then_diverges = emit_hir_block(ctx, out, then_branch, indent)?;
        if !then_diverges { out.push_str(&format!("{}cf.br ^{}\n", pad, label_merge)); }
        out.push_str(&format!("{}^{}:\n", "  ".repeat(indent - 1), label_merge));
    }
    Ok("%unit".to_string())
}

fn emit_hir_struct_lit(
    ctx: &mut HirEmitCtx, out: &mut String, name: &str,
    fields: &[(String, Expr)], ty: &Type, indent: usize, pad: &str
) -> Result<String, String> {
    let id = ctx.next_id();
    let poll_ty = resolve_hir_type(ty);

    match name {
        "Poll::Pending" => {
            let undef = format!("%poll_undef_{}", id);
            let disc = format!("%poll_disc_{}", id);
            let result = format!("%poll_pending_{}", id);
            out.push_str(&format!("{}{} = llvm.mlir.undef : {}\n", pad, undef, poll_ty));
            out.push_str(&format!("{}{} = arith.constant 0 : i32\n", pad, disc));
            out.push_str(&format!("{}{} = llvm.insertvalue {}, {}[0] : {}\n", pad, result, disc, undef, poll_ty));
            Ok(result)
        }
        "Poll::Ready" => {
            let undef = format!("%poll_undef_{}", id);
            let disc = format!("%poll_disc_{}", id);
            let tagged = format!("%poll_tagged_{}", id);
            out.push_str(&format!("{}{} = llvm.mlir.undef : {}\n", pad, undef, poll_ty));
            out.push_str(&format!("{}{} = arith.constant 1 : i32\n", pad, disc));
            out.push_str(&format!("{}{} = llvm.insertvalue {}, {}[0] : {}\n", pad, tagged, disc, undef, poll_ty));
            if !fields.is_empty() {
                let payload_val = emit_hir_expr(ctx, out, &fields[0].1, indent)?;
                let final_reg = format!("%poll_ready_{}", id);
                out.push_str(&format!("{}{} = llvm.insertvalue {}, {}[1] : {}\n", pad, final_reg, payload_val, tagged, poll_ty));
                Ok(final_reg)
            } else {
                Ok(tagged)
            }
        }
        _ => {
            let result = format!("%struct_{}", id);
            out.push_str(&format!("{}{} = llvm.mlir.undef : {}\n", pad, result, poll_ty));
            Ok(result)
        }
    }
}


// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::ids::VarId;
    use crate::hir::items::{Visibility, Param};
    use crate::hir::expr::Expr;

    // ── Crash Site 1: Zero-Cost Erasure ───────────────────────────────

    #[test]
    fn test_assume_erased() {
        // StmtKind::Assume must produce zero bytes of MLIR.
        let assume_stmt = Stmt {
            kind: StmtKind::Assume(Expr {
                kind: ExprKind::Binary {
                    op: BinOp::Lt,
                    lhs: Box::new(Expr {
                        kind: ExprKind::Var(VarId(0)),
                        ty: Type::I64,
                        span: proc_macro2::Span::call_site(),
                    }),
                    rhs: Box::new(Expr {
                        kind: ExprKind::Literal(Literal::Int(10)),
                        ty: Type::I64,
                        span: proc_macro2::Span::call_site(),
                    }),
                },
                ty: Type::Bool,
                span: proc_macro2::Span::call_site(),
            }),
            span: proc_macro2::Span::call_site(),
        };

        let mut ctx = HirEmitCtx::new();
        let mut out = String::new();
        let result = emit_hir_stmt(&mut ctx, &mut out, &assume_stmt, 2);

        assert!(result.is_ok());
        assert!(!result.expect("Assume stmt emission succeeded"), "Assume should not be a terminator");
        assert!(out.is_empty(), "Assume must emit zero bytes, got: '{}'", out);
    }

    #[test]
    fn test_requires_erased() {
        // ExprKind::Requires must produce zero bytes of MLIR.
        let requires_expr = Expr {
            kind: ExprKind::Requires(Box::new(Expr {
                kind: ExprKind::Literal(Literal::Bool(true)),
                ty: Type::Bool,
                span: proc_macro2::Span::call_site(),
            })),
            ty: Type::Unit,
            span: proc_macro2::Span::call_site(),
        };

        let mut ctx = HirEmitCtx::new();
        let mut out = String::new();
        let result = emit_hir_expr(&mut ctx, &mut out, &requires_expr, 2);

        assert!(result.is_ok());
        assert_eq!(result.expect("Requires expr emission succeeded"), "%unit");
        assert!(out.is_empty(), "Requires must emit zero bytes, got: '{}'", out);
    }

    // ── Crash Site 2: Poll<T> ABI ─────────────────────────────────────

    #[test]
    fn test_poll_pending_layout() {
        // Poll::Pending must emit {i32=0, T=undef} as !llvm.struct<(i32, T)>
        let pending_expr = Expr {
            kind: ExprKind::StructLit {
                name: "Poll::Pending".into(),
                type_args: vec![],
                fields: vec![],
            },
            ty: Type::Concrete("Poll".into(), vec![Type::Unit]),
            span: proc_macro2::Span::call_site(),
        };

        let mut ctx = HirEmitCtx::new();
        let mut out = String::new();
        let result = emit_hir_expr(&mut ctx, &mut out, &pending_expr, 2);

        assert!(result.is_ok());
        let mlir = out.clone();
        // Must contain undef, constant 0, insertvalue
        assert!(mlir.contains("llvm.mlir.undef"), "Missing undef in: {}", mlir);
        assert!(mlir.contains("arith.constant 0 : i32"), "Missing discriminant 0 in: {}", mlir);
        assert!(mlir.contains("llvm.insertvalue"), "Missing insertvalue in: {}", mlir);
        assert!(mlir.contains("!llvm.struct<(i32, !llvm.void)>"), "Wrong Poll type in: {}", mlir);
    }

    #[test]
    fn test_poll_ready_layout() {
        // Poll::Ready must emit {i32=1, T} as !llvm.struct<(i32, T)>
        let ready_expr = Expr {
            kind: ExprKind::StructLit {
                name: "Poll::Ready".into(),
                type_args: vec![],
                fields: vec![],
            },
            ty: Type::Concrete("Poll".into(), vec![Type::I32]),
            span: proc_macro2::Span::call_site(),
        };

        let mut ctx = HirEmitCtx::new();
        let mut out = String::new();
        let result = emit_hir_expr(&mut ctx, &mut out, &ready_expr, 2);

        assert!(result.is_ok());
        let mlir = out.clone();
        assert!(mlir.contains("llvm.mlir.undef"), "Missing undef in: {}", mlir);
        assert!(mlir.contains("arith.constant 1 : i32"), "Missing discriminant 1 in: {}", mlir);
        assert!(mlir.contains("llvm.insertvalue"), "Missing insertvalue in: {}", mlir);
        assert!(mlir.contains("!llvm.struct<(i32, i32)>"), "Wrong Poll type in: {}", mlir);
    }

    #[test]
    fn test_poll_ready_with_payload() {
        // Poll::Ready with a payload must emit insertvalue for both
        // the discriminant at [0] AND the payload at [1].
        let span = proc_macro2::Span::call_site();
        let payload_expr = Expr {
            kind: ExprKind::Literal(crate::hir::expr::Literal::Int(99)),
            ty: Type::I64,
            span,
        };
        let ready_expr = Expr {
            kind: ExprKind::StructLit {
                name: "Poll::Ready".into(),
                type_args: vec![],
                fields: vec![("0".to_string(), payload_expr)],
            },
            ty: Type::Concrete("Poll".into(), vec![Type::I64]),
            span,
        };

        let mut ctx = HirEmitCtx::new();
        let mut out = String::new();
        let result = emit_hir_expr(&mut ctx, &mut out, &ready_expr, 2);

        assert!(result.is_ok(), "Poll::Ready with payload should emit successfully");
        let mlir = out.clone();
        // Discriminant at [0]
        assert!(mlir.contains("arith.constant 1 : i32"), "Missing discriminant 1 in: {}", mlir);
        assert!(mlir.contains("[0] : !llvm.struct<(i32, i64)>"),
            "Missing insertvalue at [0] in: {}", mlir);
        // Payload at [1]
        assert!(mlir.contains("[1] : !llvm.struct<(i32, i64)>"),
            "Missing insertvalue at [1] for payload in: {}", mlir);
        // Result register should be %poll_ready_*
        assert!(result.expect("Poll::Ready emission succeeded").starts_with("%poll_ready_"),
            "Result should be a poll_ready register");
    }

    // ── Crash Site 3: Orchestrator — struct emission ──────────────────

    #[test]
    fn test_hir_struct_def() {
        // State struct must emit a valid !llvm.struct type alias
        let state_struct = Struct {
            fields: vec![
                Field { name: "__state".into(), ty: Type::I64, vis: Visibility::Private },
                Field { name: "x".into(), ty: Type::I32, vis: Visibility::Private },
                Field { name: "__local_0".into(), ty: Type::Bool, vis: Visibility::Private },
            ],
            generics: crate::hir::items::Generics::default(),
            invariants: vec![],
        };

        let mlir = emit_hir_struct_def("__AsyncState_foo", &state_struct);
        assert!(mlir.contains("!struct___AsyncState_foo"), "Missing type alias in: {}", mlir);
        assert!(mlir.contains("\"__AsyncState_foo\""), "Missing struct name in: {}", mlir);
        assert!(mlir.contains("i64"), "Missing __state type in: {}", mlir);
        assert!(mlir.contains("i32"), "Missing x type in: {}", mlir);
        assert!(mlir.contains("i1"), "Missing Bool type in: {}", mlir);
    }

    // ── Field Index Resolution ────────────────────────────────────────

    #[test]
    fn test_field_index_resolution() {
        // Verify struct-aware field index resolution
        let mut ctx = HirEmitCtx::new();
        ctx.register_struct("__AsyncState_process", &[
            Field { name: "__state".into(), ty: Type::I64, vis: Visibility::Private },
            Field { name: "x".into(), ty: Type::I64, vis: Visibility::Private },
            Field { name: "__local_0".into(), ty: Type::I64, vis: Visibility::Private },
        ]);

        assert_eq!(ctx.resolve_field_index("__AsyncState_process", "__state"), 0);
        assert_eq!(ctx.resolve_field_index("__AsyncState_process", "x"), 1);
        assert_eq!(ctx.resolve_field_index("__AsyncState_process", "__local_0"), 2);
    }

    #[test]
    fn test_resolve_hir_type_struct() {
        // Type::Struct must resolve to the MLIR type alias
        let ty = Type::Struct("__AsyncState_process".to_string());
        assert_eq!(resolve_hir_type(&ty), "!struct___AsyncState_process");
    }

    // ── End-to-End: Full Pipeline Integration ─────────────────────────

    #[test]
    fn test_end_to_end_async_compilation() {
        // The Source Code Reality:
        //
        //   async fn process(x: I64) -> I64 {
        //       let y = x + 10;
        //       yield;
        //       return y;
        //   }
        //
        // This tests the entire pipeline from HIR Fn → lower_async_fn_cfg → emit_hir_items.

        use crate::hir::async_lower::{lower_async_fn_cfg, VarInfo};
        use crate::hir::items as items;
        use crate::hir::expr::Block;
        use crate::hir::stmt::{Local, Pattern};

        // Build the parsed HIR body:
        //   let y = x + 10;    (y = VarId(1), x = VarId(0))
        //   yield;
        //   use(y);            (a return-equivalent: Semi usage of y)
        let span = proc_macro2::Span::call_site();
        let func = items::Fn {
            inputs: vec![Param { name: "x".into(), ty: Type::I64 }],
            output: Type::I64,
            body: Some(Block {
                stmts: vec![
                    // let y = x + 10;
                    Stmt {
                        kind: StmtKind::Local(Local {
                            pat: Pattern::Bind {
                                name: "y".to_string(),
                                var_id: VarId(1),
                                mutable: false,
                            },
                            ty: Some(Type::I64),
                            init: Some(Expr {
                                kind: ExprKind::Binary {
                                    op: BinOp::Add,
                                    lhs: Box::new(Expr {
                                        kind: ExprKind::Var(VarId(0)),
                                        ty: Type::I64,
                                        span,
                                    }),
                                    rhs: Box::new(Expr {
                                        kind: ExprKind::Literal(Literal::Int(10)),
                                        ty: Type::I64,
                                        span,
                                    }),
                                },
                                ty: Type::I64,
                                span,
                            }),
                        }),
                        span,
                    },
                    // yield;
                    Stmt {
                        kind: StmtKind::Semi(Expr {
                            kind: ExprKind::Yield(None),
                            ty: Type::Unit,
                            span,
                        }),
                        span,
                    },
                    // return y — explicit return to test Poll::Ready payload packing
                    Stmt {
                        kind: StmtKind::Return(Some(Expr {
                            kind: ExprKind::Var(VarId(1)),
                            ty: Type::I64,
                            span,
                        })),
                        span,
                    },
                ],
                value: None,
                ty: Type::Unit,
            }),
            generics: items::Generics::default(),
            is_async: true,
        };

        // Variable y (VarId(1)) crosses the yield boundary because it is
        // defined before yield and used after.
        let crossing = vec![
            VarInfo { var_id: VarId(1), name: "y".into(), ty: Type::I64 },
        ];

        // ── Stage 1: Frontend Lowering ────────────────────────────────
        // lower_async_fn_cfg decomposes the async fn into:
        //   Item::Struct  — __AsyncState_process { __state, x, __local_1 }
        //   Item::Fn      — __step_process(ctx: &mut __AsyncState_process) -> Poll<I64>
        let items = lower_async_fn_cfg("process", &func, &crossing, 200);

        assert_eq!(items.len(), 2, "lower_async_fn_cfg must produce exactly 2 items");
        assert_eq!(items[0].name, "__AsyncState_process");
        assert_eq!(items[1].name, "__step_process");

        // Verify struct fields: __state (i64), x (i64), __local_1 (i64)
        if let ItemKind::Struct(ref s) = items[0].kind {
            assert_eq!(s.fields.len(), 3, "state struct must have 3 fields");
            assert_eq!(s.fields[0].name, "__state");
            assert_eq!(s.fields[1].name, "x");
            assert_eq!(s.fields[2].name, "__local_1");
        } else {
            panic!("Item 0 should be a Struct");
        }

        // Verify step fn signature
        if let ItemKind::Fn(ref f) = items[1].kind {
            assert!(!f.is_async, "step fn must not be async");
            assert_eq!(f.inputs.len(), 1, "step fn takes exactly one ctx arg");
            assert_eq!(f.inputs[0].name, "ctx");
            // Return type is Poll<I64>
            assert_eq!(f.output, Type::Concrete("Poll".into(), vec![Type::I64]));
        } else {
            panic!("Item 1 should be a Fn");
        }

        // ── Stage 2: MLIR Emission ────────────────────────────────────
        let mlir_output = emit_hir_items(&items).expect("emit_hir_items failed");


        // ── Stage 3: Structural Assertions ────────────────────────────

        // 3a. Struct emission: type alias for the state struct
        assert!(mlir_output.contains("!struct___AsyncState_process"),
            "Missing state struct type alias");
        assert!(mlir_output.contains("\"__AsyncState_process\""),
            "Missing struct name in type alias");

        // 3b. Step function signature
        assert!(mlir_output.contains("func.func private @__step_process"),
            "Missing step function declaration");
        assert!(mlir_output.contains("!llvm.struct<(i32, i64)>"),
            "Missing Poll<I64> return type");

        // 3c. Trampoline: loop + cf.br
        assert!(mlir_output.contains("cf.br ^loop_header"),
            "Missing trampoline loop entry");

        // 3d. State dispatch: if ctx.__state == N
        assert!(mlir_output.contains("arith.cmpi eq"),
            "Missing state dispatch comparison");

        // 3e. Field access: llvm.getelementptr for ctx fields
        assert!(mlir_output.contains("llvm.getelementptr"),
            "Missing GEP for field access");

        // 3f. Yield suspension: state assignment + return Poll::Pending
        assert!(mlir_output.contains("arith.constant 0 : i32"),
            "Missing Poll::Pending discriminant (0)");
        assert!(mlir_output.contains("llvm.insertvalue"),
            "Missing insertvalue for Poll construction");

        // 3g. Final completion: return Poll::Ready with payload
        assert!(mlir_output.contains("arith.constant 1 : i32"),
            "Missing Poll::Ready discriminant (1)");
        // The return value (y) must be packed into field [1]
        assert!(mlir_output.contains("[1] : !llvm.struct<(i32, i64)>"),
            "Missing insertvalue for Poll::Ready payload at field [1]");

        // 3h. State transitions
        assert!(mlir_output.contains("llvm.store"),
            "Missing state variable store (ctx.__state = N)");

        // 3i. Function return
        assert!(mlir_output.contains("func.return"),
            "Missing func.return");

        // 3j. Zero-cost erasure: no Assume text in MLIR
        // (The CFG builder injects Assume nodes for while-loop conditions,
        //  but our simple body has no while, so this verifies the general erasure path.
        //  The `test_assume_erased` unit test covers the explicit path.)
        assert!(!mlir_output.contains("assume"),
            "Assume text must not appear in MLIR output");
    }
}
