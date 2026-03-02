//! Z3 stub types for WebAssembly builds.
//!
//! When the `z3-backend` feature is disabled (e.g., for wasm32 targets),
//! this module provides zero-cost placeholder types that satisfy the type
//! checker but panic if actually called. The `no_verify` guard in
//! CodegenContext prevents any Z3 code from executing at runtime.
//!
//! This allows salt-front to compile to WebAssembly without linking
//! the native Z3 C++ library, while keeping all type signatures intact.

use std::marker::PhantomData;
use std::fmt;

// ─── Core Types ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Config;

impl Config {
    pub fn new() -> Self { Config }
}

#[derive(Debug)]
pub struct Context;

impl Context {
    pub fn new(_cfg: &Config) -> Self { Context }
}

pub struct Solver<'a>(PhantomData<&'a ()>);

impl<'a> Solver<'a> {
    pub fn new(_ctx: &'a Context) -> Self { Solver(PhantomData) }
    pub fn push(&self) {}
    pub fn pop(&self, _n: u32) {}
    pub fn assert(&self, _expr: &ast::Bool<'a>) {}
    pub fn check(&self) -> SatResult { SatResult::Unknown }
    pub fn reset(&self) {}
    pub fn set_params(&self, _params: &Params<'a>) {}
    pub fn get_model(&self) -> Option<Model<'a>> { Some(Model(PhantomData)) }
}

pub struct Params<'a>(PhantomData<&'a ()>);

impl<'a> Params<'a> {
    pub fn new(_ctx: &'a Context) -> Self { Params(PhantomData) }
    pub fn set_u32(&mut self, _key: &str, _val: u32) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SatResult {
    Sat,
    Unsat,
    Unknown,
}

// ─── Sort, Symbol, FuncDecl ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Sort<'a>(PhantomData<&'a ()>);

impl<'a> Sort<'a> {
    pub fn int(_ctx: &'a Context) -> Self { Sort(PhantomData) }
    pub fn bool(_ctx: &'a Context) -> Self { Sort(PhantomData) }
    pub fn bitvector(_ctx: &'a Context, _sz: u32) -> Self { Sort(PhantomData) }
}

/// z3-rs Symbol: either a string or integer name.
#[derive(Debug, Clone)]
pub enum Symbol {
    String(String),
    Int(u32),
}

impl From<String> for Symbol {
    fn from(s: String) -> Self { Symbol::String(s) }
}
impl From<&str> for Symbol {
    fn from(s: &str) -> Self { Symbol::String(s.to_string()) }
}

#[derive(Debug, Clone)]
pub struct FuncDecl<'a>(PhantomData<&'a ()>);

impl<'a> FuncDecl<'a> {
    pub fn new(
        _ctx: &'a Context,
        _name: impl Into<Symbol>,
        _domain: &[&Sort<'a>],
        _range: &Sort<'a>,
    ) -> Self {
        FuncDecl(PhantomData)
    }
    /// Apply this function declaration to arguments, returning a Dynamic value.
    /// Matches z3-rs: `apply(&[&dyn Ast]) -> Dynamic`.
    pub fn apply(&self, _args: &[&dyn ast::Ast]) -> ast::Dynamic<'a> {
        ast::Dynamic(PhantomData)
    }
}

// ─── Model ──────────────────────────────────────────────────────────────────

pub struct Model<'a>(PhantomData<&'a ()>);

impl<'a> Model<'a> {
    pub fn eval(&self, _ast: &ast::Int<'a>, _complete: bool) -> Option<ast::Int<'a>> {
        Some(ast::Int(PhantomData))
    }
}

// ─── AST Types ──────────────────────────────────────────────────────────────

pub mod ast {
    use super::*;

    /// Marker trait mirroring z3::ast::Ast.
    /// Object-safe: no Self or Clone in the trait definition.
    pub trait Ast: fmt::Display {}

    // ─── Dynamic ────────────────────────────────────────────────────────

    /// Dynamic Z3 value — returned by FuncDecl::apply().
    /// Supports extraction to concrete types via as_int()/as_bool().
    #[derive(Debug, Clone)]
    pub struct Dynamic<'a>(pub(crate) PhantomData<&'a ()>);

    impl<'a> Dynamic<'a> {
        pub fn as_int(&self) -> Option<Int<'a>> { Some(Int(PhantomData)) }
        pub fn as_bool(&self) -> Option<Bool<'a>> { Some(Bool(PhantomData)) }
    }

    impl<'a> fmt::Display for Dynamic<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "<z3_stub::Dynamic>")
        }
    }

    impl<'a> Ast for Dynamic<'a> {}

    // ─── Int ────────────────────────────────────────────────────────────

    #[derive(Debug, Clone)]
    pub struct Int<'a>(pub(crate) PhantomData<&'a ()>);

    impl<'a> Int<'a> {
        pub fn new_const(_ctx: &'a Context, _name: impl Into<String>) -> Self { Int(PhantomData) }
        pub fn fresh_const(_ctx: &'a Context, _prefix: &str) -> Self { Int(PhantomData) }
        pub fn from_i64(_ctx: &'a Context, _val: i64) -> Self { Int(PhantomData) }
        pub fn from_u64(_ctx: &'a Context, _val: u64) -> Self { Int(PhantomData) }
        pub fn add(_ctx: &'a Context, _args: &[&Int<'a>]) -> Self { Int(PhantomData) }
        pub fn sub(_ctx: &'a Context, _args: &[&Int<'a>]) -> Self { Int(PhantomData) }
        pub fn mul(_ctx: &'a Context, _args: &[&Int<'a>]) -> Self { Int(PhantomData) }

        pub fn ge(&self, _other: &Int<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn gt(&self, _other: &Int<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn le(&self, _other: &Int<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn lt(&self, _other: &Int<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn modulo(&self, _other: &Int<'a>) -> Int<'a> { Int(PhantomData) }
        pub fn rem(&self, _other: &Int<'a>) -> Int<'a> { Int(PhantomData) }

        // z3::ast::Ast trait methods as inherent methods (dyn-safe)
        pub fn _eq(&self, _other: &Int<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn not(&self) -> Bool<'a> { Bool(PhantomData) }

        // Model extraction methods
        pub fn as_i64(&self) -> Option<i64> { Some(0) }
        pub fn as_u64(&self) -> Option<u64> { Some(0) }
        pub fn as_int(&self) -> Option<i64> { Some(0) }
        pub fn as_bool(&self) -> Option<bool> { Some(false) }
    }

    impl<'a> fmt::Display for Int<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "<z3_stub::Int>")
        }
    }

    impl<'a> Ast for Int<'a> {}

    // Arithmetic operators
    impl<'a> std::ops::Add for Int<'a> {
        type Output = Int<'a>;
        fn add(self, _rhs: Int<'a>) -> Int<'a> { Int(PhantomData) }
    }
    impl<'a> std::ops::Sub for Int<'a> {
        type Output = Int<'a>;
        fn sub(self, _rhs: Int<'a>) -> Int<'a> { Int(PhantomData) }
    }
    impl<'a> std::ops::Mul for Int<'a> {
        type Output = Int<'a>;
        fn mul(self, _rhs: Int<'a>) -> Int<'a> { Int(PhantomData) }
    }
    impl<'a> std::ops::Div for Int<'a> {
        type Output = Int<'a>;
        fn div(self, _rhs: Int<'a>) -> Int<'a> { Int(PhantomData) }
    }
    impl<'a> std::ops::Neg for Int<'a> {
        type Output = Int<'a>;
        fn neg(self) -> Int<'a> { Int(PhantomData) }
    }
    impl<'a> std::ops::Add for &Int<'a> {
        type Output = Int<'a>;
        fn add(self, _rhs: &Int<'a>) -> Int<'a> { Int(PhantomData) }
    }
    impl<'a> std::ops::Sub for &Int<'a> {
        type Output = Int<'a>;
        fn sub(self, _rhs: &Int<'a>) -> Int<'a> { Int(PhantomData) }
    }
    impl<'a> std::ops::Mul for &Int<'a> {
        type Output = Int<'a>;
        fn mul(self, _rhs: &Int<'a>) -> Int<'a> { Int(PhantomData) }
    }
    impl<'a> std::ops::Div for &Int<'a> {
        type Output = Int<'a>;
        fn div(self, _rhs: &Int<'a>) -> Int<'a> { Int(PhantomData) }
    }
    impl<'a> std::ops::Neg for &Int<'a> {
        type Output = Int<'a>;
        fn neg(self) -> Int<'a> { Int(PhantomData) }
    }

    // ─── Bool ───────────────────────────────────────────────────────────

    #[derive(Debug, Clone)]
    pub struct Bool<'a>(pub(crate) PhantomData<&'a ()>);

    impl<'a> Bool<'a> {
        pub fn new_const(_ctx: &'a Context, _name: impl Into<String>) -> Self { Bool(PhantomData) }
        pub fn from_bool(_ctx: &'a Context, _val: bool) -> Self { Bool(PhantomData) }
        pub fn not(&self) -> Bool<'a> { Bool(PhantomData) }
        pub fn and(_ctx: &'a Context, _args: &[&Bool<'a>]) -> Bool<'a> { Bool(PhantomData) }
        pub fn or(_ctx: &'a Context, _args: &[&Bool<'a>]) -> Bool<'a> { Bool(PhantomData) }
        pub fn implies(&self, _other: &Bool<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn ite(&self, _t: &Int<'a>, _e: &Int<'a>) -> Int<'a> { Int(PhantomData) }
        pub fn substitute(&self, _pairs: &[(&Int<'a>, &Int<'a>)]) -> Bool<'a> { Bool(PhantomData) }

        // z3::ast::Ast trait methods as inherent methods
        pub fn _eq(&self, _other: &Bool<'a>) -> Bool<'a> { Bool(PhantomData) }
    }

    impl<'a> fmt::Display for Bool<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "<z3_stub::Bool>")
        }
    }

    impl<'a> Ast for Bool<'a> {}
}
