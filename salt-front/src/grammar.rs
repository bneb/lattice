use syn::{
    parse::{Parse, ParseStream},
    Ident, Token, Expr,
    punctuated::Punctuated,
    parenthesized, parse_quote,
};
use proc_macro2::{TokenStream, TokenTree};
use crate::keywords::*;
pub mod attr;
pub mod pattern;
use attr::Attribute;
use pattern::Pattern;
/// Represents the entire source file
#[derive(Clone, Debug)]
pub struct SaltFile {
    pub package: Option<PackageDecl>,
    pub imports: Vec<ImportDecl>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct SaltBlock {
    pub stmts: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Syn(syn::Stmt),
    Invariant(Expr),
    Expr(Expr, bool), // (expr, has_semi)
    While(SaltWhile),
    For(SaltFor),
    If(SaltIf),
    Match(SaltMatch),
    LetElse(LetElse),
    Move(Expr),
    MapWindow { addr: Expr, size: Expr, region: Ident, body: SaltBlock },
    WithRegion { region: Ident, body: SaltBlock },
    Unsafe(SaltBlock),
    Return(Option<Expr>),
    Break,
    Continue,
    Loop(SaltBlock),
}

#[derive(Clone, Debug)]
pub struct SaltWhile {
    pub cond: Expr,
    pub body: SaltBlock,
}

#[derive(Clone, Debug)]
pub struct SaltFor {
    pub pat: syn::Pat,
    pub iter: Expr, 
    pub body: SaltBlock,
}

#[derive(Clone, Debug)]
pub struct SaltIf {
    pub cond: Expr,
    pub then_branch: SaltBlock,
    pub else_branch: Option<Box<SaltElse>>,
}

#[derive(Clone, Debug)]
pub enum SaltElse {
    Block(SaltBlock),
    If(Box<SaltIf>),
}

/// Match expression: `match expr { pattern => body, ... }`
#[derive(Clone, Debug)]
pub struct SaltMatch {
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
}

/// A single arm in a match expression
#[derive(Clone, Debug)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: SaltBlock,
}

/// Let-else statement: `let Pattern = expr else { diverging_block };`
#[derive(Clone, Debug)]
pub struct LetElse {
    pub pattern: Pattern,
    pub init: Expr,
    pub else_block: SaltBlock,
}

impl SaltFile {
    pub fn get_concept(&self, name: &str) -> Option<&SaltConcept> {
        self.items.iter().find_map(|item| {
            if let Item::Concept(c) = item {
                if c.name == name { return Some(c); }
            }
            None
        })
    }

