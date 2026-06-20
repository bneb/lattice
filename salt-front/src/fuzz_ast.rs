use arbitrary::Arbitrary;
use crate::grammar::*;
use syn::{parse_quote, Ident};


// Helper to manage available variables during AST generation
struct Scope {
    defined_vars: Vec<(String, FuzzType)>,
}

impl Scope {
    fn new() -> Self {
        Scope { defined_vars: Vec::new() }
    }
    
    fn add_var(&mut self, name: String, ty: FuzzType) {
        self.defined_vars.push((name, ty));
    }
}
    


#[derive(Arbitrary, Debug, Clone)]
pub struct FuzzSaltFile {
    pub fns: Vec<FuzzFn>,
}

impl FuzzSaltFile {
    pub fn to_salt(&self) -> SaltFile {
        let items = self.fns.iter().map(|f| Item::Fn(f.to_salt())).collect();
        SaltFile {
            package: None,
            imports: Vec::new(),
            items,
        }
    }
}

#[derive(Arbitrary, Debug, Clone)]
pub struct FuzzFn {
    pub name: String,
    pub args: Vec<(String, FuzzType)>,
    pub ret_ty: FuzzType,
    pub body: FuzzBlock,
    pub ret_to_arg: bool, // If true, try to return one of the args
}


fn sanitize_ident(s: &str) -> String {
    let mut clean: String = s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if clean.is_empty() || clean.chars().next().unwrap().is_numeric() {
        clean = format!("_{}", clean);
    }
    clean
}

