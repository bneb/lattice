//! High-Level Intermediate Representation (HIR)
//!
//! The HIR is a typed, semantic representation of the Salt program.
//! It is generated from the AST and used for type checking, trait resolution,
//! and eventually lower to MLIR.

pub mod ids;
pub mod items;
pub mod expr;
pub mod stmt;
pub mod types;
pub mod scope;
pub mod lower;
pub mod typeck;
pub mod vc;
pub mod async_lower;
pub mod verify_pulse_bounds;