    pub fn has_impl(&self, concept_name: &str, target_ty: &syn::Type) -> bool {
        self.items.iter().any(|item| {
            if let Item::Impl(SaltImpl::Concept { concept_name: cn, target_ty: tt }) = item {
                // Simplified type comparison for MVP (compare token strings)
                if cn == concept_name {
                    let target_str = quote::quote!(#target_ty).to_string();
                    let impl_str = quote::quote!(#tt).to_string();
                    return target_str == impl_str;
                }
            }
            false
        })
    }

    pub fn empty() -> Self {
        Self {
            package: None,
            imports: Vec::new(),
            items: Vec::new(),
        }
    }

    pub fn get_use_namespaces(&self) -> Vec<String> {
        let mut namespaces = Vec::new();
        for imp in &self.imports {
            let base = imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(".");
            // For group imports (import a.b.{x, y}), we only need to load the module 'a.b'
            // The items x and y will be registered when the module is loaded.
            namespaces.push(base);
        }
        namespaces
    }
}

#[derive(Clone, Debug)]
pub enum Item {
    Fn(SaltFn),
    Struct(StructDef),
    Global(GlobalDef),
    Concept(SaltConcept),
    Trait(SaltTrait),  // [V4.0] Trait definitions
    Impl(SaltImpl),
    ExternFn(ExternFnDecl),
    Enum(EnumDef),
    Const(ConstDef),
}

// [SOVEREIGN V2.0] Grammar Lockdown
// First-class SynType used by the parser to enforce pointer safety semantics.
// This replaces direct usage of syn::Type in the Salt AST.
#[derive(Clone, Debug, PartialEq)]
pub enum SynType {
    /// First-class Pointer: Ptr<T>
    /// Explicitly parsed and separated from generic paths.
    Pointer(Box<SynType>),
    /// First-class Reference: &T
    Reference(Box<SynType>, bool),
    /// [SOVEREIGN PHASE 3] Shaped Tensor: Tensor<T, {Rank, D1, D2...}>
    /// Carries compile-time shape metadata for @ operator dispatch.
    /// Example: Tensor<f32, {2, 128, 784}> -> 2D matrix [128 x 784]
    ShapedTensor {
        element: Box<SynType>,
        rank: usize,
        dims: Vec<TensorDim>,
    },
    /// Standard Path (e.g. i32, Vec<T>, std::Result)
    Path(SynPath),
    /// Array: [T; N]
    Array(Box<SynType>, Expr),
    /// Tuple: (A, B, C)
    Tuple(SynTuple),
    /// Function pointer: fn(T1, T2) -> R
    FnPtr(Vec<SynType>, Option<Box<SynType>>),
    /// Fallback/Other types (e.g. Infer, Slice - mapped as needed)
    Other(String), 
}

/// Dimension in a shaped tensor - can be static, dynamic, or symbolic
#[derive(Clone, Debug, PartialEq)]
pub enum TensorDim {
    Static(usize),       // Compile-time constant: 128
    Dynamic,             // Unknown at compile time: ?
    Symbolic(String),    // Named constant: HIDDEN_SIZE
}

#[derive(Clone, Debug, PartialEq)]
pub struct SynPath {
    pub segments: Vec<SynPathSegment>,
}

impl SynPath {
    pub fn to_string(&self) -> String {
        self.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SynPathSegment {
    pub ident: Ident,
    pub args: Vec<SynType>, // Simplified args: we assume AngleBracketed type args mostly
}

#[derive(Clone, Debug, PartialEq)]
pub struct SynTuple {
    pub elems: Vec<SynType>,
}

impl Parse for SynType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // [GRAMMAR CHECK]: Peek for Explicit Ptr<T> (Sovereign Syntax)
        if input.peek(Ident) {
            let fork = input.fork();
            let id: Ident = fork.parse()?;
            if id == "Ptr" && fork.peek(Token![<]) {
                input.parse::<Ident>()?; // eat Ptr
                input.parse::<Token![<]>()?;
                let inner = input.parse::<SynType>()?;
                input.parse::<Token![>]>()?;
                return Ok(SynType::Pointer(Box::new(inner)));
            }
            
            // [SOVEREIGN PHASE 3]: Tensor<T, {Rank, D1, D2...}> shaped tensor
            if id == "Tensor" && fork.peek(Token![<]) {
                input.parse::<Ident>()?; // eat Tensor
                input.parse::<Token![<]>()?;
                
                // Parse element type
                let element = input.parse::<SynType>()?;
                
                // Expect comma before dimension block
                input.parse::<Token![,]>()?;
                
                // Parse dimension block: {Rank, D1, D2, ...}
                let content;
                syn::braced!(content in input);
                
                // Parse rank (first number)
                let rank_lit: syn::LitInt = content.parse()?;
                let rank: usize = rank_lit.base10_parse()?;
                
                // Parse dimensions
                let mut dims = Vec::new();
                while content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                    
                    if content.is_empty() {
                        break;
                    }
                    
                    // Parse dimension: static int, ? for dynamic, or ident for symbolic
                    if content.peek(syn::LitInt) {
                        let lit: syn::LitInt = content.parse()?;
                        let val: usize = lit.base10_parse()?;
                        dims.push(TensorDim::Static(val));
                    } else if content.peek(Token![?]) {
                        content.parse::<Token![?]>()?;
                        dims.push(TensorDim::Dynamic);
                    } else if content.peek(Ident) {
                        let name: Ident = content.parse()?;
                        dims.push(TensorDim::Symbolic(name.to_string()));
                    } else {
                        return Err(syn::Error::new(content.span(), "Expected dimension: integer, ?, or identifier"));
                    }
                }
                
                // Validate rank matches dimension count
                if dims.len() != rank {
                    return Err(syn::Error::new(
                        rank_lit.span(),
                        format!("Tensor rank {} doesn't match {} dimensions provided", rank, dims.len())
                    ));
                }
                
                input.parse::<Token![>]>()?;
                
                return Ok(SynType::ShapedTensor {
                    element: Box::new(element),
                    rank,
                    dims,
                });
            }
        }

        // [GRAMMAR CHECK]: Peek for Explicit Reference &T or &mut T
        if input.peek(Token![&]) {
            input.parse::<Token![&]>()?;
            let is_mut = if input.peek(Token![mut]) {
                input.parse::<Token![mut]>()?;
                true
            } else {
                false
            };
            let inner = input.parse::<SynType>()?;
            return Ok(SynType::Reference(Box::new(inner), is_mut));
        }

        // [CROSS-MODULE STRUCT] Intercept dotted module paths: module.StructName
        // Salt uses `.` as the module separator (not `::`), so `addr.PhysAddr`
        // in type position is parsed as a field access by syn::Type. We intercept
        // `Ident.Ident` and build a multi-segment SynPath.
        if input.peek(Ident) {
            let fork = input.fork();
            let _: Ident = fork.parse()?;
            if fork.peek(Token![.]) {
                let _: Token![.] = fork.parse()?;
                if fork.peek(Ident) {
                    // Pattern confirmed: Ident.Ident — consume from the real input
                    let first: Ident = input.parse()?;
                    input.parse::<Token![.]>()?;
                    let second: Ident = input.parse()?;
                    
                    // Parse optional generic args: module.Struct<T, U>
                    let mut args = Vec::new();
                    if input.peek(Token![<]) {
                        input.parse::<Token![<]>()?;
                        loop {
                            if input.peek(Token![>]) { break; }
                            args.push(input.parse::<SynType>()?);
                            if !input.peek(Token![,]) { break; }
                            input.parse::<Token![,]>()?;
                        }
                        input.parse::<Token![>]>()?;
                    }
                    
                    let segments = vec![
                        SynPathSegment { ident: first, args: vec![] },
                        SynPathSegment { ident: second, args },
                    ];
                    return Ok(SynType::Path(SynPath { segments }));
                }
            }
        }
        
        // Fallback: Parse as syn::Type and convert
        // This handles Arrays, Tuples, and standard Paths (validating no NativePtr)
        let std_ty: syn::Type = input.parse()?;
        Self::from_std(std_ty)
    }
}

impl SynType {
    pub fn from_std(ty: syn::Type) -> syn::Result<Self> {
        match ty {
            syn::Type::Path(tp) => {
                let mut segments = Vec::new();
                for seg in tp.path.segments {
                    let ident = seg.ident;
                    let name = ident.to_string();
                    
                    // [SCORCHED EARTH] NativePtr Syntax Ban
                    if name == "NativePtr" || name == "NodePtr" {
                         return Err(syn::Error::new(ident.span(), format!("Legacy type '{}' is abolished. Use 'Ptr<T>' instead.", name)));
                    }
                    
                    if name == "Ptr" {
                        // If we encounter Ptr here (e.g. as std::core::ptr::Ptr or generic arg), 
                        // we should try to promote it to Pointer if it has single type arg.
                        // But strictly, strict parsing catches 'Ptr<...>' at top level.
                        // Inside generic args, we rely on recursion.
                    }

                    let mut args = Vec::new();
                    if let syn::PathArguments::AngleBracketed(ab) = seg.arguments {
                        for arg in ab.args {
                            if let syn::GenericArgument::Type(inner) = arg {
                                args.push(Self::from_std(inner)?);
                            } else if let syn::GenericArgument::Const(_c) = arg {
                                // Mapped as Other/Expression handling? 
                                // For now, maybe represent as SynType::Other for complexity?
                                // Or we just don't support const generics fully in this simplified pass yet?
                                // Let's support simple consts if needed or map to Concrete args logic later.
                                // For MVP "Grammar Lockdown": just stick to types.
                            }
                        }
                    }
                    segments.push(SynPathSegment { ident, args });
                }
                
                // Check if it's Ptr<T> form (Path with 1 segment "Ptr" and 1 arg)
                if segments.len() == 1 && segments[0].ident == "Ptr" && segments[0].args.len() == 1 {
                    return Ok(SynType::Pointer(Box::new(segments.pop().unwrap().args.pop().unwrap())));
                }

                Ok(SynType::Path(SynPath { segments }))
            },
            syn::Type::Array(ta) => {
                let inner = Self::from_std(*ta.elem)?;
                Ok(SynType::Array(Box::new(inner), ta.len))
            },
            syn::Type::Tuple(tt) => {
                let mut elems = Vec::new();
                for e in tt.elems {
                    elems.push(Self::from_std(e)?);
                }
                Ok(SynType::Tuple(SynTuple { elems }))
            },
             syn::Type::Ptr(tp) => {
                 // Raw pointer *const T / *mut T -> Map to Pointer (Naked) effectively
                 // Or keep distinct? User said "Ptr<T>" is the sovereign syntax.
                 // We map *T to Ptr<T> (Naked).
                 let inner = Self::from_std(*tp.elem)?;
                 // We don't distinguish const/mut in Ptr<T> generic yet (it is_mutable by default in our model)
                 // But let's map it.
                 Ok(SynType::Pointer(Box::new(inner)))
             },
             syn::Type::Reference(tr) => {
                 let inner = Self::from_std(*tr.elem)?;
                 Ok(SynType::Reference(Box::new(inner), tr.mutability.is_some()))
             },
             syn::Type::BareFn(bf) => {
                 // [SOVEREIGN] First-class function pointer types
                 let mut args = Vec::new();
                 for arg in &bf.inputs {
                     args.push(Self::from_std(arg.ty.clone())?);
                 }
                 let ret = match &bf.output {
                     syn::ReturnType::Default => None,
                     syn::ReturnType::Type(_, ty) => Some(Box::new(Self::from_std((**ty).clone())?)),
                 };
                 Ok(SynType::FnPtr(args, ret))
             },
            _ => Ok(SynType::Other(quote::quote!(#ty).to_string()))
        }
    }
}

#[derive(Clone, Debug)]
pub struct PackageDecl {
    pub name: Punctuated<Ident, Token![.]>,
}

#[derive(Clone, Debug)]
pub struct ImportDecl {
    pub name: Punctuated<Ident, Token![.]>,
    pub alias: Option<Ident>,
    pub group: Option<Vec<Ident>>,
}

#[derive(Clone, Debug)]
pub struct GlobalDef {
    pub name: Ident,
    pub colon_token: Token![:],
    pub ty: SynType,
    pub init: Option<Expr>,
}

#[derive(Clone, Debug)]
pub struct ConstDef {
    pub name: Ident,
    pub ty: SynType,
    pub value: Expr,
}

#[derive(Clone, Debug)]
pub struct StructDef {
    pub attributes: Vec<Attribute>,
    pub name: Ident,
    pub generics: Option<Generics>,
    pub fields: Vec<FieldDef>,
    pub invariants: Vec<Expr>,
}

#[derive(Clone, Debug)]
pub struct FieldDef {
    pub attributes: Vec<Attribute>,
    pub name: Ident,
    pub colon_token: Token![:],
    pub ty: SynType,
}

#[derive(Clone, Debug)]
pub struct EnumDef {
    pub name: Ident,
    pub generics: Option<Generics>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Clone, Debug)]
pub struct EnumVariant {
    pub name: Ident,
    pub ty: Option<SynType>, 
}

#[derive(Clone, Debug)]
pub struct SaltConcept {
    pub name: Ident,
    pub generics: Option<Generics>,
    pub param: Ident,
    pub param_ty: SynType,
    pub requires: Expr,
}

/// [V4.0] A trait definition: `trait Foo<T> { fn bar(&self) -> T; }`
#[derive(Clone, Debug)]
pub struct SaltTrait {
    pub name: Ident,
    pub generics: Option<Generics>,
    pub methods: Vec<TraitMethodSig>,
}

/// [V4.0] A method signature in a trait (no body)
#[derive(Clone, Debug)]
pub struct TraitMethodSig {
    pub name: Ident,
    pub generics: Option<Generics>,
    pub args: Punctuated<Arg, Token![,]>,
    pub ret_type: Option<SynType>,
}

#[derive(Clone, Debug)]
pub enum SaltImpl {
    Concept { concept_name: Ident, target_ty: SynType },
    Methods { target_ty: SynType, methods: Vec<SaltFn>, generics: Option<Generics> },
    /// [V4.0] `impl Trait for Type { ... }`
    Trait { trait_name: Ident, target_ty: SynType, methods: Vec<SaltFn>, generics: Option<Generics> },
}

#[derive(Clone, Debug)]
pub struct ExternFnDecl {
    pub attributes: Vec<Attribute>,
    pub is_pub: bool,
    pub name: Ident,
    pub args: Punctuated<Arg, Token![,]>,
    pub ret_type: Option<SynType>,
    pub requires: Vec<Expr>,
}

// Attribute struct moved to attr.rs

#[derive(Clone, Debug)]
pub struct SaltFn {
    pub attributes: Vec<Attribute>,
    pub is_pub: bool,
    pub name: Ident,
    pub generics: Option<Generics>,
    pub args: Punctuated<Arg, Token![,]>,
    pub ret_type: Option<SynType>,
    pub requires: Vec<Expr>,
    pub ensures: Vec<Expr>,
    pub body: SaltBlock,
}

#[derive(Clone, Debug)]
pub enum GenericParam {
    Type { name: Ident, constraint: Option<Ident> },
    Const { name: Ident, ty: SynType },
}

#[derive(Clone, Debug)]
pub struct Generics {
    pub params: Punctuated<GenericParam, Token![,]>,
}
 // The Concept name

#[derive(Clone, Debug)]
pub struct Arg {
    pub name: Ident,
    pub ty: Option<SynType>, // None for 'self'
}


fn parse_user_ident(input: ParseStream) -> syn::Result<Ident> {
    let id: Ident = input.parse()?;
    let s = id.to_string();
    if s.contains("__") {
        return Err(syn::Error::new(id.span(), "Identifiers cannot contain double underscores '__' as it is reserved for symbol mangling."));
    }
    Ok(id)
}

impl Parse for SaltFile {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let pkg_decl = if input.peek(package) {
            Some(input.parse()?)
        } else {
            None
        };

        let mut imports = Vec::new();
        while input.peek(import) {
            imports.push(input.parse()?);
        }

        let mut items = Vec::new();
        while !input.is_empty() {
             let fork = input.fork();
             // Skip attributes to see what the item is
             while fork.peek(Token![@]) {
                 if fork.peek(Token![@]) {
                     let _ = fork.parse::<Attribute>(); // Best effort
                 }
             }

             if fork.peek(Token![pub]) { let _ = fork.parse::<Token![pub]>()?; }
             
             if fork.peek(Token![extern]) || fork.peek(Token![fn]) {
                 if fork.peek(Token![extern]) {
                     items.push(Item::ExternFn(input.parse()?));
                 } else {
                     items.push(Item::Fn(input.parse()?));
                 }
             } else if input.peek(Token![struct]) || (input.peek(Token![@]) && fork.peek(Token![struct])) {
                 items.push(Item::Struct(input.parse()?));
             } else if input.peek(global) || input.peek(var) {
                 items.push(Item::Global(input.parse()?));
             } else if input.peek(concept) {
                 items.push(Item::Concept(input.parse()?));
             } else if input.peek(Token![trait]) {
                 // [V4.0] Parse trait definitions
                 items.push(Item::Trait(input.parse()?));
             } else if input.peek(Token![impl]) {
                 items.push(Item::Impl(input.parse()?));
             } else if input.peek(Token![enum]) {
                 items.push(Item::Enum(input.parse()?));
             } else if input.peek(Token![const]) {
                 items.push(Item::Const(input.parse()?));
             } else {
                 // Fallback to avoid infinite loop: consume one token if we can't identify the item
                 let _ = input.parse::<TokenTree>()?;
             }
        }
        
        Ok(SaltFile { package: pkg_decl, imports, items })
    }
}

impl Parse for PackageDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<package>()?;
        let name: Punctuated<Ident, Token![.]> = Punctuated::parse_separated_nonempty_with(input, parse_user_ident)?;
        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
        }
        Ok(PackageDecl { name })
    }
}

