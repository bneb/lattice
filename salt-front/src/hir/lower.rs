//! AST → HIR Lowering
//!
//! Converts the parsed Salt AST (`grammar::SaltFile`) into a typed HIR representation.
//! This is the first phase of semantic analysis: structural lowering without type inference.
//!
//! ## Design Principles
//!
//! 1. **Faithful Translation**: Every AST construct maps to exactly one HIR construct.
//! 2. **Type Bridging**: Uses `Type::from_syn_with_generics` for SynType → Type conversion.
//! 3. **DefId Assignment**: Each top-level item gets a unique `DefId` for later resolution.
//! 4. **No Inference**: Types that cannot be resolved from syntax alone are left as `Type::Unit`
//!    or `Type::Generic(...)` for the type-checking phase to resolve.

use std::collections::HashSet;

use crate::grammar::{self, SaltFile, SynType};
use crate::hir::ids::{DefId, VarId};
use crate::hir::items::*;
use crate::hir::expr::{Expr, ExprKind, Block, Literal, BinOp, UnOp};
use crate::hir::stmt::{Stmt, StmtKind, Local, Pattern};
use crate::hir::scope::ScopeStack;
use crate::types::Type;

/// Context for AST → HIR lowering.
/// Tracks DefId allocation, VarId allocation, and lexical scopes.
pub struct LoweringContext {
    next_def_id: u32,
    next_var_id: u32,
    scopes: ScopeStack,
    /// Generic names in scope for the current function/item.
    current_generic_names: HashSet<String>,
    pub var_name_map: std::collections::HashMap<String, VarId>,
}

impl Default for LoweringContext {
    fn default() -> Self {
        Self::new()
    }
}

impl LoweringContext {
    pub fn new() -> Self {
        Self {
            next_def_id: 0,
            next_var_id: 0,
            scopes: ScopeStack::new(),
            current_generic_names: HashSet::new(),
            var_name_map: std::collections::HashMap::new(),
        }
    }

    /// Allocate a fresh DefId.
    pub fn alloc_def_id(&mut self) -> DefId {
        let id = DefId(self.next_def_id);
        self.next_def_id += 1;
        id
    }

    /// Allocate a fresh VarId for a local variable.
    pub fn alloc_var_id(&mut self) -> VarId {
        let id = VarId(self.next_var_id);
        self.next_var_id += 1;
        id
    }

    /// Lower an entire SaltFile into a vector of HIR Items.
    pub fn lower_file(&mut self, file: &SaltFile) -> Vec<Item> {
        file.items.iter().filter_map(|item| self.lower_item(item)).collect()
    }

    /// Lower a single AST Item into an HIR Item.
    pub fn lower_item(&mut self, item: &grammar::Item) -> Option<Item> {
        match item {
            grammar::Item::Fn(f) => self.lower_fn_item(f),
            grammar::Item::Struct(s) => self.lower_struct_item(s),
            grammar::Item::Enum(e) => self.lower_enum_item(e),
            grammar::Item::Trait(t) => self.lower_trait_item(t),
            grammar::Item::Impl(i) => self.lower_impl_item(i),
            grammar::Item::ExternFn(e) => self.lower_extern_fn_item(e),
            grammar::Item::Global(g) => self.lower_global_item(g),
            grammar::Item::Const(c) => self.lower_const_item(c),
            grammar::Item::Concept(_) => None, // Concepts are legacy; skip in HIR
        }
    }

    /// Lower a SaltFn to an HIR Item::Fn.
    fn lower_fn_item(&mut self, f: &grammar::SaltFn) -> Option<Item> {
        let id = self.alloc_def_id();
        let name = f.name.to_string();
        let generic_names = self.collect_generic_names(&f.generics);

        let inputs: Vec<Param> = f.args.iter().map(|arg| {
            let arg_name = arg.name.to_string();
            let ty = arg.ty.as_ref()
                .and_then(|t| Type::from_syn_with_generics(t, &generic_names))
                .unwrap_or(Type::SelfType); // &self args have no explicit type
            Param { name: arg_name, ty }
        }).collect();

        let output = f.ret_type.as_ref()
            .and_then(|t| Type::from_syn_with_generics(t, &generic_names))
            .unwrap_or(Type::Unit);

        let generics = self.lower_generics(&f.generics, &generic_names);

        let vis = if f.is_pub { Visibility::Public } else { Visibility::Private };

        // Body lowering: set up scopes and lower the function body
        self.current_generic_names = generic_names.clone();
        self.scopes = ScopeStack::new();
        self.next_var_id = 0;

        // Bind function arguments into the root scope
        for input in &inputs {
            let var_id = self.alloc_var_id();
            self.scopes.insert(input.name.clone(), var_id);
            self.var_name_map.insert(input.name.clone(), var_id);
        }

        let body = self.lower_salt_block(&f.body);

        Some(Item {
            id,
            name,
            vis,
            kind: ItemKind::Fn(Fn {
                inputs,
                output,
                body: Some(body),
                generics,
                is_async: false,
            }),
            span: f.name.span(),
        })
    }

    /// Lower a StructDef to an HIR Item::Struct.
    fn lower_struct_item(&mut self, s: &grammar::StructDef) -> Option<Item> {
        let id = self.alloc_def_id();
        let name = s.name.to_string();
        let generic_names = self.collect_generic_names(&s.generics);

        let fields: Vec<Field> = s.fields.iter().map(|f| {
            let ty = Type::from_syn_with_generics(&f.ty, &generic_names)
                .unwrap_or(Type::Unit);
            Field {
                name: f.name.to_string(),
                ty,
                vis: Visibility::Public, // Salt structs have public fields by default
            }
        }).collect();

        let generics = self.lower_generics(&s.generics, &generic_names);

        Some(Item {
            id,
            name,
            vis: Visibility::Public,
            kind: ItemKind::Struct(Struct {
                fields,
                generics,
                invariants: s.invariants.iter()
                    .map(|e| self.lower_syn_expr(e))
                    .collect(),
            }),
            span: s.name.span(),
        })
    }

    /// Lower an EnumDef to an HIR Item::Enum.
    fn lower_enum_item(&mut self, e: &grammar::EnumDef) -> Option<Item> {
        let id = self.alloc_def_id();
        let name = e.name.to_string();
        let generic_names = self.collect_generic_names(&e.generics);

        let variants: Vec<Variant> = e.variants.iter().map(|v| {
            let data = if v.tys.is_empty() {
                VariantData::Unit
            } else {
                let resolved: Vec<Type> = v.tys.iter().map(|ty| {
                    Type::from_syn_with_generics(ty, &generic_names).unwrap_or(Type::Unit)
                }).collect();
                VariantData::Tuple(resolved)
            };
            Variant {
                name: v.name.to_string(),
                data,
            }
        }).collect();

        let generics = self.lower_generics(&e.generics, &generic_names);

        Some(Item {
            id,
            name,
            vis: Visibility::Public,
            kind: ItemKind::Enum(Enum { variants, generics }),
            span: e.name.span(),
        })
    }

