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
use crate::grammar::{SaltFile, SaltBlock, Stmt, SaltIf, SaltElse, Item};

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
            Value::Bool(b) => if *b { 1 } else { 0 },
            _ => 0,
        }
    }

    pub fn as_i64(&self) -> i64 {
        match self {
            Value::I32(v) => *v as i64,
            Value::I64(v) => *v,
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
    max_steps: usize,
    steps: usize,
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

    fn call_function(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        self.check_steps()?;

        // Built-in functions
        match name {
            "println" => {
                if let Some(arg) = args.first() {
                    writeln!(self.stdout, "{}", arg).ok();
                } else {
                    writeln!(self.stdout).ok();
                }
                return Ok(Value::Unit);
            }
            "print" => {
                if let Some(arg) = args.first() {
                    write!(self.stdout, "{}", arg).ok();
                }
                return Ok(Value::Unit);
            }
            "abs" => {
                if let Some(arg) = args.first() { return Ok(Value::I64(arg.as_i64().abs())); }
                return Ok(Value::I64(0));
            }
            "max" => {
                if args.len() >= 2 { return Ok(Value::I64(args[0].as_i64().max(args[1].as_i64()))); }
                return Ok(Value::I64(0));
            }
            "min" => {
                if args.len() >= 2 { return Ok(Value::I64(args[0].as_i64().min(args[1].as_i64()))); }
                return Ok(Value::I64(0));
            }
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

            // Salt's own While
            Stmt::While(salt_while) => {
                loop {
                    let cond = self.eval_expr(&salt_while.cond, scope)?;
                    if cond.is_return() { return Ok(cond); }
                    if !cond.as_bool() { break; }
                    let result = self.exec_block(&salt_while.body, scope)?;
                    if result.is_return() { return Ok(result); }
                }
                Ok(Value::Unit)
            }

            // Salt's own For..in
            Stmt::For(salt_for) => {
                let iter_name = self.extract_pat_name(&salt_for.pat);

                // Expect a range expression
                if let syn::Expr::Range(range) = &salt_for.iter {
                    let start = if let Some(s) = &range.start {
                        self.eval_expr(s, scope)?.as_i64()
                    } else { 0 };
                    let end = if let Some(e) = &range.end {
                        self.eval_expr(e, scope)?.as_i64()
                    } else { return Err("Unbounded range".to_string()); };

                    for i in start..end {
                        scope.insert(iter_name.clone(), Value::I64(i));
                        let result = self.exec_block(&salt_for.body, scope)?;
                        if result.is_return() { return Ok(result); }
                    }
                    Ok(Value::Unit)
                } else {
                    Err("Only range-based for loops supported in interpreter".to_string())
                }
            }

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

    fn exec_syn_stmt(&mut self, stmt: &syn::Stmt, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
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

    fn extract_pat_name(&self, pat: &syn::Pat) -> String {
        match pat {
            syn::Pat::Ident(ident) => ident.ident.to_string(),
            syn::Pat::Type(pt) => self.extract_pat_name(&pt.pat),
            _ => "_".to_string(),
        }
    }


    fn eval_expr(&mut self, expr: &syn::Expr, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
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
        match &bin.op {
            syn::BinOp::AddAssign(_) | syn::BinOp::SubAssign(_) |
            syn::BinOp::MulAssign(_) | syn::BinOp::DivAssign(_) |
            syn::BinOp::RemAssign(_) => {
                let right = self.eval_expr(&bin.right, scope)?;
                if right.is_return() { return Ok(right); }
                if let syn::Expr::Path(p) = &*bin.left {
                    if let Some(ident) = p.path.get_ident() {
                        let name = ident.to_string();
                        let current = scope.get(&name).cloned().unwrap_or(Value::I64(0));
                        let l = current.as_i64();
                        let r = right.as_i64();
                        let new_val = match &bin.op {
                            syn::BinOp::AddAssign(_) => Value::I64(l.wrapping_add(r)),
                            syn::BinOp::SubAssign(_) => Value::I64(l.wrapping_sub(r)),
                            syn::BinOp::MulAssign(_) => Value::I64(l.wrapping_mul(r)),
                            syn::BinOp::DivAssign(_) => if r != 0 { Value::I64(l / r) } else { return Err("Division by zero".into()); },
                            syn::BinOp::RemAssign(_) => if r != 0 { Value::I64(l % r) } else { return Err("Modulo by zero".into()); },
                            _ => unreachable!(),
                        };
                        scope.insert(name, new_val);
                        return Ok(Value::Unit);
                    }
                }
                return Ok(Value::Unit);
            }
            _ => {}
        }

        let left = self.eval_expr(&bin.left, scope)?;
        if left.is_return() { return Ok(left); }

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
            syn::BinOp::Add(_) => Ok(Value::I64(l.wrapping_add(r))),
            syn::BinOp::Sub(_) => Ok(Value::I64(l.wrapping_sub(r))),
            syn::BinOp::Mul(_) => Ok(Value::I64(l.wrapping_mul(r))),
            syn::BinOp::Div(_) => { if r == 0 { return Err("Division by zero".into()); } Ok(Value::I64(l / r)) },
            syn::BinOp::Rem(_) => { if r == 0 { return Err("Modulo by zero".into()); } Ok(Value::I64(l % r)) },
            syn::BinOp::Eq(_) => Ok(Value::Bool(l == r)),
            syn::BinOp::Ne(_) => Ok(Value::Bool(l != r)),
            syn::BinOp::Lt(_) => Ok(Value::Bool(l < r)),
            syn::BinOp::Le(_) => Ok(Value::Bool(l <= r)),
            syn::BinOp::Gt(_) => Ok(Value::Bool(l > r)),
            syn::BinOp::Ge(_) => Ok(Value::Bool(l >= r)),
            syn::BinOp::BitAnd(_) => Ok(Value::I64(l & r)),
            syn::BinOp::BitOr(_) => Ok(Value::I64(l | r)),
            syn::BinOp::BitXor(_) => Ok(Value::I64(l ^ r)),
            syn::BinOp::Shl(_) => Ok(Value::I64(l << r)),
            syn::BinOp::Shr(_) => Ok(Value::I64(l >> r)),
            _ => Ok(Value::Unit),
        }
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

    fn eval_expr_call(&mut self, call: &syn::ExprCall, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let mut args = Vec::new();
        for arg in &call.args {
            let val = self.eval_expr(arg, scope)?;
            if val.is_return() { return Ok(val); }
            args.push(val);
        }

        let fn_name = match &*call.func {
            syn::Expr::Path(p) => {
                p.path.segments.iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::")
            }
            _ => return Err("Unsupported call target".into()),
        };

        self.call_function(&fn_name, &args)
    }

    fn eval_expr_method_call(&mut self, mc: &syn::ExprMethodCall, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let receiver = self.eval_expr(&mc.receiver, scope)?;
        if receiver.is_return() { return Ok(receiver); }
        let method = mc.method.to_string();
        match method.as_str() {
            "abs" => Ok(Value::I64(receiver.as_i64().abs())),
            _ => Ok(receiver),
        }
    }

    fn eval_expr_if(&mut self, if_expr: &syn::ExprIf, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let cond = self.eval_expr(&if_expr.cond, scope)?;
        if cond.is_return() { return Ok(cond); }
        if cond.as_bool() {
            self.exec_syn_block(&if_expr.then_branch, scope)
        } else if let Some((_, else_branch)) = &if_expr.else_branch {
            self.eval_expr(else_branch, scope)
        } else {
            Ok(Value::Unit)
        }
    }

    fn eval_expr_while(&mut self, while_expr: &syn::ExprWhile, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        loop {
            let cond = self.eval_expr(&while_expr.cond, scope)?;
            if cond.is_return() { return Ok(cond); }
            if !cond.as_bool() { break; }
            let result = self.exec_syn_block(&while_expr.body, scope)?;
            if result.is_return() { return Ok(result); }
        }
        Ok(Value::Unit)
    }

    fn eval_expr_for_loop(&mut self, for_loop: &syn::ExprForLoop, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let iter_name = self.extract_pat_name(&for_loop.pat);
        if let syn::Expr::Range(range) = &*for_loop.expr {
            let start = if let Some(s) = &range.start { self.eval_expr(s, scope)?.as_i64() } else { 0 };
            let end = if let Some(e) = &range.end { self.eval_expr(e, scope)?.as_i64() } else { return Err("Unbounded range".into()); };
            for i in start..end {
                scope.insert(iter_name.clone(), Value::I64(i));
                let result = self.exec_syn_block(&for_loop.body, scope)?;
                if result.is_return() { return Ok(result); }
            }
            Ok(Value::Unit)
        } else {
            Err("Only range-based for loops supported".into())
        }
    }

    fn eval_expr_cast(&mut self, cast: &syn::ExprCast, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let val = self.eval_expr(&cast.expr, scope)?;
        if val.is_return() { return Ok(val); }
        if let syn::Type::Path(tp) = &*cast.ty {
            if let Some(seg) = tp.path.segments.last() {
                match seg.ident.to_string().as_str() {
                    "i32" => return Ok(Value::I32(val.as_i64() as i32)),
                    "i64" => return Ok(Value::I64(val.as_i64())),
                    "bool" => return Ok(Value::Bool(val.as_bool())),
                    _ => {}
                }
            }
        }
        Ok(val)
    }

    fn exec_syn_block(&mut self, block: &syn::Block, scope: &mut HashMap<String, Value>) -> Result<Value, String> {
        let mut last = Value::Unit;
        for stmt in &block.stmts {
            last = self.exec_syn_stmt(stmt, scope)?;
            if last.is_return() { return Ok(last); }
        }
        Ok(last)
    }

    fn eval_fstring(&mut self, template: &str, scope: &HashMap<String, Value>) -> Result<Value, String> {
        let mut result = String::new();
        let mut chars = template.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '{' {
                let mut var_expr = String::new();
                let mut depth = 1;
                while let Some(&nc) = chars.peek() {
                    if nc == '{' { depth += 1; }
                    if nc == '}' { depth -= 1; if depth == 0 { chars.next(); break; } }
                    var_expr.push(nc);
                    chars.next();
                }
                let var_name = var_expr.trim().to_string();
                if let Some(val) = scope.get(&var_name) {
                    write!(result, "{}", val).ok();
                } else {
                    // Try evaluating as simple expression
                    // For now just try to look up + for "x as i64" patterns, strip " as ..."
                    let base = var_name.split(" as ").next().unwrap_or(&var_name).trim();
                    if let Some(val) = scope.get(base) {
                        write!(result, "{}", val).ok();
                    } else {
                        write!(result, "{{{}}}", var_name).ok();
                    }
                }
            } else if c == '\\' {
                if let Some(nc) = chars.next() {
                    match nc { 'n' => result.push('\n'), 't' => result.push('\t'), _ => { result.push('\\'); result.push(nc); } }
                }
            } else {
                result.push(c);
            }
        }
        Ok(Value::Str(result))
    }

    fn check_steps(&mut self) -> Result<(), String> {
        self.steps += 1;
        if self.steps > self.max_steps {
            Err(format!("Execution limit exceeded ({} steps). Possible infinite loop.", self.max_steps))
        } else {
            Ok(())
        }
    }
}
