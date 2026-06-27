//! Type system utilities for the Salt codegen module.
//!
//! This module provides:
//! - `canonical`: TypeID system for O(1) type identity comparison
//! - `provenance`: Pointer provenance tracking for GEP optimization

pub mod canonical;
pub mod provenance;
pub mod layout;
pub mod substitution;

pub use canonical::{TypeID, TypeIDRegistry};
pub use provenance::{ProvenanceMap, OriginMap, GlobalLVN};