    /// Lower a SaltTrait to an HIR Item::Trait.
    fn lower_trait_item(&mut self, t: &grammar::SaltTrait) -> Option<Item> {
        let id = self.alloc_def_id();
        let name = t.name.to_string();
        let generic_names = self.collect_generic_names(&t.generics);

        let items: Vec<TraitItem> = t.methods.iter().map(|m| {
            let method_generic_names = {
                let mut names = generic_names.clone();
                if let Some(g) = &m.generics {
                    for p in &g.params {
                        match p {
                            grammar::GenericParam::Type { name, .. } => { names.insert(name.to_string()); }
                            grammar::GenericParam::Const { name, .. } => { names.insert(name.to_string()); }
                        }
                    }
                }
                names
            };

            let inputs: Vec<Param> = m.args.iter().map(|arg| {
                let arg_name = arg.name.to_string();
                let ty = arg.ty.as_ref()
                    .and_then(|t| Type::from_syn_with_generics(t, &method_generic_names))
                    .unwrap_or(Type::SelfType);
                Param { name: arg_name, ty }
            }).collect();

            let output = m.ret_type.as_ref()
                .and_then(|t| Type::from_syn_with_generics(t, &method_generic_names))
                .unwrap_or(Type::Unit);

            let method_generics = self.lower_generics(&m.generics, &method_generic_names);

            TraitItem::Fn {
                name: m.name.to_string(),
                func: Fn {
                    inputs,
                    output,
                    body: None,
                    generics: method_generics,
                    is_async: false,
                },
            }
        }).collect();

        let generics = self.lower_generics(&t.generics, &generic_names);

        Some(Item {
            id,
            name,
            vis: Visibility::Public,
            kind: ItemKind::Trait(Trait { generics, items }),
            span: t.name.span(),
        })
    }

    /// Lower a SaltImpl to an HIR Item::Impl.
    fn lower_impl_item(&mut self, i: &grammar::SaltImpl) -> Option<Item> {
        let id = self.alloc_def_id();
        match i {
            grammar::SaltImpl::Methods { target_ty, methods, generics } => {
                let generic_names = self.collect_generic_names(generics);
                let self_ty = Type::from_syn_with_generics(target_ty, &generic_names)
                    .unwrap_or(Type::Unit);

                let items: Vec<ImplItem> = methods.iter().map(|m| {
                    let method_generic_names = {
                        let mut names = generic_names.clone();
                        names.extend(self.collect_generic_names(&m.generics));
                        names
                    };
                    let inputs: Vec<Param> = m.args.iter().map(|arg| {
                        let arg_name = arg.name.to_string();
                        let ty = arg.ty.as_ref()
                            .and_then(|t| Type::from_syn_with_generics(t, &method_generic_names))
                            .unwrap_or(Type::SelfType);
                        Param { name: arg_name, ty }
                    }).collect();
                    let output = m.ret_type.as_ref()
                        .and_then(|t| Type::from_syn_with_generics(t, &method_generic_names))
                        .unwrap_or(Type::Unit);
                    let method_generics = self.lower_generics(&m.generics, &method_generic_names);

                    ImplItem::Fn { name: m.name.to_string(), func: Fn {
                        inputs,
                        output,
                        body: None,
                        generics: method_generics,
                        is_async: false,
                    }}
                }).collect();

                let impl_generics = self.lower_generics(generics, &generic_names);

                Some(Item {
                    id,
                    name: String::new(), // Impl blocks don't have names
                    vis: Visibility::Public,
                    kind: ItemKind::Impl(Impl {
                        generics: impl_generics,
                        trait_ref: None,
                        self_ty,
                        items,
                    }),
                    span: proc_macro2::Span::call_site(),
                })
            }
            grammar::SaltImpl::Trait { trait_name, target_ty, methods, generics } => {
                let generic_names = self.collect_generic_names(generics);
                let self_ty = Type::from_syn_with_generics(target_ty, &generic_names)
                    .unwrap_or(Type::Unit);
                let trait_ty = Type::Struct(trait_name.to_string());

                let items: Vec<ImplItem> = methods.iter().map(|m| {
                    let method_generic_names = {
                        let mut names = generic_names.clone();
                        names.extend(self.collect_generic_names(&m.generics));
                        names
                    };
                    let inputs: Vec<Param> = m.args.iter().map(|arg| {
                        let arg_name = arg.name.to_string();
                        let ty = arg.ty.as_ref()
                            .and_then(|t| Type::from_syn_with_generics(t, &method_generic_names))
                            .unwrap_or(Type::SelfType);
                        Param { name: arg_name, ty }
                    }).collect();
                    let output = m.ret_type.as_ref()
                        .and_then(|t| Type::from_syn_with_generics(t, &method_generic_names))
                        .unwrap_or(Type::Unit);
                    let method_generics = self.lower_generics(&m.generics, &method_generic_names);

                    ImplItem::Fn { name: m.name.to_string(), func: Fn {
                        inputs,
                        output,
                        body: None,
                        generics: method_generics,
                        is_async: false,
                    }}
                }).collect();

                let impl_generics = self.lower_generics(generics, &generic_names);

                Some(Item {
                    id,
                    name: trait_name.to_string(),
                    vis: Visibility::Public,
                    kind: ItemKind::Impl(Impl {
                        generics: impl_generics,
                        trait_ref: Some(trait_ty),
                        self_ty,
                        items,
                    }),
                    span: trait_name.span(),
                })
            }
            grammar::SaltImpl::Concept { .. } => None, // Legacy concept impls skipped
        }
    }

    /// Lower an ExternFnDecl to an HIR Item::Fn (with no body).
    fn lower_extern_fn_item(&mut self, e: &grammar::ExternFnDecl) -> Option<Item> {
        let id = self.alloc_def_id();
        let name = e.name.to_string();
        let generic_names = HashSet::new(); // Extern fns typically have no generics

        let inputs: Vec<Param> = e.args.iter().map(|arg| {
            let arg_name = arg.name.to_string();
            let ty = arg.ty.as_ref()
                .and_then(|t| Type::from_syn_with_generics(t, &generic_names))
                .unwrap_or(Type::Unit);
            Param { name: arg_name, ty }
        }).collect();

        let output = e.ret_type.as_ref()
            .and_then(|t| Type::from_syn_with_generics(t, &generic_names))
            .unwrap_or(Type::Unit);

        let vis = if e.is_pub { Visibility::Public } else { Visibility::Private };

        Some(Item {
            id,
            name,
            vis,
            kind: ItemKind::Fn(Fn {
                inputs,
                output,
                body: None,
                generics: Generics::default(),
                is_async: false,
            }),
            span: e.name.span(),
        })
    }

    /// Lower a GlobalDef to an HIR Item::Global.
    fn lower_global_item(&mut self, g: &grammar::GlobalDef) -> Option<Item> {
        let id = self.alloc_def_id();
        let name = g.name.to_string();
        let generic_names = HashSet::new();
        let ty = Type::from_syn_with_generics(&g.ty, &generic_names)
            .unwrap_or(Type::Unit);

        Some(Item {
            id,
            name,
            vis: Visibility::Public,
            kind: ItemKind::Global(Global { ty, init: None }),
            span: g.name.span(),
        })
    }

    /// Lower a ConstDef to an HIR Item::Global (constants are globals with known values).
    fn lower_const_item(&mut self, c: &grammar::ConstDef) -> Option<Item> {
        let id = self.alloc_def_id();
        let name = c.name.to_string();
        let generic_names = HashSet::new();
        let ty = Type::from_syn_with_generics(&c.ty, &generic_names)
            .unwrap_or(Type::Unit);

        Some(Item {
            id,
            name,
            vis: Visibility::Public,
            kind: ItemKind::Global(Global { ty, init: None }),
            span: c.name.span(),
        })
    }

