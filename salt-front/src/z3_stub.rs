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
    pub fn real(_ctx: &'a Context) -> Self { Sort(PhantomData) }
    pub fn bool(_ctx: &'a Context) -> Self { Sort(PhantomData) }
    pub fn string(_ctx: &'a Context) -> Self { Sort(PhantomData) }
    pub fn bitvector(_ctx: &'a Context, _sz: u32) -> Self { Sort(PhantomData) }
    pub fn array(_ctx: &'a Context, _domain: &Sort<'a>, _range: &Sort<'a>) -> Self { Sort(PhantomData) }
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
        pub fn as_array(&self) -> Option<Array<'a>> { Some(Array(PhantomData)) }
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
        pub fn to_real(&self) -> Real<'a> { Real(PhantomData) }

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

    // ─── Array ──────────────────────────────────────────────────────────

    #[derive(Debug, Clone)]
    pub struct Array<'a>(pub(crate) PhantomData<&'a ()>);

    impl<'a> Array<'a> {
        pub fn new_const(_ctx: &'a Context, _name: impl Into<Symbol>, _domain: &Sort<'a>, _range: &Sort<'a>) -> Self { Array(PhantomData) }
        pub fn store(&self, _index: &Int<'a>, _value: &Int<'a>) -> Array<'a> { Array(PhantomData) }
        pub fn select(&self, _index: &Int<'a>) -> Dynamic<'a> { Dynamic(PhantomData) }
    }

    impl<'a> fmt::Display for Array<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "<z3_stub::Array>") }
    }

    impl<'a> Ast for Array<'a> {}

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
        pub fn substitute<T: Ast>(&self, _pairs: &[(&T, &T)]) -> Bool<'a> { Bool(PhantomData) }

        // z3::ast::Ast trait methods as inherent methods
        pub fn _eq(&self, _other: &Bool<'a>) -> Bool<'a> { Bool(PhantomData) }
    }

    #[derive(Debug, Clone)]
    pub struct Pattern<'a>(pub(crate) PhantomData<&'a ()>);

    impl<'a> Pattern<'a> {
        pub fn new(_ctx: &'a Context, _terms: &[&dyn Ast]) -> Self { Pattern(PhantomData) }
    }

    impl<'a> fmt::Display for Pattern<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "<z3_stub::Pattern>") }
    }

    pub fn forall_const<'a>(_ctx: &'a Context, _bound: &[&dyn Ast], _patterns: &[&Pattern<'a>], _body: &Bool<'a>) -> Bool<'a> { Bool(PhantomData) }
    pub fn exists_const<'a>(_ctx: &'a Context, _bound: &[&dyn Ast], _patterns: &[&Pattern<'a>], _body: &Bool<'a>) -> Bool<'a> { Bool(PhantomData) }

    impl<'a> fmt::Display for Bool<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "<z3_stub::Bool>")
        }
    }

    impl<'a> Ast for Bool<'a> {}

    // ─── Real (exact rational arithmetic) ──────────────────────────

    #[derive(Debug, Clone)]
    pub struct Real<'a>(pub(crate) PhantomData<&'a ()>);

    impl<'a> Real<'a> {
        pub fn new_const(_ctx: &'a Context, _name: impl Into<String>) -> Self { Real(PhantomData) }
        pub fn fresh_const(_ctx: &'a Context, _prefix: &str) -> Self { Real(PhantomData) }
        pub fn from_int(_int: &Int<'a>) -> Self { Real(PhantomData) }
        pub fn from_real_str(_ctx: &'a Context, _num: &str, _den: &str) -> Option<Self> { Some(Real(PhantomData)) }
        pub fn to_real(&self) -> Real<'a> { Real(PhantomData) }

        // Comparisons
        pub fn lt(&self, _other: &Real<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn le(&self, _other: &Real<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn gt(&self, _other: &Real<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn ge(&self, _other: &Real<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn _eq(&self, _other: &Real<'a>) -> Bool<'a> { Bool(PhantomData) }
    }

    impl<'a> fmt::Display for Real<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "<z3_stub::Real>") }
    }

    impl<'a> Ast for Real<'a> {}

    impl<'a> std::ops::Add for &Real<'a> { type Output = Real<'a>; fn add(self, _: &Real<'a>) -> Real<'a> { Real(PhantomData) } }
    impl<'a> std::ops::Sub for &Real<'a> { type Output = Real<'a>; fn sub(self, _: &Real<'a>) -> Real<'a> { Real(PhantomData) } }
    impl<'a> std::ops::Mul for &Real<'a> { type Output = Real<'a>; fn mul(self, _: &Real<'a>) -> Real<'a> { Real(PhantomData) } }
    impl<'a> std::ops::Div for &Real<'a> { type Output = Real<'a>; fn div(self, _: &Real<'a>) -> Real<'a> { Real(PhantomData) } }
    impl<'a> std::ops::Neg for &Real<'a> { type Output = Real<'a>; fn neg(self) -> Real<'a> { Real(PhantomData) } }

    // ─── BV (bitvector) ────────────────────────────────────────────

    #[derive(Debug, Clone)]
    pub struct BV<'a>(pub(crate) PhantomData<&'a ()>);

    impl<'a> BV<'a> {
        pub fn new_const(_ctx: &'a Context, _name: impl Into<String>, _sz: u32) -> Self { BV(PhantomData) }
        pub fn fresh_const(_ctx: &'a Context, _prefix: &str, _sz: u32) -> Self { BV(PhantomData) }
        pub fn from_i64(_ctx: &'a Context, _val: i64, _sz: u32) -> Self { BV(PhantomData) }
        pub fn from_u64(_ctx: &'a Context, _val: u64, _sz: u32) -> Self { BV(PhantomData) }
        pub fn as_i64(&self) -> Option<i64> { None }
        pub fn to_int(&self, _signed: bool) -> Int<'a> { Int(PhantomData) }
        pub fn from_int(_ast: &Int<'a>, _sz: u32) -> Self { BV(PhantomData) }
        pub fn get_size(&self) -> u32 { 0 }
        // Comparisons
        pub fn _eq(&self, _other: &BV<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn bvult(&self, _other: &BV<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn bvslt(&self, _other: &BV<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn bvule(&self, _other: &BV<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn bvsle(&self, _other: &BV<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn bvuge(&self, _other: &BV<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn bvsge(&self, _other: &BV<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn bvugt(&self, _other: &BV<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn bvsgt(&self, _other: &BV<'a>) -> Bool<'a> { Bool(PhantomData) }
        // Arithmetic
        pub fn bvadd(&self, _other: &BV<'a>) -> BV<'a> { BV(PhantomData) }
        pub fn bvsub(&self, _other: &BV<'a>) -> BV<'a> { BV(PhantomData) }
        pub fn bvmul(&self, _other: &BV<'a>) -> BV<'a> { BV(PhantomData) }
        // Bitwise
        pub fn bvand(&self, _other: &BV<'a>) -> BV<'a> { BV(PhantomData) }
        pub fn bvor(&self, _other: &BV<'a>) -> BV<'a> { BV(PhantomData) }
        pub fn bvxor(&self, _other: &BV<'a>) -> BV<'a> { BV(PhantomData) }
        pub fn bvnot(&self) -> BV<'a> { BV(PhantomData) }
        // Shifts
        pub fn bvshl(&self, _other: &BV<'a>) -> BV<'a> { BV(PhantomData) }
        pub fn bvlshr(&self, _other: &BV<'a>) -> BV<'a> { BV(PhantomData) }
        pub fn bvashr(&self, _other: &BV<'a>) -> BV<'a> { BV(PhantomData) }
    }

    impl<'a> fmt::Display for BV<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "<z3_stub::BV>") }
    }

    impl<'a> Ast for BV<'a> {}

    // ─── Z3 String ──────────────────────────────────────────────────

    #[derive(Debug, Clone)]
    pub struct String<'a>(pub(crate) PhantomData<&'a ()>);

    impl<'a> String<'a> {
        pub fn new_const(_ctx: &'a Context, _name: impl Into<std::string::String>) -> Self { String(PhantomData) }
        pub fn fresh_const(_ctx: &'a Context, _prefix: &str) -> Self { String(PhantomData) }
        pub fn from_str(_ctx: &'a Context, _s: &str) -> Result<Self, std::ffi::NulError> { Ok(String(PhantomData)) }
        pub fn length(&self) -> Int<'a> { Int(PhantomData) }
        pub fn _eq(&self, _other: &String<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn contains(&self, _substr: &String<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn prefix(&self, _prefix: &String<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn suffix(&self, _suffix: &String<'a>) -> Bool<'a> { Bool(PhantomData) }
        pub fn substr(&self, _offset: &Int<'a>, _length: &Int<'a>) -> String<'a> { String(PhantomData) }
        pub fn regex_matches(&self, _regex: &Regexp<'a>) -> Bool<'a> { Bool(PhantomData) }
    }

    impl<'a> fmt::Display for String<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "<z3_stub::String>") }
    }

    impl<'a> Ast for String<'a> {}

    // ─── Regexp ─────────────────────────────────────────────────────

    #[derive(Debug, Clone)]
    pub struct Regexp<'a>(pub(crate) PhantomData<&'a ()>);

    impl<'a> Regexp<'a> {
        pub fn literal(_ctx: &'a Context, _pattern: &str) -> Self { Regexp(PhantomData) }
    }

    impl<'a> fmt::Display for Regexp<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "<z3_stub::Regexp>") }
    }

    impl<'a> Ast for Regexp<'a> {}
}
