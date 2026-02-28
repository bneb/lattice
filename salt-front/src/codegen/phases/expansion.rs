//! Phase 2: Expansion State
//! Contains monomorphization and specialization state - write-heavy during expansion.

use std::collections::{BTreeMap, HashMap, VecDeque};
use crate::types::Type;
use crate::codegen::collector::MonomorphizationTask;
use crate::evaluator::Evaluator;

/// Monomorphizer work queue state
#[derive(Default)]
pub struct MonomorphizerState {
    pub work_queue: VecDeque<SpecializationTask>,
    pub pending_set: std::collections::HashSet<String>,
    pub is_frozen: bool,
}

impl MonomorphizerState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A pending type specialization task
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpecializationTask {
    pub template_name: String,
    pub args: Vec<Type>,
    pub mangled_name: String,
    pub is_enum: bool,
}

/// Phase 2: Monomorphization and specialization (write-heavy)
#[derive(Default)]
pub struct ExpansionState {
    /// Cached specializations: (template_name, type_args) -> mangled_name
    pub specializations: HashMap<(String, Vec<Type>), String>,
    /// Pending monomorphization tasks
    pub pending_generations: VecDeque<MonomorphizationTask>,
    /// Monomorphizer work queue state
    pub monomorphizer: MonomorphizerState,
    /// Current type substitution map (generic name -> concrete type)
    pub current_type_map: BTreeMap<String, Type>,
    /// Current generic arguments being processed
    pub current_generic_args: Vec<Type>,
    /// Current Self type in impl block
    pub current_self_ty: Option<Type>,
    /// Current return type being compiled
    pub current_ret_ty: Option<Type>,
    /// [v0.9.2] Current postcondition (ensures) expressions for Z3 verification at return sites
    pub current_ensures: Vec<syn::Expr>,
    /// Current function name being compiled
    pub current_fn_name: String,
    // --- Absorbed from CodegenContext façade ---
    /// Comptime expression evaluator (const-fold, enum discriminants)
    pub evaluator: Evaluator,
    /// Suppress monomorphization during pre-scan phase
    pub suppress_specialization: bool,
}

impl ExpansionState {
    pub fn new() -> Self {
        Self {
            specializations: HashMap::new(),
            pending_generations: VecDeque::new(),
            monomorphizer: MonomorphizerState::new(),
            current_type_map: BTreeMap::new(),
            current_generic_args: Vec::new(),
            current_self_ty: None,
            current_ret_ty: None,
            current_ensures: Vec::new(),
            current_fn_name: String::new(),
            evaluator: Evaluator::new(),
            suppress_specialization: false,
        }
    }
}