    // ─── Helpers ──────────────────────────────────────────────────────────

    /// Collect the set of generic parameter names from an optional Generics AST node.
    fn collect_generic_names(&self, generics: &Option<grammar::Generics>) -> HashSet<String> {
        let mut names = HashSet::new();
        if let Some(g) = generics {
            for p in &g.params {
                match p {
                    grammar::GenericParam::Type { name, .. } => { names.insert(name.to_string()); }
                    grammar::GenericParam::Const { name, .. } => { names.insert(name.to_string()); }
                }
            }
        }
        names
    }

    /// Lower an optional Generics AST node into the HIR Generics representation.
    fn lower_generics(&self, generics: &Option<grammar::Generics>, generic_names: &HashSet<String>) -> Generics {
        match generics {
            None => Generics::default(),
            Some(g) => {
                let params = g.params.iter().map(|p| {
                    match p {
                        grammar::GenericParam::Type { name, .. } => {
                            GenericParam::Type(name.to_string())
                        }
                        grammar::GenericParam::Const { name, ty } => {
                            let resolved_ty = Type::from_syn_with_generics(ty.as_ref(), generic_names)
                                .unwrap_or(Type::Unit);
                            GenericParam::Const(name.to_string(), resolved_ty)
                        }
                    }
                }).collect();
                Generics { params }
            }
        }
    }

    // ─── Body Lowering ────────────────────────────────────────────────────

    /// Lower a SaltBlock to an HIR Block.
    pub fn lower_salt_block(&mut self, block: &grammar::SaltBlock) -> Block {
        self.scopes.push_scope();
        let mut stmts = Vec::new();
        for stmt in &block.stmts {
            if let Some(s) = self.lower_salt_stmt(stmt) {
                stmts.push(s);
            }
        }
        self.scopes.pop_scope();
        Block { stmts, value: None, ty: Type::Unit }
    }

    /// Lower a Salt AST Stmt to an HIR Stmt.
    fn lower_salt_stmt(&mut self, stmt: &grammar::Stmt) -> Option<Stmt> {
        let span = proc_macro2::Span::call_site();
        match stmt {
            grammar::Stmt::Syn(syn_stmt) => self.lower_syn_stmt(syn_stmt),
            grammar::Stmt::Expr(expr, has_semi) => {
                let hir_expr = self.lower_syn_expr(expr);
                match hir_expr.kind {
                    ExprKind::Return(opt_ret) => {
                        Some(Stmt { kind: StmtKind::Return(opt_ret.map(|e| *e)), span })
                    }
                    _ => {
                        let kind = if *has_semi {
                            StmtKind::Semi(hir_expr)
                        } else {
                            StmtKind::Expr(hir_expr)
                        };
                        Some(Stmt { kind, span })
                    }
                }
            }
            grammar::Stmt::Return(opt_expr) => {
                let hir_expr = opt_expr.as_ref().map(|e| self.lower_syn_expr(e));
                Some(Stmt { kind: StmtKind::Return(hir_expr), span })
            }
            grammar::Stmt::If(salt_if) => {
                let hir_expr = self.lower_salt_if(salt_if);
                Some(Stmt { kind: StmtKind::Expr(hir_expr), span })
            }
            grammar::Stmt::While(w) => {
                let cond = self.lower_syn_expr(&w.cond);
                let body = self.lower_salt_block(&w.body);
                Some(Stmt { kind: StmtKind::While { cond, body }, span })
            }
            grammar::Stmt::Loop(block) => {
                let body = self.lower_salt_block(block);
                Some(Stmt { kind: StmtKind::Loop(body), span })
            }
            grammar::Stmt::Break => Some(Stmt { kind: StmtKind::Break, span }),
            grammar::Stmt::Continue => Some(Stmt { kind: StmtKind::Continue, span }),
            grammar::Stmt::For(salt_for) => {
                let var_name = Self::extract_pat_name(&salt_for.pat);
                let var_id = self.alloc_var_id();
                self.scopes.insert(var_name.clone(), var_id);
                self.var_name_map.insert(var_name.clone(), var_id);
                let iter_expr = self.lower_syn_expr(&salt_for.iter);
                let body = self.lower_salt_block(&salt_for.body);
                Some(Stmt {
                    kind: StmtKind::For { var: var_id, var_name, iter: iter_expr, body },
                    span,
                })
            }
            // Match, LetElse, Move, MapWindow, WithRegion, Unsafe, Invariant
            // are lowered as expression stubs for now
            _ => {
                // Fallback: emit as a unit expression
                Some(Stmt {
                    kind: StmtKind::Expr(Expr {
                        kind: ExprKind::Literal(Literal::Bool(false)),
                        ty: Type::Unit,
                        span,
                    }),
                    span,
                })
            }
        }
    }

    /// Lower a syn::Stmt (used for `let` bindings) to an HIR Stmt.
    fn lower_syn_stmt(&mut self, stmt: &syn::Stmt) -> Option<Stmt> {
        let span = proc_macro2::Span::call_site();
        match stmt {
            syn::Stmt::Local(local) => {
                // 1. Lower the init expression FIRST (before binding the name)
                let init = local.init.as_ref().map(|init| self.lower_syn_expr(&init.expr));

                // 2. Extract the binding name
                let name = Self::extract_syn_pat_name(&local.pat);

                // 3. Extract optional type annotation
                let ty_ann = local.pat.clone();
                let ty = Self::extract_type_annotation(&ty_ann, &self.current_generic_names);

                // 4. Generate a unique VarId and bind in current scope
                let var_id = self.alloc_var_id();
                self.scopes.insert(name.clone(), var_id);

                // 5. Emit the HIR Local
                Some(Stmt {
                    kind: StmtKind::Local(Local {
                        pat: Pattern::Bind { name, var_id, mutable: Self::is_mutable_pat(&local.pat) },
                        ty,
                        init,
                    }),
                    span,
                })
            }
            syn::Stmt::Expr(expr, _semi) => {
                let hir_expr = self.lower_syn_expr(expr);
                match hir_expr.kind {
                    ExprKind::Return(opt_ret) => {
                        Some(Stmt { kind: StmtKind::Return(opt_ret.map(|e| *e)), span })
                    }
                    _ => {
                        Some(Stmt { kind: StmtKind::Semi(hir_expr), span })
                    }
                }
            }
            _ => None,
        }
    }

    /// Lower a SaltIf to an HIR Expr::If.
    fn lower_salt_if(&mut self, salt_if: &grammar::SaltIf) -> Expr {
        let cond = self.lower_syn_expr(&salt_if.cond);
        let then_branch = self.lower_salt_block(&salt_if.then_branch);
        let else_branch = salt_if.else_branch.as_ref().map(|eb| {
            match eb.as_ref() {
                grammar::SaltElse::Block(block) => {
                    let b = self.lower_salt_block(block);
                    Box::new(Expr {
                        kind: ExprKind::Block(b),
                        ty: Type::Unit,
                        span: proc_macro2::Span::call_site(),
                    })
                }
                grammar::SaltElse::If(nested_if) => {
                    Box::new(self.lower_salt_if(nested_if))
                }
            }
        });
        Expr {
            kind: ExprKind::If {
                cond: Box::new(cond),
                then_branch,
                else_branch,
            },
            ty: Type::Unit,
            span: proc_macro2::Span::call_site(),
        }
    }