impl Parse for ImportDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<import>()?;
        let mut name: Punctuated<Ident, Token![.]> = Punctuated::new();
        
        while !input.is_empty() && !input.peek(syn::token::Brace) && !input.peek(Token![as]) && !input.peek(Token![;]) {
            name.push_value(input.parse()?);
            if input.peek(Token![.]) {
                name.push_punct(input.parse()?);
            } else {
                break;
            }
        }

        let group = if input.peek(syn::token::Brace) {
            let content;
            syn::braced!(content in input);
            let idents: Punctuated<Ident, Token![,]> = content.parse_terminated(Ident::parse, Token![,])?;
            Some(idents.into_iter().collect())
        } else {
            None
        };

        let alias = if input.peek(Token![as]) {
            input.parse::<Token![as]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
        }
        Ok(ImportDecl { name, alias, group })
    }
}

impl Parse for GlobalDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(global) {
            input.parse::<global>()?;
        } else {
            input.parse::<var>()?;
        }
        let name: Ident = parse_user_ident(input)?;
        let colon_token: Token![:] = input.parse()?;
        let ty: SynType = input.parse()?;
        
        let init = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        } else {
            None
        };

        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
        }
        Ok(GlobalDef { name, colon_token, ty, init })
    }
}

impl Parse for ConstDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![const]>()?;
        let name: Ident = parse_user_ident(input)?;
        input.parse::<Token![:]>()?;
        let ty: SynType = input.parse()?;
        input.parse::<Token![=]>()?;
        let value: Expr = input.parse()?;
        input.parse::<Token![;]>()?;
        Ok(ConstDef { name, ty, value })
    }
}

