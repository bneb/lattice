use crate::grammar::{Stmt, SaltElse};
use syn::visit::{self, Visit};
use std::collections::HashSet;

struct MutationVisitor {
    mutated: HashSet<String>,
}
impl<'ast> Visit<'ast> for MutationVisitor {
    fn visit_expr(&mut self, i: &'ast syn::Expr) {
        if let syn::Expr::Assign(assign) = i {
            let mut curr = &*assign.left;
            while let syn::Expr::Field(f) = curr { curr = &*f.base; }
            while let syn::Expr::Index(idx) = curr { curr = &*idx.expr; }
            if let syn::Expr::Path(p) = curr {
                if let Some(id) = p.path.get_ident() { self.mutated.insert(id.to_string()); }
            }
        }
        visit::visit_expr(self, i);
    }
}

pub(crate) fn collect_mutations(stmts: &[Stmt]) -> HashSet<String> {
    let mut visitor = MutationVisitor { mutated: HashSet::new() };
    for stmt in stmts { collect_mutations_in_stmt(&mut visitor, stmt); }
    visitor.mutated
}

fn collect_mutations_in_stmt(visitor: &mut MutationVisitor, stmt: &Stmt) {
    match stmt {
        Stmt::Syn(s) => visitor.visit_stmt(s),
        Stmt::While(w) => {
            visitor.visit_expr(&w.cond);
            for s in &w.body.stmts { collect_mutations_in_stmt(visitor, s); }
        }
        Stmt::If(f) => {
            visitor.visit_expr(&f.cond);
            for s in &f.then_branch.stmts { collect_mutations_in_stmt(visitor, s); }
            if let Some(eb) = &f.else_branch {
                match eb.as_ref() {
                    SaltElse::Block(b) => { for s in &b.stmts { collect_mutations_in_stmt(visitor, s); } }
                    SaltElse::If(nested) => { collect_mutations_in_stmt(visitor, &Stmt::If(nested.as_ref().clone())); }
                }
            }
        }
        // Handle bare expression statements (e.g., `x = x + 1;`)
        // Without this, assignments at function body level are not detected by
        // collect_mutations, so mutated parameters aren't promoted to alloca.
        Stmt::Expr(e, _) => visitor.visit_expr(e),
        Stmt::For(f) => {
            visitor.visit_expr(&f.iter);
            for s in &f.body.stmts { collect_mutations_in_stmt(visitor, s); }
        }
        Stmt::Loop(b) | Stmt::Unsafe(b) | Stmt::DynamicCheck(b) => {
            for s in &b.stmts { collect_mutations_in_stmt(visitor, s); }
        }
        Stmt::Match(m) => {
            visitor.visit_expr(&m.scrutinee);
            for arm in &m.arms {
                for s in &arm.body.stmts { collect_mutations_in_stmt(visitor, s); }
            }
        }
        _ => {}
    }
}


// ============================================================================
// Arena escape analysis helpers (Scope Ladder)
// ============================================================================

/// Detect if an expression is an Arena constructor call: `Arena::new(...)`.
/// Matches path calls where the last two segments are "Arena" and "new".
pub(crate) fn is_arena_constructor(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(c) => {
            if let syn::Expr::Path(p) = &*c.func {
                let segs: Vec<_> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                // Match Arena::new or ...::Arena::new
                if segs.len() >= 2 {
                    return segs[segs.len() - 2] == "Arena" && segs[segs.len() - 1] == "new";
                }
            }
            false
        }
        _ => false,
    }
}

/// Extract the arena receiver name from an `arena.alloc(...)` or `arena.alloc_array(...)` call.
/// Returns Some("arena") if the expression is a method call with method name "alloc" or
/// "alloc_array" and the receiver is a simple identifier.
pub(crate) fn extract_arena_alloc_receiver(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(m) => {
            let method = m.method.to_string();
            if method == "alloc" || method == "alloc_array" {
                // Check if receiver is a simple ident (e.g., `arena`)
                if let syn::Expr::Path(p) = &*m.receiver {
                    if let Some(ident) = p.path.get_ident() {
                        return Some(ident.to_string());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the simple variable name from a return expression, traversing
/// casts and parens. For `return n`, returns Some("n"). For `return n as Ptr<T>`,
/// also returns Some("n").
pub(crate) fn extract_return_var_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) => {
            p.path.get_ident().map(|ident| ident.to_string())
        }
        syn::Expr::Cast(c) => extract_return_var_name(&c.expr),
        syn::Expr::Paren(p) => extract_return_var_name(&p.expr),
        _ => None,
    }
}

/// Extract the arena variable name from an `ArenaAllocator { arena: my_arena }` struct literal.
/// Returns Some("my_arena") if the expression is a struct literal whose path ends with
/// "ArenaAllocator" and has a field "arena" whose value is a simple identifier.
pub(crate) fn extract_arena_allocator_source(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Struct(s) => {
            // Check if the struct name ends with "ArenaAllocator"
            let last_seg = s.path.segments.last()?;
            if last_seg.ident != "ArenaAllocator" {
                return None;
            }
            // Find the "arena" field
            for field in &s.fields {
                if let syn::Member::Named(ident) = &field.member {
                    if ident == "arena" {
                        // Extract the value — must be a simple ident
                        if let syn::Expr::Path(p) = &field.expr {
                            if let Some(ident) = p.path.get_ident() {
                                return Some(ident.to_string());
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the allocator variable name from a `Vec::new(alloc, cap)` or
/// `Vec::<T, A>::new(alloc, cap)` call. Returns Some("alloc") —
/// the first argument, which is the allocator.
///
/// Matches both forms:
/// - Path call: `Vec::new(alloc, cap)` / `Vec::<i64, HeapAllocator>::new(alloc, cap)`
/// - The last two path segments must be ["Vec" or similar, "new"]
pub(crate) fn extract_vec_new_allocator(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Call(c) => {
            if let syn::Expr::Path(p) = &*c.func {
                let segs: Vec<_> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                // Match Vec::new, std::collections::vec::Vec::new, etc.
                if segs.len() >= 2 && segs[segs.len() - 1] == "new" {
                    let type_name = &segs[segs.len() - 2];
                    if type_name == "Vec" {
                        // First argument is the allocator
                        if let Some(syn::Expr::Path(arg_p)) = c.args.first() {
                                if let Some(ident) = arg_p.path.get_ident() {
                                    return Some(ident.to_string());
                                }
                            }
                    }
                }
            }
            None
        }
        _ => None,
    }
}