    /// Lower a syn::Expr to an HIR Expr.
    /// This is the core expression lowering that resolves variables via the ScopeStack.
pub fn lower_syn_expr(&mut self, expr: &syn::Expr) -> Expr {
        let span = proc_macro2::Span::call_site();
        match expr {
            syn::Expr::Lit(lit) => self.lower_syn_lit_expr(lit, span),
            syn::Expr::Path(path) => self.lower_syn_path_expr(path, span),
            syn::Expr::Binary(bin) => self.lower_syn_binary_expr(bin, span),
            syn::Expr::Unary(un) => self.lower_syn_unary_expr(un, span),
            syn::Expr::Call(call) => self.lower_syn_call_expr(call, span),
            syn::Expr::Assign(assign) => self.lower_syn_assign_expr(assign, span),
            syn::Expr::Field(field) => self.lower_syn_field_expr(field, span),
            syn::Expr::Index(idx) => self.lower_syn_index_expr(idx, span),
            syn::Expr::Paren(p) => self.lower_syn_expr(&p.expr),
            syn::Expr::Block(block) => self.lower_syn_block_expr(block, span),
            syn::Expr::Return(ret) => self.lower_syn_return_expr(ret, span),
            syn::Expr::Cast(cast) => self.lower_syn_cast_expr(cast, span),
            syn::Expr::Struct(s) => self.lower_syn_struct_expr(s, span),
            syn::Expr::MethodCall(mc) => self.lower_syn_method_call_expr(mc, span),
            _ => Expr { kind: ExprKind::Literal(Literal::Int(0)), ty: Type::Unit, span },
        }
    }

    fn lower_syn_lit_expr(&mut self, lit: &syn::ExprLit, span: proc_macro2::Span) -> Expr {
        let literal = match &lit.lit {
            syn::Lit::Int(i) => Literal::Int(i.base10_parse::<i64>().unwrap_or(0)),
            syn::Lit::Float(f) => Literal::Float(f.base10_parse::<f64>().unwrap_or(0.0)),
            syn::Lit::Bool(b) => Literal::Bool(b.value),
            syn::Lit::Str(s) => Literal::String(s.value()),
            _ => Literal::Int(0),
        };
        Expr { kind: ExprKind::Literal(literal), ty: Type::Unit, span }
    }

    fn lower_syn_path_expr(&mut self, path: &syn::ExprPath, span: proc_macro2::Span) -> Expr {
        let name = path.path.segments.last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        if let Some(var_id) = self.scopes.resolve(&name) {
            Expr { kind: ExprKind::Var(var_id), ty: Type::Unit, span }
        } else {
            Expr { kind: ExprKind::UnresolvedIdent(name), ty: Type::Unit, span }
        }
    }

    fn lower_syn_binary_expr(&mut self, bin: &syn::ExprBinary, span: proc_macro2::Span) -> Expr {
        let lhs = self.lower_syn_expr(&bin.left);
        let rhs = self.lower_syn_expr(&bin.right);
        let op = Self::lower_bin_op(&bin.op);
        Expr {
            kind: ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
            ty: Type::Unit,
            span,
        }
    }

    fn lower_syn_unary_expr(&mut self, un: &syn::ExprUnary, span: proc_macro2::Span) -> Expr {
        let inner = self.lower_syn_expr(&un.expr);
        let op = match un.op {
            syn::UnOp::Not(_) => UnOp::Not,
            syn::UnOp::Neg(_) => UnOp::Neg,
            syn::UnOp::Deref(_) => UnOp::Deref,
            _ => UnOp::Not,
        };
        Expr {
            kind: ExprKind::Unary { op, expr: Box::new(inner) },
            ty: Type::Unit,
            span,
        }
    }

    fn lower_syn_call_expr(&mut self, call: &syn::ExprCall, span: proc_macro2::Span) -> Expr {
        if let syn::Expr::Path(path) = &*call.func {
            if let Some(ident) = path.path.get_ident() {
                let fn_name = ident.to_string();
                if fn_name == "requires" || fn_name == "ensures" {
                    assert!(
                        call.args.len() == 1,
                        "{} takes exactly 1 argument, found {}",
                        fn_name, call.args.len()
                    );
                    let cond = self.lower_syn_expr(&call.args[0]);
                    let kind = if fn_name == "requires" {
                        ExprKind::Requires(Box::new(cond))
                    } else {
                        ExprKind::Ensures(Box::new(cond))
                    };
                    return Expr { kind, ty: Type::Unit, span };
                }

                if fn_name == "yield_now" {
                    let val = if call.args.is_empty() {
                        None
                    } else {
                        assert!(
                            call.args.len() == 1,
                            "yield_now takes 0 or 1 argument, found {}",
                            call.args.len()
                        );
                        Some(Box::new(self.lower_syn_expr(&call.args[0])))
                    };
                    return Expr {
                        kind: ExprKind::Yield(val),
                        ty: Type::Unit,
                        span,
                    };
                }
            }
        }

        let callee = self.lower_syn_expr(&call.func);
        let args: Vec<Expr> = call.args.iter()
            .map(|a| self.lower_syn_expr(a)).collect();
        Expr {
            kind: ExprKind::Call { callee: Box::new(callee), args },
            ty: Type::Unit,
            span,
        }
    }

    fn lower_syn_assign_expr(&mut self, assign: &syn::ExprAssign, span: proc_macro2::Span) -> Expr {
        let lhs = self.lower_syn_expr(&assign.left);
        let rhs = self.lower_syn_expr(&assign.right);
        Expr {
            kind: ExprKind::Assign { lhs: Box::new(lhs), rhs: Box::new(rhs) },
            ty: Type::Unit,
            span,
        }
    }

    fn lower_syn_field_expr(&mut self, field: &syn::ExprField, span: proc_macro2::Span) -> Expr {
        let base = self.lower_syn_expr(&field.base);
        let name = match &field.member {
            syn::Member::Named(id) => id.to_string(),
            syn::Member::Unnamed(idx) => idx.index.to_string(),
        };
        Expr {
            kind: ExprKind::Field { base: Box::new(base), field: name },
            ty: Type::Unit,
            span,
        }
    }

    fn lower_syn_index_expr(&mut self, idx: &syn::ExprIndex, span: proc_macro2::Span) -> Expr {
        let base = self.lower_syn_expr(&idx.expr);
        let index = self.lower_syn_expr(&idx.index);
        Expr {
            kind: ExprKind::Index { base: Box::new(base), index: Box::new(index) },
            ty: Type::Unit,
            span,
        }
    }

    fn lower_syn_block_expr(&mut self, block: &syn::ExprBlock, span: proc_macro2::Span) -> Expr {
        self.scopes.push_scope();
        let mut stmts = Vec::new();
        for stmt in &block.block.stmts {
            if let Some(s) = self.lower_syn_stmt(stmt) {
                stmts.push(s);
            }
        }
        self.scopes.pop_scope();
        Expr {
            kind: ExprKind::Block(Block { stmts, value: None, ty: Type::Unit }),
            ty: Type::Unit,
            span,
        }
    }