impl Parse for GenericParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![const]) {
            input.parse::<Token![const]>()?;
            let name: Ident = parse_user_ident(input)?;
            input.parse::<Token![:]>()?;
            let ty: SynType = input.parse()?;
            Ok(GenericParam::Const { name, ty })
        } else {
            let name: Ident = parse_user_ident(input)?;
            let constraint = if input.peek(Token![:]) {
                input.parse::<Token![:]>()?;
                Some(input.parse()?)
            } else {
                None
            };
            Ok(GenericParam::Type { name, constraint })
        }
    }
}

impl Parse for Generics {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _: Token![<] = input.parse()?;
        let mut params = Punctuated::new();
        while !input.peek(Token![>]) {
            let p: GenericParam = input.parse()?;
            params.push_value(p);
            if input.peek(Token![>]) {
                break;
            }
            let sep: Token![,] = input.parse()?;
            params.push_punct(sep);
        }
        let _: Token![>] = input.parse()?;
        Ok(Generics { params })
    }
}

impl Parse for StructDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse struct-level attributes (e.g., @atomic)
        let attributes = attr::parse_attributes(input)?;

        // Skip optional `pub`
        if input.peek(Token![pub]) {
            input.parse::<Token![pub]>()?;
        }

        input.parse::<Token![struct]>()?;
        let name: Ident = parse_user_ident(input)?;
        
        let generics = if input.peek(Token![<]) {
            Some(input.parse()?)
        } else {
            None
        };

        let content;
        syn::braced!(content in input);
        
        // Use Vec<FieldDef> parsing logic
        let mut fields = Vec::new();
        let mut invariants = Vec::new();

        while !content.is_empty() {
            if content.peek(Token![@]) {
                 let fork = content.fork();
                 fork.parse::<Token![@]>()?;
                 if fork.peek(invariant) {
                     content.parse::<Token![@]>()?;
                     content.parse::<invariant>()?;
                     while content.peek(crate::keywords::requires) {
                         content.parse::<crate::keywords::requires>()?;
                         let e: Expr = content.parse()?;
                         content.parse::<Token![;]>()?;
                         invariants.push(e);
                     }
                     continue;
                 }
            }

            fields.push(content.parse()?);
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }
        
        Ok(StructDef { attributes, name, generics, fields, invariants })
    }
}

