//! Lightweight AST interpreter for Salt programs.
//!
//! Tree-walking interpreter that executes basic Salt programs from the
//! parsed Salt AST (grammar types), without MLIR emission or LLVM.
//! Designed for the WebAssembly REPL to provide instant "Run" feedback.
//!
//! Supported: arithmetic, variables, functions, if/else, while, for..in,
//! println(), f-strings, recursion, casts, compound assignment.

use std::collections::HashMap;
use std::fmt::Write;
use crate::grammar::{SaltFile, SaltBlock, SaltWhile, SaltFor, Stmt, SaltIf, SaltElse, Item};

/// Runtime value.
#[derive(Clone, Debug)]
pub enum Value {
    I32(i32),
    I64(i64),
    Bool(bool),
    Str(String),
    Unit,
    Return(Box<Value>),
}

impl Value {
    pub fn as_i32(&self) -> i32 {
        match self {
            Value::I32(v) => *v,
            Value::I64(v) => *v as i32,
            #[allow(clippy::collapsible_match)]
            Value::Bool(b) => if *b { 1 } else { 0 },
            _ => 0,
        }
    }

    pub fn as_i64(&self) -> i64 {
        match self {
            Value::I32(v) => *v as i64,
            Value::I64(v) => *v,
            #[allow(clippy::collapsible_match)]
            Value::Bool(b) => if *b { 1 } else { 0 },
            _ => 0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::I32(v) => *v != 0,
            Value::I64(v) => *v != 0,
            Value::Str(s) => !s.is_empty(),
            Value::Unit => false,
            Value::Return(v) => v.as_bool(),
        }
    }

    pub fn is_return(&self) -> bool {
        matches!(self, Value::Return(_))
    }

    pub fn unwrap_return(self) -> Value {
        match self {
            Value::Return(v) => *v,
            other => other,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::I32(v) => write!(f, "{}", v),
            Value::I64(v) => write!(f, "{}", v),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{}", s),
            Value::Unit => write!(f, "()"),
            Value::Return(v) => write!(f, "{}", v),
        }
    }
}

/// Stored function definition.
#[derive(Clone)]
struct FnDef {
    params: Vec<String>,
    body: SaltBlock,
}