impl FuzzFn {
    pub fn to_salt(&self) -> SaltFn {
        let clean_name = sanitize_ident(&self.name);
        let fn_name = Ident::new(&format!("fn_{}", clean_name), proc_macro2::Span::call_site());
        
        let mut scope = Scope::new();
        let mut pun_args = syn::punctuated::Punctuated::new();
        
        for (i, (arg_name, arg_ty)) in self.args.iter().enumerate() {
            let clean_arg = sanitize_ident(arg_name);
            let valid_name = format!("arg_{}_{}", i, clean_arg);
            scope.add_var(valid_name.clone(), arg_ty.clone());
            
            let ident = Ident::new(&valid_name, proc_macro2::Span::call_site());
            pun_args.push(crate::grammar::Arg {
                name: ident,
                ty: Some(arg_ty.to_syn()),
                is_mut: false,
            });
            if i < self.args.len() - 1 {
                pun_args.push_punct(syn::token::Comma::default());
            }
        }

        let body = self.body.to_salt(&mut scope);
        
        let default_ret_val: syn::Expr = match self.ret_ty {
            FuzzType::I32 => parse_quote!(0),
            FuzzType::I64 => parse_quote!(0i64),
            FuzzType::F64 => parse_quote!(0.0f64),
        };
        let ret_type = Some(self.ret_ty.to_syn());
        
        // Append valid return value if missing
        let mut stmts = body.stmts;
        stmts.push(Stmt::Syn(parse_quote!(return #default_ret_val;)));

        SaltFn {
            attributes: vec![],
            is_pub: false,
            name: fn_name,
            generics: None,
            args: pun_args,
            ret_type,
            requires: Vec::new(),
            ensures: Vec::new(),
            body: SaltBlock { stmts },
        }
    }
}

#[derive(Arbitrary, Debug, Clone)]
pub struct FuzzBlock {
    pub stmts: Vec<FuzzStmt>,
}

impl FuzzBlock {
    fn to_salt(&self, scope: &mut Scope) -> SaltBlock {
        let mut stmts = Vec::new();
        for s in &self.stmts {
           if let Some(stmt) = s.to_salt(scope) {
               stmts.push(stmt);
           }
        }
        SaltBlock { stmts }
    }
}

#[derive(Arbitrary, Debug, Clone)]
pub enum FuzzStmt {
    Let { name: u8, ty: FuzzType, val: FuzzExpr },
    Assign { var_idx: usize, val: FuzzExpr }, // Try to assign to existing var
    Expr(FuzzExpr),
    // While(Box<FuzzExpr>, FuzzBlock),
}

impl FuzzStmt {
    fn to_salt(&self, scope: &mut Scope) -> Option<Stmt> {
        match self {
            FuzzStmt::Let { name, ty, val } => {
                let var_name = format!("v_{}", name);
                let syn_ty = ty.to_syn();
                // Simple init for now: use a literal compatible with type
                // or use the generated expr if compatible.
                // For Alpha, FORCE simple literals to ensure compilation
                let init_expr = val.to_salt(scope, ty);
                
                scope.add_var(var_name.clone(), ty.clone());
                
                let var_ident = Ident::new(&var_name, proc_macro2::Span::call_site());
                Some(Stmt::Syn(parse_quote! {
                    let mut #var_ident: #syn_ty = #init_expr;
                }))
            },
            FuzzStmt::Assign { var_idx, val } => {
                if scope.defined_vars.is_empty() { return None; }
                let idx = var_idx % scope.defined_vars.len();
                let (name, ty) = &scope.defined_vars[idx];
                let var_ident = Ident::new(name, proc_macro2::Span::call_site());
                let expr = val.to_salt(scope, ty);
                Some(Stmt::Syn(parse_quote! {
                    #var_ident = #expr;
                }))
            },
            FuzzStmt::Expr(e) => {
                 let expr = e.to_salt(scope, &FuzzType::I32);
                 Some(Stmt::Syn(parse_quote!( #expr; )))
            }
        }
    }
}

#[derive(Arbitrary, Debug, Clone)]
pub enum FuzzExpr {
    Lit(i32),
    Var(usize), // Index into scope
    BinOp(Box<FuzzExpr>, Op, Box<FuzzExpr>),
}

#[derive(Arbitrary, Debug, Clone)]
pub enum Op { Add, Sub, Mul }

impl FuzzExpr {
    fn to_salt(&self, scope: &Scope, target_ty: &FuzzType) -> syn::Expr {
        match self {
            FuzzExpr::Lit(val) => {
                // Adjust literal based on type? For now assume integers work
                if matches!(target_ty, FuzzType::F64) {
                    let f = *val as f64;
                    parse_quote!(#f)
                } else {
                    // i32
                     parse_quote!(#val)
                }
            },
            FuzzExpr::Var(idx) => {
                if scope.defined_vars.is_empty() { 
                    return parse_quote!(0); 
                }
                let real_idx = idx % scope.defined_vars.len();
                let (name, _) = &scope.defined_vars[real_idx];
                let ident = Ident::new(name, proc_macro2::Span::call_site());
                parse_quote!(#ident)
            },
            FuzzExpr::BinOp(lhs, op, rhs) => {
                let l = lhs.to_salt(scope, target_ty);
                let r = rhs.to_salt(scope, target_ty);
                match op {
                    Op::Add => parse_quote!(#l + #r),
                    Op::Sub => parse_quote!(#l - #r),
                    Op::Mul => parse_quote!(#l * #r),
                }
            }
        }
    }
}

#[derive(Arbitrary, Debug, Clone, PartialEq)]
pub enum FuzzType {
    I32,
    I64,
    F64,
}

impl FuzzType {
    fn to_syn(&self) -> SynType {
        let ty: syn::Type = match self {
            FuzzType::I32 => parse_quote!(i32),
            FuzzType::I64 => parse_quote!(i64),
            FuzzType::F64 => parse_quote!(f64),
        };
        SynType::from_std(ty).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzz_file_empty() {
        let fuzz = FuzzSaltFile { fns: vec![] };
        let salt = fuzz.to_salt();
        assert!(salt.items.is_empty());
    }

    #[test]
    fn test_fuzz_fn_to_salt_structure() {
        let fuzz_fn = FuzzFn {
            name: "test".into(),
            args: vec![],
            ret_ty: FuzzType::I64,
            body: FuzzBlock { stmts: vec![] },
            ret_to_arg: false,
        };
        let salt_fn = fuzz_fn.to_salt();
        // FuzzFn name gets converted to Ident — verify structure, not exact string
        assert!(salt_fn.args.is_empty());
        // ret_ty I64 should produce Some(i64) return type
        assert!(salt_fn.ret_type.is_some());
    }

    #[test]
    fn test_fuzz_type_to_syn_all_variants() {
        // Verify all FuzzType variants produce valid SynType
        FuzzType::I32.to_syn();
        FuzzType::I64.to_syn();
        FuzzType::F64.to_syn();
    }

    #[test]
    fn test_sanitize_ident_valid() {
        let clean = sanitize_ident("foo_bar");
        assert!(!clean.is_empty());
        assert!(!clean.contains(|c: char| !c.is_alphanumeric() && c != '_'));
    }

    #[test]
    fn test_sanitize_ident_special_chars() {
        let clean = sanitize_ident("foo bar baz");
        assert!(!clean.contains(' '));
        assert!(!clean.is_empty());
    }

    #[test]
    fn test_scope_add_and_lookup() {
        let mut scope = Scope::new();
        scope.add_var("x".into(), FuzzType::I64);
        scope.add_var("y".into(), FuzzType::F64);
        assert_eq!(scope.defined_vars.len(), 2);
    }
}