impl Parse for FieldDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attributes = attr::parse_attributes(input)?;
        if input.peek(Token![pub]) {
            input.parse::<Token![pub]>()?;
        }
        let name: Ident = parse_user_ident(input)?;
        let colon_token: Token![:] = input.parse()?;
        let ty: SynType = input.parse()?;
        Ok(FieldDef { attributes, name, colon_token, ty })
    }
}

impl Parse for EnumDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![enum]>()?;
        let name: Ident = parse_user_ident(input)?;
        
        let generics = if input.peek(Token![<]) {
            Some(input.parse()?)
        } else {
            None
        };

        let content;
        syn::braced!(content in input);
        
        let mut variants = Vec::new();
        while !content.is_empty() {
            variants.push(content.parse()?);
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }
        
        Ok(EnumDef { name, generics, variants })
    }
}

impl Parse for EnumVariant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![pub]) {
            input.parse::<Token![pub]>()?;
        }
        let name: Ident = parse_user_ident(input)?;
        let ty = if input.peek(syn::token::Paren) {
             let content;
             parenthesized!(content in input);
             Some(content.parse()?)
        } else {
             None
        };
        Ok(EnumVariant { name, ty })
    }
}

impl Parse for SaltConcept {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<concept>()?;
        let name: Ident = parse_user_ident(input)?;
        
        // Parse <T>
        let generics = if input.peek(Token![<]) {
             Some(input.parse()?)
        } else {
            None
        };
        
        let outer_content;
        syn::braced!(outer_content in input);
        
        // Parse requires(v: T) { v > 0 }
        outer_content.parse::<requires>()?;
        let content;
        parenthesized!(content in outer_content);
        
        // v: T
        let param: Ident = parse_user_ident(&content)?;
        content.parse::<Token![:]>()?;
        let param_ty: SynType = content.parse()?;
        
        let body_content;
        syn::braced!(body_content in outer_content);
        let requires: Expr = body_content.parse()?;
        
        Ok(SaltConcept { name, generics, param, param_ty, requires })
    }
}

/// [V4.0] Parse trait method signature: `fn name(&self) -> RetType;`
impl Parse for TraitMethodSig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![fn]>()?;
        let name: Ident = parse_user_ident(input)?;
        
        // Parse generics <T>
        let generics = if input.peek(Token![<]) {
            Some(input.parse()?)
        } else {
            None
        };
        
        // Parse (args)
        let content;
        parenthesized!(content in input);
        let args: Punctuated<Arg, Token![,]> = Punctuated::parse_terminated(&content)?;
        
        // Parse -> RetType
        let ret_type = if input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        
        // Consume semicolon
        input.parse::<Token![;]>()?;
        
        Ok(TraitMethodSig { name, generics, args, ret_type })
    }
}

/// [V4.0] Parse trait definition: `trait Foo<T> { fn bar(&self) -> T; }`
impl Parse for SaltTrait {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![trait]>()?;
        let name: Ident = parse_user_ident(input)?;
        
        // Parse generics <T>
        let generics = if input.peek(Token![<]) {
            Some(input.parse()?)
        } else {
            None
        };
        
        // Parse { method signatures }
        let content;
        syn::braced!(content in input);
        
        let mut methods = Vec::new();
        while !content.is_empty() {
            methods.push(content.parse()?);
        }
        
        Ok(SaltTrait { name, generics, methods })
    }
}

impl Parse for SaltImpl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![impl]>()?;
        
        // Parse <T>
        let generics = if input.peek(Token![<]) {
             Some(input.parse()?)
        } else {
            None
        };

        // Collect tokens until brace or semicolon, tracking 'for' keyword
        let mut before_for = TokenStream::new();
        let mut after_for = TokenStream::new();
        let mut found_for = false;
        
        while !input.peek(syn::token::Brace) && !input.peek(Token![;]) && !input.is_empty() {
            let tt: TokenTree = input.parse()?;
            
            // Check for 'for' keyword
            if let TokenTree::Ident(ref ident) = tt {
                if ident == "for" && !found_for {
                    found_for = true;
                    continue;
                }
            }
            
            if found_for {
                after_for.extend(std::iter::once(tt));
            } else {
                before_for.extend(std::iter::once(tt));
            }
        }
        
        if input.peek(Token![;]) {
            // impl Concept<Type>;
            input.parse::<Token![;]>()?;
            let target_ty_full: SynType = syn::parse2(if found_for { after_for } else { before_for })?;
            
            if let SynType::Path(tp) = &target_ty_full {
                // Simplified Concept Extraction for SynType::Path
                let segment = tp.segments.last().unwrap();
                let concept_name = segment.ident.clone();
                if let Some(first_arg) = segment.args.first() {
                     return Ok(SaltImpl::Concept { concept_name, target_ty: first_arg.clone() });
                }
            }
            return Err(input.error("Invalid concept implementation syntax, expected Concept<Type>;"));
        } else if found_for {
            // [V4.0] impl Trait for Type { ... }
            let trait_name: Ident = syn::parse2(before_for)?;
            let target_ty: SynType = syn::parse2(after_for)?;
            
            let content;
            syn::braced!(content in input);
            let mut methods = Vec::new();
            while !content.is_empty() {
                methods.push(content.parse()?);
            }
            Ok(SaltImpl::Trait { trait_name, target_ty, methods, generics })
        } else {
            // impl Type { ... }
            let target_ty_full: SynType = syn::parse2(before_for)?;
            let content;
            syn::braced!(content in input);
            let mut methods = Vec::new();
            while !content.is_empty() {
                methods.push(content.parse()?);
            }
            Ok(SaltImpl::Methods { target_ty: target_ty_full, methods, generics })
        }
    }
}