    fn lower_syn_return_expr(&mut self, ret: &syn::ExprReturn, span: proc_macro2::Span) -> Expr {
        let val = ret.expr.as_ref().map(|e| Box::new(self.lower_syn_expr(e)));
        Expr { kind: ExprKind::Return(val), ty: Type::Unit, span }
    }

    fn lower_syn_cast_expr(&mut self, cast: &syn::ExprCast, span: proc_macro2::Span) -> Expr {
        let inner = self.lower_syn_expr(&cast.expr);
        let target_ty = Type::from_syn_with_generics(
            &SynType::from_std((*cast.ty).clone()).unwrap_or(SynType::Other("unknown".into())),
            &self.current_generic_names,
        ).unwrap_or(Type::Unit);
        Expr {
            kind: ExprKind::Cast { expr: Box::new(inner), ty: target_ty },
            ty: Type::Unit,
            span,
        }
    }

    fn lower_syn_struct_expr(&mut self, s: &syn::ExprStruct, span: proc_macro2::Span) -> Expr {
        let name = s.path.segments.last()
            .map(|seg| seg.ident.to_string())
            .unwrap_or_default();
        let fields: Vec<(String, Expr)> = s.fields.iter().map(|f| {
            let field_name = match &f.member {
                syn::Member::Named(id) => id.to_string(),
                syn::Member::Unnamed(idx) => idx.index.to_string(),
            };
            let val = self.lower_syn_expr(&f.expr);
            (field_name, val)
        }).collect();
        Expr {
            kind: ExprKind::StructLit { name, type_args: vec![], fields },
            ty: Type::Unit,
            span,
        }
    }

    fn lower_syn_method_call_expr(&mut self, mc: &syn::ExprMethodCall, span: proc_macro2::Span) -> Expr {
        let receiver = self.lower_syn_expr(&mc.receiver);
        let method_name = mc.method.to_string();
        let mut args: Vec<Expr> = vec![receiver];
        for a in &mc.args {
            args.push(self.lower_syn_expr(a));
        }
        let callee = Expr {
            kind: ExprKind::UnresolvedIdent(method_name),
            ty: Type::Unit,
            span,
        };
        Expr {
            kind: ExprKind::Call { callee: Box::new(callee), args },
            ty: Type::Unit,
            span,
        }
    }


    // ─── Expression Helpers ───────────────────────────────────────────────

    fn lower_bin_op(op: &syn::BinOp) -> BinOp {
        match op {
            syn::BinOp::Add(_) => BinOp::Add,
            syn::BinOp::Sub(_) => BinOp::Sub,
            syn::BinOp::Mul(_) => BinOp::Mul,
            syn::BinOp::Div(_) => BinOp::Div,
            syn::BinOp::Rem(_) => BinOp::Rem,
            syn::BinOp::Eq(_) => BinOp::Eq,
            syn::BinOp::Ne(_) => BinOp::Ne,
            syn::BinOp::Lt(_) => BinOp::Lt,
            syn::BinOp::Le(_) => BinOp::Le,
            syn::BinOp::Gt(_) => BinOp::Gt,
            syn::BinOp::Ge(_) => BinOp::Ge,
            syn::BinOp::And(_) => BinOp::And,
            syn::BinOp::Or(_) => BinOp::Or,
            syn::BinOp::BitAnd(_) => BinOp::BitAnd,
            syn::BinOp::BitOr(_) => BinOp::BitOr,
            syn::BinOp::BitXor(_) => BinOp::BitXor,
            syn::BinOp::Shl(_) => BinOp::Shl,
            syn::BinOp::Shr(_) => BinOp::Shr,
            syn::BinOp::AddAssign(_) => BinOp::AddAssign,
            syn::BinOp::SubAssign(_) => BinOp::SubAssign,
            syn::BinOp::MulAssign(_) => BinOp::MulAssign,
            syn::BinOp::DivAssign(_) => BinOp::DivAssign,
            syn::BinOp::RemAssign(_) => BinOp::RemAssign,
            _ => BinOp::Add,
        }
    }

    fn extract_pat_name(pat: &syn::Pat) -> String {
        match pat {
            syn::Pat::Ident(pi) => pi.ident.to_string(),
            _ => "_".to_string(),
        }
    }

    fn extract_syn_pat_name(pat: &syn::Pat) -> String {
        match pat {
            syn::Pat::Ident(pi) => pi.ident.to_string(),
            syn::Pat::Type(pt) => Self::extract_syn_pat_name(&pt.pat),
            _ => "_".to_string(),
        }
    }

    fn is_mutable_pat(pat: &syn::Pat) -> bool {
        match pat {
            syn::Pat::Ident(pi) => pi.mutability.is_some(),
            syn::Pat::Type(pt) => Self::is_mutable_pat(&pt.pat),
            _ => false,
        }
    }

