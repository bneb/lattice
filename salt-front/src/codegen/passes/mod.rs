//! KeuOS Compiler Passes
//!
//! This module contains the compiler passes for the Pulsed Capability model:
//! - `pulse_injection`: Injects Context parameters into @pulse functions
//! - `yield_injection`: Inserts deadline checks at loop back-edges
//! - `sync_verifier`: Z3 verification that sync functions don't perform I/O
//! - `call_graph`: Fixed-point call graph analysis for blocking/context propagation
//! - `liveness`: Cross-yield variable liveness analysis
//! - `async_to_state`: Coroutine-to-state-machine transformation
//! - `io_backend`: Hardware-agnostic I/O backend (kqueue/io_uring)

pub mod pulse_injection;
pub mod yield_injection;
pub mod yield_mlir;
pub mod sync_verifier;
pub mod call_graph;
#[cfg(test)]
mod call_graph_tests;
pub mod liveness;
pub mod async_to_state;
pub mod io_backend;
pub mod lowering_config;
pub mod entry_point;
pub mod binary_audit;
pub mod loop_invariant;