impl Parse for ExternFnDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attributes = attr::parse_attributes(input)?;


        let is_pub = if input.peek(Token![pub]) {
            input.parse::<Token![pub]>()?;
            true
        } else {
            false
        };

        input.parse::<Token![extern]>()?;
        input.parse::<Token![fn]>()?;
        let name: Ident = parse_user_ident(input)?;
        
        let content;
        parenthesized!(content in input);
        let args: Punctuated<Arg, Token![,]> = content.parse_terminated(Arg::parse, Token![,])?;
        
        let ret_type = if input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        
        // Parse Contract: requires { ... } or requires expr;
        let mut requires = Vec::new();
        while input.peek(crate::keywords::requires) {
            input.parse::<crate::keywords::requires>()?;
            // Check if braced block (old syntax?) or expr
            if input.peek(syn::token::Brace) {
                 let content;
                 syn::braced!(content in input);
                 let e: Expr = content.parse()?;
                 requires.push(e);
            } else {
                 let e: Expr = input.parse()?;
                 if input.peek(Token![;]) {
                     input.parse::<Token![;]>()?;
                 }
                 requires.push(e);
            }
        }

        // Consume trailing semicolon
        input.parse::<Token![;]>()?;
        
        Ok(ExternFnDecl { attributes, is_pub, name, args, ret_type, requires })
    }
}

impl Parse for SaltFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attributes = attr::parse_attributes(input)?;


        let is_pub = if input.peek(Token![pub]) {
            input.parse::<Token![pub]>()?;
            true
        } else {
            false
        };

        input.parse::<Token![fn]>()?;
        let name: Ident = parse_user_ident(input)?;

        // Parse <T, R>
        let generics = if input.peek(Token![<]) {
             Some(input.parse()?)
        } else {
            None
        };
        
        let content;
        parenthesized!(content in input);
        let args: Punctuated<Arg, Token![,]> = content.parse_terminated(Arg::parse, Token![,])?;

        let ret_type = if input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            Some(input.parse()?)
        } else {
            None
        };

        // Parse Contract: requires expr
        let mut requires = Vec::new();
        while input.peek(crate::keywords::requires) {
            input.parse::<crate::keywords::requires>()?;
            let e: Expr = input.parse()?;
             if input.peek(Token![;]) {
                 input.parse::<Token![;]>()?;
             }
            requires.push(e);
        }

        // [V4.0] Parse ensures clause (postconditions)
        let mut ensures = Vec::new();
        while input.peek(crate::keywords::ensures) {
            input.parse::<crate::keywords::ensures>()?;
            let e: Expr = input.parse()?;
            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
            }
            ensures.push(e);
        }

        let body: SaltBlock = input.parse()?;
        Ok(SaltFn { attributes, is_pub, name, generics, args, ret_type, requires, ensures, body })
    }
}

impl Parse for SaltBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);
        let mut stmts = Vec::new();
        while !content.is_empty() {
             stmts.push(content.parse()?);
        }
        Ok(SaltBlock { stmts })
    }
}

impl Parse for Stmt {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fork = input.fork();
        if fork.parse::<crate::keywords::salt_return>().is_ok() {
             input.parse::<crate::keywords::salt_return>()?;
             if input.peek(Token![;]) {
                 input.parse::<Token![;]>()?;
                 return Ok(Stmt::Return(None));
             } else {
                 let e: Expr = input.parse()?;
                 input.parse::<Token![;]>()?;
                 return Ok(Stmt::Return(Some(e)));
             }
        }
    
        if input.peek(invariant) {
            input.parse::<invariant>()?;
            let expr: Expr = input.parse()?;
            input.parse::<Token![;]>()?;
            return Ok(Stmt::Invariant(expr));
        }


        // Check for @decorator or [attribute]
        // Check for @decorator
        if input.peek(Token![@]) {
            // V2.0: Loop-level decorators removed (@pulse eliminated)
            // @yielding is now function-level only
            return Err(input.error("Loop-level decorators removed in V2.0. Use @yielding on functions instead."));
        }

        if input.peek(Token![loop]) {
            input.parse::<Token![loop]>()?;
            let body: SaltBlock = input.parse()?;
            return Ok(Stmt::Loop(body));
        }

        if input.peek(Token![while]) {
            return Ok(Stmt::While(input.parse()?));
        }

        if input.peek(Token![for]) {
            return Ok(Stmt::For(input.parse()?));
        }


        if input.peek(Token![if]) {
            return Ok(Stmt::If(input.parse()?));
        }

        // Match expression: match expr { pattern => body, ... }
        if input.peek(Token![match]) {
            return Ok(Stmt::Match(input.parse()?));
        }

        if input.peek(Token![move]) {
             input.parse::<Token![move]>()?;
             let expr: Expr = input.parse()?;
             // Consume semicolon
             input.parse::<Token![;]>()?;
             return Ok(Stmt::Move(expr));
        }