    fn extract_type_annotation(pat: &syn::Pat, generic_names: &HashSet<String>) -> Option<Type> {
        match pat {
            syn::Pat::Type(pt) => {
                let syn_ty = SynType::from_std((*pt.ty).clone()).ok()?;
                Type::from_syn_with_generics(&syn_ty, generic_names)
            }
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests — Written FIRST (TDD). The lowering implementation above was written
// to make these pass.
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::SaltFile;
    use crate::preprocess;
    use crate::types::Type;

    /// Helper: parse preprocessed Salt source into a SaltFile.
    fn parse_salt(source: &str) -> SaltFile {
        let preprocessed = preprocess(source);
        syn::parse_str::<SaltFile>(&preprocessed)
            .unwrap_or_else(|e| panic!("Failed to parse Salt source: {}\nSource:\n{}", e, preprocessed))
    }

    /// Helper: lower a SaltFile and return the resulting HIR items.
    fn lower(source: &str) -> Vec<Item> {
        let file = parse_salt(source);
        let mut ctx = LoweringContext::new();
        ctx.lower_file(&file)
    }

    // ═════════════════════════════════════════════════════════════════════
    // 1. Function lowering
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lower_simple_fn() {
        let items = lower("fn add(a: i32, b: i32) -> i32 { return a + b; }");
        assert_eq!(items.len(), 1);

        let item = &items[0];
        assert_eq!(item.name, "add");
        assert!(matches!(item.vis, Visibility::Private));

        match &item.kind {
            ItemKind::Fn(f) => {
                assert_eq!(f.inputs.len(), 2);
                assert_eq!(f.inputs[0].name, "a");
                assert_eq!(f.inputs[0].ty, Type::I32);
                assert_eq!(f.inputs[1].name, "b");
                assert_eq!(f.inputs[1].ty, Type::I32);
                assert_eq!(f.output, Type::I32);
                assert!(f.generics.params.is_empty());
            }
            other => panic!("Expected ItemKind::Fn, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_pub_fn() {
        let items = lower("pub fn hello() { }");
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0].vis, Visibility::Public));
    }

    #[test]
    fn test_lower_fn_unit_return() {
        let items = lower("fn noop() { }");
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            ItemKind::Fn(f) => assert_eq!(f.output, Type::Unit),
            other => panic!("Expected Fn, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_fn_with_generics() {
        let items = lower("fn identity<T>(x: T) -> T { return x; }");
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            ItemKind::Fn(f) => {
                assert_eq!(f.generics.params.len(), 1);
                assert!(matches!(&f.generics.params[0], GenericParam::Type(name) if name == "T"));
                assert_eq!(f.inputs[0].ty, Type::Generic("T".to_string()));
                assert_eq!(f.output, Type::Generic("T".to_string()));
            }
            other => panic!("Expected Fn, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_fn_with_ptr_param() {
        let items = lower("fn read(p: Ptr<u8>) -> u8 { return 0; }");
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            ItemKind::Fn(f) => {
                match &f.inputs[0].ty {
                    Type::Pointer { element, .. } => assert_eq!(**element, Type::U8),
                    other => panic!("Expected Ptr<u8>, got {:?}", other),
                }
            }
            other => panic!("Expected Fn, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_fn_with_reference_param() {
        let items = lower("fn borrow(x: &i64) -> i64 { return 0; }");
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            ItemKind::Fn(f) => {
                match &f.inputs[0].ty {
                    Type::Reference(inner, false) => assert_eq!(**inner, Type::I64),
                    other => panic!("Expected &i64, got {:?}", other),
                }
            }
            other => panic!("Expected Fn, got {:?}", other),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // 2. Struct lowering
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lower_simple_struct() {
        let items = lower("pub struct Point { pub x: f64, pub y: f64, }");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Point");

        match &items[0].kind {
            ItemKind::Struct(s) => {
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.fields[0].name, "x");
                assert_eq!(s.fields[0].ty, Type::F64);
                assert_eq!(s.fields[1].name, "y");
                assert_eq!(s.fields[1].ty, Type::F64);
                assert!(s.generics.params.is_empty());
            }
            other => panic!("Expected Struct, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_generic_struct() {
        let items = lower("pub struct Wrapper<T> { pub value: T, }");
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            ItemKind::Struct(s) => {
                assert_eq!(s.generics.params.len(), 1);
                assert!(matches!(&s.generics.params[0], GenericParam::Type(n) if n == "T"));
                assert_eq!(s.fields[0].ty, Type::Generic("T".to_string()));
            }
            other => panic!("Expected Struct, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_struct_with_ptr_field() {
        let items = lower("pub struct Node { pub data: i64, pub next: Ptr<u8>, }");
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            ItemKind::Struct(s) => {
                assert_eq!(s.fields[1].name, "next");
                match &s.fields[1].ty {
                    Type::Pointer { element, .. } => assert_eq!(**element, Type::U8),
                    other => panic!("Expected Ptr<u8>, got {:?}", other),
                }
            }
            other => panic!("Expected Struct, got {:?}", other),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // 3. Enum lowering
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lower_simple_enum() {
        let items = lower("pub enum Color { Red, Green, Blue, }");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Color");

        match &items[0].kind {
            ItemKind::Enum(e) => {
                assert_eq!(e.variants.len(), 3);
                assert_eq!(e.variants[0].name, "Red");
                assert!(matches!(e.variants[0].data, VariantData::Unit));
                assert_eq!(e.variants[1].name, "Green");
                assert_eq!(e.variants[2].name, "Blue");
            }
            other => panic!("Expected Enum, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_enum_with_payload() {
        let items = lower("pub enum Option<T> { Some(T), None, }");
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            ItemKind::Enum(e) => {
                assert_eq!(e.generics.params.len(), 1);
                assert_eq!(e.variants.len(), 2);

                assert_eq!(e.variants[0].name, "Some");
                match &e.variants[0].data {
                    VariantData::Tuple(types) => {
                        assert_eq!(types.len(), 1);
                        assert_eq!(types[0], Type::Generic("T".to_string()));
                    }
                    other => panic!("Expected Tuple variant, got {:?}", other),
                }

                assert_eq!(e.variants[1].name, "None");
                assert!(matches!(e.variants[1].data, VariantData::Unit));
            }
            other => panic!("Expected Enum, got {:?}", other),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // 4. Trait lowering
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lower_trait_definition() {
        let items = lower("trait Hashable { fn hash(&self) -> u64; }");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Hashable");

        match &items[0].kind {
            ItemKind::Trait(t) => {
                assert_eq!(t.items.len(), 1);
                match &t.items[0] {
                    TraitItem::Fn { func: f, .. } => {
                        assert_eq!(f.inputs.len(), 1);
                        assert_eq!(f.inputs[0].name, "self");
                        assert_eq!(f.output, Type::U64);
                    }
                }
            }
            other => panic!("Expected Trait, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_generic_trait() {
        let items = lower("trait Container<T> { fn get(&self, idx: i64) -> T; fn len(&self) -> i64; }");
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            ItemKind::Trait(t) => {
                assert_eq!(t.generics.params.len(), 1);
                assert!(matches!(&t.generics.params[0], GenericParam::Type(n) if n == "T"));
                assert_eq!(t.items.len(), 2);

                // First method: get(&self, idx: i64) -> T
                match &t.items[0] {
                    TraitItem::Fn { func: f, .. } => {
                        assert_eq!(f.inputs.len(), 2);
                        assert_eq!(f.output, Type::Generic("T".to_string()));
                    }
                }

                // Second method: len(&self) -> i64
                match &t.items[1] {
                    TraitItem::Fn { func: f, .. } => {
                        assert_eq!(f.inputs.len(), 1);
                        assert_eq!(f.output, Type::I64);
                    }
                }
            }
            other => panic!("Expected Trait, got {:?}", other),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // 5. Impl lowering
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lower_inherent_impl() {
        let items = lower("
            pub struct Point { pub x: f64, pub y: f64, }
            impl Point {
                pub fn distance(&self) -> f64 { return 0.0; }
            }
        ");
        // Should have 2 items: Struct + Impl
        assert!(items.len() >= 2);

        // Find the Impl item
        let impl_item = items.iter().find(|i| matches!(i.kind, ItemKind::Impl(_))).unwrap();
        match &impl_item.kind {
            ItemKind::Impl(imp) => {
                assert!(imp.trait_ref.is_none(), "Inherent impl should have no trait_ref");
                assert_eq!(imp.items.len(), 1);
                match &imp.items[0] {
                    ImplItem::Fn { name, func } => {
                        assert_eq!(name, "distance");
                        assert_eq!(func.inputs.len(), 1);
                        assert_eq!(func.inputs[0].name, "self");
                        assert_eq!(func.output, Type::F64);
                    }
                }
            }
            other => panic!("Expected Impl, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_trait_impl() {
        let items = lower("
            trait Hashable { fn hash(&self) -> u64; }
            pub struct Point { pub x: f64, pub y: f64, }
            impl Hashable for Point {
                fn hash(&self) -> u64 { return 0; }
            }
        ");
        // Should have 3 items: Trait + Struct + Impl
        assert!(items.len() >= 3);

        let impl_item = items.iter().find(|i| {
            matches!(&i.kind, ItemKind::Impl(imp) if imp.trait_ref.is_some())
        }).unwrap();

        match &impl_item.kind {
            ItemKind::Impl(imp) => {
                assert_eq!(imp.trait_ref, Some(Type::Struct("Hashable".to_string())));
                assert_eq!(imp.items.len(), 1);
            }
            other => panic!("Expected Impl, got {:?}", other),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // 6. Extern fn lowering
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lower_extern_fn() {
        let items = lower("extern fn malloc(size: i64) -> Ptr<u8>;");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "malloc");

        match &items[0].kind {
            ItemKind::Fn(f) => {
                assert_eq!(f.inputs.len(), 1);
                assert_eq!(f.inputs[0].name, "size");
                assert_eq!(f.inputs[0].ty, Type::I64);
                match &f.output {
                    Type::Pointer { element, .. } => assert_eq!(**element, Type::U8),
                    other => panic!("Expected Ptr<u8>, got {:?}", other),
                }
                assert!(f.body.is_none(), "Extern fns have no body");
            }
            other => panic!("Expected Fn, got {:?}", other),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // 7. Global / Const lowering
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lower_global() {
        let items = lower("global COUNTER: i64 = 0;");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "COUNTER");
        match &items[0].kind {
            ItemKind::Global(g) => assert_eq!(g.ty, Type::I64),
            other => panic!("Expected Global, got {:?}", other),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // 8. DefId uniqueness invariant
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_def_ids_are_unique() {
        let items = lower("
            fn foo() -> i32 { return 0; }
            fn bar() -> i32 { return 1; }
            pub struct Baz { pub x: i32, }
        ");
        let ids: Vec<DefId> = items.iter().map(|i| i.id).collect();
        let unique: HashSet<DefId> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "DefIds must be unique across all items");
    }

    // ═════════════════════════════════════════════════════════════════════
    // 9. Multi-item file lowering
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lower_multi_item_file() {
        let items = lower("
            extern fn printf(fmt: Ptr<u8>) -> i32;
            pub struct Vec<T> { pub ptr: Ptr<T>, pub len: i64, pub cap: i64, }
            impl Vec<T> {
                pub fn new() -> Vec<T> { return Vec { ptr: 0 as Ptr<T>, len: 0, cap: 0 }; }
            }
            fn main() -> i32 { return 0; }
        ");
        // Should have extern fn + struct + impl + fn = at least 4 items
        assert!(items.len() >= 4, "Expected at least 4 items, got {}", items.len());

        // Verify we have the expected kinds
        let kinds: Vec<&str> = items.iter().map(|i| match &i.kind {
            ItemKind::Fn(_) => "Fn",
            ItemKind::Struct(_) => "Struct",
            ItemKind::Impl(_) => "Impl",
            ItemKind::Enum(_) => "Enum",
            ItemKind::Trait(_) => "Trait",
            ItemKind::Global(_) => "Global",
        }).collect();
        assert!(kinds.contains(&"Fn"), "Missing function");
        assert!(kinds.contains(&"Struct"), "Missing struct");
        assert!(kinds.contains(&"Impl"), "Missing impl");
    }

    // ═════════════════════════════════════════════════════════════════════
    // 10. Salt's three permanent bets
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lower_result_type() {
        // Salt Pillar 1: Result<T> with Status
        let items = lower("fn try_open(path: Ptr<u8>) -> Result<i32> { return 0; }");
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            ItemKind::Fn(f) => {
                // Result<i32> should resolve to a Concrete type
                match &f.output {
                    Type::Concrete(name, args) => {
                        assert_eq!(name, "Result");
                        assert_eq!(args.len(), 1);
                        assert_eq!(args[0], Type::I32);
                    }
                    other => panic!("Expected Result<i32>, got {:?}", other),
                }
            }
            other => panic!("Expected Fn, got {:?}", other),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // 11. Body lowering — ScopeStack + VarId resolution
    // ═════════════════════════════════════════════════════════════════════

    /// Helper: extract the function body from a lowered item.
    fn get_fn_body(items: &[Item]) -> &Block {
        match &items[0].kind {
            ItemKind::Fn(f) => f.body.as_ref().expect("Function should have a body"),
            other => panic!("Expected Fn, got {:?}", other),
        }
    }

    #[test]
    fn test_body_let_binding_creates_var_id() {
        let items = lower("fn foo() { let x = 5; }");
        let body = get_fn_body(&items);
        assert_eq!(body.stmts.len(), 1);
        match &body.stmts[0].kind {
            StmtKind::Local(local) => {
                match &local.pat {
                    Pattern::Bind { name, var_id, mutable } => {
                        assert_eq!(name, "x");
                        assert_eq!(*var_id, VarId(0)); // first VarId in fn (no args)
                        assert!(!mutable);
                    }
                    other => panic!("Expected Bind pattern, got {:?}", other),
                }
                // init should be literal 5
                match &local.init.as_ref().unwrap().kind {
                    ExprKind::Literal(Literal::Int(5)) => {}
                    other => panic!("Expected Literal(5), got {:?}", other),
                }
            }
            other => panic!("Expected Local, got {:?}", other),
        }
    }

    #[test]
    fn test_body_variable_resolution() {
        // `let x = 5; x + 1;` — x should resolve to VarId(0)
        let items = lower("fn foo() { let x = 5; x + 1; }");
        let body = get_fn_body(&items);
        assert_eq!(body.stmts.len(), 2);

        // Second stmt should be `x + 1` with x resolved to VarId(0)
        match &body.stmts[1].kind {
            StmtKind::Semi(expr) => {
                match &expr.kind {
                    ExprKind::Binary { op, lhs, rhs } => {
                        assert_eq!(*op, BinOp::Add);
                        match &lhs.kind {
                            ExprKind::Var(var_id) => assert_eq!(*var_id, VarId(0)),
                            other => panic!("Expected Var(0), got {:?}", other),
                        }
                        match &rhs.kind {
                            ExprKind::Literal(Literal::Int(1)) => {}
                            other => panic!("Expected Literal(1), got {:?}", other),
                        }
                    }
                    other => panic!("Expected Binary, got {:?}", other),
                }
            }
            other => panic!("Expected Semi expr, got {:?}", other),
        }
    }

    #[test]
    fn test_body_shadowing_gives_new_var_id() {
        // Two lets with same name: should get different VarIds
        let items = lower("fn foo() { let x = 1; let x = 2; x; }");
        let body = get_fn_body(&items);
        assert_eq!(body.stmts.len(), 3);

        // First let x: VarId(0)
        match &body.stmts[0].kind {
            StmtKind::Local(local) => {
                match &local.pat {
                    Pattern::Bind { var_id, .. } => assert_eq!(*var_id, VarId(0)),
                    _ => panic!("Expected Bind"),
                }
            }
            _ => panic!("Expected Local"),
        }

        // Second let x: VarId(1) (shadow)
        match &body.stmts[1].kind {
            StmtKind::Local(local) => {
                match &local.pat {
                    Pattern::Bind { var_id, .. } => assert_eq!(*var_id, VarId(1)),
                    _ => panic!("Expected Bind"),
                }
            }
            _ => panic!("Expected Local"),
        }

        // Usage of x should resolve to VarId(1) (the shadow)
        match &body.stmts[2].kind {
            StmtKind::Semi(expr) => {
                match &expr.kind {
                    ExprKind::Var(var_id) => assert_eq!(*var_id, VarId(1)),
                    other => panic!("Expected Var(1), got {:?}", other),
                }
            }
            _ => panic!("Expected Semi"),
        }
    }

    #[test]
    fn test_body_fn_args_get_var_ids() {
        // Function args should be bound as VarId(0) and VarId(1)
        let items = lower("fn add(a: i32, b: i32) -> i32 { return a + b; }");
        let body = get_fn_body(&items);
        assert_eq!(body.stmts.len(), 1);

        // return statement: lower_syn_stmt returns StmtKind::Return
        match &body.stmts[0].kind {
            StmtKind::Return(Some(expr)) => {
                match &expr.kind {
                    ExprKind::Binary { lhs, rhs, .. } => {
                        match &lhs.kind {
                            ExprKind::Var(var_id) => assert_eq!(*var_id, VarId(0)), // a
                            other => panic!("Expected Var(0) for 'a', got {:?}", other),
                        }
                        match &rhs.kind {
                            ExprKind::Var(var_id) => assert_eq!(*var_id, VarId(1)), // b
                            other => panic!("Expected Var(1) for 'b', got {:?}", other),
                        }
                    }
                    other => panic!("Expected Binary, got {:?}", other),
                }
            }
            other => panic!("Expected Return(Some(_)), got {:?}", other),
        }
    }

    #[test]
    fn test_body_unresolved_ident() {
        // A name that isn't in scope should be UnresolvedIdent
        let items = lower("fn foo() { some_function(); }");
        let body = get_fn_body(&items);
        assert_eq!(body.stmts.len(), 1);
        match &body.stmts[0].kind {
            StmtKind::Semi(expr) => {
                match &expr.kind {
                    ExprKind::Call { callee, .. } => {
                        match &callee.kind {
                            ExprKind::UnresolvedIdent(name) => assert_eq!(name, "some_function"),
                            other => panic!("Expected UnresolvedIdent, got {:?}", other),
                        }
                    }
                    other => panic!("Expected Call, got {:?}", other),
                }
            }
            _ => panic!("Expected Semi"),
        }
    }

    #[test]
    fn test_body_if_lowering() {
        let items = lower("fn foo(x: i32) { if x > 0 { let y = 1; } }");
        let body = get_fn_body(&items);
        assert_eq!(body.stmts.len(), 1);
        match &body.stmts[0].kind {
            StmtKind::Expr(expr) => {
                match &expr.kind {
                    ExprKind::If { cond, then_branch, else_branch } => {
                        // Condition should be `x > 0`
                        match &cond.kind {
                            ExprKind::Binary { op, .. } => assert_eq!(*op, BinOp::Gt),
                            other => panic!("Expected Binary Gt, got {:?}", other),
                        }
                        // Then branch should have 1 stmt
                        assert_eq!(then_branch.stmts.len(), 1);
                        assert!(else_branch.is_none());
                    }
                    other => panic!("Expected If, got {:?}", other),
                }
            }
            other => panic!("Expected Expr stmt, got {:?}", other),
        }
    }

    #[test]
    fn test_body_while_lowering() {
        let items = lower("fn foo() { while true { break; } }");
        let body = get_fn_body(&items);
        assert_eq!(body.stmts.len(), 1);
        match &body.stmts[0].kind {
            StmtKind::While { cond, body: while_body } => {
                match &cond.kind {
                    ExprKind::Literal(Literal::Bool(true)) => {}
                    other => panic!("Expected Literal(true), got {:?}", other),
                }
                assert_eq!(while_body.stmts.len(), 1);
                assert!(matches!(while_body.stmts[0].kind, StmtKind::Break));
            }
            other => panic!("Expected While, got {:?}", other),
        }
    }

    #[test]
    fn test_body_return_with_value() {
        let items = lower("fn foo() -> i32 { return 42; }");
        let body = get_fn_body(&items);
        assert_eq!(body.stmts.len(), 1);
        // lower_syn_stmt extracts Return to StmtKind::Return
        match &body.stmts[0].kind {
            StmtKind::Return(Some(inner)) => {
                match &inner.kind {
                    ExprKind::Literal(Literal::Int(42)) => {}
                    other => panic!("Expected Literal(42), got {:?}", other),
                }
            }
            other => panic!("Expected Return(Some(_)), got {:?}", other),
        }
    }

    #[test]
    fn test_body_mutable_let() {
        let items = lower("fn foo() { let mut count = 0; count = 1; }");
        let body = get_fn_body(&items);
        assert_eq!(body.stmts.len(), 2);
        match &body.stmts[0].kind {
            StmtKind::Local(local) => {
                match &local.pat {
                    Pattern::Bind { name, mutable, .. } => {
                        assert_eq!(name, "count");
                        assert!(*mutable);
                    }
                    _ => panic!("Expected Bind"),
                }
            }
            _ => panic!("Expected Local"),
        }
        // Assignment should resolve `count` to its VarId
        match &body.stmts[1].kind {
            StmtKind::Semi(expr) => {
                match &expr.kind {
                    ExprKind::Assign { lhs, rhs } => {
                        match &lhs.kind {
                            ExprKind::Var(var_id) => assert_eq!(*var_id, VarId(0)),
                            other => panic!("Expected Var(0), got {:?}", other),
                        }
                        match &rhs.kind {
                            ExprKind::Literal(Literal::Int(1)) => {}
                            other => panic!("Expected Literal(1), got {:?}", other),
                        }
                    }
                    other => panic!("Expected Assign, got {:?}", other),
                }
            }
            _ => panic!("Expected Semi"),
        }
    }

    #[test]
    fn test_body_field_access() {
        let items = lower("fn foo() { let p = Point { x: 1, y: 2 }; p.x; }");
        let body = get_fn_body(&items);
        // Should have let + field access
        assert!(body.stmts.len() >= 2);
        match &body.stmts[1].kind {
            StmtKind::Semi(expr) => {
                match &expr.kind {
                    ExprKind::Field { base, field } => {
                        assert_eq!(field, "x");
                        match &base.kind {
                            ExprKind::Var(var_id) => assert_eq!(*var_id, VarId(0)), // p
                            other => panic!("Expected Var(0), got {:?}", other),
                        }
                    }
                    other => panic!("Expected Field, got {:?}", other),
                }
            }
            _ => panic!("Expected Semi"),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 6: Contract Lowering Interception
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_lowering_requires_intercept() {
        let items = lower("fn check(x: i64) { requires(x < 10); }");

        let func = match &items[0].kind {
            ItemKind::Fn(f) => f,
            other => panic!("Expected Fn, got {:?}", other),
        };
        let body = func.body.as_ref().expect("body");
        assert!(!body.stmts.is_empty(), "Expected at least one statement");

        match &body.stmts[0].kind {
            StmtKind::Semi(expr) => {
                match &expr.kind {
                    ExprKind::Requires(cond) => {
                        match &cond.kind {
                            ExprKind::Binary { op, .. } => assert_eq!(*op, BinOp::Lt),
                            other => panic!("Expected Binary, got {:?}", other),
                        }
                    }
                    other => panic!("Expected Requires, got {:?}", other),
                }
            }
            other => panic!("Expected Semi, got {:?}", other),
        }
    }

    #[test]
    fn test_lowering_ensures_intercept() {
        let items = lower("fn post(y: i64) { ensures(y > 0); }");

        let func = match &items[0].kind {
            ItemKind::Fn(f) => f,
            other => panic!("Expected Fn, got {:?}", other),
        };
        let body = func.body.as_ref().expect("body");
        assert!(!body.stmts.is_empty());

        match &body.stmts[0].kind {
            StmtKind::Semi(expr) => {
                match &expr.kind {
                    ExprKind::Ensures(cond) => {
                        match &cond.kind {
                            ExprKind::Binary { op, .. } => assert_eq!(*op, BinOp::Gt),
                            other => panic!("Expected Binary, got {:?}", other),
                        }
                    }
                    other => panic!("Expected Ensures, got {:?}", other),
                }
            }
            other => panic!("Expected Semi, got {:?}", other),
        }
    }
}
