//! SaltAsyncToState — async-to-state-machine coroutine transformation.
//!
//! This module transforms `async fn` with `yield` points into explicit
//! state machine structs. Each yield point becomes a state transition,
//! and variables that live across yield boundaries are "lifted" into
//! the state struct's fields.
//!
//! # Architecture
//!
//! 1. **Variable Lifting Analysis**: Walk the HIR body to identify variables
//!    whose definitions dominate a yield point and whose uses are reachable
//!    from a different yield point (i.e., they "cross" a suspension boundary).
//!
//! 2. **State Struct Synthesis**: Generate a struct with one field per
//!    lifted variable, plus a `state: u32` discriminant.
//!
//! 3. **Resume Function Emission**: Lower the original body into a
//!    `match self.state { 0 => ..., 1 => ..., }` dispatch.

use std::collections::HashSet;
use crate::hir::ids::{DefId, VarId};
use crate::hir::expr::{Expr, ExprKind};
use crate::hir::stmt::{Stmt, StmtKind, Pattern};
use crate::hir::items::{self, Item, ItemKind, Param, Visibility, Field};
use crate::hir::types::Type;

/// Metadata for a variable that crosses a yield boundary.
/// Carries enough information to generate a struct field.
#[derive(Clone, Debug)]
pub struct VarInfo {
    pub var_id: VarId,
    pub name: String,
    pub ty: Type,
}