         if input.peek(crate::keywords::with) {
              input.parse::<crate::keywords::with>()?;
              let region: Ident = input.parse()?;
              let body: SaltBlock = input.parse()?;
              return Ok(Stmt::WithRegion { region, body });
         }

         if input.peek(Token![unsafe]) {
              input.parse::<Token![unsafe]>()?;
              let body: SaltBlock = input.parse()?;
              return Ok(Stmt::Unsafe(body));
         }

         if input.peek(Token![break]) {
              input.parse::<Token![break]>()?;
              input.parse::<Token![;]>()?;
              return Ok(Stmt::Break);
         }

         if input.peek(Token![continue]) {
              input.parse::<Token![continue]>()?;
              input.parse::<Token![;]>()?;
              return Ok(Stmt::Continue);
         }

         if input.peek(crate::keywords::region) {
             input.parse::<crate::keywords::region>()?;
             
             if input.peek(syn::token::Paren) {
                  // Form: region("name") { ... }
                  let content;
                  parenthesized!(content in input);
                  let _name: Expr = content.parse()?;
                  let body: SaltBlock = input.parse()?;
                  // Map to WithRegion for now
                  let dummy = Ident::new("region_block", proc_macro2::Span::call_site());
                  return Ok(Stmt::WithRegion { region: dummy, body });
             } else {
                  // Form: region name(addr, size) { ... }
                  let region: Ident = input.parse()?;
                  let content;
                  parenthesized!(content in input);
                  let addr: Expr = content.parse()?;
                  content.parse::<Token![,]>()?;
                  let size: Expr = content.parse()?;
                  let body: SaltBlock = input.parse()?;
                  return Ok(Stmt::MapWindow { addr, size, region, body });
             }
         }

         if input.peek(crate::keywords::map_window) {
             input.parse::<crate::keywords::map_window>()?;
             let content;
             parenthesized!(content in input);
             let addr: Expr = content.parse()?;
             content.parse::<Token![,]>()?;
             let size: Expr = content.parse()?;
             content.parse::<Token![,]>()?;
             let region: Ident = content.parse()?;
             input.parse::<Token![;]>()?;
             return Ok(Stmt::MapWindow { addr, size, region, body: SaltBlock { stmts: vec![] } });
        }

         // Let statement (handles both regular let and let-else)
         if input.peek(Token![let]) {
              // Try to parse as let-else first using lookahead
              let fork = input.fork();
              fork.parse::<Token![let]>()?;
              
              // Check if this could be a pattern by looking for `=` followed by `else`
              // Skip tokens until we find `=`
              let mut depth = 0;
              let mut could_be_let_else = false;
              while !fork.is_empty() {
                  if fork.peek(Token![<]) { depth += 1; }
                  if fork.peek(Token![>]) && depth > 0 { depth -= 1; }
                  if depth == 0 && fork.peek(Token![=]) {
                       fork.parse::<Token![=]>()?;
                       // KEY FIX: If the RHS starts with `if`, this is `let x = if ... { ... } else { ... }`
                       // which is a regular let with an if-else *expression* initializer, NOT a let-else.
                       // The `else` belongs to the if-expression, not to a destructuring let-else pattern.
                       if fork.peek(Token![if]) {
                           break; // Not a let-else, skip to syn parsing
                       }
                       // Now skip the expression until we find `else` or `;`
                       let mut expr_depth = 0;
                       while !fork.is_empty() {
                           if fork.peek(syn::token::Paren) || fork.peek(syn::token::Brace) || fork.peek(Token![<]) {
                               expr_depth += 1;
                           }
                           if (fork.peek(Token![>]) || fork.peek(syn::token::Paren) || fork.peek(syn::token::Brace)) && expr_depth > 0 {
                               expr_depth -= 1;
                           }
                           if expr_depth == 0 && fork.peek(Token![else]) {
                               could_be_let_else = true;
                               break;
                           }
                           if fork.peek(Token![;]) {
                               break;
                           }
                           let _ = fork.parse::<TokenTree>()?;
                       }
                       break;
                  }
                  let _ = fork.parse::<TokenTree>();
              }
              
              if could_be_let_else {
                  return Ok(Stmt::LetElse(input.parse()?));
              }
              
              return Ok(Stmt::Syn(input.parse()?));
         }

         // Fallback to Expr + Optional Semicolon
         let expr: Expr = input.parse()?;
         let mut has_semi = false;
         if input.peek(Token![;]) {
             input.parse::<Token![;]>()?;
             has_semi = true;
         }
         Ok(Stmt::Expr(expr, has_semi))
    }
}

impl Parse for SaltWhile {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![while]>()?;
        
        let mut tokens = TokenStream::new();
        while !input.peek(syn::token::Brace) && !input.is_empty() {
            tokens.extend(std::iter::once(input.parse::<TokenTree>()?));
        }
        let cond: Expr = syn::parse2(tokens)?;

        // Removed legacy stride parsing logic
        // SaltWhile no longer parses stride inside the body loop
        
        let body: SaltBlock = input.parse()?;
        Ok(SaltWhile { cond, body })
    }
}

impl Parse for SaltFor {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![for]>()?;
        let pat: syn::Pat = syn::Pat::parse_single(input)?;
        input.parse::<Token![in]>()?;
        
        let mut tokens = TokenStream::new();
        while !input.peek(syn::token::Brace) && !input.is_empty() {
            tokens.extend(std::iter::once(input.parse::<TokenTree>()?));
        }
        let iter: Expr = syn::parse2(tokens)?;
        let body: SaltBlock = input.parse()?;
        Ok(SaltFor { pat, iter, body })
    }
}

impl Parse for SaltIf {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![if]>()?;
        
