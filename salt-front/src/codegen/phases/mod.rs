//! Phased State Containers for CodegenContext
//!
//! This module organizes CodegenContext's 50+ RefCell fields into logical phases:
//! - `DiscoveryState`: Templates, registries, imports (read-mostly)
//! - `ExpansionState`: Monomorphization, specializations (write-heavy)
//! - `EmissionState`: MLIR buffers, counters, caches
//! - `VerificationState`: Z3 solver, symbolic tracking
//! - `ControlFlowState`: Loop labels, cleanup stack

pub mod discovery;
pub mod expansion;
pub mod emission;
pub mod verification;
pub mod control_flow;
pub mod resolution;
pub mod purity;

pub use discovery::DiscoveryState;
pub use expansion::{ExpansionState, MonomorphizerState, SpecializationTask};
pub use emission::{EmissionState, StringInterner, TensorLayout};
pub use verification::VerificationState;
pub use control_flow::{ControlFlowState, CleanupTask};