/// Synthesize the `__AsyncState_{func_name}` environment struct.
///
/// The struct contains:
/// 1. `__state: I64` — coroutine instruction pointer
/// 2. One field per function parameter (params cross from init to first resume)
/// 3. `__local_{var_id}` for each variable that lives across a yield boundary
pub fn generate_state_struct(
    func_name: &str,
    params: &[Param],
    crossing_vars: &[VarInfo],
) -> Item {
    let struct_name = format!("__AsyncState_{}", func_name);

    let mut fields = Vec::new();

    // 1. The instruction pointer discriminant
    fields.push(Field {
        name: "__state".to_string(),
        ty: Type::I64,
        vis: Visibility::Private,
    });

    // 2. All function parameters — they inherently cross from
    //    the init call to the first resume.
    for param in params {
        fields.push(Field {
            name: param.name.clone(),
            ty: param.ty.clone(),
            vis: Visibility::Private,
        });
    }

    // 3. All lifted local variables that cross yield boundaries
    for var_info in crossing_vars {
        fields.push(Field {
            name: format!("__local_{}", var_info.var_id.0),
            ty: var_info.ty.clone(),
            vis: Visibility::Private,
        });
    }

    Item {
        id: DefId(0), // will be assigned by later passes
        name: struct_name,
        vis: Visibility::Private,
        kind: ItemKind::Struct(items::Struct {
            fields,
            generics: items::Generics::default(),
            invariants: vec![],
        }),
        span: proc_macro2::Span::call_site(),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Stage 2: Variable Rewrite Pass
// ═══════════════════════════════════════════════════════════════════

/// Rewrite lifted variables in a statement list.
///
/// Every `ExprKind::Var(var_id)` where `var_id` appears in `rewrites`
/// is replaced by `ExprKind::Field { base: Var(ctx_var_id), field }`.
///
/// `rewrites` maps VarId → field name in the state struct.
/// For parameters: VarId → param_name
/// For crossing locals: VarId → "__local_{var_id}"
pub fn rewrite_lifted_vars(
    stmts: &mut [Stmt],
    ctx_var_id: VarId,
    rewrites: &std::collections::HashMap<VarId, String>,
    state_struct_name: &str,
) {
    for stmt in stmts.iter_mut() {
        rewrite_stmt(stmt, ctx_var_id, rewrites, state_struct_name);
    }
}

fn rewrite_stmt(
    stmt: &mut Stmt,
    ctx_var_id: VarId,
    rewrites: &std::collections::HashMap<VarId, String>,
    state_struct_name: &str,
) {
    match &mut stmt.kind {
        StmtKind::Expr(expr) | StmtKind::Semi(expr) => {
            rewrite_expr(expr, ctx_var_id, rewrites, state_struct_name);
        }
        StmtKind::Local(local) => {
            if let Some(init) = &mut local.init {
                rewrite_expr(init, ctx_var_id, rewrites, state_struct_name);
            }
        }
        StmtKind::While { cond, body } => {
            rewrite_expr(cond, ctx_var_id, rewrites, state_struct_name);
            for s in &mut body.stmts { rewrite_stmt(s, ctx_var_id, rewrites, state_struct_name); }
            if let Some(v) = &mut body.value { rewrite_expr(v, ctx_var_id, rewrites, state_struct_name); }
        }
        StmtKind::For { iter, body, .. } => {
            rewrite_expr(iter, ctx_var_id, rewrites, state_struct_name);
            for s in &mut body.stmts { rewrite_stmt(s, ctx_var_id, rewrites, state_struct_name); }
            if let Some(v) = &mut body.value { rewrite_expr(v, ctx_var_id, rewrites, state_struct_name); }
        }
        StmtKind::Loop(body) => {
            for s in &mut body.stmts { rewrite_stmt(s, ctx_var_id, rewrites, state_struct_name); }
            if let Some(v) = &mut body.value { rewrite_expr(v, ctx_var_id, rewrites, state_struct_name); }
        }
        StmtKind::Return(Some(expr)) => {
            rewrite_expr(expr, ctx_var_id, rewrites, state_struct_name);
        }
        StmtKind::Assume(expr) => {
            rewrite_expr(expr, ctx_var_id, rewrites, state_struct_name);
        }
        _ => {}
    }
}

fn make_ctx_field_expr(ctx_var_id: VarId, field_name: &str, _ty: Type, span: proc_macro2::Span, state_struct_name: &str) -> ExprKind {
    ExprKind::Field {
        base: Box::new(Expr {
            kind: ExprKind::Var(ctx_var_id),
            ty: Type::Reference(
                Box::new(Type::Struct(state_struct_name.to_string())),
                true, // mutable reference
            ),
            span,
        }),
        field: field_name.to_string(),
    }
}

fn rewrite_expr(
    expr: &mut Expr,
    ctx_var_id: VarId,
    rewrites: &std::collections::HashMap<VarId, String>,
    state_struct_name: &str,
) {
    // Check if this is a Var that needs rewriting
    if let ExprKind::Var(var_id) = &expr.kind {
        if let Some(field_name) = rewrites.get(var_id) {
            expr.kind = make_ctx_field_expr(ctx_var_id, field_name, expr.ty.clone(), expr.span, state_struct_name);
            return;
        }
    }

    // Recurse into sub-expressions
    match &mut expr.kind {
        ExprKind::Binary { lhs, rhs, .. } => {
            rewrite_expr(lhs, ctx_var_id, rewrites, state_struct_name);
            rewrite_expr(rhs, ctx_var_id, rewrites, state_struct_name);
        }
        ExprKind::Unary { expr: inner, .. } => {
            rewrite_expr(inner, ctx_var_id, rewrites, state_struct_name);
        }
        ExprKind::Call { callee, args } => {
            rewrite_expr(callee, ctx_var_id, rewrites, state_struct_name);
            for arg in args { rewrite_expr(arg, ctx_var_id, rewrites, state_struct_name); }
        }
        ExprKind::If { cond, then_branch, else_branch } => {
            rewrite_expr(cond, ctx_var_id, rewrites, state_struct_name);
            for s in &mut then_branch.stmts { rewrite_stmt(s, ctx_var_id, rewrites, state_struct_name); }
            if let Some(v) = &mut then_branch.value { rewrite_expr(v, ctx_var_id, rewrites, state_struct_name); }
            if let Some(e) = else_branch { rewrite_expr(e, ctx_var_id, rewrites, state_struct_name); }
        }
        ExprKind::Block(block) => {
            for s in &mut block.stmts { rewrite_stmt(s, ctx_var_id, rewrites, state_struct_name); }
            if let Some(v) = &mut block.value { rewrite_expr(v, ctx_var_id, rewrites, state_struct_name); }
        }
        ExprKind::Field { base, .. } => {
            rewrite_expr(base, ctx_var_id, rewrites, state_struct_name);
        }
        ExprKind::Assign { lhs, rhs } => {
            rewrite_expr(lhs, ctx_var_id, rewrites, state_struct_name);
            rewrite_expr(rhs, ctx_var_id, rewrites, state_struct_name);
        }
        ExprKind::Index { base, index } => {
            rewrite_expr(base, ctx_var_id, rewrites, state_struct_name);
            rewrite_expr(index, ctx_var_id, rewrites, state_struct_name);
        }
        ExprKind::Ref(inner) => {
            rewrite_expr(inner, ctx_var_id, rewrites, state_struct_name);
        }
        ExprKind::Cast { expr: inner, .. } => {
            rewrite_expr(inner, ctx_var_id, rewrites, state_struct_name);
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            rewrite_expr(receiver, ctx_var_id, rewrites, state_struct_name);
            for arg in args { rewrite_expr(arg, ctx_var_id, rewrites, state_struct_name); }
        }
        ExprKind::StructLit { fields, .. } => {
            for (_, val) in fields { rewrite_expr(val, ctx_var_id, rewrites, state_struct_name); }
        }
        ExprKind::Yield(Some(v)) => { rewrite_expr(v, ctx_var_id, rewrites, state_struct_name); }
        ExprKind::Yield(None) => {}
        ExprKind::Return(Some(v)) => { rewrite_expr(v, ctx_var_id, rewrites, state_struct_name); }
        ExprKind::Return(None) => {}
        ExprKind::Requires(inner) | ExprKind::Ensures(inner) => {
            rewrite_expr(inner, ctx_var_id, rewrites, state_struct_name);
        }
        // Terminals: Literal, Path, Var (non-matching), UnresolvedIdent,
        // While, Loop, Break, Continue — no sub-expressions to rewrite
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════
// Stage 3: Step Function — Control Flow Splitting
// ═══════════════════════════════════════════════════════════════════

/// Split a flat statement list at yield boundaries.
///
/// Returns `N+1` segments for `N` top-level yield points.
/// Yield statements themselves are consumed (not included in segments).
///
/// Phase 8 handles the flat sequential case only — yields inside
/// nested `if`/`while`/`loop` are deferred to Phase 9.
pub fn split_at_yields(stmts: &[Stmt]) -> Vec<Vec<Stmt>> {
    let mut segments: Vec<Vec<Stmt>> = vec![vec![]];

    for stmt in stmts {
        // Check if this statement IS a yield (top-level)
        let is_top_level_yield = match &stmt.kind {
            StmtKind::Semi(expr) | StmtKind::Expr(expr) => {
                matches!(&expr.kind, ExprKind::Yield(_))
            }
            _ => false,
        };

        if is_top_level_yield {
            // Start a new segment
            segments.push(vec![]);
        } else {
            // Append to current segment
            segments.last_mut().unwrap().push(stmt.clone());
        }
    }

    segments
}

/// Generate the step function `__step_{func_name}`.
///
/// The step function takes `ctx: &mut __AsyncState_{func_name}` and returns
/// `Poll<T>` where T is the original function's return type.
///
/// The body is a chain of `if ctx.__state == i { ... }` blocks.
/// - State 0..N-2: execute segment, set `ctx.__state = next`, return Pending
/// - State N-1 (final): execute segment, set `ctx.__state = -1`, return Ready
pub fn generate_step_fn(
    func_name: &str,
    state_struct_name: &str,
    segments: &[Vec<Stmt>],
    return_type: &Type,
    ctx_var_id: VarId,
) -> Item {
    let step_fn_name = format!("__step_{}", func_name);
    let span = proc_macro2::Span::call_site();

    let mut body_stmts: Vec<Stmt> = Vec::new();
    let num_segments = segments.len();

    for (i, segment) in segments.iter().enumerate() {
        let is_final = i == num_segments - 1;

        // Build branch body: segment stmts + state transition + return
        let mut branch_stmts: Vec<Stmt> = segment.clone();

        // ctx.__state = {next} (or -1 for final)
        let next_state = if is_final { -1i64 } else { (i + 1) as i64 };
        branch_stmts.push(Stmt {
            kind: StmtKind::Semi(Expr {
                kind: ExprKind::Assign {
                    lhs: Box::new(Expr {
                        kind: ExprKind::Field {
                            base: Box::new(Expr {
                                kind: ExprKind::Var(ctx_var_id),
                                ty: Type::Reference(
                                    Box::new(Type::Struct(state_struct_name.to_string())),
                                    true,
                                ),
                                span,
                            }),
                            field: "__state".to_string(),
                        },
                        ty: Type::I64,
                        span,
                    }),
                    rhs: Box::new(Expr {
                        kind: ExprKind::Literal(crate::hir::expr::Literal::Int(next_state)),
                        ty: Type::I64,
                        span,
                    }),
                },
                ty: Type::Unit,
                span,
            }),
            span,
        });

        // return Poll::Pending or Poll::Ready
        let poll_variant = if is_final { "Ready" } else { "Pending" };
        branch_stmts.push(Stmt {
            kind: StmtKind::Return(Some(Expr {
                kind: ExprKind::StructLit {
                    name: format!("Poll::{}", poll_variant),
                    type_args: vec![],
                    fields: vec![],
                },
                ty: Type::Concrete("Poll".into(), vec![return_type.clone()]),
                span,
            })),
            span,
        });

        // if ctx.__state == i { branch_stmts }
        let cond = Expr {
            kind: ExprKind::Binary {
                op: crate::hir::expr::BinOp::Eq,
                lhs: Box::new(Expr {
                    kind: ExprKind::Field {
                        base: Box::new(Expr {
                            kind: ExprKind::Var(ctx_var_id),
                            ty: Type::Reference(
                                Box::new(Type::Struct(state_struct_name.to_string())),
                                true,
                            ),
                            span,
                        }),
                        field: "__state".to_string(),
                    },
                    ty: Type::I64,
                    span,
                }),
                rhs: Box::new(Expr {
                    kind: ExprKind::Literal(crate::hir::expr::Literal::Int(i as i64)),
                    ty: Type::I64,
                    span,
                }),
            },
            ty: Type::Bool,
            span,
        };

        let then_block = crate::hir::expr::Block {
            stmts: branch_stmts,
            value: None,
            ty: Type::Unit,
        };

        body_stmts.push(Stmt {
            kind: StmtKind::Expr(Expr {
                kind: ExprKind::If {
                    cond: Box::new(cond),
                    then_branch: then_block,
                    else_branch: None,
                },
                ty: Type::Unit,
                span,
            }),
            span,
        });
    }

    let body = crate::hir::expr::Block {
        stmts: body_stmts,
        value: None,
        ty: Type::Unit,
    };

    Item {
        id: DefId(0),
        name: step_fn_name,
        vis: Visibility::Private,
        kind: ItemKind::Fn(items::Fn {
            inputs: vec![Param {
                name: "ctx".to_string(),
                ty: Type::Reference(
                    Box::new(Type::Struct(state_struct_name.to_string())),
                    true, // mutable reference
                ),
            }],
            output: Type::Concrete("Poll".into(), vec![return_type.clone()]),
            body: Some(body),
            generics: items::Generics::default(),
            is_async: false, // the step fn itself is synchronous
        }),
        span,
    }
}

/// Top-level orchestrator: transform an async fn into a state struct + step fn.
///
/// Returns a `Vec<Item>` containing:
/// 1. The `__AsyncState_{func_name}` environment struct
/// 2. The `__step_{func_name}` step function
///
/// `next_var_id` provides the next available VarId for minting the `ctx`
/// parameter. This prevents VarId collisions with existing variables.
pub fn lower_async_fn(
    func_name: &str,
    func: &items::Fn,
    crossing_var_infos: &[VarInfo],
    next_var_id: u32,
) -> Vec<Item> {
    // 1. Synthesize environment struct
    let state_struct = generate_state_struct(func_name, &func.inputs, crossing_var_infos);
    let state_struct_name = state_struct.name.clone();

    // 2. Build rewrite map (crossing locals → __local_{id})
    let mut rewrites = std::collections::HashMap::new();
    for vi in crossing_var_infos {
        rewrites.insert(vi.var_id, format!("__local_{}", vi.var_id.0));
    }

    // 3. Mint a fresh VarId for the ctx parameter
    let ctx_var_id = VarId(next_var_id);

    // 4. Clone and rewrite the body
    let mut body_stmts = func.body.as_ref()
        .map(|b| b.stmts.clone())
        .unwrap_or_default();
    rewrite_lifted_vars(&mut body_stmts, ctx_var_id, &rewrites, &state_struct_name);

    // 5. Split at yields
    let segments = split_at_yields(&body_stmts);

    // 6. Generate step function
    let step_fn = generate_step_fn(
        func_name,
        &state_struct_name,
        &segments,
        &func.output,
        ctx_var_id,
    );

    vec![state_struct, step_fn]
}

// ═══════════════════════════════════════════════════════════════════
// Phase 9: CFG Flattening — Basic Blocks and Nested Yields
// ═══════════════════════════════════════════════════════════════════

/// A terminator ends a basic block and defines control flow edges.
#[derive(Clone, Debug)]
pub enum Terminator {
    /// Unconditional jump to another state (no yield, no return).
    Goto(usize),
    /// Conditional jump: if `cond` is true → target_true, else → target_false.
    Branch { cond: Expr, target_true: usize, target_false: usize },
    /// Yield execution. On next resume, enter `resume_state`.
    Yield { resume_state: usize },
    /// Function complete — return from coroutine.
    Return,
}

/// A straight-line sequence of statements ending with a terminator.
/// Each BasicBlock.id maps 1:1 to a `ctx.__state` discriminant.
#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub id: usize,
    pub stmts: Vec<Stmt>,
    pub terminator: Terminator,
}

/// Builds a Control Flow Graph from an AST statement list.
///
/// Only control flow structures containing yields are split into
/// multiple basic blocks. Yield-free `while`/`if`/`loop` remain
/// as monolithic HIR statements within a single block.
struct CfgBuilder {
    blocks: Vec<BasicBlock>,
    next_id: usize,
}

impl CfgBuilder {
    fn new() -> Self {
        Self { blocks: Vec::new(), next_id: 0 }
    }

    /// Allocate a new block ID without creating the block yet.
    fn alloc_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Commit a finished block.
    fn push_block(&mut self, block: BasicBlock) {
        self.blocks.push(block);
    }

    /// Build the CFG from a top-level statement list.
    /// Returns the sorted Vec of BasicBlocks.
    fn build(mut self, stmts: &[Stmt]) -> Vec<BasicBlock> {
        let entry = self.alloc_id();
        self.lower_stmts(stmts, entry, None, None);

        // Sort by id for deterministic output
        self.blocks.sort_by_key(|b| b.id);
        self.blocks
    }

    /// Lower a slice of statements starting in `current_block_id`.
    ///
    /// `exit_target`: if Some, the final block Goto's there instead of Return.
    /// This is used for loop bodies and if-branches that must converge.
    ///
    /// `assume_prefix`: if Some, inject an `Assume(expr)` statement at the
    /// very start of the first block in this scope. Used to carry branch
    /// conditions across CFG edges for Z3 invariant propagation.
    fn lower_stmts(
        &mut self,
        stmts: &[Stmt],
        mut current_id: usize,
        exit_target: Option<usize>,
        assume_prefix: Option<Expr>,
    ) {
        let mut current_stmts: Vec<Stmt> = Vec::new();

        // Inject the invariant at the top of the first block
        if let Some(assume_expr) = assume_prefix {
            current_stmts.push(Stmt {
                kind: StmtKind::Assume(assume_expr),
                span: proc_macro2::Span::call_site(),
            });
        }

        for stmt in stmts {
            // Check: is this a top-level yield?
            let is_yield = match &stmt.kind {
                StmtKind::Semi(expr) | StmtKind::Expr(expr) => {
                    matches!(&expr.kind, ExprKind::Yield(_))
                }
                _ => false,
            };

            if is_yield {
                // End current block with Yield terminator
                let resume_id = self.alloc_id();
                self.push_block(BasicBlock {
                    id: current_id,
                    stmts: std::mem::take(&mut current_stmts),
                    terminator: Terminator::Yield { resume_state: resume_id },
                });
                current_id = resume_id;
                continue;
            }

            // Check: is this a while/loop with yield inside?
            match &stmt.kind {
                StmtKind::While { cond, body } if body.stmts.iter().any(contains_yield) => {
                    // Emit current accumulated stmts as a Goto to the condition block
                    let cond_id = self.alloc_id();
                    self.push_block(BasicBlock {
                        id: current_id,
                        stmts: std::mem::take(&mut current_stmts),
                        terminator: Terminator::Goto(cond_id),
                    });

                    // Condition block: Branch { cond, body_start, exit }
                    let body_start_id = self.alloc_id();
                    let exit_id = self.alloc_id();
                    self.push_block(BasicBlock {
                        id: cond_id,
                        stmts: vec![],
                        terminator: Terminator::Branch {
                            cond: cond.clone(),
                            target_true: body_start_id,
                            target_false: exit_id,
                        },
                    });

                    // Flatten the body, looping back to cond_id.
                    // Inject Assume(cond) — inside the loop, the condition holds.
                    self.lower_stmts(
                        &body.stmts,
                        body_start_id,
                        Some(cond_id),
                        Some(cond.clone()),
                    );

                    // Continue building from exit_id.
                    // The exit block inherits Assume(NOT cond) — the loop terminated.
                    let negated_cond = Expr {
                        kind: ExprKind::Unary {
                            op: crate::hir::expr::UnOp::Not,
                            expr: Box::new(cond.clone()),
                        },
                        ty: Type::Bool,
                        span: proc_macro2::Span::call_site(),
                    };
                    current_stmts.push(Stmt {
                        kind: StmtKind::Assume(negated_cond),
                        span: proc_macro2::Span::call_site(),
                    });
                    current_id = exit_id;
                    continue;
                }

                StmtKind::Loop(body) if body.stmts.iter().any(contains_yield) => {
                    // loop { body } → unconditional entry into body, body loops back
                    let loop_start_id = self.alloc_id();
                    self.push_block(BasicBlock {
                        id: current_id,
                        stmts: std::mem::take(&mut current_stmts),
                        terminator: Terminator::Goto(loop_start_id),
                    });

                    // Flatten the body, looping back to loop_start_id
                    // No assume_prefix for unconditional loops
                    self.lower_stmts(&body.stmts, loop_start_id, Some(loop_start_id), None);

                    // A loop with yield never naturally exits (break would be needed).
                    // Allocate an unreachable exit to keep the builder consistent.
                    let exit_id = self.alloc_id();
                    current_id = exit_id;
                    continue;
                }

                StmtKind::Expr(expr) | StmtKind::Semi(expr) => {
                    if let ExprKind::If { cond, then_branch, else_branch } = &expr.kind {
                        let then_has_yield = then_branch.stmts.iter().any(contains_yield);
                        let else_has_yield = else_branch.as_ref().is_some_and(|e| {
                            if let ExprKind::Block(b) = &e.kind {
                                b.stmts.iter().any(contains_yield)
                            } else {
                                expr_contains_yield(e)
                            }
                        });

                        if then_has_yield || else_has_yield {
                            // Flatten if/else with yields
                            let then_id = self.alloc_id();
                            let else_id = self.alloc_id();
                            let join_id = self.alloc_id();

                            self.push_block(BasicBlock {
                                id: current_id,
                                stmts: std::mem::take(&mut current_stmts),
                                terminator: Terminator::Branch {
                                    cond: *cond.clone(),
                                    target_true: then_id,
                                    target_false: else_id,
                                },
                            });

                            // Negated condition for false branch
                            let negated_cond = Expr {
                                kind: ExprKind::Unary {
                                    op: crate::hir::expr::UnOp::Not,
                                    expr: cond.clone(),
                                },
                                ty: Type::Bool,
                                span: proc_macro2::Span::call_site(),
                            };

                            // Flatten then branch → join with Assume(cond)
                            self.lower_stmts(
                                &then_branch.stmts,
                                then_id,
                                Some(join_id),
                                Some(*cond.clone()),
                            );

                            // Flatten else branch → join with Assume(NOT cond)
                            if let Some(else_expr) = else_branch {
                                if let ExprKind::Block(b) = &else_expr.kind {
                                    self.lower_stmts(
                                        &b.stmts,
                                        else_id,
                                        Some(join_id),
                                        Some(negated_cond),
                                    );
                                } else {
                                    // Single-expression else: wrap in a block
                                    let else_stmt = Stmt {
                                        kind: StmtKind::Expr((**else_expr).clone()),
                                        span: else_expr.span,
                                    };
                                    self.lower_stmts(
                                        &[else_stmt],
                                        else_id,
                                        Some(join_id),
                                        Some(negated_cond),
                                    );
                                }
                            } else {
                                // No else branch → empty block that goes to join
                                // with Assume(NOT cond)
                                self.push_block(BasicBlock {
                                    id: else_id,
                                    stmts: vec![Stmt {
                                        kind: StmtKind::Assume(negated_cond),
                                        span: proc_macro2::Span::call_site(),
                                    }],
                                    terminator: Terminator::Goto(join_id),
                                });
                            }

                            current_id = join_id;
                            continue;
                        }
                    }
                }

                _ => {}
            }

            // Default: append statement to current block
            current_stmts.push(stmt.clone());
        }

        // Finalize the last block
        let terminator = match exit_target {
            Some(target) => Terminator::Goto(target),
            None => Terminator::Return,
        };
        self.push_block(BasicBlock {
            id: current_id,
            stmts: current_stmts,
            terminator,
        });
    }
}

/// Build a CFG from a statement list.
pub fn build_cfg(stmts: &[Stmt]) -> Vec<BasicBlock> {
    CfgBuilder::new().build(stmts)
}

/// Generate a step function from a CFG (basic block graph).
///
/// The body is a `loop { if ctx.__state == 0 { ... } if ctx.__state == 1 { ... } }`.
/// This trampoline pattern re-dispatches Goto/Branch transitions immediately
/// without returning to the caller. Only Yield returns Poll::Pending.
pub fn generate_step_fn_from_cfg(
    func_name: &str,
    state_struct_name: &str,
    blocks: &[BasicBlock],
    return_type: &Type,
    ctx_var_id: VarId,
) -> Item {
    let step_fn_name = format!("__step_{}", func_name);
    let span = proc_macro2::Span::call_site();

    let mut dispatch_stmts: Vec<Stmt> = Vec::new();

    for block in blocks {
        let mut branch_stmts: Vec<Stmt> = block.stmts.clone();

        // Append terminator code
        match &block.terminator {
            Terminator::Goto(target) => {
                // ctx.__state = target; continue; (trampoline re-dispatch)
                branch_stmts.push(make_state_assign(ctx_var_id, *target as i64, span, state_struct_name));
                branch_stmts.push(Stmt {
                    kind: StmtKind::Continue,
                    span,
                });
            }
            Terminator::Branch { cond, target_true, target_false } => {
                // if cond { ctx.__state = target_true; } else { ctx.__state = target_false; }
                // continue;
                let then_block = crate::hir::expr::Block {
                    stmts: vec![make_state_assign(ctx_var_id, *target_true as i64, span, state_struct_name)],
                    value: None,
                    ty: Type::Unit,
                };
                let else_block = crate::hir::expr::Block {
                    stmts: vec![make_state_assign(ctx_var_id, *target_false as i64, span, state_struct_name)],
                    value: None,
                    ty: Type::Unit,
                };
                branch_stmts.push(Stmt {
                    kind: StmtKind::Expr(Expr {
                        kind: ExprKind::If {
                            cond: Box::new(cond.clone()),
                            then_branch: then_block,
                            else_branch: Some(Box::new(Expr {
                                kind: ExprKind::Block(else_block),
                                ty: Type::Unit,
                                span,
                            })),
                        },
                        ty: Type::Unit,
                        span,
                    }),
                    span,
                });
                branch_stmts.push(Stmt {
                    kind: StmtKind::Continue,
                    span,
                });
            }
            Terminator::Yield { resume_state } => {
                // ctx.__state = resume; return Poll::Pending;
                branch_stmts.push(make_state_assign(ctx_var_id, *resume_state as i64, span, state_struct_name));
                branch_stmts.push(Stmt {
                    kind: StmtKind::Return(Some(Expr {
                        kind: ExprKind::StructLit {
                            name: "Poll::Pending".to_string(),
                            type_args: vec![],
                            fields: vec![],
                        },
                        ty: Type::Concrete("Poll".into(), vec![return_type.clone()]),
                        span,
                    })),
                    span,
                });
            }
            Terminator::Return => {
                // Extract the user's return value (e.g., `return y`) from block stmts
                // before replacing with Poll::Ready. The CFG builder keeps the original
                // StmtKind::Return in block.stmts; we extract its inner expression
                // and pack it into Poll::Ready's fields for correct payload delivery.
                let return_val = extract_return_expr(&mut branch_stmts);

                // ctx.__state = -1; return Poll::Ready(val);
                branch_stmts.push(make_state_assign(ctx_var_id, -1, span, state_struct_name));
                let ready_fields = match return_val {
                    Some(expr) => vec![("0".to_string(), expr)],
                    None => vec![],
                };
                branch_stmts.push(Stmt {
                    kind: StmtKind::Return(Some(Expr {
                        kind: ExprKind::StructLit {
                            name: "Poll::Ready".to_string(),
                            type_args: vec![],
                            fields: ready_fields,
                        },
                        ty: Type::Concrete("Poll".into(), vec![return_type.clone()]),
                        span,
                    })),
                    span,
                });
            }
        }

        // if ctx.__state == block.id { branch_stmts }
        let cond = Expr {
            kind: ExprKind::Binary {
                op: crate::hir::expr::BinOp::Eq,
                lhs: Box::new(Expr {
                    kind: ExprKind::Field {
                        base: Box::new(Expr {
                            kind: ExprKind::Var(ctx_var_id),
                            ty: Type::Reference(
                                Box::new(Type::Struct(state_struct_name.to_string())),
                                true,
                            ),
                            span,
                        }),
                        field: "__state".to_string(),
                    },
                    ty: Type::I64,
                    span,
                }),
                rhs: Box::new(Expr {
                    kind: ExprKind::Literal(crate::hir::expr::Literal::Int(block.id as i64)),
                    ty: Type::I64,
                    span,
                }),
            },
            ty: Type::Bool,
            span,
        };

        let then_block = crate::hir::expr::Block {
            stmts: branch_stmts,
            value: None,
            ty: Type::Unit,
        };

        dispatch_stmts.push(Stmt {
            kind: StmtKind::Expr(Expr {
                kind: ExprKind::If {
                    cond: Box::new(cond),
                    then_branch: then_block,
                    else_branch: None,
                },
                ty: Type::Unit,
                span,
            }),
            span,
        });
    }

    // Wrap all dispatch stmts in `loop { ... }` for the trampoline
    let loop_body = crate::hir::expr::Block {
        stmts: dispatch_stmts,
        value: None,
        ty: Type::Unit,
    };

    let body = crate::hir::expr::Block {
        stmts: vec![Stmt {
            kind: StmtKind::Loop(loop_body),
            span,
        }],
        value: None,
        ty: Type::Unit,
    };

    Item {
        id: DefId(0),
        name: step_fn_name,
        vis: Visibility::Private,
        kind: ItemKind::Fn(items::Fn {
            inputs: vec![Param {
                name: "ctx".to_string(),
                ty: Type::Reference(
                    Box::new(Type::Struct(state_struct_name.to_string())),
                    true,
                ),
            }],
            output: Type::Concrete("Poll".into(), vec![return_type.clone()]),
            body: Some(body),
            generics: items::Generics::default(),
            is_async: false,
        }),
        span,
    }
}

/// Extract the return expression from a block's statements.
///
/// The CFG builder preserves `StmtKind::Return(Some(expr))` in block stmts.
/// This function scans backwards, removes the first Return(Some) it finds,
/// and returns the inner expression so the caller can pack it into Poll::Ready.
/// Returns None for void returns or if no return statement exists.
fn extract_return_expr(stmts: &mut Vec<Stmt>) -> Option<Expr> {
    // Scan backwards since return is typically the last statement
    for i in (0..stmts.len()).rev() {
        if let StmtKind::Return(Some(_)) = &stmts[i].kind {
            if let StmtKind::Return(Some(expr)) = stmts.remove(i).kind {
                return Some(expr);
            }
        }
    }
    None
}

/// Helper: generate `ctx.__state = value;`
fn make_state_assign(ctx_var_id: VarId, value: i64, span: proc_macro2::Span, state_struct_name: &str) -> Stmt {
    Stmt {
        kind: StmtKind::Semi(Expr {
            kind: ExprKind::Assign {
                lhs: Box::new(Expr {
                    kind: ExprKind::Field {
                        base: Box::new(Expr {
                            kind: ExprKind::Var(ctx_var_id),
                            ty: Type::Reference(
                                Box::new(Type::Struct(state_struct_name.to_string())),
                                true,
                            ),
                            span,
                        }),
                        field: "__state".to_string(),
                    },
                    ty: Type::I64,
                    span,
                }),
                rhs: Box::new(Expr {
                    kind: ExprKind::Literal(crate::hir::expr::Literal::Int(value)),
                    ty: Type::I64,
                    span,
                }),
            },
            ty: Type::Unit,
            span,
        }),
        span,
    }
}

/// Top-level orchestrator using CFG-based lowering (Phase 9).
///
/// Replaces the linear `split_at_yields` + `generate_step_fn` pipeline
/// with `build_cfg` + `generate_step_fn_from_cfg`, supporting nested yields.
pub fn lower_async_fn_cfg(
    func_name: &str,
    func: &items::Fn,
    crossing_var_infos: &[VarInfo],
    next_var_id: u32,
) -> Vec<Item> {
    // 1. Synthesize environment struct
    let state_struct = generate_state_struct(func_name, &func.inputs, crossing_var_infos);
    let state_struct_name = state_struct.name.clone();

    // 2. Build rewrite map
    let mut rewrites = std::collections::HashMap::new();
    for vi in crossing_var_infos {
        rewrites.insert(vi.var_id, format!("__local_{}", vi.var_id.0));
    }

    // 3. Mint a fresh VarId for the ctx parameter
    let ctx_var_id = VarId(next_var_id);

    // 4. Clone and rewrite the body
    let mut body_stmts = func.body.as_ref()
        .map(|b| b.stmts.clone())
        .unwrap_or_default();
    rewrite_lifted_vars(&mut body_stmts, ctx_var_id, &rewrites, &state_struct_name);

    // 5. Build CFG (Phase 9 path — handles nested yields)
    let blocks = build_cfg(&body_stmts);

    // 6. Generate step function from CFG
    let step_fn = generate_step_fn_from_cfg(
        func_name,
        &state_struct_name,
        &blocks,
        &func.output,
        ctx_var_id,
    );

    vec![state_struct, step_fn]
}

/// Identifies variables that are live across yield boundaries.
///
/// A variable "crosses" a yield if it is defined before a yield point
/// and used after that yield point resumes.
pub fn find_crossing_vars(body: &[Stmt]) -> HashSet<VarId> {
    let mut crossing = HashSet::new();
    let mut defined_before_yield = HashSet::new();
    let mut seen_yield = false;

    for stmt in body {
        // Collect definitions
        if let StmtKind::Local(local) = &stmt.kind {
            if !seen_yield {
                collect_pattern_vars(&local.pat, &mut defined_before_yield);
            }
        }

        // Check for yield
        if contains_yield(stmt) {
            seen_yield = true;
        }

        // After a yield, any use of a pre-yield variable is a crossing
        if seen_yield {
            let used = collect_used_vars_in_stmt(stmt);
            for var in used {
                if defined_before_yield.contains(&var) {
                    crossing.insert(var);
                }
            }
        }
    }

    crossing
}

/// Extract VarIds from a pattern binding.
fn collect_pattern_vars(pat: &Pattern, out: &mut HashSet<VarId>) {
    match pat {
        Pattern::Bind { var_id, .. } => { out.insert(*var_id); }
        Pattern::Tuple(pats) => {
            for p in pats { collect_pattern_vars(p, out); }
        }
        Pattern::Wildcard => {}
    }
}

/// Returns true if the statement (or any sub-expression) contains a Yield.
fn contains_yield(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Expr(expr) | StmtKind::Semi(expr) => expr_contains_yield(expr),
        StmtKind::Local(local) => {
            local.init.as_ref().is_some_and(expr_contains_yield)
        }
        StmtKind::While { cond, body } => {
            expr_contains_yield(cond)
                || body.stmts.iter().any(contains_yield)
        }
        StmtKind::For { iter, body, .. } => {
            expr_contains_yield(iter)
                || body.stmts.iter().any(contains_yield)
        }
        StmtKind::Loop(body) => body.stmts.iter().any(contains_yield),
        StmtKind::Return(opt) => opt.as_ref().is_some_and(expr_contains_yield),
        StmtKind::Assume(expr) => expr_contains_yield(expr),
        _ => false,
    }
}

/// Recursively check if an expression contains a Yield node.
fn expr_contains_yield(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Yield(_) => true,
        ExprKind::Block(block) => {
            block.stmts.iter().any(contains_yield)
                || block.value.as_ref().is_some_and(|e| expr_contains_yield(e))
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            expr_contains_yield(lhs) || expr_contains_yield(rhs)
        }
        ExprKind::Unary { expr, .. } => expr_contains_yield(expr),
        ExprKind::Call { callee, args } => {
            expr_contains_yield(callee) || args.iter().any(expr_contains_yield)
        }
        ExprKind::If { cond, then_branch, else_branch } => {
            expr_contains_yield(cond)
                || then_branch.stmts.iter().any(contains_yield)
                || else_branch.as_ref().is_some_and(|e| expr_contains_yield(e))
        }
        _ => false,
    }
}

/// Collect all VarIds referenced in a statement.
fn collect_used_vars_in_stmt(stmt: &Stmt) -> Vec<VarId> {
    let mut vars = Vec::new();
    match &stmt.kind {
        StmtKind::Expr(expr) | StmtKind::Semi(expr) => collect_used_vars(expr, &mut vars),
        StmtKind::Local(local) => {
            if let Some(init) = &local.init {
                collect_used_vars(init, &mut vars);
            }
        }
        StmtKind::Assume(expr) => collect_used_vars(expr, &mut vars),
        _ => {}
    }
    vars
}

/// Collect all VarIds used in an expression.
fn collect_used_vars(expr: &Expr, out: &mut Vec<VarId>) {
    match &expr.kind {
        ExprKind::Var(id) => out.push(*id),
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_used_vars(lhs, out);
            collect_used_vars(rhs, out);
        }
        ExprKind::Unary { expr, .. } => collect_used_vars(expr, out),
        ExprKind::Call { callee, args } => {
            collect_used_vars(callee, out);
            for arg in args { collect_used_vars(arg, out); }
        }
        ExprKind::If { cond, then_branch, else_branch } => {
            collect_used_vars(cond, out);
            for s in &then_branch.stmts {
                if let StmtKind::Expr(e) | StmtKind::Semi(e) = &s.kind {
                    collect_used_vars(e, out);
                }
            }
            if let Some(e) = else_branch { collect_used_vars(e, out); }
        }
        ExprKind::Block(block) => {
            for s in &block.stmts {
                if let StmtKind::Expr(e) | StmtKind::Semi(e) = &s.kind {
                    collect_used_vars(e, out);
                }
            }
            if let Some(v) = &block.value { collect_used_vars(v, out); }
        }
        ExprKind::Yield(Some(v)) => { collect_used_vars(v, out); }
        ExprKind::Yield(None) => {}
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::ids::VarId;
    use crate::hir::expr::*;
    use crate::hir::stmt::*;
    use crate::hir::types::Type;

    fn mk_var_expr(id: u32) -> Expr {
        Expr { kind: ExprKind::Var(VarId(id)), ty: Type::I64, span: proc_macro2::Span::call_site() }
    }

    fn mk_yield_stmt() -> Stmt {
        Stmt {
            kind: StmtKind::Semi(Expr {
                kind: ExprKind::Yield(None),
                ty: Type::Unit,
                span: proc_macro2::Span::call_site(),
            }),
            span: proc_macro2::Span::call_site(),
        }
    }

    fn mk_let_stmt(id: u32) -> Stmt {
        Stmt {
            kind: StmtKind::Local(Local {
                pat: Pattern::Bind {
                    name: format!("v{}", id),
                    var_id: VarId(id),
                    mutable: false,
                },
                ty: Some(Type::I64),
                init: Some(Expr {
                    kind: ExprKind::Literal(Literal::Int(42)),
                    ty: Type::I64,
                    span: proc_macro2::Span::call_site(),
                }),
            }),
            span: proc_macro2::Span::call_site(),
        }
    }

    fn mk_use_stmt(id: u32) -> Stmt {
        Stmt {
            kind: StmtKind::Semi(mk_var_expr(id)),
            span: proc_macro2::Span::call_site(),
        }
    }

    #[test]
    fn test_is_async_flag_default() {
        // Verify Fn struct has is_async field
        let f = crate::hir::items::Fn {
            inputs: vec![],
            output: Type::Unit,
            body: None,
            generics: crate::hir::items::Generics::default(),
            is_async: false,
        };
        assert!(!f.is_async);
    }

    #[test]
    fn test_yield_typechecks_to_unit() {
        // yield; should produce Type::Unit
        let mut ctx = crate::hir::typeck::TypeckContext::new();
        let mut expr = Expr {
            kind: ExprKind::Yield(None),
            ty: Type::Unit,
            span: proc_macro2::Span::call_site(),
        };
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Unit);
    }

    #[test]
    fn test_yield_with_value_typechecks() {
        // yield(42) should also produce Type::Unit
        let mut ctx = crate::hir::typeck::TypeckContext::new();
        let mut expr = Expr {
            kind: ExprKind::Yield(Some(Box::new(Expr {
                kind: ExprKind::Literal(Literal::Int(42)),
                ty: Type::I64,
                span: proc_macro2::Span::call_site(),
            }))),
            ty: Type::Unit,
            span: proc_macro2::Span::call_site(),
        };
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Unit);
    }

    #[test]
    fn test_yield_analysis_detects_crossing() {
        // let v0 = 42;
        // yield;
        // use(v0);  <-- v0 crosses the yield boundary
        let stmts = vec![
            mk_let_stmt(0),
            mk_yield_stmt(),
            mk_use_stmt(0),
        ];

        let crossing = find_crossing_vars(&stmts);
        assert!(crossing.contains(&VarId(0)), "v0 should cross the yield boundary");
    }

    // ── Stage 1: Environment Struct Synthesis ────────────────────────

    #[test]
    fn test_state_struct_has_state_field() {
        let item = generate_state_struct("my_coro", &[], &[]);
        let fields = extract_struct_fields(&item);
        assert_eq!(fields[0].0, "__state");
        assert_eq!(fields[0].1, Type::I64);
    }

    #[test]
    fn test_state_struct_lifts_params() {
        let params = vec![
            Param { name: "x".into(), ty: Type::I64 },
            Param { name: "y".into(), ty: Type::Bool },
        ];
        let item = generate_state_struct("adder", &params, &[]);
        let fields = extract_struct_fields(&item);
        // fields[0] = __state, fields[1] = x, fields[2] = y
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[1].0, "x");
        assert_eq!(fields[1].1, Type::I64);
        assert_eq!(fields[2].0, "y");
        assert_eq!(fields[2].1, Type::Bool);
    }

    #[test]
    fn test_state_struct_lifts_crossing_vars() {
        let crossing = vec![
            VarInfo { var_id: VarId(7), name: "counter".into(), ty: Type::I64 },
            VarInfo { var_id: VarId(12), name: "flag".into(), ty: Type::Bool },
        ];
        let item = generate_state_struct("ticker", &[], &crossing);
        let fields = extract_struct_fields(&item);
        // fields[0] = __state, fields[1] = __local_7, fields[2] = __local_12
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[1].0, "__local_7");
        assert_eq!(fields[1].1, Type::I64);
        assert_eq!(fields[2].0, "__local_12");
        assert_eq!(fields[2].1, Type::Bool);
    }

    #[test]
    fn test_state_struct_name() {
        let item = generate_state_struct("poll_socket", &[], &[]);
        assert_eq!(item.name, "__AsyncState_poll_socket");
    }

    /// Helper: extract (name, type) pairs from a struct Item.
    fn extract_struct_fields(item: &Item) -> Vec<(String, Type)> {
        match &item.kind {
            ItemKind::Struct(s) => {
                s.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect()
            }
            _ => panic!("expected ItemKind::Struct"),
        }
    }

    // ── Stage 2: Variable Rewrite Pass ──────────────────────────────

    #[test]
    fn test_rewrite_var_to_field() {
        // VarId(0) is a crossing variable → should be rewritten to ctx.__local_0
        let ctx_var_id = VarId(99); // freshly minted for the step fn
        let mut rewrites = std::collections::HashMap::new();
        rewrites.insert(VarId(0), "__local_0".to_string());

        let mut stmts = vec![mk_use_stmt(0)];
        rewrite_lifted_vars(&mut stmts, ctx_var_id, &rewrites, "__AsyncState_test");

        // The expression should now be Field { base: Var(99), field: "__local_0" }
        if let StmtKind::Semi(expr) = &stmts[0].kind {
            match &expr.kind {
                ExprKind::Field { base, field } => {
                    assert_eq!(field, "__local_0");
                    match &base.kind {
                        ExprKind::Var(id) => assert_eq!(*id, VarId(99)),
                        other => panic!("expected Var(99) as base, got {:?}", other),
                    }
                }
                other => panic!("expected Field, got {:?}", other),
            }
        } else {
            panic!("expected Semi stmt");
        }
    }

    #[test]
    fn test_rewrite_preserves_non_crossing() {
        // VarId(5) is NOT in the rewrite map → should remain Var(5)
        let ctx_var_id = VarId(99);
        let mut rewrites = std::collections::HashMap::new();
        rewrites.insert(VarId(0), "__local_0".to_string());

        let mut stmts = vec![mk_use_stmt(5)]; // VarId(5) not in rewrites
        rewrite_lifted_vars(&mut stmts, ctx_var_id, &rewrites, "__AsyncState_test");

        if let StmtKind::Semi(expr) = &stmts[0].kind {
            match &expr.kind {
                ExprKind::Var(id) => assert_eq!(*id, VarId(5)),
                other => panic!("expected Var(5) preserved, got {:?}", other),
            }
        } else {
            panic!("expected Semi stmt");
        }
    }

    #[test]
    fn test_rewrite_params_to_field() {
        // Param VarId(10) named "x" → should become ctx.x
        let ctx_var_id = VarId(99);
        let mut rewrites = std::collections::HashMap::new();
        rewrites.insert(VarId(10), "x".to_string());

        let mut stmts = vec![
            Stmt {
                kind: StmtKind::Semi(Expr {
                    kind: ExprKind::Var(VarId(10)),
                    ty: Type::I64,
                    span: proc_macro2::Span::call_site(),
                }),
                span: proc_macro2::Span::call_site(),
            },
        ];
        rewrite_lifted_vars(&mut stmts, ctx_var_id, &rewrites, "__AsyncState_test");

        if let StmtKind::Semi(expr) = &stmts[0].kind {
            match &expr.kind {
                ExprKind::Field { base, field } => {
                    assert_eq!(field, "x");
                    match &base.kind {
                        ExprKind::Var(id) => assert_eq!(*id, VarId(99)),
                        other => panic!("expected Var(99), got {:?}", other),
                    }
                }
                other => panic!("expected Field, got {:?}", other),
            }
        } else {
            panic!("expected Semi stmt");
        }
    }

    // ── Stage 3: Step Function — Control Flow Splitting ─────────────

    #[test]
    fn test_split_no_yields() {
        // No yields → 1 segment containing all statements
        let stmts = vec![mk_use_stmt(0), mk_use_stmt(1)];
        let segments = split_at_yields(&stmts);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].len(), 2);
    }

    #[test]
    fn test_split_single_yield() {
        // s0; yield; s1 → 2 segments
        let stmts = vec![
            mk_use_stmt(0),
            mk_yield_stmt(),
            mk_use_stmt(1),
        ];
        let segments = split_at_yields(&stmts);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].len(), 1, "pre-yield segment");
        assert_eq!(segments[1].len(), 1, "post-yield segment");
    }

    #[test]
    fn test_split_two_yields() {
        // s0; yield; s1; yield; s2 → 3 segments
        let stmts = vec![
            mk_use_stmt(0),
            mk_yield_stmt(),
            mk_use_stmt(1),
            mk_yield_stmt(),
            mk_use_stmt(2),
        ];
        let segments = split_at_yields(&stmts);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].len(), 1);
        assert_eq!(segments[1].len(), 1);
        assert_eq!(segments[2].len(), 1);
    }

    #[test]
    fn test_step_fn_segment_count() {
        // 2 segments → step fn body has 2 if-branches
        let segments = vec![
            vec![mk_use_stmt(0)],
            vec![mk_use_stmt(1)],
        ];
        let step_fn = generate_step_fn(
            "coro",
            "__AsyncState_coro",
            &segments,
            &Type::Unit,
            VarId(99),
        );
        match &step_fn.kind {
            ItemKind::Fn(f) => {
                let body = f.body.as_ref().unwrap();
                assert_eq!(body.stmts.len(), 2, "should have 2 if-branches");
                assert_eq!(step_fn.name, "__step_coro");
                // Check parameter type
                assert_eq!(f.inputs.len(), 1);
                assert_eq!(f.inputs[0].name, "ctx");
            }
            _ => panic!("expected Fn item"),
        }
    }

    #[test]
    fn test_lower_async_fn_produces_two_items() {
        use crate::hir::expr::Block;

        // Build a minimal async fn: let v0 = 42; yield; use(v0);
        let func = items::Fn {
            inputs: vec![Param { name: "n".into(), ty: Type::I64 }],
            output: Type::I64,
            body: Some(Block {
                stmts: vec![
                    mk_let_stmt(0),
                    mk_yield_stmt(),
                    mk_use_stmt(0),
                ],
                value: None,
                ty: Type::Unit,
            }),
            generics: items::Generics::default(),
            is_async: true,
        };

        let crossing = vec![
            VarInfo { var_id: VarId(0), name: "v0".into(), ty: Type::I64 },
        ];

        let items = lower_async_fn("my_coro", &func, &crossing, 100);
        assert_eq!(items.len(), 2);

        // Item 0: the state struct
        assert_eq!(items[0].name, "__AsyncState_my_coro");
        let fields = extract_struct_fields(&items[0]);
        // __state + param "n" + __local_0
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].0, "__state");
        assert_eq!(fields[1].0, "n");
        assert_eq!(fields[2].0, "__local_0");

        // Item 1: the step function
        assert_eq!(items[1].name, "__step_my_coro");
        match &items[1].kind {
            ItemKind::Fn(f) => {
                assert!(!f.is_async, "step fn should not be async");
                assert_eq!(f.inputs[0].name, "ctx");
                // 2 segments (split at 1 yield) → 2 if-branches in body
                let body = f.body.as_ref().unwrap();
                assert_eq!(body.stmts.len(), 2);
            }
            _ => panic!("expected Fn item for step function"),
        }
    }

    // ── Phase 9: CFG Flattening — Nested Yields ────────────────────

    /// Helper: a simple boolean condition expression (x < 10)
    fn mk_cond_expr() -> Expr {
        Expr {
            kind: ExprKind::Binary {
                op: BinOp::Lt,
                lhs: Box::new(mk_var_expr(0)),
                rhs: Box::new(Expr {
                    kind: ExprKind::Literal(Literal::Int(10)),
                    ty: Type::I64,
                    span: proc_macro2::Span::call_site(),
                }),
            },
            ty: Type::Bool,
            span: proc_macro2::Span::call_site(),
        }
    }

    /// Helper: `while cond { stmts }`
    fn mk_while_stmt(cond: Expr, body_stmts: Vec<Stmt>) -> Stmt {
        Stmt {
            kind: StmtKind::While {
                cond,
                body: Block {
                    stmts: body_stmts,
                    value: None,
                    ty: Type::Unit,
                },
            },
            span: proc_macro2::Span::call_site(),
        }
    }

    /// Helper: `loop { stmts }`
    fn mk_loop_stmt(body_stmts: Vec<Stmt>) -> Stmt {
        Stmt {
            kind: StmtKind::Loop(Block {
                stmts: body_stmts,
                value: None,
                ty: Type::Unit,
            }),
            span: proc_macro2::Span::call_site(),
        }
    }

    /// Helper: `if cond { then_stmts }`
    fn mk_if_stmt(cond: Expr, then_stmts: Vec<Stmt>) -> Stmt {
        Stmt {
            kind: StmtKind::Expr(Expr {
                kind: ExprKind::If {
                    cond: Box::new(cond),
                    then_branch: Block {
                        stmts: then_stmts,
                        value: None,
                        ty: Type::Unit,
                    },
                    else_branch: None,
                },
                ty: Type::Unit,
                span: proc_macro2::Span::call_site(),
            }),
            span: proc_macro2::Span::call_site(),
        }
    }

    #[test]
    fn test_contains_yield_in_while() {
        let stmt = mk_while_stmt(mk_cond_expr(), vec![mk_yield_stmt()]);
        assert!(contains_yield(&stmt), "while containing yield must be detected");
    }

    #[test]
    fn test_contains_yield_in_loop() {
        let stmt = mk_loop_stmt(vec![mk_use_stmt(0), mk_yield_stmt()]);
        assert!(contains_yield(&stmt), "loop containing yield must be detected");
    }

    #[test]
    fn test_cfg_linear_no_yields() {
        // s0; s1; → 1 block with Return
        let stmts = vec![mk_use_stmt(0), mk_use_stmt(1)];
        let blocks = build_cfg(&stmts);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].stmts.len(), 2);
        assert!(matches!(blocks[0].terminator, Terminator::Return));
    }

    #[test]
    fn test_cfg_single_yield() {
        // s0; yield; s1; → 2 blocks (Yield + Return)
        let stmts = vec![mk_use_stmt(0), mk_yield_stmt(), mk_use_stmt(1)];
        let blocks = build_cfg(&stmts);
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0].terminator, Terminator::Yield { .. }));
        assert!(matches!(blocks[1].terminator, Terminator::Return));
    }

    #[test]
    fn test_cfg_two_yields() {
        // s0; yield; s1; yield; s2; → 3 blocks
        let stmts = vec![
            mk_use_stmt(0), mk_yield_stmt(),
            mk_use_stmt(1), mk_yield_stmt(),
            mk_use_stmt(2),
        ];
        let blocks = build_cfg(&stmts);
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[0].terminator, Terminator::Yield { .. }));
        assert!(matches!(blocks[1].terminator, Terminator::Yield { .. }));
        assert!(matches!(blocks[2].terminator, Terminator::Return));
    }

    #[test]
    fn test_cfg_while_with_yield() {
        // while x < 10 { x = x + 1; yield; }
        // Expected blocks:
        //   0: Goto(1)          — entry → condition
        //   1: Branch(cond, 2, 3) — condition check
        //   2: body pre-yield → Yield(4)
        //   3: exit → Return
        //   4: post-yield → Goto(1) (loop back)
        let body = vec![mk_use_stmt(0), mk_yield_stmt()];
        let stmts = vec![mk_while_stmt(mk_cond_expr(), body)];
        let blocks = build_cfg(&stmts);

        // Should have 5 blocks: entry(Goto), cond(Branch), body(Yield), exit(Return), resume(Goto)
        assert_eq!(blocks.len(), 5);

        // Block 0: Goto to condition
        assert!(matches!(blocks[0].terminator, Terminator::Goto(_)));

        // Block 1: Branch (condition check)
        assert!(matches!(blocks[1].terminator, Terminator::Branch { .. }));

        // Find the Yield block
        let yield_block = blocks.iter().find(|b| matches!(b.terminator, Terminator::Yield { .. }));
        assert!(yield_block.is_some(), "should have a Yield block");

        // Find the exit block (Return)
        let exit_block = blocks.iter().find(|b| matches!(b.terminator, Terminator::Return));
        assert!(exit_block.is_some(), "should have an exit Return block");
    }

    #[test]
    fn test_cfg_while_without_yield() {
        // while x < 10 { x = x + 1; } — NO yield → stays monolithic
        let body = vec![mk_use_stmt(0)];
        let stmts = vec![mk_while_stmt(mk_cond_expr(), body)];
        let blocks = build_cfg(&stmts);

        // 1 block containing the while as a regular stmt
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].stmts.len(), 1);
        assert!(matches!(blocks[0].terminator, Terminator::Return));
    }

    #[test]
    fn test_cfg_if_with_yield() {
        // if cond { yield; } — yield in then branch
        let stmts = vec![mk_if_stmt(mk_cond_expr(), vec![mk_yield_stmt()])];
        let blocks = build_cfg(&stmts);

        // Entry(Branch) → then(Yield) + else(Goto(join)) + resume(Goto(join)) + join(Return)
        assert!(blocks.len() >= 4, "if-with-yield should produce at least 4 blocks, got {}", blocks.len());

        // Should have a Branch terminator
        let branch_block = blocks.iter().find(|b| matches!(b.terminator, Terminator::Branch { .. }));
        assert!(branch_block.is_some(), "should have a Branch block");
    }

    #[test]
    fn test_step_fn_from_cfg_basic() {
        // 2-block CFG: block 0 Yield(1), block 1 Return
        let blocks = vec![
            BasicBlock { id: 0, stmts: vec![mk_use_stmt(0)], terminator: Terminator::Yield { resume_state: 1 } },
            BasicBlock { id: 1, stmts: vec![mk_use_stmt(1)], terminator: Terminator::Return },
        ];
        let step_fn = generate_step_fn_from_cfg(
            "coro", "__AsyncState_coro", &blocks, &Type::Unit, VarId(99),
        );
        match &step_fn.kind {
            ItemKind::Fn(f) => {
                let body = f.body.as_ref().unwrap();
                // Body should be a single Loop stmt (the trampoline)
                assert_eq!(body.stmts.len(), 1);
                assert!(matches!(body.stmts[0].kind, StmtKind::Loop(_)),
                    "step fn body should be a trampoline loop");
                // Inside the loop: 2 if-branches (one per block)
                if let StmtKind::Loop(loop_body) = &body.stmts[0].kind {
                    assert_eq!(loop_body.stmts.len(), 2, "should have 2 if-branches in trampoline");
                }
            }
            _ => panic!("expected Fn item"),
        }
    }

    #[test]
    fn test_lower_async_fn_with_while_yield() {
        use crate::hir::expr::Block;

        // async fn ticker() { let v0 = 42; while cond { yield; } use(v0); }
        let func = items::Fn {
            inputs: vec![],
            output: Type::Unit,
            body: Some(Block {
                stmts: vec![
                    mk_let_stmt(0),
                    mk_while_stmt(mk_cond_expr(), vec![mk_yield_stmt()]),
                    mk_use_stmt(0),
                ],
                value: None,
                ty: Type::Unit,
            }),
            generics: items::Generics::default(),
            is_async: true,
        };

        let crossing = vec![
            VarInfo { var_id: VarId(0), name: "v0".into(), ty: Type::I64 },
        ];

        let items = lower_async_fn_cfg("ticker", &func, &crossing, 100);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "__AsyncState_ticker");
        assert_eq!(items[1].name, "__step_ticker");

        // The step fn should have more than 2 blocks (while produces extra blocks)
        match &items[1].kind {
            ItemKind::Fn(f) => {
                let body = f.body.as_ref().unwrap();
                // Trampoline loop
                assert_eq!(body.stmts.len(), 1);
                if let StmtKind::Loop(loop_body) = &body.stmts[0].kind {
                    // while-with-yield: entry + cond + body + exit + resume = 5+ blocks
                    assert!(loop_body.stmts.len() >= 4,
                        "while-yield should produce 4+ if-branches, got {}",
                        loop_body.stmts.len());
                }
            }
            _ => panic!("expected Fn item"),
        }
    }

    // ── Phase 10 (P2): State Invariant Engine — CFG Injection Tests ──────

    #[test]
    fn test_cfg_while_injects_assume_in_body() {
        // while x < 10 { yield; }
        // The body block should start with Assume(x < 10)
        let body = vec![mk_yield_stmt()];
        let stmts = vec![mk_while_stmt(mk_cond_expr(), body)];
        let blocks = build_cfg(&stmts);

        // Find the body block (the one with a Yield terminator)
        let yield_block = blocks.iter()
            .find(|b| matches!(b.terminator, Terminator::Yield { .. }))
            .expect("should have a Yield block");

        // It should start with an Assume stmt
        assert!(!yield_block.stmts.is_empty(), "body block should have stmts");
        assert!(matches!(yield_block.stmts[0].kind, StmtKind::Assume(_)),
            "body block should start with Assume(cond), got {:?}", yield_block.stmts[0].kind);
    }

    #[test]
    fn test_cfg_while_injects_negated_assume_at_exit() {
        // while x < 10 { yield; }; use(x);
        // The exit block (Return) should contain Assume(NOT(x < 10))
        let body = vec![mk_yield_stmt()];
        let stmts = vec![mk_while_stmt(mk_cond_expr(), body), mk_use_stmt(0)];
        let blocks = build_cfg(&stmts);

        // Find the exit block (Return terminator, with stmts)
        let exit_block = blocks.iter()
            .find(|b| matches!(b.terminator, Terminator::Return) && !b.stmts.is_empty())
            .expect("should have an exit block with stmts");

        // First stmt should be Assume(NOT cond)
        assert!(matches!(exit_block.stmts[0].kind, StmtKind::Assume(_)),
            "exit block should start with Assume(NOT cond), got {:?}", exit_block.stmts[0].kind);

        // Verify it's a negation (Unary Not)
        if let StmtKind::Assume(ref expr) = exit_block.stmts[0].kind {
            assert!(matches!(expr.kind, ExprKind::Unary { op: crate::hir::expr::UnOp::Not, .. }),
                "exit Assume should wrap NOT, got {:?}", expr.kind);
        }
    }

    #[test]
    fn test_cfg_if_injects_assume_in_then() {
        // if x < 10 { yield; }
        // The then block should start with Assume(x < 10)
        let stmts = vec![mk_if_stmt(mk_cond_expr(), vec![mk_yield_stmt()])];
        let blocks = build_cfg(&stmts);

        // Find the block with a Yield terminator (the then branch)
        let yield_block = blocks.iter()
            .find(|b| matches!(b.terminator, Terminator::Yield { .. }))
            .expect("should have a Yield block");

        assert!(!yield_block.stmts.is_empty(), "then block should have stmts");
        assert!(matches!(yield_block.stmts[0].kind, StmtKind::Assume(_)),
            "then block should start with Assume(cond)");
    }

    #[test]
    fn test_cfg_if_injects_negated_assume_in_else() {
        // if x < 10 { yield; }
        // The else block (no user-written else → empty) should have Assume(NOT cond)
        let stmts = vec![mk_if_stmt(mk_cond_expr(), vec![mk_yield_stmt()])];
        let blocks = build_cfg(&stmts);

        // The else block: no yield, Goto terminator, contains Assume
        let else_block = blocks.iter()
            .find(|b| matches!(b.terminator, Terminator::Goto(_))
                && b.stmts.iter().any(|s| matches!(s.kind, StmtKind::Assume(_))))
            .expect("should have an else block with Assume(NOT cond)");

        if let StmtKind::Assume(ref expr) = else_block.stmts[0].kind {
            assert!(matches!(expr.kind, ExprKind::Unary { op: crate::hir::expr::UnOp::Not, .. }),
                "else Assume should wrap NOT");
        }
    }

    #[test]
    fn test_invariant_across_yield() {
        // End-to-end: while x < 10 { yield; }
        // After CFG build, the body's Assume(x < 10) should be provable by Z3.
        // This is the defining test: Z3 can read the CFG across a yield boundary.
        let body = vec![mk_yield_stmt()];
        let stmts = vec![mk_while_stmt(mk_cond_expr(), body)];
        let blocks = build_cfg(&stmts);

        // Extract the Assume expression from the body block
        let yield_block = blocks.iter()
            .find(|b| matches!(b.terminator, Terminator::Yield { .. }))
            .unwrap();
        let assume_expr = match &yield_block.stmts[0].kind {
            StmtKind::Assume(expr) => expr.clone(),
            other => panic!("expected Assume, got {:?}", other),
        };

        // Now feed it to Z3
        use crate::z3_shim::{Config as Z3Config, Context as Z3Context, ast::Int};
        let z3_cfg = Z3Config::new();
        let z3_ctx = Z3Context::new(&z3_cfg);
        let mut vc = crate::hir::vc::VerificationContext::new(&z3_ctx);

        // Declare x as VarId(0) — matching mk_cond_expr which uses mk_var_expr(0)
        let _x = vc.declare_symbolic(VarId(0));

        // Translate the Assume expression to Z3
        let z3_assume = vc.lower_assume_expr(&assume_expr)
            .expect("should translate x < 10 to Z3");

        // Inject it as ground truth
        vc.assume_condition(&z3_assume);

        // Now prove that x < 20 — trivially true if x < 10
        let x = vc.get_symbolic(VarId(0)).unwrap().clone();
        let twenty = Int::from_i64(&z3_ctx, 20);
        let result = vc.prove_requires(&x.lt(&twenty));
        assert!(result.is_ok(),
            "Z3 should prove x < 20 given Assume(x < 10) from CFG edge");

        // And prove that x < 5 is NOT provable — x could be 8
        let five = Int::from_i64(&z3_ctx, 5);
        let result_fail = vc.prove_requires(&x.lt(&five));
        assert!(result_fail.is_err(),
            "Z3 should NOT prove x < 5 from Assume(x < 10)");
    }

    // ── extract_return_expr tests ─────────────────────────────────────

    #[test]
    fn test_extract_return_expr_finds_return() {
        // Given stmts containing a Return(Some(expr)), extract_return_expr
        // must remove it and return the inner expression.
        let span = proc_macro2::Span::call_site();
        let return_expr = Expr {
            kind: ExprKind::Var(VarId(42)),
            ty: Type::I64,
            span,
        };
        let mut stmts = vec![
            Stmt { kind: StmtKind::Semi(Expr {
                kind: ExprKind::Var(VarId(0)),
                ty: Type::I64,
                span,
            }), span },
            Stmt { kind: StmtKind::Return(Some(return_expr.clone())), span },
        ];

        let result = extract_return_expr(&mut stmts);
        assert!(result.is_some(), "Should extract the return expression");
        // The Return stmt should have been removed
        assert_eq!(stmts.len(), 1, "Return stmt should be removed from stmts");
        // The extracted expression should match
        if let ExprKind::Var(var_id) = &result.unwrap().kind {
            assert_eq!(*var_id, VarId(42));
        } else {
            panic!("Extracted expression should be Var(42)");
        }
    }

    #[test]
    fn test_extract_return_expr_none_when_no_return() {
        // When stmts contain no Return, extract_return_expr returns None
        // and leaves stmts unchanged.
        let span = proc_macro2::Span::call_site();
        let mut stmts = vec![
            Stmt { kind: StmtKind::Semi(Expr {
                kind: ExprKind::Var(VarId(0)),
                ty: Type::I64,
                span,
            }), span },
            Stmt { kind: StmtKind::Continue, span },
        ];

        let result = extract_return_expr(&mut stmts);
        assert!(result.is_none(), "Should return None when no Return exists");
        assert_eq!(stmts.len(), 2, "Stmts should be unchanged");
    }

    #[test]
    fn test_step_fn_packs_return_into_poll_ready() {
        // The Return block in generate_step_fn_from_cfg must pack the
        // user's return value into Poll::Ready's fields, not leave them empty.
        let span = proc_macro2::Span::call_site();
        let ctx_var_id = VarId(100);

        // A single block: `return Var(1);` with Terminator::Return
        let blocks = vec![
            BasicBlock {
                id: 0,
                stmts: vec![
                    Stmt {
                        kind: StmtKind::Return(Some(Expr {
                            kind: ExprKind::Var(VarId(1)),
                            ty: Type::I64,
                            span,
                        })),
                        span,
                    },
                ],
                terminator: Terminator::Return,
            },
        ];

        let step_item = generate_step_fn_from_cfg(
            "my_fn",
            "__AsyncState_my_fn",
            &blocks,
            &Type::I64,
            ctx_var_id,
        );

        // Extract the step function body
        if let ItemKind::Fn(ref f) = step_item.kind {
            let body = f.body.as_ref().expect("step fn must have body");
            // Find the Poll::Ready StructLit in the generated body
            let mlir_str = format!("{:?}", body);
            assert!(mlir_str.contains("Poll::Ready"),
                "Step fn body must contain Poll::Ready");
            // The fields must NOT be empty — they must contain the return value
            // Walk the AST to find the StructLit
            fn find_poll_ready_fields(stmts: &[Stmt]) -> Option<bool> {
                for stmt in stmts {
                    match &stmt.kind {
                        StmtKind::Return(Some(expr)) => {
                            if let ExprKind::StructLit { name, fields, .. } = &expr.kind {
                                if name == "Poll::Ready" {
                                    return Some(!fields.is_empty());
                                }
                            }
                        }
                        StmtKind::Expr(expr) | StmtKind::Semi(expr) => {
                            if let ExprKind::If { then_branch, else_branch, .. } = &expr.kind {
                                if let Some(result) = find_poll_ready_fields(&then_branch.stmts) {
                                    return Some(result);
                                }
                                if let Some(else_expr) = else_branch {
                                    if let ExprKind::Block(b) = &else_expr.kind {
                                        if let Some(result) = find_poll_ready_fields(&b.stmts) {
                                            return Some(result);
                                        }
                                    }
                                }
                            }
                        }
                        StmtKind::Loop(body) => {
                            if let Some(result) = find_poll_ready_fields(&body.stmts) {
                                return Some(result);
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            let has_payload = find_poll_ready_fields(&body.stmts)
                .expect("Must find Poll::Ready in step fn body");
            assert!(has_payload,
                "Poll::Ready must have non-empty fields (the return value)");
        } else {
            panic!("generate_step_fn_from_cfg must return Item::Fn");
        }
    }
}