        // Scan for the opening brace of the block to isolate condition
        // This avoids 'Struct { ... }' ambiguity
        let mut cond_tokens = proc_macro2::TokenStream::new();
        loop {
            if input.peek(syn::token::Brace) {
                break;
            }
            if input.is_empty() {
                return Err(input.error("Expected block after if condition"));
            }
            cond_tokens.extend(std::iter::once(input.parse::<proc_macro2::TokenTree>()?));
        }
        
        // Parse condition from isolated tokens
        let cond: Expr = syn::parse2(cond_tokens)?;
        let then_branch: SaltBlock = input.parse()?;
        
        let else_branch = if input.peek(Token![else]) {
            input.parse::<Token![else]>()?;
            if input.peek(Token![if]) {
                 let nested: SaltIf = input.parse()?;
                 Some(Box::new(SaltElse::If(Box::new(nested))))
            } else {
                 let block: SaltBlock = input.parse()?;
                 Some(Box::new(SaltElse::Block(block)))
            }
        } else {
            None
        };
        
        Ok(SaltIf { cond, then_branch, else_branch })
    }
}

impl Parse for Arg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![&]) || input.peek(Token![self]) || input.peek(Token![mut]) {
            let mut is_ref = false;
            let mut is_mut = false;
            
            if input.peek(Token![&]) {
                input.parse::<Token![&]>()?;
                is_ref = true;
                if input.peek(Token![mut]) {
                    input.parse::<Token![mut]>()?;
                    is_mut = true;
                }
            } else if input.peek(Token![mut]) {
                input.parse::<Token![mut]>()?;
                is_mut = true;
            }

            if input.peek(Token![self]) {
                input.parse::<Token![self]>()?;
                let name = Ident::new("self", proc_macro2::Span::call_site());
                
                // Construct Type
                let mut ty = SynType::from_std(parse_quote!(Self)).unwrap();
                if is_ref {
                     ty = SynType::Reference(Box::new(ty), is_mut);
                }

                if input.peek(Token![:]) {
                    input.parse::<Token![:]>()?;
                    let explicit_ty: SynType = input.parse()?;
                    return Ok(Arg { name, ty: Some(explicit_ty) });
                }
                return Ok(Arg { name, ty: Some(ty) });
            }
        }
        
        let name: Ident = parse_user_ident(input)?;
        input.parse::<Token![:]>()?;
        let ty: SynType = input.parse()?;
        Ok(Arg { name, ty: Some(ty) })
    }
}

/// Parse match expression: `match scrutinee { pattern => body, ... }`
impl Parse for SaltMatch {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![match]>()?;
        
        // Parse scrutinee expression (everything before the brace)
        let mut scrutinee_tokens = TokenStream::new();
        while !input.peek(syn::token::Brace) && !input.is_empty() {
            scrutinee_tokens.extend(std::iter::once(input.parse::<TokenTree>()?));
        }
        let scrutinee: Expr = syn::parse2(scrutinee_tokens)?;
        
        // Parse arms inside braces
        let content;
        syn::braced!(content in input);
        
        let mut arms = Vec::new();
        while !content.is_empty() {
            arms.push(content.parse()?);
            // Optional trailing comma
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }
        
        Ok(SaltMatch { scrutinee, arms })
    }
}

/// Parse match arm: `pattern => body` or `pattern if guard => body`
impl Parse for MatchArm {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let pattern: Pattern = input.parse()?;
        
        // Optional guard: `if condition`
        let guard = if input.peek(Token![if]) {
            input.parse::<Token![if]>()?;
            // Collect tokens until `=>`
            let mut guard_tokens = TokenStream::new();
            while !input.peek(Token![=>]) && !input.is_empty() {
                guard_tokens.extend(std::iter::once(input.parse::<TokenTree>()?));
            }
            Some(syn::parse2(guard_tokens)?)
        } else {
            None
        };
        
        input.parse::<Token![=>]>()?;
        
        // Body can be either a block `{ ... }` or a single expression
        let body = if input.peek(syn::token::Brace) {
            input.parse()?
        } else {
            // Single expression - wrap in a block
            let mut expr_tokens = TokenStream::new();
            let mut depth = 0;
            while !input.is_empty() {
                // Track brace/paren depth
                if input.peek(syn::token::Brace) || input.peek(syn::token::Paren) || input.peek(syn::token::Bracket) {
                    depth += 1;
                }
                
                // Stop at comma (arm separator) if at depth 0
                if depth == 0 && input.peek(Token![,]) {
                    break;
                }
                
                let token: TokenTree = input.parse()?;
                
                // Decrement depth on closing
                if matches!(&token, TokenTree::Group(g) if 
                    matches!(g.delimiter(), proc_macro2::Delimiter::Brace | proc_macro2::Delimiter::Parenthesis | proc_macro2::Delimiter::Bracket)) {
                    if depth > 0 { depth -= 1; }
                }
                
                expr_tokens.extend(std::iter::once(token));
            }
            
            let expr: Expr = syn::parse2(expr_tokens)?;
            SaltBlock { stmts: vec![Stmt::Expr(expr, false)] }
        };
        
        Ok(MatchArm { pattern, guard, body })
    }
}

/// Parse let-else: `let pattern = expr else { diverging_block };`
impl Parse for LetElse {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![let]>()?;
        
        // Parse pattern
        let pattern: Pattern = input.parse()?;
        
        input.parse::<Token![=]>()?;
        
        // Parse initializer expression (until `else`)
        let mut init_tokens = TokenStream::new();
        while !input.peek(Token![else]) && !input.is_empty() {
            init_tokens.extend(std::iter::once(input.parse::<TokenTree>()?));
        }
        let init: Expr = syn::parse2(init_tokens)?;
        
        input.parse::<Token![else]>()?;
        
        // Parse else block (must diverge)
        let else_block: SaltBlock = input.parse()?;
        
        // Optional trailing semicolon
        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
        }
        
        Ok(LetElse { pattern, init, else_block })
    }
}
