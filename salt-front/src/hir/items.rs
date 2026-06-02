use crate::hir::ids::DefId;
use crate::hir::types::Type;
use crate::hir::expr::Block;

#[derive(Clone, Debug)]
pub struct Item {
    pub id: DefId,
    pub name: String,
    pub vis: Visibility,
    pub kind: ItemKind,
    pub span: proc_macro2::Span,
}

#[derive(Clone, Debug)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Fn(Fn),
    Struct(Struct),
    Enum(Enum),
    Trait(Trait),
    Impl(Impl),
    Global(Global),
}

#[derive(Clone, Debug)]
pub struct Fn {
    pub inputs: Vec<Param>,
    pub output: Type,
    pub body: Option<Block>, // Option for trait methods without body
    pub generics: Generics,
    pub is_async: bool,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Clone, Debug)]
pub struct Struct {
    pub fields: Vec<Field>,
    pub generics: Generics,
    pub invariants: Vec<crate::hir::expr::Expr>,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub vis: Visibility,
}

#[derive(Clone, Debug)]
pub struct Enum {
    pub variants: Vec<Variant>,
    pub generics: Generics,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub name: String,
    pub data: VariantData,
}

#[derive(Clone, Debug)]
pub enum VariantData {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<Field>),
}

#[derive(Clone, Debug)]
pub struct Trait {
    pub generics: Generics,
    pub items: Vec<TraitItem>,
}

#[derive(Clone, Debug)]
pub enum TraitItem {
    Fn { name: String, func: Fn },
    // Const, Type, etc.
}

#[derive(Clone, Debug)]
pub struct Impl {
    pub generics: Generics,
    pub trait_ref: Option<Type>, // The trait being implemented, if any
    pub self_ty: Type,
    pub items: Vec<ImplItem>,
}

#[derive(Clone, Debug)]
pub enum ImplItem {
    Fn { name: String, func: Fn },
}

#[derive(Clone, Debug)]
pub struct Global {
    pub ty: Type,
    pub init: Option<crate::hir::expr::Expr>,
}

#[derive(Clone, Debug, Default)]
pub struct Generics {
    pub params: Vec<GenericParam>,
}

#[derive(Clone, Debug)]
pub enum GenericParam {
    Type(String),
    Const(String, Type),
}