/// The interpreter.
pub struct Interpreter {
    functions: HashMap<String, FnDef>,
    pub stdout: String,
    pub(crate) max_steps: usize,
    pub(crate) steps: usize,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            stdout: String::new(),
            max_steps: 1_000_000,
            steps: 0,
        }
    }

    /// Execute a parsed Salt program.
    pub fn run(&mut self, file: &SaltFile) -> Result<Value, String> {
        // Phase 1: Collect all function definitions
        for item in &file.items {
            if let Item::Fn(f) = item {
                let name = f.name.to_string();
                let params: Vec<String> = f.args.iter()
                    .map(|arg| arg.name.to_string())
                    .collect();

                self.functions.insert(name, FnDef {
                    params,
                    body: f.body.clone(),
                });
            }
        }

        // Phase 2: Call main()
        if self.functions.contains_key("main") {
            let result = self.call_function("main", &[])?;
            Ok(result.unwrap_return())
        } else {
            Err("No main() function found".to_string())
        }
    }

    pub(crate) fn call_function(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        self.check_steps()?;

        // Built-in functions
        match name {
            "println" => return self.call_builtin_println(args),
            "print" => return self.call_builtin_print(args),
            "abs" => return Ok(args.first().map(|a| Value::I64(a.as_i64().abs())).unwrap_or(Value::I64(0))),
            "max" => return Ok(if args.len() >= 2 { Value::I64(args[0].as_i64().max(args[1].as_i64())) } else { Value::I64(0) }),
            "min" => return Ok(if args.len() >= 2 { Value::I64(args[0].as_i64().min(args[1].as_i64())) } else { Value::I64(0) }),
            _ => {}
        }

        let func = self.functions.get(name).cloned()
            .ok_or_else(|| format!("Undefined function: {}", name))?;

        let mut scope: HashMap<String, Value> = HashMap::new();
        for (i, param_name) in func.params.iter().enumerate() {
            if let Some(val) = args.get(i) {
                scope.insert(param_name.clone(), val.clone());
            }
        }

        let result = self.exec_block(&func.body, &mut scope)?;
        // Unwrap Return at function boundary: the Return wrapper is a
        // control-flow signal for stopping block execution inside the function.
        // The caller must receive the plain value.
        Ok(result.unwrap_return())
    }

    fn call_builtin_println(&mut self, args: &[Value]) -> Result<Value, String> {
        if let Some(arg) = args.first() {
            writeln!(self.stdout, "{}", arg).ok();
        } else {
            writeln!(self.stdout).ok();
        }
        Ok(Value::Unit)
    }

    fn call_builtin_print(&mut self, args: &[Value]) -> Result<Value, String> {
        if let Some(arg) = args.first() {
            write!(self.stdout, "{}", arg).ok();
        }
        Ok(Value::Unit)
    }

    fn exec_block(&mut self, block: &SaltBlock, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let mut last = Value::Unit;
        for stmt in &block.stmts {
            last = self.exec_stmt(stmt, scope)?;
            if last.is_return() {
                return Ok(last);
            }
        }
        Ok(last)
    }

    fn exec_stmt(&mut self, stmt: &Stmt, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        self.check_steps()?;
        match stmt {
            // Delegate to syn-level statement handling
            Stmt::Syn(syn_stmt) => self.exec_syn_stmt(syn_stmt, scope),

            // Expression statement
            Stmt::Expr(expr, _has_semi) => self.eval_expr(expr, scope),

            // Salt's own If
            Stmt::If(salt_if) => self.exec_salt_if(salt_if, scope),

            Stmt::While(salt_while) => self.exec_stmt_while(salt_while, scope),
            Stmt::For(salt_for) => self.exec_stmt_for(salt_for, scope),

            // Return
            Stmt::Return(expr) => {
                let val = if let Some(e) = expr {
                    self.eval_expr(e, scope)?
                } else {
                    Value::Unit
                };
                Ok(Value::Return(Box::new(val.unwrap_return())))
            }

            // Break/Continue (simplified: just return Unit)
            Stmt::Break => Ok(Value::Unit),
            Stmt::Continue => Ok(Value::Unit),

            // Loop
            Stmt::Loop(block) => {
                loop {
                    let result = self.exec_block(block, scope)?;
                    if result.is_return() { return Ok(result); }
                }
            }

            // Match
            Stmt::Match(salt_match) => self.exec_salt_match(salt_match, scope),

            // Invariant, Move, MapWindow, WithRegion, Unsafe, LetElse — skip
            _ => Ok(Value::Unit),
        }
    }

    fn exec_stmt_while(&mut self, sw: &SaltWhile, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        loop {
            let cond = self.eval_expr(&sw.cond, scope)?;
            if cond.is_return() { return Ok(cond); }
            if !cond.as_bool() { break; }
            let result = self.exec_block(&sw.body, scope)?;
            if result.is_return() { return Ok(result); }
        }
        Ok(Value::Unit)
    }

    fn exec_stmt_for(&mut self, sf: &SaltFor, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let iter_name = self.extract_pat_name(&sf.pat);
        if let syn::Expr::Range(range) = &sf.iter {
            let start = if let Some(s) = &range.start { self.eval_expr(s, scope)?.as_i64() } else { 0 };
            let end = if let Some(e) = &range.end { self.eval_expr(e, scope)?.as_i64() } else { return Err("Unbounded range".to_string()); };
            for i in start..end {
                scope.insert(iter_name.clone(), Value::I64(i));
                let result = self.exec_block(&sf.body, scope)?;
                if result.is_return() { return Ok(result); }
            }
            Ok(Value::Unit)
        } else {
            Err("Only range-based for loops supported in interpreter".to_string())
        }
    }

    fn exec_salt_if(&mut self, salt_if: &SaltIf, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let cond = self.eval_expr(&salt_if.cond, scope)?;
        if cond.is_return() { return Ok(cond); }
        if cond.as_bool() {
            self.exec_block(&salt_if.then_branch, scope)
        } else if let Some(else_branch) = &salt_if.else_branch {
            match else_branch.as_ref() {
                SaltElse::Block(block) => self.exec_block(block, scope),
                SaltElse::If(nested_if) => self.exec_salt_if(nested_if, scope),
            }
        } else {
            Ok(Value::Unit)
        }
    }

    fn exec_salt_match(&mut self, salt_match: &crate::grammar::SaltMatch, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let scrutinee_val = self.eval_expr(&salt_match.scrutinee, scope)?;
        if scrutinee_val.is_return() { return Ok(scrutinee_val); }

        for arm in &salt_match.arms {
            let mut match_scope = scope.clone();
            if self.pattern_matches(&arm.pattern, &scrutinee_val, &mut match_scope) {
                if let Some(guard_expr) = &arm.guard {
                    let guard_val = self.eval_expr(guard_expr, &mut match_scope)?;
                    if !guard_val.as_bool() {
                        continue;
                    }
                }
                return self.exec_block(&arm.body, &mut match_scope);
            }
        }
        
        Err("Pattern matching failed: no arms matched".into())
    }

    fn pattern_matches(&self, pattern: &crate::grammar::pattern::Pattern, value: &Value, scope: &mut HashMap<String, Value>) -> bool {
        use crate::grammar::pattern::Pattern;
        match pattern {
            Pattern::Wildcard | Pattern::Rest => true,
            Pattern::Literal(lit) => {
                match lit {
                    syn::Lit::Int(li) => {
                        let parsed: i64 = li.base10_parse().unwrap_or(0);
                        value.as_i64() == parsed
                    },
                    syn::Lit::Bool(lb) => value.as_bool() == lb.value,
                    syn::Lit::Str(ls) => {
                        if let Value::Str(vs) = value {
                            vs == &ls.value()
                        } else {
                            false
                        }
                    },
                    _ => false,
                }
            },
            Pattern::Ident { name, .. } => {
                scope.insert(name.to_string(), value.clone());
                true
            },
            Pattern::Or(patterns) => {
                patterns.iter().any(|p| self.pattern_matches(p, value, scope))
            },
            _ => false,
        }
    }

    pub(crate) fn exec_syn_stmt(&mut self, stmt: &syn::Stmt, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        match stmt {
            syn::Stmt::Local(local) => {
                let val = if let Some(init) = &local.init {
                    self.eval_expr(&init.expr, scope)?
                } else {
                    Value::Unit
                };
                if val.is_return() { return Ok(val); }
                self.bind_local_pat(&local.pat, val, scope);
                Ok(Value::Unit)
            }
            syn::Stmt::Expr(expr, _) => self.eval_expr(expr, scope),
            _ => Ok(Value::Unit),
        }
    }

    fn bind_local_pat(&self, pat: &syn::Pat, val: Value, scope: &mut HashMap<String, Value>) {
        match pat {
            syn::Pat::Ident(ident) => {
                scope.insert(ident.ident.to_string(), val);
            }
            syn::Pat::Type(pt) => {
                self.bind_local_pat(&pt.pat, val, scope);
            }
            _ => {}
        }
    }

    pub(crate) fn extract_pat_name(&self, pat: &syn::Pat) -> String {
        match pat {
            syn::Pat::Ident(ident) => ident.ident.to_string(),
            syn::Pat::Type(pt) => self.extract_pat_name(&pt.pat),
            _ => "_".to_string(),
        }
    }


    pub(crate) fn eval_expr(&mut self, expr: &syn::Expr, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        self.check_steps()?;
        match expr {
            syn::Expr::Lit(lit) => match &lit.lit {
                syn::Lit::Int(i) => {
                    let val: i64 = i.base10_parse().unwrap_or(0);
                    if val > i32::MAX as i64 || val < i32::MIN as i64 {
                        Ok(Value::I64(val))
                    } else {
                        Ok(Value::I32(val as i32))
                    }
                }
                syn::Lit::Bool(b) => Ok(Value::Bool(b.value)),
                syn::Lit::Str(s) => Ok(Value::Str(s.value())),
                _ => Ok(Value::Unit),
            },
            syn::Expr::Path(p) => {
                if let Some(ident) = p.path.get_ident() {
                    let name = ident.to_string();
                    if name == "true" { return Ok(Value::Bool(true)); }
                    if name == "false" { return Ok(Value::Bool(false)); }
                    if let Some(val) = scope.get(&name) {
                        Ok(val.clone())
                    } else {
                        Ok(Value::Str(name))
                    }
                } else {
                    Ok(Value::Unit)
                }
            }
            syn::Expr::Binary(bin) => self.eval_expr_binary(bin, scope),
            syn::Expr::Unary(un) => self.eval_expr_unary(un, scope),
            syn::Expr::Assign(assign) => self.eval_expr_assign(assign, scope),
            syn::Expr::Call(call) => self.eval_expr_call(call, scope),
            syn::Expr::MethodCall(mc) => self.eval_expr_method_call(mc, scope),
            syn::Expr::If(if_expr) => self.eval_expr_if(if_expr, scope),
            syn::Expr::While(while_expr) => self.eval_expr_while(while_expr, scope),
            syn::Expr::ForLoop(for_loop) => self.eval_expr_for_loop(for_loop, scope),
            syn::Expr::Return(ret) => {
                let val = if let Some(expr) = &ret.expr {
                    self.eval_expr(expr, scope)?
                } else {
                    Value::Unit
                };
                Ok(Value::Return(Box::new(val.unwrap_return())))
            }
            syn::Expr::Block(block) => self.exec_syn_block(&block.block, scope),
            syn::Expr::Paren(p) => self.eval_expr(&p.expr, scope),
            syn::Expr::Cast(cast) => self.eval_expr_cast(cast, scope),
            syn::Expr::Macro(m) => {
                let macro_name = m.mac.path.segments.iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                if macro_name == "__fstring__" {
                    let tokens = m.mac.tokens.to_string();
                    let template = tokens.trim().trim_matches('"');
                    return self.eval_fstring(template, scope);
                }
                Ok(Value::Unit)
            }
            syn::Expr::Range(_) => Ok(Value::Unit),
            syn::Expr::Tuple(t) => {
                if let Some(last) = t.elems.last() { self.eval_expr(last, scope) } else { Ok(Value::Unit) }
            }
            _ => Ok(Value::Unit),
        }
    }

    fn eval_expr_binary(&mut self, bin: &syn::ExprBinary, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        if let Some(val) = self.try_eval_compound_assign(bin, scope)? {
            return Ok(val);
        }

        let left = self.eval_expr(&bin.left, scope)?;
        if left.is_return() { return Ok(left); }

        // Short-circuit evaluation (And, Or)
        match &bin.op {
            syn::BinOp::And(_) => {
                if !left.as_bool() { return Ok(Value::Bool(false)); }
                let right = self.eval_expr(&bin.right, scope)?;
                return Ok(Value::Bool(right.as_bool()));
            }
            syn::BinOp::Or(_) => {
                if left.as_bool() { return Ok(Value::Bool(true)); }
                let right = self.eval_expr(&bin.right, scope)?;
                return Ok(Value::Bool(right.as_bool()));
            }
            _ => {}
        }

        let right = self.eval_expr(&bin.right, scope)?;
        if right.is_return() { return Ok(right); }

        let l = left.as_i64();
        let r = right.as_i64();

        match &bin.op {
            syn::BinOp::Add(_) | syn::BinOp::Sub(_) | syn::BinOp::Mul(_) |
            syn::BinOp::Div(_) | syn::BinOp::Rem(_) => self.eval_arithmetic_op(&bin.op, l, r),
            syn::BinOp::Eq(_) | syn::BinOp::Ne(_) | syn::BinOp::Lt(_) |
            syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_) => self.eval_comparison_op(&bin.op, l, r),
            syn::BinOp::BitAnd(_) => Ok(Value::I64(l & r)),
            syn::BinOp::BitOr(_) => Ok(Value::I64(l | r)),
            syn::BinOp::BitXor(_) => Ok(Value::I64(l ^ r)),
            syn::BinOp::Shl(_) => Ok(Value::I64(l << r)),
            syn::BinOp::Shr(_) => Ok(Value::I64(l >> r)),
            _ => Ok(Value::Unit),
        }
    }

    fn try_eval_compound_assign(
        &mut self, bin: &syn::ExprBinary, scope: &mut HashMap<String, Value>,
    ) -> Result<Option<Value>, String> {
        if !matches!(bin.op, syn::BinOp::AddAssign(_) | syn::BinOp::SubAssign(_) |
            syn::BinOp::MulAssign(_) | syn::BinOp::DivAssign(_) | syn::BinOp::RemAssign(_))
        {
            return Ok(None);
        }
        let right = self.eval_expr(&bin.right, scope)?;
        if right.is_return() { return Ok(Some(right)); }
        let syn::Expr::Path(p) = &*bin.left else { return Ok(Some(Value::Unit)); };
        let Some(ident) = p.path.get_ident() else { return Ok(Some(Value::Unit)); };
        let name = ident.to_string();
        let current = scope.get(&name).cloned().unwrap_or(Value::I64(0));
        let (l, r) = (current.as_i64(), right.as_i64());
        let new_val = match &bin.op {
            syn::BinOp::AddAssign(_) => Value::I64(l.wrapping_add(r)),
            syn::BinOp::SubAssign(_) => Value::I64(l.wrapping_sub(r)),
            syn::BinOp::MulAssign(_) => Value::I64(l.wrapping_mul(r)),
            syn::BinOp::DivAssign(_) => Value::I64(l.checked_div(r).ok_or("Division by zero")?),
            syn::BinOp::RemAssign(_) => Value::I64(l.checked_rem(r).ok_or("Modulo by zero")?),
            _ => unreachable!(),
        };
        scope.insert(name, new_val);
        Ok(Some(Value::Unit))
    }

    fn eval_arithmetic_op(&self, op: &syn::BinOp, l: i64, r: i64) -> Result<Value, String> {
        match op {
            syn::BinOp::Add(_) => Ok(Value::I64(l.wrapping_add(r))),
            syn::BinOp::Sub(_) => Ok(Value::I64(l.wrapping_sub(r))),
            syn::BinOp::Mul(_) => Ok(Value::I64(l.wrapping_mul(r))),
            syn::BinOp::Div(_) => Ok(Value::I64(l.checked_div(r).ok_or("Division by zero")?)),
            syn::BinOp::Rem(_) => Ok(Value::I64(l.checked_rem(r).ok_or("Modulo by zero")?)),
            _ => unreachable!(),
        }
    }

    fn eval_comparison_op(&self, op: &syn::BinOp, l: i64, r: i64) -> Result<Value, String> {
        Ok(Value::Bool(match op {
            syn::BinOp::Eq(_) => l == r, syn::BinOp::Ne(_) => l != r,
            syn::BinOp::Lt(_) => l < r, syn::BinOp::Le(_) => l <= r,
            syn::BinOp::Gt(_) => l > r, syn::BinOp::Ge(_) => l >= r,
            _ => unreachable!(),
        }))
    }

    fn eval_expr_unary(&mut self, un: &syn::ExprUnary, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let val = self.eval_expr(&un.expr, scope)?;
        if val.is_return() { return Ok(val); }
        match un.op {
            syn::UnOp::Neg(_) => Ok(Value::I64(-val.as_i64())),
            syn::UnOp::Not(_) => Ok(Value::Bool(!val.as_bool())),
            _ => Ok(val),
        }
    }

    fn eval_expr_assign(&mut self, assign: &syn::ExprAssign, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let val = self.eval_expr(&assign.right, scope)?;
        if val.is_return() { return Ok(val); }
        if let syn::Expr::Path(p) = &*assign.left {
            if let Some(ident) = p.path.get_ident() {
                scope.insert(ident.to_string(), val);
            }
        }
        Ok(Value::Unit)
    }

}
