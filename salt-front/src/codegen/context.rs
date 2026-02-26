use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::collections::{HashMap, HashSet, VecDeque};
use crate::grammar::{SaltFile, SaltFn, Item, ImportDecl, StructDef, EnumDef};
use crate::registry::{Registry, StructInfo, EnumInfo};
use crate::types::{Type, TypeKey};
use crate::evaluator::Evaluator;
use z3;
use crate::common::mangling::Mangler;
use crate::codegen::collector::MonomorphizationTask;
use crate::codegen::emit_fn;

pub struct StringInterner {
    pool: HashSet<Rc<str>>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self { pool: HashSet::new() }
    }

    pub fn intern(&mut self, s: &str) -> Rc<str> {
        if let Some(interned) = self.pool.get(s) {
            return Rc::clone(interned);
        }
        let rc: Rc<str> = Rc::from(s);
        self.pool.insert(Rc::clone(&rc));
        rc
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpecializationTask {
    pub template_name: String,
    pub args: Vec<Type>,
    pub mangled_name: String,
    pub is_enum: bool,
}

/// [V4.0 SCORCHED EARTH] F-string segment for native expansion
#[derive(Clone, Debug)]
pub enum FStringSegment {
    Literal(String),
    Expr(String, Option<String>), // (expression, optional format spec)
}

pub struct MonomorphizerState {
    pub work_queue: VecDeque<SpecializationTask>,
    pub pending_set: HashSet<String>,
    pub is_frozen: bool,
}

impl MonomorphizerState {
    pub fn new() -> Self {
        Self {
            work_queue: VecDeque::new(),
            pending_set: HashSet::new(),
            is_frozen: false,
        }
    }
}

/// A cleanup task representing a resource that must be freed at scope exit.
/// Used by the RAII-Lite system to implement Implicit Scoped Drop.
#[derive(Clone, Debug)]
pub struct CleanupTask {
    /// The MLIR SSA value (the Vec struct/pointer to clean up)
    pub value: String,
    /// The drop function to call (e.g., "std__collections__vec__Vec__drop_u8")
    pub drop_fn: String,
    /// The variable name (for debugging and Z3 tracking)
    pub var_name: String,
    /// The type of the owned resource
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LocalKind {
    Ptr(String),
    SSA(String),
}

/// CodegenContext: Compiler state organized into logical phases
/// 
/// # Phased Organization (Linus/Graydon Hardening)
/// State is grouped by compiler phase for clarity and future parallelization:
/// - **Discovery**: Templates, registries, imports (read-mostly after init)
/// - **Expansion**: Monomorphizer, specializations (write-heavy during expansion)  
/// - **Emission**: MLIR buffers, counters, caches (write-heavy during codegen)
/// - **ControlFlow**: Loop labels, cleanup stack (scope-managed)
pub struct CodegenContext<'a> {
    // === Phased State Containers ===
    pub discovery: RefCell<crate::codegen::phases::DiscoveryState>,
    pub expansion: RefCell<crate::codegen::phases::ExpansionState>,
    pub emission: RefCell<crate::codegen::phases::EmissionState>,
    pub control_flow: RefCell<crate::codegen::phases::ControlFlowState>,
    
    // === Verification State (has lifetime, cannot be Default) ===
    // === Verification State (has lifetime, cannot be Default) ===
    pub z3_ctx: &'a z3::Context,
    pub z3_solver: RefCell<z3::Solver<'a>>,
    pub symbolic_tracker: RefCell<HashMap<String, z3::ast::Int<'a>>>,
    pub ownership_tracker: RefCell<crate::codegen::verification::Z3StateTracker<'a>>,
    pub elided_checks: RefCell<usize>,
    pub total_checks: RefCell<usize>,
    
    // === Immutable Configuration ===
    pub file: RefCell<&'a SaltFile>,
    pub registry: Option<&'a Registry>,
    pub release_mode: bool,
    pub consuming_fns: HashMap<String, HashSet<usize>>,
    pub suppress_specialization: Cell<bool>,
    pub target_platform: crate::codegen::passes::io_backend::TargetPlatform,
    /// Controls whether alias scope metadata is emitted on load/store ops.
    /// Set to false via --disable-alias-scopes to produce mlir-opt-compatible MLIR.
    pub emit_alias_scopes: bool,
    /// When true, skip Z3 ownership/leak verification and salt.verify op emission.
    /// Set via --no-verify CLI flag for fast iteration builds.
    pub no_verify: bool,
    pub lib_mode: bool,
    /// When true, enforce Mode B SIP safety checks (reject inttoptr, etc.).
    /// Set via --sip CLI flag. Kernel code uses lib_mode without sip_mode.
    pub sip_mode: bool,
    /// When true, emit MLIR `loc()` annotations for DWARF debug info.
    /// Set via -g / --debug-info CLI flag.
    pub debug_info: bool,
    /// Source file path for debug info `loc()` annotations.
    pub source_file: String,
    
    // === Per-function State ===
    pub evaluator: RefCell<Evaluator>,
    pub current_package: RefCell<Option<crate::grammar::PackageDecl>>,
    
    // === Malloc Tracking (DAG-based) ===
    /// Standalone tracker with dependency graph for malloc'd pointer flow.
    /// Tracks allocations, casts, struct construction, returns, and field-assigns.
    pub malloc_tracker: RefCell<crate::codegen::verification::MallocTracker>,
    /// Pending malloc result: set by expr/mod.rs when a malloc call is emitted,
    /// consumed by stmt.rs when the let-binding stores the result.
    pub pending_malloc_result: RefCell<Option<String>>,
    
    // === Pointer State Tracking (3-State Machine) ===
    /// Flow-sensitive pointer state tracker: Valid / Empty / Optional.
    /// Compile-time only — zero runtime overhead.
    pub pointer_tracker: RefCell<crate::codegen::verification::PointerStateTracker>,
    /// Pending pointer state: set by emit_call when a Ptr::empty() or Box::new() is emitted,
    /// consumed by stmt.rs when the let-binding stores the result.
    pub pending_pointer_state: RefCell<Option<crate::codegen::verification::PointerState>>,
    
    // === Arena Escape Analysis (Scope Ladder) ===
    /// Depth-based taint tracker: every pointer inherits its arena's scope depth.
    /// Enforces: return depth ≤ 1, assignment depth(rhs) ≤ depth(lhs).
    pub arena_escape_tracker: RefCell<crate::codegen::verification::ArenaEscapeTracker>,
    /// Pending arena provenance: set when Arena::alloc is called,
    /// consumed by stmt.rs to register the pointer's taint depth.
    pub pending_arena_provenance: RefCell<Option<String>>,
}

/// Type alias to canonical TensorLayout in phases module
pub type TensorLayout = crate::codegen::phases::TensorLayout;


/// Configuration snapshot — immutable view of CodegenContext config fields.
/// Passed by value into LoweringContext to avoid needing the RefCell.
#[derive(Clone, Copy)]
pub struct CodegenConfig<'a> {
    pub file: &'a SaltFile,
    pub registry: Option<&'a Registry>,
    pub release_mode: bool,
    pub consuming_fns: &'a HashMap<String, HashSet<usize>>,
    pub target_platform: crate::codegen::passes::io_backend::TargetPlatform,
    pub emit_alias_scopes: bool,
    pub no_verify: bool,
    pub lib_mode: bool,
    pub sip_mode: bool,
    pub debug_info: bool,
    pub source_file: &'a str,
}

/// LoweringContext: A "view struct" holding direct &mut references to phase structs.
/// Eliminates RefCell runtime panics by using Rust's compile-time borrow checker.
/// Created from CodegenContext via as_lowering_ctx().
pub struct LoweringContext<'a, 'ctx> {
    pub discovery: &'a mut crate::codegen::phases::DiscoveryState,
    pub expansion: &'a mut crate::codegen::phases::ExpansionState,
    pub emission: &'a mut crate::codegen::phases::EmissionState,
    pub control_flow: &'a mut crate::codegen::phases::ControlFlowState,
    pub z3_ctx: &'ctx z3::Context,
    pub z3_solver: &'a mut z3::Solver<'ctx>,
    pub symbolic_tracker: &'a mut std::collections::HashMap<String, z3::ast::Int<'ctx>>,
    pub ownership_tracker: &'a mut crate::codegen::verification::Z3StateTracker<'ctx>,
    pub elided_checks: &'a mut usize,
    pub total_checks: &'a mut usize,
    pub evaluator: &'a mut crate::evaluator::Evaluator,
    pub malloc_tracker: &'a mut crate::codegen::verification::MallocTracker,
    pub pointer_tracker: &'a mut crate::codegen::verification::PointerStateTracker,
    pub arena_escape_tracker: &'a mut crate::codegen::verification::ArenaEscapeTracker,
    pub pending_malloc_result: &'a mut Option<String>,
    pub pending_pointer_state: &'a mut Option<crate::codegen::verification::PointerState>,
    pub current_package: &'a mut Option<crate::grammar::PackageDecl>,
    pub suppress_specialization: &'a Cell<bool>,
    pub config: CodegenConfig<'a>,
}

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    // =========================================================================
    // Core Lookup Methods
    // =========================================================================

    pub fn resolve_global(&self, name: &str) -> Option<Type> {
        if let Some(ty) = self.discovery.globals.get(name) {
            return Some(ty.clone());
        }
        None
    }

    pub fn resolve_type(&self, name: &str) -> Option<Type> {
        if let Some(ty) = self.expansion.current_type_map.get(name) {
            return Some(ty.clone());
        }
        if let Some(ty) = self.discovery.globals.get(name) {
            return Some(ty.clone());
        }
        None
    }

    pub fn find_struct_by_name(&self, name: &str) -> Option<crate::registry::StructInfo> {
        for info in self.discovery.struct_registry.values() {
            if info.name == name || info.name.ends_with(&format!("__{}", name)) {
                return Some(info.clone());
            }
        }
        None
    }

    pub fn find_struct_by_key(&self, key: &crate::types::TypeKey) -> Option<crate::registry::StructInfo> {
        self.discovery.struct_registry.get(key).cloned()
    }

    pub fn find_enum_by_name(&self, name: &str) -> Option<crate::registry::EnumInfo> {
        for info in self.discovery.enum_registry.values() {
            if info.name == name || info.name.ends_with(&format!("__{}", name)) {
                return Some(info.clone());
            }
        }
        None
    }

    pub fn find_enum_by_key(&self, key: &crate::types::TypeKey) -> Option<crate::registry::EnumInfo> {
        self.discovery.enum_registry.get(key).cloned()
    }

    pub fn mangle_fn_name(&self, name: &str) -> String {
        if name.contains("__") {
            return name.to_string();
        }
        if let Some(pkg) = &self.config.file.package {
            let prefix = crate::codegen::Mangler::mangle(
                &pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()
            );
            format!("{}__{}", prefix, name)
        } else {
            name.to_string()
        }
    }

    pub fn package_prefix(&self) -> String {
        if let Some(pkg) = &self.config.file.package {
            crate::codegen::Mangler::mangle(
                &pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()
            ) + "__"
        } else {
            String::new()
        }
    }

    // =========================================================================
    // MLIR Type Helpers
    // =========================================================================

    pub fn to_mlir_type(&self, ty: &Type) -> String {
        ty.to_mlir_type_simple()
    }

    pub fn to_mlir_storage_type(&self, ty: &Type) -> String {
        ty.to_mlir_storage_type_simple()
    }

    pub fn size_of(&mut self, ty: &Type) -> usize {
        ty.size_of(&self.discovery.struct_registry)
    }

    // =========================================================================
    // Scope & Variable Management
    // =========================================================================

    pub fn push_cleanup_scope(&mut self) {
        self.control_flow.cleanup_stack.push(Vec::new());
    }

    pub fn pop_cleanup_scope(&mut self) -> Vec<crate::codegen::phases::CleanupTask> {
        self.control_flow.cleanup_stack.pop().unwrap_or_default()
    }

    pub fn register_owned_resource(&mut self, value: &str, drop_fn: &str, var_name: &str, ty: Type) {
        if let Some(scope) = self.control_flow.cleanup_stack.last_mut() {
            scope.push(crate::codegen::phases::CleanupTask {
                value: value.to_string(),
                drop_fn: drop_fn.to_string(),
                var_name: var_name.to_string(),
                ty,
            });
        }
        self.ownership_tracker.register_allocation(var_name, self.z3_solver);
    }

    pub fn mark_consumed(&mut self, var_name: &str) {
        self.control_flow.consumed_vars.insert(var_name.to_string());
    }

    pub fn is_consumed(&self, var_name: &str) -> bool {
        self.control_flow.consumed_vars.contains(var_name)
    }

    pub fn mark_devoured(&mut self, var_name: &str) {
        self.control_flow.devoured_vars.insert(var_name.to_string());
    }

    pub fn is_devoured(&self, var_name: &str) -> bool {
        self.control_flow.devoured_vars.contains(var_name)
    }

    // =========================================================================
    // External Declaration Management
    // =========================================================================

    pub fn ensure_func_declared(&mut self, name: &str, arg_tys: &[Type], ret_ty: &Type) -> Result<(), String> {
        // Skip only if already declared in pending_func_decls OR has a full body.
        // Do NOT skip for external_decls — FFI functions need forward declarations
        // precisely because they will never get a body emitted.
        if self.emission.pending_func_decls.contains_key(name) || self.emission.defined_functions.contains(name) {
            return Ok(());
        }
        self.emission.external_decls.insert(name.to_string());
        let arg_strs: Vec<String> = arg_tys.iter().map(|t| t.to_mlir_type_simple()).collect();
        let ret_str = if *ret_ty == Type::Unit { "()".to_string() } else { ret_ty.to_mlir_type_simple() };
        let decl = format!(
            "  func.func private @{}({}) -> {}\n",
            name,
            arg_strs.join(", "),
            ret_str
        );
        self.emission.pending_func_decls.insert(name.to_string(), decl);
        Ok(())
    }

    pub fn ensure_extern_declared_raw(&mut self, name: &str, signature: &str) {
        if self.emission.pending_func_decls.contains_key(name) || self.emission.defined_functions.contains(name) {
            return;
        }
        self.emission.external_decls.insert(name.to_string());
        let decl = format!("  func.func private @{}{}\n", name, signature);
        self.emission.pending_func_decls.insert(name.to_string(), decl);
    }

    // =========================================================================
    // Z3/Verification Helpers
    // =========================================================================

    pub fn z3_register_symbolic_int(&mut self, ssa_name: &str) {
        let sym = z3::ast::Int::new_const(self.z3_ctx, ssa_name);
        self.symbolic_tracker.insert(ssa_name.to_string(), sym);
    }

    pub fn z3_try_prove_positive(&self, ssa_name: &str) -> bool {
        if let Some(sym) = self.symbolic_tracker.get(ssa_name) {
            let zero = z3::ast::Int::from_i64(self.z3_ctx, 0);
            let cond = sym.ge(&zero);
            self.z3_solver.push();
            self.z3_solver.assert(&cond.not());
            let result = self.z3_solver.check();
            self.z3_solver.pop(1);
            result == z3::SatResult::Unsat
        } else {
            false
        }
    }

    pub fn mark_released(&mut self, var_name: &str) {
        let z3_solver_ptr = self.z3_solver as *const z3::Solver;
        let solver_ref = unsafe { &*z3_solver_ptr };
        self.ownership_tracker.mark_released(var_name, solver_ref).ok();
    }

    // =========================================================================
    // Global LVN Cache
    // =========================================================================

    pub fn lvn_lookup(&self, key: &str) -> Option<String> {
        self.emission.global_lvn.get_cached(key).cloned()
    }

    pub fn lvn_insert(&mut self, key: String, value: String) {
        self.emission.global_lvn.cache_value(key, value);
    }

    pub fn lvn_invalidate(&mut self) {
        self.emission.global_lvn.clear();
    }

    // =========================================================================
    // MLIR String Builder Helpers
    // =========================================================================

    pub fn emit_addressof(&mut self, out: &mut String, res: &str, name: &str) -> Result<(), String> {
        let is_func = self.emission.defined_functions.contains(name)
            || self.emission.external_decls.contains(name)
            || matches!(self.resolve_global(name), Some(Type::Fn(_, _)));
        if is_func {
            let ty = self.resolve_global(name).unwrap_or(Type::Unit);
            if let Type::Fn(args, ret) = ty {
                let ac: Vec<String> = args.iter().map(|t| t.to_mlir_type_simple()).collect();
                let rs = if let Type::Unit = *ret { "()".to_string() } else { ret.to_mlir_type_simple() };
                let sig = format!("({}) -> {}", ac.join(", "), rs);
                let tmp = format!("{}__fn", res);
                out.push_str(&format!("    {} = func.constant @{} : {}\n", tmp, name, sig));
                out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : {} to !llvm.ptr\n", res, tmp, sig));
            } else {
                let tmp = format!("{}__fn", res);
                out.push_str(&format!("    {} = func.constant @{} : () -> ()\n", tmp, name));
                out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : () -> () to !llvm.ptr\n", res, tmp));
            }
        } else {
            out.push_str(&format!("    {} = llvm.mlir.addressof @{} : !llvm.ptr\n", res, name));
        }
        Ok(())
    }

    // =========================================================================
    // Phase Field Accessors (direct &/&mut — zero RefCell)
    // =========================================================================

    // --- Discovery Phase ---
    pub fn struct_templates(&self) -> &std::collections::HashMap<String, crate::grammar::StructDef> { &self.discovery.struct_templates }
    pub fn struct_templates_mut(&mut self) -> &mut std::collections::HashMap<String, crate::grammar::StructDef> { &mut self.discovery.struct_templates }
    pub fn enum_templates(&self) -> &std::collections::HashMap<String, crate::grammar::EnumDef> { &self.discovery.enum_templates }
    pub fn enum_templates_mut(&mut self) -> &mut std::collections::HashMap<String, crate::grammar::EnumDef> { &mut self.discovery.enum_templates }
    pub fn struct_registry(&self) -> &std::collections::HashMap<crate::types::TypeKey, crate::registry::StructInfo> { &self.discovery.struct_registry }
    pub fn struct_registry_mut(&mut self) -> &mut std::collections::HashMap<crate::types::TypeKey, crate::registry::StructInfo> { &mut self.discovery.struct_registry }
    pub fn enum_registry(&self) -> &std::collections::HashMap<crate::types::TypeKey, crate::registry::EnumInfo> { &self.discovery.enum_registry }
    pub fn enum_registry_mut(&mut self) -> &mut std::collections::HashMap<crate::types::TypeKey, crate::registry::EnumInfo> { &mut self.discovery.enum_registry }
    pub fn trait_registry(&self) -> &crate::codegen::trait_registry::TraitRegistry { &self.discovery.trait_registry }
    pub fn trait_registry_mut(&mut self) -> &mut crate::codegen::trait_registry::TraitRegistry { &mut self.discovery.trait_registry }
    pub fn globals(&self) -> &std::collections::BTreeMap<String, Type> { &self.discovery.globals }
    pub fn globals_mut(&mut self) -> &mut std::collections::BTreeMap<String, Type> { &mut self.discovery.globals }
    pub fn imports(&self) -> &Vec<crate::grammar::ImportDecl> { &self.discovery.imports }
    pub fn imports_mut(&mut self) -> &mut Vec<crate::grammar::ImportDecl> { &mut self.discovery.imports }
    pub fn generic_impls(&self) -> &std::collections::HashMap<String, (crate::grammar::SaltFn, Vec<crate::grammar::ImportDecl>)> { &self.discovery.generic_impls }
    pub fn generic_impls_mut(&mut self) -> &mut std::collections::HashMap<String, (crate::grammar::SaltFn, Vec<crate::grammar::ImportDecl>)> { &mut self.discovery.generic_impls }
    pub fn entity_registry(&self) -> &crate::codegen::collector::EntityRegistry { &self.discovery.entity_registry }
    pub fn entity_registry_mut(&mut self) -> &mut crate::codegen::collector::EntityRegistry { &mut self.discovery.entity_registry }
    pub fn string_prefix_handlers(&self) -> &std::collections::HashMap<String, String> { &self.discovery.string_prefix_handlers }
    pub fn string_prefix_handlers_mut(&mut self) -> &mut std::collections::HashMap<String, String> { &mut self.discovery.string_prefix_handlers }

    // --- Expansion Phase ---
    pub fn specializations(&self) -> &std::collections::HashMap<(String, Vec<Type>), String> { &self.expansion.specializations }
    pub fn specializations_mut(&mut self) -> &mut std::collections::HashMap<(String, Vec<Type>), String> { &mut self.expansion.specializations }
    pub fn pending_generations(&self) -> &std::collections::VecDeque<crate::codegen::collector::MonomorphizationTask> { &self.expansion.pending_generations }
    pub fn pending_generations_mut(&mut self) -> &mut std::collections::VecDeque<crate::codegen::collector::MonomorphizationTask> { &mut self.expansion.pending_generations }
    pub fn current_type_map(&self) -> &std::collections::BTreeMap<String, Type> { &self.expansion.current_type_map }
    pub fn current_type_map_mut(&mut self) -> &mut std::collections::BTreeMap<String, Type> { &mut self.expansion.current_type_map }
    pub fn current_generic_args(&self) -> &Vec<Type> { &self.expansion.current_generic_args }
    pub fn current_generic_args_mut(&mut self) -> &mut Vec<Type> { &mut self.expansion.current_generic_args }
    pub fn current_self_ty(&self) -> &Option<Type> { &self.expansion.current_self_ty }
    pub fn current_self_ty_mut(&mut self) -> &mut Option<Type> { &mut self.expansion.current_self_ty }
    pub fn current_ret_ty(&self) -> &Option<Type> { &self.expansion.current_ret_ty }
    pub fn current_ret_ty_mut(&mut self) -> &mut Option<Type> { &mut self.expansion.current_ret_ty }
    pub fn current_fn_name(&self) -> &String { &self.expansion.current_fn_name }
    pub fn current_fn_name_mut(&mut self) -> &mut String { &mut self.expansion.current_fn_name }
    pub fn monomorphizer(&self) -> &crate::codegen::phases::MonomorphizerState { &self.expansion.monomorphizer }
    pub fn monomorphizer_mut(&mut self) -> &mut crate::codegen::phases::MonomorphizerState { &mut self.expansion.monomorphizer }

    // --- Emission Phase ---
    pub fn next_id(&mut self) -> usize { self.emission.next_id() }

    /// Generate an MLIR `loc("file":line:col)` annotation for the given span.
    /// Returns empty string when debug info is disabled.
    pub fn loc_tag(&self, span: proc_macro2::Span) -> String {
        if !self.config.debug_info || self.config.source_file.is_empty() {
            return String::new();
        }
        let start = span.start();
        format!(" loc(\"{}\":{}:{})", self.config.source_file, start.line, start.column)
    }
    pub fn alloca_out(&self) -> &String { &self.emission.alloca_out }
    pub fn alloca_out_mut(&mut self) -> &mut String { &mut self.emission.alloca_out }
    pub fn decl_out(&self) -> &String { &self.emission.decl_out }
    pub fn decl_out_mut(&mut self) -> &mut String { &mut self.emission.decl_out }
    pub fn definitions_buffer(&self) -> &String { &self.emission.definitions_buffer }
    pub fn definitions_buffer_mut(&mut self) -> &mut String { &mut self.emission.definitions_buffer }
    pub fn string_literals(&self) -> &Vec<(String, String, usize)> { &self.emission.string_literals }
    pub fn string_literals_mut(&mut self) -> &mut Vec<(String, String, usize)> { &mut self.emission.string_literals }
    pub fn defined_functions(&self) -> &std::collections::HashSet<String> { &self.emission.defined_functions }
    pub fn defined_functions_mut(&mut self) -> &mut std::collections::HashSet<String> { &mut self.emission.defined_functions }
    pub fn defined_structs(&self) -> &std::collections::HashSet<String> { &self.emission.defined_structs }
    pub fn defined_structs_mut(&mut self) -> &mut std::collections::HashSet<String> { &mut self.emission.defined_structs }
    pub fn defined_enums(&self) -> &std::collections::HashSet<String> { &self.emission.defined_enums }
    pub fn defined_enums_mut(&mut self) -> &mut std::collections::HashSet<String> { &mut self.emission.defined_enums }
    pub fn external_decls(&self) -> &std::collections::HashSet<String> { &self.emission.external_decls }
    pub fn external_decls_mut(&mut self) -> &mut std::collections::HashSet<String> { &mut self.emission.external_decls }
    pub fn initialized_globals(&self) -> &std::collections::HashSet<String> { &self.emission.initialized_globals }
    pub fn initialized_globals_mut(&mut self) -> &mut std::collections::HashSet<String> { &mut self.emission.initialized_globals }
    pub fn layout_cache(&self) -> &std::collections::HashMap<Type, (usize, usize)> { &self.emission.layout_cache }
    pub fn layout_cache_mut(&mut self) -> &mut std::collections::HashMap<Type, (usize, usize)> { &mut self.emission.layout_cache }
    pub fn tensor_layout_cache(&self) -> &std::collections::HashMap<Type, crate::codegen::phases::TensorLayout> { &self.emission.tensor_layout_cache }
    pub fn tensor_layout_cache_mut(&mut self) -> &mut std::collections::HashMap<Type, crate::codegen::phases::TensorLayout> { &mut self.emission.tensor_layout_cache }
    pub fn mlir_type_cache(&self) -> &std::collections::HashMap<Type, String> { &self.emission.mlir_type_cache }
    pub fn mlir_type_cache_mut(&mut self) -> &mut std::collections::HashMap<Type, String> { &mut self.emission.mlir_type_cache }
    pub fn struct_type_cache(&self) -> &Option<std::collections::HashMap<String, Vec<Type>>> { &self.emission.struct_type_cache }
    pub fn struct_type_cache_mut(&mut self) -> &mut Option<std::collections::HashMap<String, Vec<Type>>> { &mut self.emission.struct_type_cache }
    pub fn interner(&self) -> &crate::codegen::phases::StringInterner { &self.emission.interner }
    pub fn interner_mut(&mut self) -> &mut crate::codegen::phases::StringInterner { &mut self.emission.interner }
    pub fn emitted_types(&self) -> &std::collections::HashSet<String> { &self.emission.emitted_types }
    pub fn emitted_types_mut(&mut self) -> &mut std::collections::HashSet<String> { &mut self.emission.emitted_types }
    pub fn type_id_registry(&self) -> &crate::codegen::types::TypeIDRegistry { &self.emission.type_id_registry }
    pub fn type_id_registry_mut(&mut self) -> &mut crate::codegen::types::TypeIDRegistry { &mut self.emission.type_id_registry }
    pub fn metadata_id_counter(&self) -> &usize { &self.emission.metadata_id_counter }
    pub fn metadata_id_counter_mut(&mut self) -> &mut usize { &mut self.emission.metadata_id_counter }
    pub fn next_metadata_id(&self) -> usize { self.emission.metadata_id_counter }
    pub fn linalg_initialized(&self) -> &bool { &self.emission.linalg_initialized }
    pub fn linalg_initialized_mut(&mut self) -> &mut bool { &mut self.emission.linalg_initialized }
    pub fn buffer_body(&mut self, code: &str) { self.emission.buffer_body(code); }
    pub fn get_buffered_body(&self) -> &str { self.emission.get_buffered_body() }
    pub fn invalidate_type_cache(&mut self) { self.emission.struct_type_cache = None; }

    // --- Control Flow Phase ---
    pub fn loop_exit_stack(&self) -> &Vec<String> { &self.control_flow.loop_exit_stack }
    pub fn loop_exit_stack_mut(&mut self) -> &mut Vec<String> { &mut self.control_flow.loop_exit_stack }
    pub fn break_labels(&self) -> &Vec<String> { &self.control_flow.break_labels }
    pub fn break_labels_mut(&mut self) -> &mut Vec<String> { &mut self.control_flow.break_labels }
    pub fn continue_labels(&self) -> &Vec<String> { &self.control_flow.continue_labels }
    pub fn continue_labels_mut(&mut self) -> &mut Vec<String> { &mut self.control_flow.continue_labels }
    pub fn region_stack(&self) -> &Vec<String> { &self.control_flow.region_stack }
    pub fn region_stack_mut(&mut self) -> &mut Vec<String> { &mut self.control_flow.region_stack }
    pub fn cleanup_stack(&self) -> &Vec<Vec<crate::codegen::phases::CleanupTask>> { &self.control_flow.cleanup_stack }
    pub fn cleanup_stack_mut(&mut self) -> &mut Vec<Vec<crate::codegen::phases::CleanupTask>> { &mut self.control_flow.cleanup_stack }
    pub fn mutated_vars(&self) -> &std::collections::HashSet<String> { &self.control_flow.mutated_vars }
    pub fn mutated_vars_mut(&mut self) -> &mut std::collections::HashSet<String> { &mut self.control_flow.mutated_vars }
    pub fn consumed_vars(&self) -> &std::collections::HashSet<String> { &self.control_flow.consumed_vars }
    pub fn consumed_vars_mut(&mut self) -> &mut std::collections::HashSet<String> { &mut self.control_flow.consumed_vars }
    pub fn consumption_locs(&self) -> &std::collections::HashMap<String, String> { &self.control_flow.consumption_locs }
    pub fn consumption_locs_mut(&mut self) -> &mut std::collections::HashMap<String, String> { &mut self.control_flow.consumption_locs }
    pub fn devoured_vars(&self) -> &std::collections::HashSet<String> { &self.control_flow.devoured_vars }
    pub fn devoured_vars_mut(&mut self) -> &mut std::collections::HashSet<String> { &mut self.control_flow.devoured_vars }
    pub fn is_unsafe_block(&self) -> &bool { &self.control_flow.is_unsafe_block }
    pub fn is_unsafe_block_mut(&mut self) -> &mut bool { &mut self.control_flow.is_unsafe_block }
    pub fn no_yield(&self) -> &bool { &self.control_flow.no_yield }
    pub fn no_yield_mut(&mut self) -> &mut bool { &mut self.control_flow.no_yield }
    pub fn current_pulse(&self) -> &Option<u32> { &self.control_flow.current_pulse }
    pub fn current_pulse_mut(&mut self) -> &mut Option<u32> { &mut self.control_flow.current_pulse }
    pub fn is_hot_path(&self) -> &bool { &self.control_flow.is_hot_path }
    pub fn is_hot_path_mut(&mut self) -> &mut bool { &mut self.control_flow.is_hot_path }

    // --- Verification Phase ---
    pub fn get_symbolic_int(&self, ssa_name: &str) -> Option<z3::ast::Int<'ctx>> { self.symbolic_tracker.get(ssa_name).cloned() }

    // =========================================================================
    // MLIR Builder Pattern Helpers (zero RefCell)
    // =========================================================================

    pub fn emit_binop(&self, out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
        out.push_str(&format!("    {} = {} {}, {} : {}\n", res, op, lhs, rhs, ty));
    }
    pub fn emit_binop_fast(&self, out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
        out.push_str(&format!("    {} = {} {}, {} {{fastmath = #arith.fastmath<reassoc, contract>}} : {}\n", res, op, lhs, rhs, ty));
    }
    pub fn emit_const_int(&self, out: &mut String, res: &str, val: i64, ty: &str) {
        out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val, ty));
    }
    pub fn emit_const_float(&self, out: &mut String, res: &str, val: f64, ty: &str) {
        let val_str = if val == 0.0 { "0.0".to_string() } else { format!("{:.17e}", val) };
        out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val_str, ty));
    }
    pub fn emit_load(&self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, ptr, ty));
    }
    pub fn emit_load_scoped(&self, out: &mut String, res: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
        if !self.config.emit_alias_scopes { self.emit_load(out, res, ptr, ty); return; }
        out.push_str(&format!("    {} = llvm.load {} {{ alias_scopes = [{}], noalias = [{}] }} : !llvm.ptr -> {}\n", res, ptr, scope, noalias, ty));
    }
    pub fn emit_load_logical_with_scope(&mut self, out: &mut String, res: &str, ptr: &str, ty: &Type, scopes: Option<(&str, &str)>) -> Result<(), String> {
        let storage_ty = ty.to_mlir_storage_type_simple();
        if *ty == Type::Bool {
            let load_res = format!("%b_load_{}", self.next_id());
            if let Some((s, n)) = scopes { self.emit_load_scoped(out, &load_res, ptr, &storage_ty, s, n); }
            else { out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", load_res, ptr, storage_ty)); }
            self.emit_trunc(out, res, &load_res, "i8", "i1");
        } else if ty.k_is_ptr_type() {
            if let Some((s, n)) = scopes { self.emit_load_scoped(out, res, ptr, "!llvm.ptr", s, n); }
            else { out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !llvm.ptr\n", res, ptr)); }
        } else {
            if let Some((s, n)) = scopes { self.emit_load_scoped(out, res, ptr, &storage_ty, s, n); }
            else { out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, ptr, storage_ty)); }
        }
        Ok(())
    }
    pub fn emit_store(&self, out: &mut String, val: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", val, ptr, ty));
    }
    pub fn emit_store_scoped(&self, out: &mut String, val: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
        if !self.config.emit_alias_scopes { self.emit_store(out, val, ptr, ty); return; }
        out.push_str(&format!("    llvm.store {}, {} {{ alias_scopes = [{}], noalias = [{}] }} : {}, !llvm.ptr\n", val, ptr, scope, noalias, ty));
    }
    pub fn emit_store_logical_with_scope(&mut self, out: &mut String, val: &str, ptr: &str, ty: &Type, scopes: Option<(&str, &str)>) -> Result<(), String> {
        let storage_ty = ty.to_mlir_storage_type_simple();
        if *ty == Type::Bool {
            let zext_res = format!("%b_zext_{}", self.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i1 to i8\n", zext_res, val));
            if let Some((s, n)) = scopes { self.emit_store_scoped(out, &zext_res, ptr, &storage_ty, s, n); }
            else { out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", zext_res, ptr, storage_ty)); }
        } else if ty.k_is_ptr_type() {
            if let Some((s, n)) = scopes { self.emit_store_scoped(out, val, ptr, "!llvm.ptr", s, n); }
            else { out.push_str(&format!("    llvm.store {} , {} : !llvm.ptr, !llvm.ptr\n", val, ptr)); }
        } else {
            if let Some((s, n)) = scopes { self.emit_store_scoped(out, val, ptr, &storage_ty, s, n); }
            else { out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", val, ptr, storage_ty)); }
        }
        Ok(())
    }
    pub fn emit_alloca(&mut self, _out: &mut String, res: &str, ty: &str) {
        self.emission.alloca_out.push_str(&format!("    {} = llvm.alloca %c1_i64 x {} : (i64) -> !llvm.ptr\n", res, ty));
    }
    pub fn emit_gep_field(&self, out: &mut String, res: &str, base: &str, idx: usize, struct_ty: &str) {
        out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n", res, base, idx, struct_ty));
    }
    pub fn emit_gep(&self, out: &mut String, res: &str, base: &str, idx_var: &str, elem_ty: &str) {
        out.push_str(&format!("    {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", res, base, idx_var, elem_ty));
    }
    pub fn emit_extractvalue(&self, out: &mut String, res: &str, val: &str, idx: usize, ty: &str) {
        out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", res, val, idx, ty));
    }
    pub fn emit_extractvalue_logical(&mut self, out: &mut String, res: &str, val: &str, idx: usize, ty: &str, field_ty: &Type) -> Result<(), String> {
        if *field_ty == Type::Bool {
            let extract_res = format!("%b_extract_{}", self.next_id());
            out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", extract_res, val, idx, ty));
            self.emit_trunc(out, res, &extract_res, "i8", "i1");
        } else { self.emit_extractvalue(out, res, val, idx, ty); }
        Ok(())
    }
    pub fn emit_insertvalue(&self, out: &mut String, res: &str, elem: &str, val: &str, idx: usize, ty: &str) {
        out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, elem, val, idx, ty));
    }
    pub fn emit_insertvalue_logical(&mut self, out: &mut String, res: &str, elem: &str, val: &str, idx: usize, ty: &str, field_ty: &Type) -> Result<(), String> {
        if *field_ty == Type::Bool {
            let zext_res = format!("%b_zext_ins_{}", self.next_id());
            self.emit_cast(out, &zext_res, "arith.extui", elem, "i1", "i8");
            out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, zext_res, val, idx, ty));
        } else { self.emit_insertvalue(out, res, elem, val, idx, ty); }
        Ok(())
    }
    pub fn emit_cmp(&self, out: &mut String, res: &str, cmp_op: &str, pred: &str, lhs: &str, rhs: &str, ty: &str) {
        let comma = if cmp_op == "llvm.icmp" || cmp_op == "llvm.fcmp" { "" } else { "," };
        out.push_str(&format!("    {} = {} \"{}\"{} {}, {} : {}\n", res, cmp_op, pred, comma, lhs, rhs, ty));
    }
    pub fn emit_cast(&self, out: &mut String, res: &str, op: &str, val: &str, from_ty: &str, to_ty: &str) {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, val, from_ty, to_ty));
    }
    pub fn emit_trunc(&self, out: &mut String, res: &str, val: &str, from_ty: &str, to_ty: &str) {
        out.push_str(&format!("    {} = arith.trunci {} : {} to {}\n", res, val, from_ty, to_ty));
    }
    pub fn emit_br(&self, out: &mut String, label: &str) {
        out.push_str(&format!("    llvm.br ^{}\n", label));
    }
    pub fn emit_cond_br(&self, out: &mut String, cond: &str, true_label: &str, false_label: &str) {
        out.push_str(&format!("    llvm.cond_br {}, ^{}, ^{}\n", cond, true_label, false_label));
    }
    pub fn emit_label(&self, out: &mut String, label: &str) {
        out.push_str(&format!("  ^{}:\n", label));
    }
    pub fn emit_return(&self, out: &mut String, val: &str, ty: &str) {
        out.push_str(&format!("    llvm.return {} : {}\n", val, ty));
    }
    pub fn emit_return_void(&self, out: &mut String) {
        out.push_str("    llvm.return\n");
    }
    pub fn emit_load_exclusive(&self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    {} = \"llvm.load\"({}) {{salt.access = \"exclusive\"}} : (!llvm.ptr) -> {}\n", res, ptr, ty));
    }
    pub fn emit_load_atomic(&mut self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        let zero = format!("%atomic_zero_{}", self.next_id());
        out.push_str(&format!("    {} = arith.constant 0 : {}\n", zero, ty));
        out.push_str(&format!("    {} = llvm.atomicrmw _or {}, {} seq_cst : !llvm.ptr, {}\n", res, ptr, zero, ty));
    }
    pub fn emit_store_atomic(&mut self, out: &mut String, val: &str, ptr: &str, ty: &str) {
        let discard = format!("%atomic_discard_{}", self.next_id());
        out.push_str(&format!("    {} = llvm.atomicrmw xchg {}, {} seq_cst : !llvm.ptr, {}\n", discard, ptr, val, ty));
    }
    pub fn emit_atomicrmw(&self, out: &mut String, res: &str, op: &str, ptr: &str, val: &str, ty: &str) {
        out.push_str(&format!("    {} = llvm.atomicrmw {} {}, {} seq_cst : !llvm.ptr, {}\n", res, op, ptr, val, ty));
    }
    pub fn emit_inttoptr(&self, out: &mut String, res: &str, val: &str, from_ty: &str) {
        out.push_str(&format!("    {} = llvm.inttoptr {} : {} to !llvm.ptr\n", res, val, from_ty));
    }
    pub fn emit_verify(&mut self, out: &mut String, cond: &str, _msg: &str) {
        let true_const = format!("%verify_true_{}", self.next_id());
        let violated = format!("%verify_violated_{}", self.next_id());
        out.push_str(&format!("    {} = arith.constant true\n", true_const));
        out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", violated, cond, true_const));
        out.push_str(&format!("    scf.if {} {{\n", violated));
        out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
        out.push_str("      scf.yield\n");
        out.push_str("    }\n");
    }
    pub fn emit_noalias_metadata(&self, _out: &mut String, region_name: &str) -> (String, String) {
        let id = self.next_metadata_id();
        let scope_domain = format!("@alias_domain_{}", id);
        let scope_id = format!("@alias_scope_{}_{}", region_name, id);
        (scope_id, scope_domain)
    }
    pub fn emit_call(&self, out: &mut String, res: Option<&str>, func_name: &str, args: &str, arg_tys: &str, ret_ty: &str) {
        let mangled_func_name = self.mangle_fn_name(func_name);
        if let Some(r) = res {
            out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", r, mangled_func_name, args, arg_tys, ret_ty));
        } else {
            out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled_func_name, args, arg_tys));
        }
    }

    // =========================================================================
    // Complex Methods (zero RefCell)
    // =========================================================================

    pub fn ensure_struct_exists(&mut self, base_name: &str, params: &[Type]) -> Result<String, String> {
        let key = (base_name.to_string(), params.to_vec());
        if let Some(mangled) = self.expansion.specializations.get(&key) {
            return Ok(mangled.clone());
        }
        // [STRUCT REGISTRATION FIX] Check struct_registry for non-generic structs,
        // then fall back to specialize_template for generic specializations.
        // This mirrors CodegenContext::ensure_struct_exists which delegates to
        // specialize_template for on-demand struct registration.
        if params.is_empty() {
            for (tk, _info) in self.discovery.struct_registry.iter() {
                if tk.name == base_name || tk.mangle() == base_name {
                    return Ok(tk.mangle());
                }
            }
        }
        // Delegate to specialize_template which handles template instantiation
        Ok(self.specialize_template(base_name, params, false)?.mangle())
    }

    pub fn ensure_enum_exists(&mut self, base_name: &str, params: &[Type]) -> Result<String, String> {
        let key = (base_name.to_string(), params.to_vec());
        if let Some(mangled) = self.expansion.specializations.get(&key) {
            return Ok(mangled.clone());
        }
        // [ENUM REGISTRATION FIX] Check enum_registry for non-generic enums,
        // then fall back to specialize_template for generic specializations.
        if params.is_empty() {
            for (tk, _info) in self.discovery.enum_registry.iter() {
                if tk.name == base_name || tk.mangle() == base_name {
                    return Ok(tk.mangle());
                }
            }
        }
        // Delegate to specialize_template which handles template instantiation
        Ok(self.specialize_template(base_name, params, true)?.mangle())
    }

    pub fn io_backend(&self) -> Box<dyn crate::codegen::passes::io_backend::IoBackend> {
        crate::codegen::passes::io_backend::backend_for_target(self.config.target_platform)
    }

    pub fn align_of(&self, ty: &Type) -> usize {
        ty.align_of(&self.discovery.struct_registry)
    }

    pub fn get_struct_fields_lowering(&self, struct_name: &str) -> Option<Vec<(String, Type)>> {
        for info in self.discovery.struct_registry.values() {
            if info.name == struct_name || info.name.ends_with(&format!("__{}", struct_name)) {
                let mut fields: Vec<(String, Type)> = info.fields.iter()
                    .map(|(name, (idx, ty))| (name.clone(), ty.clone(), *idx))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|(name, ty, _)| (name, ty))
                    .collect();
                // Sort by index for stable ordering
                let mut indexed: Vec<(usize, String, Type)> = info.fields.iter()
                    .map(|(name, (idx, ty))| (*idx, name.clone(), ty.clone()))
                    .collect();
                indexed.sort_by_key(|(idx, _, _)| *idx);
                return Some(indexed.into_iter().map(|(_, name, ty)| (name, ty)).collect());
            }
        }
        None
    }

    pub fn resolve_global_func(&self, name: &str) -> Option<(Type, String)> {
        if let Some(ty) = self.discovery.globals.get(name) {
            return Some((ty.clone(), name.to_string()));
        }
        self.resolve_global(name).map(|t| (t, name.to_string()))
    }

    pub fn get_tensor_layout(&mut self, ty: &Type) -> Result<crate::codegen::phases::TensorLayout, String> {
        if let Some(layout) = self.emission.tensor_layout_cache.get(ty) {
            return Ok(layout.clone());
        }
        if let Type::Tensor(_, shape) = ty {
            let mut strides = vec![1; shape.len()];
            for i in (0..shape.len() - 1).rev() {
                strides[i] = strides[i+1] * shape[i+1];
            }
            let layout = crate::codegen::phases::TensorLayout { shape: shape.clone(), strides, is_row_major: true };
            self.emission.tensor_layout_cache.insert(ty.clone(), layout.clone());
            Ok(layout)
        } else {
            Err(format!("Type {:?} is not a tensor", ty))
        }
    }

    pub fn emit_linalg_matmul(&mut self, out: &mut String, lhs: &str, lhs_ty: &str, rhs: &str, rhs_ty: &str, acc: &str, acc_ty: &str) -> Result<String, String> {
        let res = format!("%matmul_res_{}", self.next_id());
        out.push_str(&format!("    {} = linalg.matmul ins({}, {} : {}, {}) outs({} : {}) -> {}\n",
            res, lhs, rhs, lhs_ty, rhs_ty, acc, acc_ty, acc_ty));
        Ok(res)
    }

    // =========================================================================
    // Additional Delegation Methods (Phase 2)
    // =========================================================================

    // --- Template Finders ---
    pub fn find_struct_template_by_name(&self, name: &str) -> Option<String> {
        if self.discovery.struct_templates.contains_key(name) {
            return Some(name.to_string());
        }
        for key in self.discovery.struct_templates.keys() {
            if key.ends_with(&format!("__{}", name)) {
                return Some(key.clone());
            }
        }
        None
    }

    pub fn find_enum_template_by_name(&self, name: &str) -> Option<String> {
        if self.discovery.enum_templates.contains_key(name) {
            return Some(name.to_string());
        }
        for key in self.discovery.enum_templates.keys() {
            if key.ends_with(&format!("__{}", name)) {
                return Some(key.clone());
            }
        }
        None
    }

    pub fn find_methods_for_template(&self, template_name: &str) -> Vec<String> {
        let suffix = format!("__{}", template_name);
        let mut methods = Vec::new();
        for key in self.discovery.generic_impls.keys() {
            if key.contains(&suffix) || key.starts_with(template_name) {
                methods.push(key.clone());
            }
        }
        methods
    }

    // --- Type Queries ---
    pub fn is_option_enum(&self, ty: &Type) -> Option<crate::registry::EnumInfo> {
        match ty {
            Type::Enum(name) | Type::Struct(name) => {
                for (key, info) in &self.discovery.enum_registry {
                    if info.name == *name || key.name == *name {
                        if info.variants.iter().any(|v| v.0 == "Some") && info.variants.iter().any(|v| v.0 == "None") {
                            return Some(info.clone());
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn is_result_enum(&self, ty: &Type) -> Option<crate::registry::EnumInfo> {
        match ty {
            Type::Enum(name) | Type::Struct(name) | Type::Concrete(name, _) => {
                let base = name.split("__").last().unwrap_or(name);
                for (key, info) in &self.discovery.enum_registry {
                    let info_base = info.name.split("__").last().unwrap_or(&info.name);
                    let name_match = info.name == *name 
                        || key.name == *name
                        || base == info_base
                        || info_base.starts_with(base)
                        || base.starts_with(info_base)
                        || name.ends_with(&format!("__{}", info.name))
                        || info.name.ends_with(&format!("__{}", name));
                    if name_match
                        && info.variants.iter().any(|v| v.0 == "Ok") 
                        && info.variants.iter().any(|v| v.0 == "Err") {
                        return Some(info.clone());
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn lookup_struct_by_type(&self, ty: &Type) -> Option<crate::registry::StructInfo> {
        match ty {
            Type::Struct(name) => {
                for (key, info) in &self.discovery.struct_registry {
                    if info.name == *name || key.name == *name || key.mangle() == *name {
                        return Some(info.clone());
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn get_mangled(&self, ty: &Type) -> std::rc::Rc<str> {
        std::rc::Rc::from(ty.to_mlir_type_simple())
    }

    pub fn get_physical_index(&self, _field_order: &[Type], logical_idx: usize) -> usize {
        logical_idx
    }

    pub fn is_function_defined(&self, mangled_name: &str) -> bool {
        self.emission.defined_functions.contains(mangled_name)
    }

    // --- MLIR Emit Helpers ---
    pub fn emit_load_logical(&mut self, out: &mut String, res: &str, ptr: &str, ty: &Type) -> Result<(), String> {
        let storage_ty = ty.to_mlir_storage_type_simple();
        if *ty == Type::Bool {
            let load_res = format!("%b_load_{}", self.next_id());
            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", load_res, ptr, storage_ty));
            self.emit_trunc(out, res, &load_res, "i8", "i1");
        } else if ty.k_is_ptr_type() {
            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !llvm.ptr\n", res, ptr));
        } else {
            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, ptr, storage_ty));
        }
        Ok(())
    }

    pub fn emit_store_logical(&mut self, out: &mut String, val: &str, ptr: &str, ty: &Type) -> Result<(), String> {
        let storage_ty = ty.to_mlir_storage_type_simple();
        if *ty == Type::Bool {
            let zext_res = format!("%b_zext_{}", self.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i1 to i8\n", zext_res, val));
            out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", zext_res, ptr, storage_ty));
        } else if ty.k_is_ptr_type() {
            out.push_str(&format!("    llvm.store {} , {} : !llvm.ptr, !llvm.ptr\n", val, ptr));
        } else {
            out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", val, ptr, storage_ty));
        }
        Ok(())
    }

    pub fn ensure_external_declaration(&mut self, mangled_name: &str, arg_tys: &[Type], ret_ty: &Type) -> Result<(), String> {
        self.ensure_func_declared(mangled_name, arg_tys, ret_ty)
    }

    pub fn ensure_global_declared(&mut self, name: &str, ty: &Type) -> Result<(), String> {
        if self.emission.initialized_globals.contains(name) {
            return Ok(());
        }
        // [FIX] Function symbols must NOT be emitted as llvm.mlir.global.
        // When a function is used as a pointer (e.g., passed as an argument),
        // resolve_global returns Type::Fn. Redirect to ensure_func_declared
        // which emits `func.func private` instead of `llvm.mlir.global external`.
        if self.emission.external_decls.contains(name) || self.emission.defined_functions.contains(name) {
            return Ok(());
        }
        if let Type::Fn(ref args, ref ret) = ty {
            return self.ensure_func_declared(name, args, ret);
        }
        self.emission.initialized_globals.insert(name.to_string());
        let mlir_ty = ty.to_mlir_type_simple();
        self.emission.decl_out.push_str(&format!("  llvm.mlir.global external @{}() : {} {{\n", name, mlir_ty));
        self.emission.decl_out.push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_ty));
        self.emission.decl_out.push_str(&format!("    llvm.return %0 : {}\n  }}\n", mlir_ty));
        Ok(())
    }

    // --- Affine Context ---
    pub fn enter_affine_context(&mut self) {
        self.control_flow.affine_depth += 1;
    }

    pub fn exit_affine_context(&mut self) {
        if self.control_flow.affine_depth > 0 {
            self.control_flow.affine_depth -= 1;
        }
    }

    pub fn is_in_affine_context(&self) -> bool {
        self.control_flow.affine_depth > 0
    }

    // --- Z3 Helpers ---
    pub fn mk_int(&self, val: i64) -> z3::ast::Int<'ctx> {
        z3::ast::Int::from_i64(self.z3_ctx, val)
    }

    pub fn mk_var(&self, name: &str) -> z3::ast::Int<'ctx> {
        z3::ast::Int::new_const(self.z3_ctx, name)
    }

    pub fn is_provably_safe(&self, violation: &z3::ast::Bool<'ctx>) -> bool {
        self.z3_solver.push();
        self.z3_solver.assert(violation);
        let result = self.z3_solver.check();
        self.z3_solver.pop(1);
        result == z3::SatResult::Unsat
    }

    // --- Ownership / Cleanup ---
    pub fn pop_and_emit_cleanup(&mut self, out: &mut String) -> Result<(), String> {
        let tasks = self.pop_cleanup_scope();
        for task in tasks.iter().rev() {
            // Check if consumed before emitting drop
            if self.control_flow.consumed_vars.contains(&task.var_name) {
                continue;
            }
            if self.control_flow.devoured_vars.contains(&task.var_name) {
                continue;
            }
            self.ensure_extern_declared_raw(&task.drop_fn, "(!llvm.ptr) -> ()");
            out.push_str(&format!("    func.call @{}({}) : (!llvm.ptr) -> ()\n", task.drop_fn, task.value));
        }
        Ok(())
    }

    pub fn release_by_var_name(&mut self, var_name: &str) {
        // Remove from cleanup stack
        for scope in self.control_flow.cleanup_stack.iter_mut().rev() {
            scope.retain(|task| task.var_name != var_name);
        }
    }

    pub fn transfer_ownership(&mut self, value: &str) -> Result<(), String> {
        // Remove from cleanup stack
        for scope in self.control_flow.cleanup_stack.iter_mut().rev() {
            if let Some(pos) = scope.iter().position(|task| task.value == value) {
                scope.remove(pos);
                return Ok(());
            }
        }
        Ok(())
    }

    // --- F-String Methods ---
    pub fn escape_string(&self, s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\t', "\\t")
    }

    pub fn parse_fstring_segments(&self, content: &str) -> Vec<FStringSegment> {
        let mut segments = Vec::new();
        let mut chars = content.chars().peekable();
        let mut current_literal = String::new();
        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    if chars.peek() == Some(&'{') { chars.next(); current_literal.push('{'); continue; }
                    if !current_literal.is_empty() {
                        segments.push(FStringSegment::Literal(std::mem::take(&mut current_literal)));
                    }
                    let (expr, spec) = self.parse_fstring_expr(&mut chars);
                    if !expr.is_empty() { segments.push(FStringSegment::Expr(expr, spec)); }
                }
                '}' => { if chars.peek() == Some(&'}') { chars.next(); current_literal.push('}'); } }
                '\\' => { current_literal.push('\\'); if let Some(escaped) = chars.next() { current_literal.push(escaped); } }
                _ => { current_literal.push(c); }
            }
        }
        if !current_literal.is_empty() { segments.push(FStringSegment::Literal(current_literal)); }
        segments
    }

    fn parse_fstring_expr(&self, chars: &mut std::iter::Peekable<std::str::Chars>) -> (String, Option<String>) {
        let mut expr = String::new();
        let mut spec = None;
        let mut depth = 0;
        while let Some(&c) = chars.peek() {
            match c {
                '}' if depth == 0 => { chars.next(); break; }
                '{' => { depth += 1; expr.push(chars.next().unwrap()); }
                '}' => { depth -= 1; expr.push(chars.next().unwrap()); }
                ':' if depth == 0 => {
                    chars.next();
                    let mut s = String::new();
                    while let Some(&c2) = chars.peek() {
                        if c2 == '}' { chars.next(); break; }
                        s.push(chars.next().unwrap());
                    }
                    spec = Some(s);
                    break;
                }
                _ => { expr.push(chars.next().unwrap()); }
            }
        }
        (expr, spec)
    }

    pub fn format_with_spec_v4(&self, expr: &str, spec: Option<&str>) -> String {
        if let Some(s) = spec {
            format!("fmt_{}({})", s, expr)
        } else {
            expr.to_string()
        }
    }

    pub fn determine_write_method(&self, expr: &str, spec: Option<&str>) -> (String, String) {
        if let Some(_) = spec {
            ("write_fmt".to_string(), format!("fmt({})", expr))
        } else {
            ("write_any".to_string(), expr.to_string())
        }
    }

    pub fn native_fstring_expand(&self, content: &str) -> String {
        let segments = self.parse_fstring_segments(content);
        if segments.is_empty() { return "\"\"".to_string(); }
        let has_interpolation = segments.iter().any(|s| matches!(s, FStringSegment::Expr(_, _)));
        if !has_interpolation {
            if let Some(FStringSegment::Literal(s)) = segments.first() {
                return format!("\"{}\"", self.escape_string(s));
            }
        }
        let mut literal_len = 0;
        let mut interp_count = 0;
        for seg in &segments {
            match seg {
                FStringSegment::Literal(s) => literal_len += s.len(),
                FStringSegment::Expr(_, _) => interp_count += 1,
            }
        }
        let mut code = String::new();
        code.push_str("{\n");
        code.push_str(&format!("    let mut __h = std::string::InterpolatedStringHandler::new({}, {});\n", literal_len, interp_count));
        for seg in &segments {
            match seg {
                FStringSegment::Literal(s) => {
                    let escaped = self.escape_string(s);
                    code.push_str(&format!("    __h.append_literal(\"{}\", {});\n", escaped, s.len()));
                }
                FStringSegment::Expr(expr, spec) => {
                    let formatted = self.format_with_spec_v4(expr, spec.as_deref());
                    if formatted.starts_with("fmt_") {
                        code.push_str(&format!("    __h.append_fmt({});\n", formatted));
                    } else {
                        code.push_str(&format!("    __fstring_append_expr!(__h, {});\n", formatted));
                    }
                }
            }
        }
        code.push_str("    __h.finalize()\n");
        code.push_str("}");
        code
    }

    pub fn native_hex_expand(&self, content: &str) -> String {
        let clean_hex: String = content.chars().filter(|c| !c.is_whitespace()).collect();
        if clean_hex.len() % 2 != 0 {
            return "Vec::<u8>::new()".to_string();
        }
        if clean_hex.is_empty() { return "Vec::<u8>::new()".to_string(); }
        let mut bytes = Vec::new();
        for i in (0..clean_hex.len()).step_by(2) {
            let byte_str = &clean_hex[i..i + 2];
            if u8::from_str_radix(byte_str, 16).is_err() {
                return "Vec::<u8>::new()".to_string();
            }
            bytes.push(format!("0x{}", byte_str.to_uppercase()));
        }
        format!("Vec::<u8>::from_array([{}])", bytes.join(", "))
    }

    pub fn native_target_fstring_expand(&self, target: &str, content: &str) -> String {
        let segments = self.parse_fstring_segments(content);
        if segments.is_empty() { return "{ }".to_string(); }
        let mut code = String::new();
        code.push_str("{\n");
        for seg in &segments {
            match seg {
                FStringSegment::Literal(s) => {
                    if !s.is_empty() {
                        let escaped = self.escape_string(s);
                        code.push_str(&format!("    {}.write_str(\"{}\", {});\n", target, escaped, s.len()));
                    }
                }
                FStringSegment::Expr(expr, spec) => {
                    let (method, formatted_expr) = self.determine_write_method(expr, spec.as_deref());
                    code.push_str(&format!("    {}.{}({});\n", target, method, formatted_expr));
                }
            }
        }
        code.push_str("}");
        code
    }

    // --- Resolve Method (complex delegation) ---
    pub fn resolve_method(&self, receiver_ty: &Type, method_name: &str) -> Result<(crate::grammar::SaltFn, Option<Type>, Vec<crate::grammar::ImportDecl>), String> {
        // [FIX] Extract the receiver type's base name to match against method keys.
        // This prevents Slice::offset from shadowing Ptr::offset when called on a Ptr receiver.
        // [CRITICAL FIX] Recursively peel all Reference wrappers to reach the base type.
        // Inside hydrated methods, `self` may be double-wrapped: Reference(Reference(Concrete(...)))
        // which previously fell through to None, causing non-deterministic method resolution.
        fn extract_receiver_prefix(ty: &Type) -> Option<String> {
            match ty {
                Type::Concrete(name, _) => Some(name.clone()),
                Type::Struct(name) => Some(name.clone()),
                Type::Pointer { .. } => Some("std__core__ptr__Ptr".to_string()),
                Type::Reference(inner, _) => extract_receiver_prefix(inner),
                other if other.is_numeric() || *other == Type::Bool => Some(other.mangle_suffix()),
                _ => None,
            }
        }
        let receiver_prefix = extract_receiver_prefix(receiver_ty);

        // Search generic_impls for method — prefer receiver-type-specific matches
        let mut receiver_match = None;    // Matches receiver type prefix (highest priority)
        let mut instance_method_match = None;  // Any instance method (fallback)
        let mut fallback_match = None;    // Free function (lowest priority)
        
        let method_suffix = format!("__{}", method_name);
        for (key, (func, imports)) in &self.discovery.generic_impls {
            if key.ends_with(&method_suffix) || key == method_name {
                let has_self = !func.args.is_empty() && func.args[0].name == "self";
                
                // Check if this method belongs to the receiver's type.
                // Keys may be registered with either short names (e.g. "Ptr_T__offset")
                // or fully-qualified names (e.g. "std__core__ptr__Ptr__offset").
                // We must check both the full prefix and the basename.
                let matches_receiver = if let Some(ref prefix) = receiver_prefix {
                    if key.starts_with(prefix) {
                        true
                    } else {
                        // Extract basename: "std__core__ptr__Ptr" -> "Ptr"
                        let basename = prefix.rsplit("__").next().unwrap_or(prefix);
                        // Check: "Ptr_T__offset" starts with "Ptr"
                        key.starts_with(basename)
                    }
                } else {
                    false
                };
                
                if has_self && matches_receiver && receiver_match.is_none() {
                    receiver_match = Some((func.clone(), Some(receiver_ty.clone()), imports.clone()));
                } else if has_self && instance_method_match.is_none() {
                    instance_method_match = Some((func.clone(), Some(receiver_ty.clone()), imports.clone()));
                } else if !has_self && fallback_match.is_none() {
                    fallback_match = Some((func.clone(), Some(receiver_ty.clone()), imports.clone()));
                }
            }
        }
        
        // Priority: receiver-specific > any instance method > free function
        if let Some(result) = receiver_match {
            return Ok(result);
        }
        if let Some(result) = instance_method_match {
            return Ok(result);
        }
        if let Some(result) = fallback_match {
            return Ok(result);
        }
        Err(format!("Method '{}' not found for type {:?}", method_name, receiver_ty))
    }

    // --- Require local function (complex, uses config.file) ---
    pub fn require_local_function(&mut self, mangled_name: &str) -> bool {
        if self.discovery.entity_registry.identity_map.contains(mangled_name) {
            return true;
        }

        let task_opt = {
            let file = self.config.file;
            let current_pkg_prefix = if let Some(pkg) = &file.package {
                crate::codegen::Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()) + "__"
            } else {
                String::new()
            };

            let mut result = None;
            for item in &file.items {
                if let crate::grammar::Item::Fn(f) = item {
                    let my_mangled = if f.attributes.iter().any(|a| a.name == "no_mangle") {
                        f.name.to_string()
                    } else {
                        format!("{}{}", current_pkg_prefix, f.name)
                    };
                    if my_mangled == mangled_name {
                        let path = if let Some(pkg) = &file.package {
                            pkg.name.iter().map(|id| id.to_string()).collect()
                        } else {
                            vec![]
                        };
                        let identity = crate::types::TypeKey {
                            path,
                            name: f.name.to_string(),
                            specialization: None,
                        };
                        result = Some(crate::codegen::collector::MonomorphizationTask {
                            identity,
                            mangled_name: mangled_name.to_string(),
                            func: f.clone(),
                            concrete_tys: vec![],
                            self_ty: None,
                            imports: file.imports.clone(),
                            type_map: std::collections::BTreeMap::new(),
                        });
                        break;
                    }
                }
            }
            result
        };

        if let Some(task) = task_opt {
            self.expansion.pending_generations.push_back(task);
            self.discovery.entity_registry.identity_map.insert(mangled_name.to_string());
            return true;
        }
        false
    }

    /// Scoped generic context for LoweringContext (replaces GenericContextGuard for migrated code).
    /// Uses closure pattern instead of Drop to avoid locking the &mut reference.
    pub fn with_generic_context<R>(
        &mut self,
        new_args: std::collections::BTreeMap<String, Type>,
        self_ty: Type,
        ordered_args: Vec<Type>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let old_args = std::mem::replace(&mut self.expansion.current_type_map, new_args);
        let old_self = std::mem::replace(&mut self.expansion.current_self_ty, Some(self_ty));
        let old_ordered_args = std::mem::replace(&mut self.expansion.current_generic_args, ordered_args);
        let result = f(self);
        self.expansion.current_type_map = old_args;
        self.expansion.current_self_ty = old_self;
        self.expansion.current_generic_args = old_ordered_args;
        result
    }

    // --- Path Resolution (mirrors CodegenContext impl) ---
    pub fn resolve_path_to_fqn(&self, path: &syn::Path) -> Result<crate::types::TypeKey, String> {
        let segments: Vec<String> = path.segments.iter()
            .map(|s| s.ident.to_string())
            .collect();

        if segments.is_empty() {
            return Err("Empty path encountered in scanner".into());
        }

        if let Some((pkg, item)) = crate::codegen::expr::utils::resolve_package_prefix_ctx(self, &segments) {
            let fqn_base = if item.is_empty() { pkg } else { format!("{}__{}", pkg, item) };
            let parts: Vec<&str> = fqn_base.split("__").collect();
            let name = parts.last().unwrap_or(&"").to_string();
            let path_segments = parts[..parts.len()-1].iter().map(|s| s.to_string()).collect();
            Ok(crate::types::TypeKey {
                path: path_segments,
                name,
                specialization: None,
            })
        } else {
            if segments.len() == 1 {
                let name = &segments[0];
                return Ok(crate::types::TypeKey {
                    path: vec![],
                    name: name.clone(),
                    specialization: None,
                });
            }
            Err(format!("Could not resolve path to FQN: {:?}", segments))
        }
    }

    // --- Global Type Lookup (mirrors CodegenContext impl) ---
    pub fn lookup_global_type(&self, key: &crate::types::TypeKey) -> Option<Type> {
        let mut module_path = key.path.join(".");

        if module_path.is_empty() {
            let fn_name = self.expansion.current_fn_name.clone();
            if fn_name.contains("__") {
                let parts: Vec<&str> = fn_name.split("__").collect();
                if parts.len() > 1 {
                    let pkg_parts = &parts[..parts.len()-1];
                    module_path = pkg_parts.join(".");
                }
            }

            if module_path.is_empty() {
                if let Some(pkg) = &self.config.file.package {
                    module_path = pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(".");
                }
            }
        }

        if let Some(reg) = self.config.registry {
            if let Some(module) = reg.modules.get(&module_path) {
                if let Some(ty) = module.globals.get(&key.name) {
                    let prefix = module_path.replace(".", "__");
                    let qualified_ty = match ty {
                        Type::Struct(n) if !n.contains("__") => Type::Struct(format!("{}__{}", prefix, n)),
                        Type::Enum(n) if !n.contains("__") => Type::Enum(format!("{}__{}", prefix, n)),
                        Type::Concrete(n, args) if !n.contains("__") => Type::Concrete(format!("{}__{}", prefix, n), args.clone()),
                        _ => ty.clone()
                    };
                    return Some(qualified_ty);
                }
            }
        }
        None
    }

    // --- Global Signature Lookup (mirrors CodegenContext impl) ---
    pub fn resolve_global_signature(&self, mangled_name: &str) -> Option<(String, Type)> {
        if let Some(ty) = self.discovery.globals.get(mangled_name) {
            return Some((mangled_name.to_string(), ty.clone()));
        }
        if let Some(reg) = self.config.registry {
            for mod_info in reg.modules.values() {
                let pkg_mangled = mod_info.package.replace(".", "__");
                if mangled_name.starts_with(&pkg_mangled) {
                    let item = if mangled_name.len() > pkg_mangled.len() + 2 {
                        &mangled_name[pkg_mangled.len() + 2..]
                    } else {
                        &mangled_name[pkg_mangled.len()..]
                    };
                    if let Some(ty) = mod_info.globals.get(item) {
                        return Some((mangled_name.to_string(), ty.clone()));
                    }
                    if let Some((args, ret)) = mod_info.functions.get(item) {
                        return Some((mangled_name.to_string(), Type::Fn(args.clone(), Box::new(ret.clone()))));
                    }
                }
            }
        }
        None
    }

    // --- Type Scanning (mirrors scan_types_in_fn for LoweringContext) ---
    pub fn scan_types_in_fn_lctx(&mut self, func: &crate::grammar::SaltFn) -> Result<(), String> {
        // Scan arguments
        for arg in &func.args {
            if let Some(ty) = &arg.ty {
                crate::codegen::type_bridge::resolve_type(self, ty);
            }
        }
        // Scan return type
        if let Some(ret) = &func.ret_type {
            crate::codegen::type_bridge::resolve_type(self, ret);
        }
        Ok(())
    }

    // --- Hydrate Specialization (queues task for deferred hydration) ---
    // [SPLIT-BRAIN FIX] Push to expansion.pending_generations — the SAME queue
    // that drive_codegen drains. Previously this pushed to entity_registry.worklist,
    // which nobody drained, causing callee functions (e.g., sum_sq) to be
    // forward-declared but never emitted.
    pub fn hydrate_specialization(&mut self, task: crate::codegen::context::MonomorphizationTask) -> Result<(), String> {
        let mangled_name = task.mangled_name.clone();
        // Skip if already defined or has unresolved generics
        if task.type_map.values().any(|t| t.has_generics()) {
            return Ok(());
        }
        if self.emission.defined_functions.contains(&mangled_name) {
            return Ok(());
        }
        // Dedup via entity_registry identity_map
        if self.discovery.entity_registry.identity_map.contains(&mangled_name) {
            return Ok(());
        }
        self.discovery.entity_registry.identity_map.insert(mangled_name.clone());
        // Push to the orchestrator's queue (expansion.pending_generations)
        self.expansion.pending_generations.push_back(task);
        Ok(())
    }

}


impl<'a> CodegenContext<'a> {

    /// Scoped Access Pattern: borrows all RefCell fields, constructs a
    /// LoweringContext whose lifetime is tied to this stack frame, then
    /// invokes the closure.  The RefMut guards live here, so the &mut
    /// references inside LoweringContext remain valid for the entire
    /// duration of `f`.
    pub fn with_lowering_ctx<R>(&self, f: impl FnOnce(&mut LoweringContext<'_, 'a>) -> R) -> R {
        // --- borrow all RefCells (guards live in THIS frame) ---
        let mut discovery = self.discovery.borrow_mut();
        let mut expansion = self.expansion.borrow_mut();
        let mut emission  = self.emission.borrow_mut();
        let mut control_flow = self.control_flow.borrow_mut();
        let mut z3_solver = self.z3_solver.borrow_mut();
        let mut symbolic_tracker = self.symbolic_tracker.borrow_mut();
        let mut ownership_tracker = self.ownership_tracker.borrow_mut();
        let mut elided_checks = self.elided_checks.borrow_mut();
        let mut total_checks = self.total_checks.borrow_mut();
        let mut evaluator = self.evaluator.borrow_mut();
        let mut malloc_tracker = self.malloc_tracker.borrow_mut();
        let mut pointer_tracker = self.pointer_tracker.borrow_mut();
        let mut arena_escape_tracker = self.arena_escape_tracker.borrow_mut();
        let mut pending_malloc_result = self.pending_malloc_result.borrow_mut();
        let mut pending_pointer_state = self.pending_pointer_state.borrow_mut();
        let mut current_package = self.current_package.borrow_mut();
        let file = self.file.borrow();

        let mut lctx = LoweringContext {
            discovery: &mut *discovery,
            expansion: &mut *expansion,
            emission:  &mut *emission,
            control_flow: &mut *control_flow,
            z3_ctx: self.z3_ctx,
            z3_solver: &mut *z3_solver,
            symbolic_tracker: &mut *symbolic_tracker,
            ownership_tracker: &mut *ownership_tracker,
            elided_checks: &mut *elided_checks,
            total_checks: &mut *total_checks,
            evaluator: &mut *evaluator,
            malloc_tracker: &mut *malloc_tracker,
            pointer_tracker: &mut *pointer_tracker,
            arena_escape_tracker: &mut *arena_escape_tracker,
            pending_malloc_result: &mut *pending_malloc_result,
            pending_pointer_state: &mut *pending_pointer_state,
            current_package: &mut *current_package,
            suppress_specialization: &self.suppress_specialization,
            config: CodegenConfig {
                file: &*file,
                registry: self.registry,
                release_mode: self.release_mode,
                consuming_fns: &self.consuming_fns,
                target_platform: self.target_platform,
                emit_alias_scopes: self.emit_alias_scopes,
                no_verify: self.no_verify,
                lib_mode: self.lib_mode,
                sip_mode: self.sip_mode,
                debug_info: self.debug_info,
                source_file: &self.source_file,
            },
        };

        f(&mut lctx)
    }
    pub fn new(file: &'a SaltFile, release_mode: bool, registry: Option<&'a Registry>, z3_ctx: &'a z3::Context) -> Self {
        Self {
            // Phased state containers
            discovery: RefCell::new(crate::codegen::phases::DiscoveryState::new(file)),
            expansion: RefCell::new(crate::codegen::phases::ExpansionState::new()),
            emission: RefCell::new(crate::codegen::phases::EmissionState::new()),
            control_flow: RefCell::new(crate::codegen::phases::ControlFlowState::new()),
            
            // Verification state (has lifetime)
            z3_ctx,
            z3_solver: RefCell::new(z3::Solver::new(z3_ctx)),
            symbolic_tracker: RefCell::new(HashMap::new()),
            ownership_tracker: RefCell::new(crate::codegen::verification::Z3StateTracker::new(z3_ctx)),
            elided_checks: RefCell::new(0),
            total_checks: RefCell::new(0),
            
            // Immutable configuration
            file: RefCell::new(file),
            registry,
            release_mode,
            consuming_fns: HashMap::new(),
            suppress_specialization: Cell::new(false),
            target_platform: crate::codegen::passes::io_backend::TargetPlatform::default(),
            emit_alias_scopes: true, // default: emit scopes
            no_verify: false, // default: verification enabled
            lib_mode: false,
            sip_mode: false,
            debug_info: false,
            source_file: String::new(),
            
            // Per-function state
            evaluator: RefCell::new(Evaluator::new()),
            current_package: RefCell::new(file.package.clone()),
            
            // Malloc tracking
            malloc_tracker: RefCell::new(crate::codegen::verification::MallocTracker::new()),
            pending_malloc_result: RefCell::new(None),
            
            // Pointer state tracking
            pointer_tracker: RefCell::new(crate::codegen::verification::PointerStateTracker::new()),
            pending_pointer_state: RefCell::new(None),
            
            // Arena escape analysis (Scope Ladder)
            arena_escape_tracker: RefCell::new(crate::codegen::verification::ArenaEscapeTracker::new()),
            pending_arena_provenance: RefCell::new(None),
        }
    }

    pub fn with_registry(mut self, registry: &'a Registry) -> Self {
        self.registry = Some(registry);
        self
    }

    // === Field Accessors (delegate to phased structs) ===
    // These provide backward-compatible access while state is organized by phase.
    
    // Discovery phase accessors
    pub fn struct_templates(&self) -> std::cell::Ref<'_, std::collections::HashMap<String, StructDef>> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.struct_templates)
    }
    pub fn struct_templates_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<String, StructDef>> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.struct_templates)
    }
    pub fn enum_templates(&self) -> std::cell::Ref<'_, std::collections::HashMap<String, EnumDef>> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.enum_templates)
    }
    pub fn enum_templates_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<String, EnumDef>> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.enum_templates)
    }
    pub fn struct_registry(&self) -> std::cell::Ref<'_, std::collections::HashMap<TypeKey, StructInfo>> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.struct_registry)
    }
    pub fn struct_registry_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<TypeKey, StructInfo>> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.struct_registry)
    }
    pub fn enum_registry(&self) -> std::cell::Ref<'_, std::collections::HashMap<TypeKey, EnumInfo>> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.enum_registry)
    }
    pub fn enum_registry_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<TypeKey, EnumInfo>> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.enum_registry)
    }

    /// Structurally detect whether a type is a Result enum — any enum with Ok + Err variants.
    /// Returns the EnumInfo if matched. Uses enum registry lookup, zero string hacks.
    pub fn is_result_enum(&self, ty: &Type) -> Option<EnumInfo> {
        let name = match ty {
            Type::Enum(n) | Type::Concrete(n, _) => n,
            _ => return None,
        };
        let base = name.split("__").last().unwrap_or(name);
        let registry = self.enum_registry();
        registry.values().find(|info| {
            let info_base = info.name.split("__").last().unwrap_or(&info.name);
            // Match by: exact name, FQN contains, base names, or template_name
            let name_match = info.name == *name
                || name.ends_with(&format!("__{}", info.name))
                || info.name.ends_with(&format!("__{}", name))
                || base == info_base
                || info_base.starts_with(base)
                || base.starts_with(info_base)
                || info.template_name.as_deref() == Some(base);
            // Structural gate: must have Ok + Err variants
            name_match
                && info.variants.iter().any(|(v, _, _)| v == "Ok")
                && info.variants.iter().any(|(v, _, _)| v == "Err")
        }).cloned()
    }

    /// Structurally detect whether a type is an Option enum — any enum with Some + None variants.
    /// Returns the EnumInfo if matched. Uses enum registry lookup, zero string hacks.
    pub fn is_option_enum(&self, ty: &Type) -> Option<EnumInfo> {
        let name = match ty {
            Type::Enum(n) | Type::Concrete(n, _) => n,
            _ => return None,
        };
        let base = name.split("__").last().unwrap_or(name);
        let registry = self.enum_registry();
        registry.values().find(|info| {
            let info_base = info.name.split("__").last().unwrap_or(&info.name);
            let name_match = info.name == *name
                || name.ends_with(&format!("__{}", info.name))
                || info.name.ends_with(&format!("__{}", name))
                || base == info_base
                || info_base.starts_with(base)
                || base.starts_with(info_base)
                || info.template_name.as_deref() == Some(base);
            name_match
                && info.variants.iter().any(|(v, _, _)| v == "Some")
                && info.variants.iter().any(|(v, _, _)| v == "None")
        }).cloned()
    }
    // [V4.0 SOVEREIGN] Signature-aware method resolution - the ONLY method lookup path
    pub fn trait_registry(&self) -> std::cell::Ref<'_, crate::codegen::trait_registry::TraitRegistry> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.trait_registry)
    }
    pub fn trait_registry_mut(&self) -> std::cell::RefMut<'_, crate::codegen::trait_registry::TraitRegistry> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.trait_registry)
    }
    // [V4.0] String prefix handlers for comptime string processing
    pub fn string_prefix_handlers(&self) -> std::cell::Ref<'_, std::collections::HashMap<String, String>> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.string_prefix_handlers)
    }
    pub fn string_prefix_handlers_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<String, String>> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.string_prefix_handlers)
    }
    
    /// [V4.0] Check if comptime is ready (std discovery complete)
    pub fn is_comptime_ready(&self) -> bool {
        self.discovery.borrow().comptime_ready
    }
    
    /// [V4.0] Mark comptime as ready after std library discovery
    pub fn set_comptime_ready(&self) {
        self.discovery.borrow_mut().comptime_ready = true;
    }
    
    /// [SOVEREIGN V2.0] Register a pulse function discovered during analysis
    pub fn register_pulse_function(&self, name: &str, frequency_hz: u32, tier: u8) {
        self.discovery.borrow_mut().pulse_functions.insert(name.to_string(), (frequency_hz, tier));
    }
    
    /// [SOVEREIGN V2.0] Check if a function is a pulse function and get its tier
    pub fn get_pulse_info(&self, name: &str) -> Option<(u32, u8)> {
        self.discovery.borrow().pulse_functions.get(name).copied()
    }
    
    /// [SOVEREIGN V2.0] Check if a function requires yield injection (is pulse function)
    pub fn is_pulse_function(&self, name: &str) -> bool {
        self.discovery.borrow().pulse_functions.contains_key(name)
    }

    /// [SOVEREIGN V7.0] Register a type's Sovereign Home module.
    pub fn register_type_home(&self, type_name: String, module_package: String) {
        self.discovery.borrow_mut().register_type_home(type_name, module_package);
    }

    /// [SOVEREIGN V7.0] Register a trait's home module.
    pub fn register_trait_home(&self, trait_name: String, module_package: String) {
        self.discovery.borrow_mut().register_trait_home(trait_name, module_package);
    }

    /// [SOVEREIGN V7.0] Check if this module owns the type.
    pub fn is_type_home(&self, type_name: &str, current_module: &str) -> bool {
        self.discovery.borrow().is_type_home(type_name, current_module)
    }

    /// [SOVEREIGN V7.0] Check if this module owns the trait.
    pub fn is_trait_home(&self, trait_name: &str, current_module: &str) -> bool {
        self.discovery.borrow().is_trait_home(trait_name, current_module)
    }

    /// [SOVEREIGN V7.0] Register a trait impl and check for duplicates.
    pub fn register_trait_impl(&self, type_name: String, trait_name: String, module_package: String) -> Result<(), String> {
        self.discovery.borrow_mut().register_trait_impl(type_name, trait_name, module_package)
    }

    /// [SOVEREIGN V7.0] Validate coherence of all trait implementations.
    pub fn validate_coherence(&self) -> Result<(), String> {
        self.discovery.borrow().validate_coherence()
    }

    /// [SOVEREIGN V2.0] Register liveness analysis result for a @yielding function
    pub fn register_liveness(&self, fn_name: String, result: crate::codegen::passes::liveness::LivenessResult) {
        self.discovery.borrow_mut().liveness_results.insert(fn_name, result);
    }

    /// [SOVEREIGN V2.0] Get liveness result for a function (None if synchronous)
    pub fn get_liveness(&self, fn_name: &str) -> Option<crate::codegen::passes::liveness::LivenessResult> {
        self.discovery.borrow().liveness_results.get(fn_name).cloned()
    }

    /// [PHASE 11] Register HIR items for an async function that has been lowered
    /// via lower_async_fn_cfg. The items (struct + step fn) are cached so emit_fn
    /// can bypass AST codegen and delegate directly to emit_hir_items.
    pub fn register_hir_async(&self, fn_name: &str, items: Vec<crate::hir::items::Item>) {
        self.discovery.borrow_mut().hir_async_items.insert(fn_name.to_string(), items);
    }

    /// [PHASE 11] Get HIR items for a function (None if not lowered via HIR path)
    pub fn get_hir_async_items(&self, fn_name: &str) -> Option<Vec<crate::hir::items::Item>> {
        self.discovery.borrow().hir_async_items.get(fn_name).cloned()
    }

    /// [SOVEREIGN V2.0] Get the I/O backend for the current target platform.
    /// Returns a boxed trait object implementing platform-specific I/O MLIR emission.
    pub fn io_backend(&self) -> Box<dyn crate::codegen::passes::io_backend::IoBackend> {
        crate::codegen::passes::io_backend::backend_for_target(self.target_platform)
    }
    
    /// [V4.0] Process a prefixed string literal using comptime handlers
    /// During bootstrap: returns None (use Rust fallback)
    /// After ready: returns Some(generated_code) using native expansion
    pub fn process_prefixed_string(&self, prefix: &str, content: &str) -> Option<String> {
        // Bootstrap safety: if comptime not ready, use Rust fallback
        if !self.is_comptime_ready() {
            return None;
        }
        
        // [V4.0 SCORCHED EARTH] Native f-string expansion with TraitRegistry context
        if prefix == "f" {
            return Some(self.native_fstring_expand(content));
        }
        
        // [V4.0 LIBRARY SOVEREIGNTY] Native hex string expansion
        // hex"DEADBEEF" → Vec::<u8>::from_array([0xDE, 0xAD, 0xBE, 0xEF])
        if prefix == "hex" {
            return Some(self.native_hex_expand(content));
        }
        
        // For other prefixes, check registry
        let _handler_name = self.string_prefix_handlers().get(prefix)?.clone();
        // TODO(v0.2): Invoke comptime string prefix handler once the comptime evaluator is ready
        None
    }
    
    /// [V4.0 SCORCHED EARTH] Native f-string expansion
    /// This replaces lib.rs preprocessing with TraitRegistry-aware generation
    pub fn native_fstring_expand(&self, content: &str) -> String {
        // Parse segments from f-string content
        let segments = self.parse_fstring_segments(content);
        
        if segments.is_empty() {
            return "\"\"".to_string();
        }
        
        // Check if pure literal (no interpolations)
        let has_interpolation = segments.iter().any(|s| matches!(s, FStringSegment::Expr(_, _)));
        if !has_interpolation {
            if let Some(FStringSegment::Literal(s)) = segments.first() {
                return format!("\"{}\"", self.escape_string(s));
            }
        }
        
        // Calculate sizes for InterpolatedStringHandler
        let mut literal_len = 0;
        let mut interp_count = 0;
        for seg in &segments {
            match seg {
                FStringSegment::Literal(s) => literal_len += s.len(),
                FStringSegment::Expr(_, _) => interp_count += 1,
            }
        }
        
        // Generate InterpolatedStringHandler block
        // [V4.0 FIX] Use Rust path notation (::) since syn parses . as field access
        let mut code = String::new();
        code.push_str("{\n");
        code.push_str(&format!(
            "    let mut __h = std::string::InterpolatedStringHandler::new({}, {});\n",
            literal_len, interp_count
        ));
        
        for seg in &segments {
            match seg {
                FStringSegment::Literal(s) => {
                    let escaped = self.escape_string(s);
                    code.push_str(&format!(
                        "    __h.append_literal(\"{}\", {});\n",
                        escaped, s.len()
                    ));
                }
                FStringSegment::Expr(expr, spec) => {
                    // [V4.0] TraitRegistry-aware format spec handling
                    let formatted = self.format_with_spec_v4(expr, spec.as_deref());
                    if formatted.starts_with("fmt_") {
                        // Format-spec expression (e.g., {x:.2f}) -> append_fmt
                        code.push_str(&format!(
                            "    __h.append_fmt({});\n",
                            formatted
                        ));
                    } else {
                        // [V5.0 STRUCTURAL FORMATTING] Type-aware dispatch via internal macro
                        // The __fstring_append_expr! macro resolves the expression's type at
                        // compile time and dispatches to append_i32/append_i64/append_f64/append_bool
                        // or the fmt() call chain for struct types.
                        code.push_str(&format!(
                            "    __fstring_append_expr!(__h, {});\n",
                            formatted
                        ));
                    }
                }
            }
        }
        
        code.push_str("    __h.finalize()\n");
        code.push_str("}");
        
        code
    }
    
    /// Parse f-string content into segments
    pub fn parse_fstring_segments(&self, content: &str) -> Vec<FStringSegment> {
        let mut segments = Vec::new();
        let mut chars = content.chars().peekable();
        let mut current_literal = String::new();
        
        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    // Check for escaped brace {{
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        current_literal.push('{');
                        continue;
                    }
                    
                    // Flush current literal
                    if !current_literal.is_empty() {
                        segments.push(FStringSegment::Literal(std::mem::take(&mut current_literal)));
                    }
                    
                    // Parse expression with optional format spec
                    let (expr, spec) = self.parse_fstring_expr(&mut chars);
                    if !expr.is_empty() {
                        segments.push(FStringSegment::Expr(expr, spec));
                    }
                }
                '}' => {
                    // Check for escaped brace }}
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        current_literal.push('}');
                    }
                    // Otherwise ignore stray }
                }
                '\\' => {
                    current_literal.push('\\');
                    if let Some(escaped) = chars.next() {
                        current_literal.push(escaped);
                    }
                }
                _ => {
                    current_literal.push(c);
                }
            }
        }
        
        // Flush remaining literal
        if !current_literal.is_empty() {
            segments.push(FStringSegment::Literal(current_literal));
        }
        
        segments
    }
    
    /// Parse expression inside {} including optional format spec
    fn parse_fstring_expr(&self, chars: &mut std::iter::Peekable<std::str::Chars>) -> (String, Option<String>) {
        let mut expr = String::new();
        let mut spec = None;
        let mut depth = 0;
        
        loop {
            match chars.peek() {
                None => break,
                Some(&'}') if depth == 0 => {
                    chars.next();
                    break;
                }
                Some(&':') if depth == 0 => {
                    chars.next();
                    // Parse format spec
                    let mut spec_str = String::new();
                    loop {
                        match chars.peek() {
                            None | Some(&'}') => break,
                            Some(&c) => {
                                chars.next();
                                spec_str.push(c);
                            }
                        }
                    }
                    if chars.peek() == Some(&'}') {
                        chars.next();
                    }
                    spec = Some(spec_str);
                    break;
                }
                Some(&c) => {
                    chars.next();
                    expr.push(c);
                    
                    // Track nesting
                    match c {
                        '(' | '[' | '{' => depth += 1,
                        ')' | ']' | '}' => if depth > 0 { depth -= 1; },
                        _ => {}
                    }
                }
            }
        }
        
        (expr.trim().to_string(), spec)
    }
    
    /// [V4.0] Format with spec using TraitRegistry context
    fn format_with_spec_v4(&self, expr: &str, spec: Option<&str>) -> String {
        let spec = match spec {
            Some(s) => s.trim(),
            None => return expr.to_string(),
        };
        
        // Float precision: .Nf
        if spec.ends_with('f') {
            if let Some(precision_str) = spec.strip_suffix('f') {
                let precision_str = precision_str.strip_prefix('.').unwrap_or(precision_str);
                if let Ok(precision) = precision_str.parse::<u8>() {
                    return format!("fmt_f64({}, {})", expr, precision);
                }
            }
            // Default float precision
            return format!("fmt_f64({}, 6)", expr);
        }
        
        // Integer formats
        if spec == "d" || spec.is_empty() {
            return expr.to_string();
        }
        
        // Hex format
        if spec == "x" || spec == "X" {
            return format!("fmt_hex({})", expr);
        }
        
        // Binary format
        if spec == "b" {
            return format!("fmt_bin({})", expr);
        }
        
        // Unknown spec - pass through
        expr.to_string()
    }
    
    /// Escape string for output
    fn escape_string(&self, s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\0D")
    }
    
    /// [V4.0 LIBRARY SOVEREIGNTY] Native Hex Expansion
    /// Converts hex"DEADBEEF" → Vec::<u8>::from_array([0xDE, 0xAD, 0xBE, 0xEF])
    /// Allows whitespace separators: hex"DE AD BE EF" is valid
    pub fn native_hex_expand(&self, content: &str) -> String {
        // 1. Strip whitespace/separators (allow hex"AA BB CC")
        let clean_hex: String = content.chars().filter(|c| !c.is_whitespace()).collect();
        
        // 2. Validation: Must have even length
        if clean_hex.len() % 2 != 0 {
            eprintln!("[V4.0 HEX] ERROR: Hex literal must have even length, found {}", clean_hex.len());
            return "Vec::<u8>::new()".to_string(); // Empty vec as fallback
        }
        
        if clean_hex.is_empty() {
            return "Vec::<u8>::new()".to_string();
        }
        
        // 3. Convert hex pairs to byte literals
        let mut bytes = Vec::new();
        for i in (0..clean_hex.len()).step_by(2) {
            let byte_str = &clean_hex[i..i + 2];
            // Validate hex characters
            if u8::from_str_radix(byte_str, 16).is_err() {
                eprintln!("[V4.0 HEX] ERROR: Invalid hex digit in: {}", byte_str);
                return "Vec::<u8>::new()".to_string();
            }
            bytes.push(format!("0x{}", byte_str.to_uppercase()));
        }
        
        // 4. Generate Salt source code for Vec constructor
        format!("Vec::<u8>::from_array([{}])", bytes.join(", "))
    }
    
    /// [SOVEREIGN WRITER PROTOCOL] Native Target F-String Expansion
    /// Converts target.f"Hello {x}" → { target.write_str("Hello ", 6); target.write_i32(x); }
    /// This implements zero-allocation streaming by decomposing the f-string into direct
    /// write_* calls on the target Writer, avoiding intermediate String allocation.
    pub fn native_target_fstring_expand(&self, target: &str, content: &str) -> String {
        // Parse segments from f-string content (reuses existing parser)
        let segments = self.parse_fstring_segments(content);
        
        if segments.is_empty() {
            // Empty string - just return unit
            return "{ }".to_string();
        }
        
        // Generate block with direct write calls
        let mut code = String::new();
        code.push_str("{\n");
        
        for seg in &segments {
            match seg {
                FStringSegment::Literal(s) => {
                    if !s.is_empty() {
                        let escaped = self.escape_string(s);
                        code.push_str(&format!(
                            "    {}.write_str(\"{}\", {});\n",
                            target, escaped, s.len()
                        ));
                    }
                }
                FStringSegment::Expr(expr, spec) => {
                    // [TIERED LOWERING] Determine the appropriate write method based on type/spec
                    // For now, we use heuristics. In a full implementation, we'd query type info.
                    let (method, formatted_expr) = self.determine_write_method(expr, spec.as_deref());
                    
                    code.push_str(&format!(
                        "    {}.{}({});\n",
                        target, method, formatted_expr
                    ));
                }
            }
        }
        
        code.push_str("}");
        code
    }
    
    /// [SOVEREIGN WRITER PROTOCOL] Determine the appropriate write_* method for an interpolated expression
    /// Returns (method_name, formatted_expression)
    fn determine_write_method(&self, expr: &str, spec: Option<&str>) -> (String, String) {
        // Check format spec first - it overrides type inference
        if let Some(s) = spec {
            // Float with precision: .Nf
            if s.ends_with('f') {
                let precision_str = s.strip_suffix('f').unwrap_or("").strip_prefix('.').unwrap_or("6");
                let precision = precision_str.parse::<u8>().unwrap_or(6);
                return ("write_f64_prec".to_string(), format!("{}, {}", expr, precision));
            }
            
            // Boolean
            if s == "?" {
                return ("write_bool".to_string(), expr.to_string());
            }
        }
        
        // Type inference heuristics based on expression patterns
        let expr_trimmed = expr.trim();
        
        // Check for literal patterns
        if expr_trimmed.starts_with('"') || expr_trimmed.starts_with("&\"") {
            // String literal - use write_str
            // We need to extract the length... for now fall back to write_str pattern
            return ("write_str".to_string(), format!("{}, strlen({})", expr, expr));
        }
        
        // Check for float literals (contains . or f suffix)
        if expr_trimmed.contains('.') && !expr_trimmed.contains("::") 
           || expr_trimmed.ends_with("f32") || expr_trimmed.ends_with("f64") {
            let precision = spec.and_then(|s| {
                s.strip_suffix('f')?.strip_prefix('.')?.parse::<u8>().ok()
            }).unwrap_or(6);
            return ("write_f64_prec".to_string(), format!("{}, {}", expr, precision));
        }
        
        // Check for bool literals
        if expr_trimmed == "true" || expr_trimmed == "false" {
            return ("write_bool".to_string(), expr.to_string());
        }
        
        // Check for i64 suffix
        if expr_trimmed.ends_with("i64") || expr_trimmed.ends_with("u64") {
            return ("write_i64".to_string(), expr.to_string());
        }
        
        // Default to write_i32 for integer expressions (most common case)
        ("write_i32".to_string(), expr.to_string())
    }
    
    pub fn globals(&self) -> std::cell::Ref<'_, std::collections::BTreeMap<String, Type>> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.globals)
    }
    pub fn globals_mut(&self) -> std::cell::RefMut<'_, std::collections::BTreeMap<String, Type>> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.globals)
    }
    pub fn imports(&self) -> std::cell::Ref<'_, Vec<ImportDecl>> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.imports)
    }
    pub fn imports_mut(&self) -> std::cell::RefMut<'_, Vec<ImportDecl>> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.imports)
    }
    pub fn generic_impls(&self) -> std::cell::Ref<'_, std::collections::HashMap<String, (SaltFn, Vec<ImportDecl>)>> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.generic_impls)
    }
    pub fn generic_impls_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<String, (SaltFn, Vec<ImportDecl>)>> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.generic_impls)
    }
    pub fn entity_registry(&self) -> std::cell::Ref<'_, crate::codegen::collector::EntityRegistry> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.entity_registry)
    }
    pub fn entity_registry_mut(&self) -> std::cell::RefMut<'_, crate::codegen::collector::EntityRegistry> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.entity_registry)
    }
    
    // Expansion phase accessors
    pub fn specializations(&self) -> std::cell::Ref<'_, std::collections::HashMap<(String, Vec<Type>), String>> {
        std::cell::Ref::map(self.expansion.borrow(), |e| &e.specializations)
    }
    pub fn specializations_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<(String, Vec<Type>), String>> {
        std::cell::RefMut::map(self.expansion.borrow_mut(), |e| &mut e.specializations)
    }
    pub fn pending_generations(&self) -> std::cell::Ref<'_, std::collections::VecDeque<MonomorphizationTask>> {
        std::cell::Ref::map(self.expansion.borrow(), |e| &e.pending_generations)
    }
    pub fn pending_generations_mut(&self) -> std::cell::RefMut<'_, std::collections::VecDeque<MonomorphizationTask>> {
        std::cell::RefMut::map(self.expansion.borrow_mut(), |e| &mut e.pending_generations)
    }
    pub fn monomorphizer(&self) -> std::cell::Ref<'_, crate::codegen::phases::MonomorphizerState> {
        std::cell::Ref::map(self.expansion.borrow(), |e| &e.monomorphizer)
    }
    pub fn monomorphizer_mut(&self) -> std::cell::RefMut<'_, crate::codegen::phases::MonomorphizerState> {
        std::cell::RefMut::map(self.expansion.borrow_mut(), |e| &mut e.monomorphizer)
    }
    pub fn current_type_map(&self) -> std::cell::Ref<'_, std::collections::BTreeMap<String, Type>> {
        std::cell::Ref::map(self.expansion.borrow(), |e| &e.current_type_map)
    }
    pub fn current_type_map_mut(&self) -> std::cell::RefMut<'_, std::collections::BTreeMap<String, Type>> {
        std::cell::RefMut::map(self.expansion.borrow_mut(), |e| &mut e.current_type_map)
    }
    pub fn current_generic_args(&self) -> std::cell::Ref<'_, Vec<Type>> {
        std::cell::Ref::map(self.expansion.borrow(), |e| &e.current_generic_args)
    }
    pub fn current_generic_args_mut(&self) -> std::cell::RefMut<'_, Vec<Type>> {
        std::cell::RefMut::map(self.expansion.borrow_mut(), |e| &mut e.current_generic_args)
    }
    pub fn current_self_ty(&self) -> std::cell::Ref<'_, Option<Type>> {
        std::cell::Ref::map(self.expansion.borrow(), |e| &e.current_self_ty)
    }
    pub fn current_self_ty_mut(&self) -> std::cell::RefMut<'_, Option<Type>> {
        std::cell::RefMut::map(self.expansion.borrow_mut(), |e| &mut e.current_self_ty)
    }
    pub fn current_ret_ty(&self) -> std::cell::Ref<'_, Option<Type>> {
        std::cell::Ref::map(self.expansion.borrow(), |e| &e.current_ret_ty)
    }
    pub fn current_ret_ty_mut(&self) -> std::cell::RefMut<'_, Option<Type>> {
        std::cell::RefMut::map(self.expansion.borrow_mut(), |e| &mut e.current_ret_ty)
    }
    pub fn current_fn_name(&self) -> std::cell::Ref<'_, String> {
        std::cell::Ref::map(self.expansion.borrow(), |e| &e.current_fn_name)
    }
    pub fn current_fn_name_mut(&self) -> std::cell::RefMut<'_, String> {
        std::cell::RefMut::map(self.expansion.borrow_mut(), |e| &mut e.current_fn_name)
    }
    
    // Emission phase accessors
    pub fn val_counter(&self) -> std::cell::Ref<'_, usize> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.val_counter)
    }
    pub fn val_counter_mut(&self) -> std::cell::RefMut<'_, usize> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.val_counter)
    }
    pub fn alloca_out(&self) -> std::cell::Ref<'_, String> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.alloca_out)
    }
    pub fn alloca_out_mut(&self) -> std::cell::RefMut<'_, String> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.alloca_out)
    }
    pub fn decl_out(&self) -> std::cell::Ref<'_, String> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.decl_out)
    }
    pub fn decl_out_mut(&self) -> std::cell::RefMut<'_, String> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.decl_out)
    }
    pub fn definitions_buffer(&self) -> std::cell::Ref<'_, String> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.definitions_buffer)
    }
    pub fn definitions_buffer_mut(&self) -> std::cell::RefMut<'_, String> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.definitions_buffer)
    }
    pub fn string_literals(&self) -> std::cell::Ref<'_, Vec<(String, String, usize)>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.string_literals)
    }
    pub fn string_literals_mut(&self) -> std::cell::RefMut<'_, Vec<(String, String, usize)>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.string_literals)
    }
    pub fn defined_functions(&self) -> std::cell::Ref<'_, std::collections::HashSet<String>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.defined_functions)
    }
    pub fn defined_functions_mut(&self) -> std::cell::RefMut<'_, std::collections::HashSet<String>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.defined_functions)
    }
    pub fn pending_func_decls_mut(&self) -> std::cell::RefMut<'_, std::collections::BTreeMap<String, String>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.pending_func_decls)
    }
    pub fn defined_structs(&self) -> std::cell::Ref<'_, std::collections::HashSet<String>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.defined_structs)
    }
    pub fn defined_structs_mut(&self) -> std::cell::RefMut<'_, std::collections::HashSet<String>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.defined_structs)
    }
    pub fn defined_enums(&self) -> std::cell::Ref<'_, std::collections::HashSet<String>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.defined_enums)
    }
    pub fn defined_enums_mut(&self) -> std::cell::RefMut<'_, std::collections::HashSet<String>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.defined_enums)
    }
    pub fn emitted_types(&self) -> std::cell::Ref<'_, std::collections::HashSet<String>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.emitted_types)
    }
    pub fn emitted_types_mut(&self) -> std::cell::RefMut<'_, std::collections::HashSet<String>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.emitted_types)
    }
    pub fn external_decls(&self) -> std::cell::Ref<'_, std::collections::HashSet<String>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.external_decls)
    }
    pub fn external_decls_mut(&self) -> std::cell::RefMut<'_, std::collections::HashSet<String>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.external_decls)
    }
    pub fn initialized_globals(&self) -> std::cell::Ref<'_, std::collections::HashSet<String>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.initialized_globals)
    }
    pub fn initialized_globals_mut(&self) -> std::cell::RefMut<'_, std::collections::HashSet<String>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.initialized_globals)
    }
    pub fn layout_cache(&self) -> std::cell::Ref<'_, std::collections::HashMap<Type, (usize, usize)>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.layout_cache)
    }
    pub fn layout_cache_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<Type, (usize, usize)>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.layout_cache)
    }
    pub fn tensor_layout_cache(&self) -> std::cell::Ref<'_, std::collections::HashMap<Type, crate::codegen::phases::TensorLayout>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.tensor_layout_cache)
    }
    pub fn tensor_layout_cache_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<Type, crate::codegen::phases::TensorLayout>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.tensor_layout_cache)
    }
    pub fn mlir_type_cache(&self) -> std::cell::Ref<'_, std::collections::HashMap<Type, String>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.mlir_type_cache)
    }
    pub fn mlir_type_cache_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<Type, String>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.mlir_type_cache)
    }
    pub fn struct_type_cache(&self) -> std::cell::Ref<'_, Option<std::collections::HashMap<String, Vec<Type>>>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.struct_type_cache)
    }
    pub fn struct_type_cache_mut(&self) -> std::cell::RefMut<'_, Option<std::collections::HashMap<String, Vec<Type>>>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.struct_type_cache)
    }
    pub fn interner(&self) -> std::cell::Ref<'_, crate::codegen::phases::StringInterner> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.interner)
    }
    pub fn interner_mut(&self) -> std::cell::RefMut<'_, crate::codegen::phases::StringInterner> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.interner)
    }
    pub fn metadata_id_counter(&self) -> std::cell::Ref<'_, usize> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.metadata_id_counter)
    }
    pub fn metadata_id_counter_mut(&self) -> std::cell::RefMut<'_, usize> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.metadata_id_counter)
    }
    pub fn pending_bootstrap_patches(&self) -> std::cell::Ref<'_, Vec<crate::codegen::const_eval::BootstrapPatch>> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.pending_bootstrap_patches)
    }
    pub fn pending_bootstrap_patches_mut(&self) -> std::cell::RefMut<'_, Vec<crate::codegen::const_eval::BootstrapPatch>> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.pending_bootstrap_patches)
    }
    pub fn linalg_initialized(&self) -> std::cell::Ref<'_, bool> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.linalg_initialized)
    }
    pub fn linalg_initialized_mut(&self) -> std::cell::RefMut<'_, bool> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.linalg_initialized)
    }
    
    // [VERIFIED METAL] TypeID Registry accessors
    pub fn type_id_registry(&self) -> std::cell::Ref<'_, crate::codegen::types::TypeIDRegistry> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.type_id_registry)
    }
    pub fn type_id_registry_mut(&self) -> std::cell::RefMut<'_, crate::codegen::types::TypeIDRegistry> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.type_id_registry)
    }
    
    // [VERIFIED METAL] Phase 4: Body buffer accessors
    pub fn buffer_body(&self, code: &str) {
        self.emission.borrow_mut().buffer_body(code);
    }
    
    pub fn get_buffered_body(&self) -> String {
        self.emission.borrow().get_buffered_body().to_string()
    }
    
    /// [VERIFIED METAL] Phase 4: Look up struct MLIR layout by canonical name
    /// Returns the physical MLIR layout string for a given canonical type name.
    pub fn lookup_struct_layout_by_canonical(&self, canonical_name: &str) -> Option<String> {
        let registry = self.struct_registry();
        
        // Find struct info where:
        // 1. The name matches exactly, OR
        // 2. The canonical name of the struct matches
        for info in registry.values() {
            let info_canonical = crate::types::Type::Struct(info.name.clone()).to_canonical_name();
            if info.name == canonical_name || info_canonical == canonical_name {
                // Build the MLIR struct layout
                let mut layout_parts = Vec::new();
                for field_ty in &info.field_order {
                    match self.resolve_mlir_storage_type(&field_ty) {
                        Ok(s) => layout_parts.push(s),
                        Err(_) => layout_parts.push("!llvm.ptr".to_string()),
                    }
                }
                return Some(format!("!llvm.struct<\"{}\", ({})>", info.name, layout_parts.join(", ")));
            }
        }
        
        None
    }
    
    /// [VERIFIED METAL] Phase 4: Finalize MLIR output after all specializations
    pub fn finalize_mlir_output(&self, header: &str) -> String {
        let struct_registry = self.struct_registry();
        
        // Create lookup closure
        let lookup = |canonical_name: &str| -> Option<String> {
            for info in struct_registry.values() {
                let info_canonical = crate::types::Type::Struct(info.name.clone()).to_canonical_name();
                if info.name == canonical_name || info_canonical == canonical_name {
                    let mut layout_parts = Vec::new();
                    for field_ty in &info.field_order {
                        match self.resolve_mlir_storage_type(&field_ty) {
                            Ok(s) => layout_parts.push(s),
                            Err(_) => layout_parts.push("!llvm.ptr".to_string()),
                        }
                    }
                    return Some(format!("!llvm.struct<\"{}\", ({})>", info.name, layout_parts.join(", ")));
                }
            }
            None
        };
        
        // Generate canonical aliases
        let emission = self.emission.borrow();
        let aliases = emission.generate_canonical_aliases(lookup);
        drop(emission);
        
        let mut final_output = String::new();
        final_output.push_str(&aliases);
        final_output.push_str(header);
        final_output.push_str("\n// --- FUNCTION BODIES ---\n");
        final_output.push_str(&self.get_buffered_body());
        
        final_output
    }
    
    /// [VERIFIED METAL] Phase 5: Identity-Based Struct Lookup by TypeID
    /// Resolves a TypeID to its physical StructInfo with zero string matching.
    /// 
    /// This is the core of the "Suffix Purge" - we use the TypeID (structural hash)
    /// to directly locate the exact struct, bypassing all `ends_with()` heuristics.
    pub fn lookup_struct_by_id(&self, id: crate::codegen::types::TypeID) -> Option<crate::registry::StructInfo> {
        let registry = self.type_id_registry();
        let canonical_name = registry.get_canonical_name(id)?;
        
        // Find the physical struct whose canonical name matches the ID's name
        // This uses Phase 1's normalization logic for matching
        let struct_reg = self.struct_registry();
        struct_reg.values().find(|info| {
            let info_canonical = crate::types::Type::Struct(info.name.clone()).to_canonical_name();
            info.name == canonical_name || info_canonical == canonical_name
        }).cloned()
    }
    
    /// [VERIFIED METAL] Phase 5: Identity-Based Struct Lookup by Type
    /// Convenience method that extracts TypeID from a Type and looks up the StructInfo.
    /// 
    /// This is the primary entry point for field access hardening.
    /// Instead of suffix matching, we compute the TypeID and do a direct lookup.
    pub fn lookup_struct_by_type(&self, ty: &crate::types::Type) -> Option<crate::registry::StructInfo> {
        // First, try to resolve via TypeID
        let canonical_name = ty.to_canonical_name();
        if let Some(type_id) = self.type_id_registry().lookup(&canonical_name) {
            if let Some(info) = self.lookup_struct_by_id(type_id) {
                return Some(info);
            }
        }
        
        // Fallback: direct name lookup (for types not yet in registry)
        let canonical_name = ty.to_canonical_name();
        let struct_reg = self.struct_registry();
        struct_reg.values().find(|info| {
            let info_canonical = crate::types::Type::Struct(info.name.clone()).to_canonical_name();
            info.name == canonical_name || info_canonical == canonical_name
        }).cloned()
    }
    
    /// [VERIFIED METAL] Phase 5: Find struct by name with canonical fallback
    /// This replaces all `ends_with()` heuristics with TypeID-based lookup.
    /// 
    /// Priority order:
    /// 1. Exact name match
    /// 2. TypeID canonical lookup
    /// 3. Suffix fallback (shortest match wins)
    pub fn find_struct_by_name(&self, name: &str) -> Option<crate::registry::StructInfo> {
        let struct_reg = self.struct_registry();
        
        // 1. Exact match
        if let Some(info) = struct_reg.values().find(|i| i.name == name) {
            return Some(info.clone());
        }
        
        // 2. TypeID canonical lookup
        let ty = crate::types::Type::Struct(name.to_string());
        if let Some(info) = self.lookup_struct_by_type(&ty) {
            return Some(info);
        }
        
        // 3. Suffix fallback - pick shortest match (most specific)
        let suffix = format!("__{}", name);
        let mut best_match: Option<crate::registry::StructInfo> = None;
        for info in struct_reg.values() {
            if info.name.ends_with(&suffix) {
                if best_match.as_ref().map_or(true, |b| info.name.len() < b.name.len()) {
                    best_match = Some(info.clone());
                }
            }
        }
        best_match
    }
    
    /// [VERIFIED METAL] Phase 5: Find template by name with suffix fallback
    /// Returns the template key if found.
    pub fn find_struct_template_by_name(&self, name: &str) -> Option<String> {
        let templates = self.struct_templates();
        
        // 1. Exact match
        if templates.contains_key(name) {
            return Some(name.to_string());
        }
        
        // 2. Suffix fallback - pick shortest match
        let suffix = format!("__{}", name);
        let mut best_match: Option<String> = None;
        for k in templates.keys() {
            if k.ends_with(&suffix) {
                if best_match.as_ref().map_or(true, |b| k.len() < b.len()) {
                    best_match = Some(k.clone());
                }
            }
        }
        best_match
    }
    
    /// [VERIFIED METAL] Phase 5: Find enum template by name with suffix fallback
    pub fn find_enum_template_by_name(&self, name: &str) -> Option<String> {
        let templates = self.enum_templates();
        
        // 1. Exact match
        if templates.contains_key(name) {
            return Some(name.to_string());
        }
        
        // 2. Suffix fallback - pick shortest match
        let suffix = format!("__{}", name);
        let mut best_match: Option<String> = None;
        for k in templates.keys() {
            if k.ends_with(&suffix) {
                if best_match.as_ref().map_or(true, |b| k.len() < b.len()) {
                    best_match = Some(k.clone());
                }
            }
        }
        best_match
    }
    
    // Control flow phase accessors
    pub fn loop_exit_stack(&self) -> std::cell::Ref<'_, Vec<String>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.loop_exit_stack)
    }
    pub fn loop_exit_stack_mut(&self) -> std::cell::RefMut<'_, Vec<String>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.loop_exit_stack)
    }
    pub fn break_labels(&self) -> std::cell::Ref<'_, Vec<String>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.break_labels)
    }
    pub fn break_labels_mut(&self) -> std::cell::RefMut<'_, Vec<String>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.break_labels)
    }
    pub fn continue_labels(&self) -> std::cell::Ref<'_, Vec<String>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.continue_labels)
    }
    pub fn continue_labels_mut(&self) -> std::cell::RefMut<'_, Vec<String>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.continue_labels)
    }
    pub fn region_stack(&self) -> std::cell::Ref<'_, Vec<String>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.region_stack)
    }
    pub fn region_stack_mut(&self) -> std::cell::RefMut<'_, Vec<String>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.region_stack)
    }
    pub fn cleanup_stack(&self) -> std::cell::Ref<'_, Vec<Vec<crate::codegen::phases::CleanupTask>>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.cleanup_stack)
    }
    pub fn cleanup_stack_mut(&self) -> std::cell::RefMut<'_, Vec<Vec<crate::codegen::phases::CleanupTask>>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.cleanup_stack)
    }
    pub fn mutated_vars(&self) -> std::cell::Ref<'_, std::collections::HashSet<String>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.mutated_vars)
    }
    pub fn mutated_vars_mut(&self) -> std::cell::RefMut<'_, std::collections::HashSet<String>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.mutated_vars)
    }
    pub fn consumed_vars(&self) -> std::cell::Ref<'_, std::collections::HashSet<String>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.consumed_vars)
    }
    pub fn consumed_vars_mut(&self) -> std::cell::RefMut<'_, std::collections::HashSet<String>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.consumed_vars)
    }
    pub fn consumption_locs(&self) -> std::cell::Ref<'_, std::collections::HashMap<String, String>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.consumption_locs)
    }
    pub fn consumption_locs_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<String, String>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.consumption_locs)
    }
    pub fn devoured_vars(&self) -> std::cell::Ref<'_, std::collections::HashSet<String>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.devoured_vars)
    }
    pub fn devoured_vars_mut(&self) -> std::cell::RefMut<'_, std::collections::HashSet<String>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.devoured_vars)
    }
    pub fn affine_depth(&self) -> std::cell::Ref<'_, usize> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.affine_depth)
    }
    pub fn affine_depth_mut(&self) -> std::cell::RefMut<'_, usize> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.affine_depth)
    }
    pub fn is_unsafe_block(&self) -> std::cell::Ref<'_, bool> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.is_unsafe_block)
    }
    pub fn is_unsafe_block_mut(&self) -> std::cell::RefMut<'_, bool> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.is_unsafe_block)
    }
    pub fn no_yield(&self) -> std::cell::Ref<'_, bool> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.no_yield)
    }
    pub fn no_yield_mut(&self) -> std::cell::RefMut<'_, bool> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.no_yield)
    }
    pub fn current_pulse(&self) -> std::cell::Ref<'_, Option<u32>> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.current_pulse)
    }
    pub fn current_pulse_mut(&self) -> std::cell::RefMut<'_, Option<u32>> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.current_pulse)
    }
    pub fn is_hot_path(&self) -> std::cell::Ref<'_, bool> {
        std::cell::Ref::map(self.control_flow.borrow(), |c| &c.is_hot_path)
    }
    pub fn is_hot_path_mut(&self) -> std::cell::RefMut<'_, bool> {
        std::cell::RefMut::map(self.control_flow.borrow_mut(), |c| &mut c.is_hot_path)
    }

    pub fn invalidate_type_cache(&self) {
        *self.struct_type_cache_mut() = None;
    }

    // === RAII-Lite: Implicit Scoped Drop Methods ===

    /// Enter a new lexical scope (e.g., function body, loop body)
    pub fn push_cleanup_scope(&self) {
        self.cleanup_stack_mut().push(Vec::new());
    }

    /// Register an owned resource for cleanup at scope exit
    /// Also registers with Z3StateTracker for formal verification
    pub fn register_owned_resource(&self, value: &str, drop_fn: &str, var_name: &str, ty: Type) {
        if let Some(scope) = self.cleanup_stack_mut().last_mut() {
            scope.push(crate::codegen::phases::CleanupTask {
                value: value.to_string(),
                drop_fn: drop_fn.to_string(),
                var_name: var_name.to_string(),
                ty,
            });
        }
        
        // [V1.1] Z3 Ownership Ledger: Register BIRTH event
        // Use var_name for better error messages (maps to source variable)
        self.ownership_tracker.borrow_mut().register_allocation(
            var_name,
            &self.z3_solver.borrow()
        );
    }

    /// Pop the current scope and emit cleanup calls for all remaining resources
    /// Also marks resources as Released in Z3StateTracker
    pub fn pop_and_emit_cleanup(&self, out: &mut String) -> Result<(), String> {
        if let Some(tasks) = self.cleanup_stack_mut().pop() {
            // Emit in reverse order (LIFO - last allocated, first freed)
            for task in tasks.into_iter().rev() {
                self.ownership_tracker.borrow_mut().mark_released(
                    &task.var_name,
                    &self.z3_solver.borrow()
                )?;
                
                // Emit the drop function call
                let mlir_ty = self.resolve_mlir_type(&task.ty)?;
                out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", 
                    task.drop_fn, task.value, mlir_ty));
            }
        }
        Ok(())
    }

    /// Transfer ownership of a resource (e.g., when returning it)
    /// Removes the resource from cleanup tracking so it won't be freed
    /// Also marks as Moved in Z3StateTracker
    pub fn transfer_ownership(&self, value: &str) -> Result<(), String> {
        let mut stack = self.cleanup_stack_mut();
        for scope in stack.iter_mut() {
            if let Some(pos) = scope.iter().position(|t: &crate::codegen::phases::CleanupTask| t.value == value) {
                let _task = scope.remove(pos);
                
                // [V1.1] Z3 Ownership Ledger: Register MOVE event
                self.ownership_tracker.borrow_mut().mark_moved(
                    value, // We track by value name in SSA
                    &self.z3_solver.borrow()
                )?;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Remove a resource from the cleanup stack by its SOURCE variable name.
    /// Called when the user explicitly calls .free() or .drop() on a variable,
    /// so the RAII system won't emit a duplicate cleanup call.
    pub fn release_by_var_name(&self, var_name: &str) {
        let mut stack = self.cleanup_stack_mut();
        for scope in stack.iter_mut() {
            if let Some(pos) = scope.iter().position(|t: &crate::codegen::phases::CleanupTask| t.var_name == var_name) {
                scope.remove(pos);
                return;
            }
        }
    }
    pub fn mk_int(&self, val: i64) -> z3::ast::Int<'a> {
        z3::ast::Int::from_i64(self.z3_ctx, val)
    }
    pub fn mk_var(&self, name: &str) -> z3::ast::Int<'a> {
        z3::ast::Int::new_const(self.z3_ctx, name)
    }
    pub fn push_solver(&self) {
        self.z3_solver.borrow().push();
    }
    pub fn pop_solver(&self) {
        self.z3_solver.borrow().pop(1);
    }
    pub fn add_assertion(&self, expr: &z3::ast::Bool<'a>) {
        self.z3_solver.borrow().assert(expr);
    }
    /// [Z3 VERIFICATION] Check if a violation condition is provably unsatisfiable.
    /// 
    /// Returns `true` if Z3 can prove the violation is impossible (UNSAT),
    /// meaning the code is provably safe. Returns `false` if Z3 finds a 
    /// counterexample (SAT) or times out (Unknown).
    pub fn is_provably_safe(&self, violation: &z3::ast::Bool<'a>) -> bool {
        
        
        // Create a fresh solver for this check (isolated from main solver state)
        let solver = z3::Solver::new(self.z3_ctx);
        
        // Set timeout to 100ms to prevent hangs on complex expressions
        let mut params = z3::Params::new(self.z3_ctx);
        params.set_u32("timeout", 100);
        solver.set_params(&params);
        
        // Assert the violation and check if it's satisfiable
        solver.assert(violation);
        
        match solver.check() {
            z3::SatResult::Unsat => {
                // No counterexample exists - code is provably safe
                true
            }
            z3::SatResult::Sat => {
                // Counterexample found - violation is possible
                false
            }
            z3::SatResult::Unknown => {
                // Timeout or complexity limit - conservatively return false
                eprintln!("Z3: Unknown result (timeout?) for violation check");
                false
            }
        }
    }
    pub fn register_symbolic_int(&self, ssa_name: String, val: z3::ast::Int<'a>) {
        self.symbolic_tracker.borrow_mut().insert(ssa_name, val);
    }
    pub fn get_symbolic_int(&self, ssa_name: &str) -> Option<z3::ast::Int<'a>> {
        self.symbolic_tracker.borrow().get(ssa_name).cloned()
    }

    pub fn find_methods_for_template(&self, template_name: &str) -> Vec<String> {
        // [V4.0 SOVEREIGN] Delegate to TraitRegistry
        self.trait_registry().find_methods_for_type(template_name)
    }

    /// SOVEREIGN RESOLUTION: Unified Pointer Peeling
    /// Instead of checking Reference/Owned/NativePtr separately, we peel the 
    /// first-class Type::Pointer variant.
    pub fn resolve_method(&self, receiver_ty: &Type, method_name: &str) -> Result<(SaltFn, Option<Type>, Vec<ImportDecl>), String> {
        let mut current_ty = receiver_ty.clone();
        let mut depth = 0;
        
        loop {
            if depth > 10 { break; }
            depth += 1;

            // [V4.0 SOVEREIGN] Lookup via TraitRegistry signature-aware resolution
            if let Some(key) = current_ty.to_key() {

                // Try exact key lookup
                if let Some(result) = self.trait_registry().get_legacy(&key, method_name) {
                    return Ok(result);
                }
                // Try template key lookup
                let template_key = key.to_template();

                if let Some(result) = self.trait_registry().get_legacy(&template_key, method_name) {
                    return Ok(result);
                }
            }

            // 2. UNIFIED DEREF: Peeling the Sovereign Pointer
            // This replaces legacy Reference/Owned/NativePtr branches.
            if let Type::Pointer { element, .. } = current_ty {
                current_ty = (*element).clone();
                continue;
            }

            break; 
        }

        Err(format!("Method '{}' not found for type '{}' (peeled depth: {})", method_name, receiver_ty.mangle_suffix(), depth))
    }

    #[allow(dead_code)]
    fn find_template_base(&self, name: &str) -> Option<String> {
        // 1. Check Registry Metadata
        if let Some(info) = self.struct_registry().values().find(|i| &i.name == name) {
            if let Some(tn) = &info.template_name { return Some(tn.clone()); }
        }
        if let Some(info) = self.enum_registry().values().find(|i| &i.name == name) {
            if let Some(tn) = &info.template_name { return Some(tn.clone()); }
        }

        // 2. Suffix Heuristic (Deep Search)
        // Check Struct Templates
        {
            let templates = self.struct_templates();
            for t_name in templates.keys() {
                if name.starts_with(t_name) && name.len() > t_name.len() {
                    if name.chars().nth(t_name.len()) == Some('_') {
                        return Some(t_name.clone()); 
                    }
                }
            }
        }
        // Check Enum Templates
        {
            let templates = self.enum_templates();
            for t_name in templates.keys() {
                if name.starts_with(t_name) && name.len() > t_name.len() {
                    if name.chars().nth(t_name.len()) == Some('_') {
                        return Some(t_name.clone());
                    }
                }
            }
        }
        
        None
    }

    /// Resolves a field name to its stable MLIR index for GEP operations.
    pub fn get_field_index(&self, key: &TypeKey, field_name: &str) -> Result<usize, String> {
        let registry = self.struct_registry();
        
        // 1. Fetch the specialized struct info using the key direclty
        // The registry is keyed by TypeKey now.
        let struct_info = registry.get(key).ok_or_else(|| {
            format!("Monomorphized layout for '{}' not found in registry", key.mangle())
        })?;

        // 2. Find the index of the field
        struct_info.fields.get(field_name).map(|(idx, _)| *idx)
            .ok_or_else(|| format!("Field '{}' does not exist on type '{}'", field_name, key.mangle()))
    }

    pub fn resolve_gep(
        &self,
        out: &mut String, // Assuming we write to string buffer usually? Or do we return value?
        // User snippet returned mlir::Value and used `self.builder`.
        // Current codegen writes to `out: &mut String`.
        // I will adapt to current style: return register name string.
        base_ptr: &str, 
        key: &TypeKey, 
        field_name: &str
    ) -> Result<String, String> {
        // 1. Resolve the specialized index (e.g., 'len' is index 1)
        let index = self.get_field_index(key, field_name)?;
        
        // 2. Resolve the MLIR struct type
        // logic to get mlir type string
        // We can construct a dummy Type::Struct/Concrete from key to get mlir type string
        let _dummy_ty = if let Some(args) = &key.specialization {
             Type::Concrete(key.mangle(), args.clone()) // This might be circular if mangle uses args?
             // Actually Type::Concrete expects "BaseName".
             // We should reconstruct the Type from Key.
        } else {
             Type::Struct(key.mangle())
        };
        // Use TypeKey to reconstruct Type properly for to_mlir_type lookup?
        // Actually to_mlir_type uses registry lookup.
        // We can just use the mangled name for the explicit struct type in GEP?
        // LLVM GEP needs the type Pointee.
        
        let struct_mlir_ty = format!("!llvm.struct<\"{}\">", key.mangle());
        // Or better verify it exists?
        
        let res = format!("%gep_{}_{}", field_name, self.next_id());
        
        // 3. Emit GEP
        // %res = llvm.getelementptr %base[0, index] : (!llvm.ptr) -> !llvm.ptr, !llvm.struct<...>
        // Note: The second type in GEP result is the Element Type?
        // MLIR llvm.getelementptr syntax:
        // %res = llvm.getelementptr %base[0, %idx] : (!llvm.ptr, i32) -> !llvm.ptr, !llvm.struct<...>
        
        out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n", 
            res, base_ptr, index, struct_mlir_ty));
            
        Ok(res)
    }

    // =========================================================================
    // BRIDGE METHODS: CodegenContext → LoweringContext delegation
    // These allow impl CodegenContext code to call functions that have been
    // migrated to &mut LoweringContext. Each creates a temporary LoweringContext
    // via with_lowering_ctx and delegates.
    // =========================================================================

    /// Bridge: specialize_template (migrated to LoweringContext in type_bridge.rs)
    pub fn specialize_template(&self, base_name: &str, concrete_tys: &[Type], is_enum: bool) -> Result<crate::types::TypeKey, String> {
        self.with_lowering_ctx(|lctx| lctx.specialize_template(base_name, concrete_tys, is_enum))
    }

    /// Bridge: scan_function_for_calls (migrated to LoweringContext in seeker.rs)
    pub fn scan_function_for_calls(&self, func: &crate::grammar::SaltFn) -> Result<Vec<crate::codegen::collector::MonomorphizationTask>, String> {
        self.with_lowering_ctx(|lctx| lctx.scan_function_for_calls(func))
    }

    /// Bridge: to_mlir_type via LoweringContext
    pub fn resolve_mlir_type(&self, ty: &Type) -> Result<String, String> {
        self.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx))
    }

    /// Bridge: to_mlir_storage_type via LoweringContext
    pub fn resolve_mlir_storage_type(&self, ty: &Type) -> Result<String, String> {
        self.with_lowering_ctx(|lctx| ty.to_mlir_storage_type(lctx))
    }

    /// Bridge: resolve_type via LoweringContext (type_bridge)
    pub fn bridge_resolve_type(&self, ty: &crate::grammar::SynType) -> Type {
        self.with_lowering_ctx(|lctx| crate::codegen::type_bridge::resolve_type(lctx, ty))
    }

    /// Bridge: resolve_codegen_type via LoweringContext (type_bridge)
    pub fn bridge_resolve_codegen_type(&self, ty: &Type) -> Type {
        self.with_lowering_ctx(|lctx| crate::codegen::type_bridge::resolve_codegen_type(lctx, ty))
    }

    /// Bridge: emit_global_def via LoweringContext (type_bridge)
    pub fn bridge_emit_global_def(&self, out: &mut String, g: &crate::grammar::GlobalDef) -> Result<(), String> {
        self.with_lowering_ctx(|lctx| crate::codegen::type_bridge::emit_global_def(lctx, out, g))
    }

    /// Bridge: emit_const via LoweringContext (type_bridge)
    pub fn bridge_emit_const(&self, out: &mut String, c: &crate::grammar::ConstDef) -> Result<(), String> {
        self.with_lowering_ctx(|lctx| crate::codegen::type_bridge::emit_const(lctx, out, c))
    }

    /// Bridge: resolve_package_prefix_ctx via LoweringContext (expr/utils)
    pub fn bridge_resolve_package_prefix(&self, segments: &[String]) -> Option<(String, String)> {
        self.with_lowering_ctx(|lctx| crate::codegen::expr::utils::resolve_package_prefix_ctx(lctx, segments))
    }

    /// Bridge: request_specialization via LoweringContext (type_bridge)
    pub fn request_specialization(&self, func_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        self.with_lowering_ctx(|lctx| lctx.request_specialization(func_name, concrete_tys, self_ty))
    }
}


pub struct GenericContextGuard<'b, 'a> {
    ctx: &'b CodegenContext<'a>,
    old_args: std::collections::BTreeMap<String, Type>,
    old_self: Option<Type>,
    old_ordered_args: Vec<Type>,
}

impl<'b, 'a> GenericContextGuard<'b, 'a> {
    pub fn new(ctx: &'b CodegenContext<'a>, new_args: std::collections::BTreeMap<String, Type>, self_ty: Type, ordered_args: Vec<Type>) -> Self {
        let old_args = std::mem::replace(&mut *ctx.current_type_map_mut(), new_args);
        let old_self = std::mem::replace(&mut *ctx.current_self_ty_mut(), Some(self_ty));
        let old_ordered_args = std::mem::replace(&mut *ctx.current_generic_args_mut(), ordered_args);
        Self { ctx, old_args, old_self, old_ordered_args }
    }
}

impl<'b, 'a> Drop for GenericContextGuard<'b, 'a> {
    fn drop(&mut self) {
        *self.ctx.current_type_map_mut() = self.old_args.clone();
        *self.ctx.current_self_ty_mut() = self.old_self.clone();
        *self.ctx.current_generic_args_mut() = self.old_ordered_args.clone();
    }
}

pub struct ImportContextGuard<'b, 'a> {
    ctx: &'b CodegenContext<'a>,
    old_imports: Vec<ImportDecl>,
}

impl<'b, 'a> ImportContextGuard<'b, 'a> {
    pub fn new(ctx: &'b CodegenContext<'a>, new_imports: Vec<ImportDecl>) -> Self {
        let old_imports = std::mem::replace(&mut *ctx.imports_mut(), new_imports);
        Self { ctx, old_imports }
    }
}

impl<'b, 'a> Drop for ImportContextGuard<'b, 'a> {
    fn drop(&mut self) {
        *self.ctx.imports_mut() = self.old_imports.clone();
    }
}

impl<'a> CodegenContext<'a> {

    pub fn get_struct_types(&self) -> HashMap<String, Vec<Type>> {
        if let Some(cache) = self.struct_type_cache().as_ref() {
            return cache.clone();
        }
        let mut map = HashMap::new();
        // Iterate over struct_registry (keyed by TypeKey)
        // We use info.name for the string map key (mangled name)
        for (_key, info) in self.struct_registry().iter() {
            // ...
            let n: String = info.name.clone();
            map.insert(n, info.field_order.clone());
        }
        if let Some(reg) = self.registry {
            for mod_info in reg.modules.values() {
                for (name, info) in &mod_info.structs {
                    let n: String = name.clone();
                    map.insert(n, info.iter().map(|(_, ty)| ty.clone()).collect());
                }
            }
        }
        *self.struct_type_cache_mut() = Some(map.clone());
        map
    }

    pub fn resolve_path_to_fqn(&self, path: &syn::Path) -> Result<TypeKey, String> {
        // 1. Extract raw segments (e.g., ["Vec", "new"])
        let segments: Vec<String> = path.segments.iter()
            .map(|s| s.ident.to_string())
            .collect();

        if segments.is_empty() {
            return Err("Empty path encountered in scanner".into());
        }

        // 2. Resolve the prefix using 'resolve_package_prefix'
        // This leverages the ImportContextGuard state.
        // e.g., "Vec" -> "std__collections__vec__Vec"
        if let Some((pkg, item)) = self.bridge_resolve_package_prefix(&segments) {
             let fqn_base = if item.is_empty() { pkg } else { format!("{}__{}", pkg, item) };
             
             // 3. Construct the TypeKey
             // We assume the first part of the FQN is the namespace path, 
             // and the last part is the template name.
             let parts: Vec<&str> = fqn_base.split("__").collect();
             let name = parts.last().unwrap_or(&"").to_string();
             let path_segments = parts[..parts.len()-1].iter().map(|s| s.to_string()).collect();

             Ok(TypeKey {
                 path: path_segments,
                 name,
                 specialization: None, // Scanner determines specialization later
             })
        } else {
             // Local Fallback (or failure)
             // Handle simple local function calls in scripts (no package)
             if segments.len() == 1 {
                 let name = &segments[0];
                 // If it's in globals (which includes local file functions in main), assume valid.
                 // Hydration will verify existence later via resolve_global_to_task.
                 return Ok(TypeKey {
                     path: vec![], // Local root
                     name: name.clone(),
                     specialization: None,
                 });
             }

             Err(format!("Could not resolve path to FQN: {:?}", segments))
        }
    }

    pub fn mangle_fn_name(&self, name: &str) -> Rc<str> {
        // Main and Externals remain special
        if name == "main" || self.external_decls().contains(name) {
            return self.interner_mut().intern(name);
        }

        // Avoid double-mangling if already fully qualified
        if name.starts_with("std__") || name.starts_with("core__") || name.starts_with("benchmarks__") {
             return self.interner_mut().intern(name);
        }
        if let Some(reg) = self.registry {
            for mod_info in reg.modules.values() {
                let pkg_prefix = Mangler::mangle(&mod_info.package.split('.').collect::<Vec<_>>()) + "__";
                if name.starts_with(&pkg_prefix) {
                    return self.interner_mut().intern(name);
                }
            }
        }

        let mangled = if let Some(pkg) = self.current_package.borrow().as_ref() {
            let pkg_name = Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
            let pkg_prefix = format!("{}__{}", pkg_name, "");
            if name.starts_with(&pkg_prefix) {
                name.to_string()
            } else {
                Mangler::mangle(&[pkg_name.as_str(), name])
            }
        } else {
            name.to_string()
        };

        self.interner_mut().intern(&mangled)
    }

    pub fn get_mangled(&self, ty: &Type) -> Rc<str> {
        let m = ty.mangle_suffix();
        self.interner_mut().intern(&m)
    }

    pub fn get_layout(&self, ty: &Type) -> (usize, usize) {
        if let Some(layout) = self.layout_cache().get(ty) {
            return *layout;
        }
        
        let size = ty.size_of(&*self.struct_registry());
        let align = ty.align_of(&*self.struct_registry());
        
        self.layout_cache_mut().insert(ty.clone(), (size, align));
        (size, align)
    }

    pub fn size_of(&self, ty: &Type) -> usize {
        self.get_layout(ty).0
    }

    pub fn align_of(&self, ty: &Type) -> usize {
        self.get_layout(ty).1
    }

    pub fn get_physical_index(&self, _field_order: &[Type], logical_idx: usize) -> usize {
        // With !llvm.struct, padding is implicit and handled by LLVM.
        // The logical index (field index) maps 1:1 to the physical element index.
        logical_idx
    }

    pub fn register_builtins(&mut self) {
        self.struct_templates_mut().insert("Window".to_string(), syn::parse_str("struct Window<T, R> { data: &T, len: usize }").unwrap());

        self.globals_mut().insert("sys_write".to_string(), Type::Fn(vec![Type::I32, Type::Reference(Box::new(Type::U8), false), Type::I64], Box::new(Type::I64)));
        self.globals_mut().insert("sys_read".to_string(), Type::Fn(vec![Type::I32, Type::Reference(Box::new(Type::U8), false), Type::I64], Box::new(Type::I64)));
        self.globals_mut().insert("sys_exit".to_string(), Type::Fn(vec![Type::I32], Box::new(Type::Unit)));
        // Note: Do NOT add these to defined_functions — they are FFI functions
        // without emitted bodies. Adding them to defined_functions would suppress
        // their func.func private forward declarations in the MLIR output.
    }

    // --- Unified Driver Extensions ---

    pub fn should_flatten_type(&self, task: &crate::codegen::collector::MonomorphizationTask) -> bool {
        // "Linus" Rule: Check if the return type or self type implies a single-field wrapper that should be erased.
        // Actually, for "Structural Identity" logic, we care if the *Concrete Instantiation* is just a wrapper.
        // e.g. Ptr<T> -> i64.
        
        // Check Self Type
        if let Some(st) = &task.self_ty {
            // FIX: Do NOT flatten methods on Ptr types (e.g. from_raw). We need the symbol to exist.
            // if st.k_is_ptr_type() { return true; }
            // Generalize: Check if struct has 1 field which is primitive
            if let Type::Concrete(base, _) = st {
                 if let Some(def) = self.struct_templates().get(base) {
                     if def.fields.len() == 1 {
                         // Check if field is primitive (this is hard without mapping args, 
                         // but for now Ptr is the main target).
                         // We can rely on k_is_ptr_type covering the Ptr case.
                     }
                 }
            }
        }
        false
    }

    pub fn hydrate_task(&self, task: crate::codegen::collector::MonomorphizationTask) -> Result<(), String> {
        // 1. Activate Context (Generics)
        // 1. Activate Context (Generics) - Guard handles type map AND ordered args now
        let _guard = GenericContextGuard::new(self, task.type_map.clone(), Type::Unit, task.concrete_tys.clone());
        
        // 2. Resolve Self
        if let Some(raw_self) = &task.self_ty {
             let resolved_self = self.bridge_resolve_codegen_type(raw_self);
             *self.current_self_ty_mut() = Some(resolved_self);
        } else {
             *self.current_self_ty_mut() = None;
        }

        // 3. Set Imports & Function Context
        let old_imports = self.imports().clone();
        *self.imports_mut() = task.imports.clone();
        
        let old_fn_name = self.current_fn_name().clone();
        *self.current_fn_name_mut() = task.mangled_name.clone();
        
        // Infer Package Context from Mangled Name (Copied from emit_specialized_generation logic)
        let old_pkg = self.current_package.borrow().clone();
        let parts: Vec<&str> = task.mangled_name.split("__").collect();
        if parts.len() > 1 {
            let mut best_pkg = old_pkg.clone();
            for i in (1..parts.len()).rev() {
                 let candidate = parts[0..i].join(".");
                 let exists = self.registry.as_ref().map_or(false, |r| r.modules.contains_key(&candidate));
                 if exists {
                     let pkg_str = format!("package {};", candidate);
                     if let Ok(pkg) = syn::parse_str::<crate::grammar::PackageDecl>(&pkg_str) {
                         best_pkg = Some(pkg);
                         break;
                     }
                 }
            }
            *self.current_package.borrow_mut() = best_pkg;
        }

        // 4. Discovery Scan
        let new_tasks_res = self.scan_function_for_calls(&task.func);
        
        // Restore before exit

        
        *self.imports_mut() = old_imports;
        *self.current_fn_name_mut() = old_fn_name;
        *self.current_package.borrow_mut() = old_pkg;
        
        // Process Scan Result
        let new_tasks = new_tasks_res?;
        for t in new_tasks {
            self.entity_registry_mut().request_specialization(t);
        }
        
        // 5. Mark as Hydrated in Registry
        let def = crate::codegen::collector::SpecializedFn {
            func: task.func.clone(),
            concrete_tys: task.concrete_tys.clone(),
            self_ty: task.self_ty.clone(),
            imports: task.imports.clone(),
            is_flattened: false,
        };
        self.entity_registry_mut().mark_hydrated(task.mangled_name.clone(), def);
        
        Ok(())
    }


    /// SCORCHED EARTH: Removed legacy Ptr bootstrap.
    /// Ptr<T> is now a first-class grammar construct and requires no import injection.
    pub fn inject_self_imports(&self, file: &crate::grammar::SaltFile) {
        let pkg_prefix = if let Some(pkg) = &file.package {
            Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()) + "__"
        } else { String::new() };

        let mut self_imports = Vec::new();
        if !pkg_prefix.is_empty() {
            for item in &file.items {
                 let (ident_name, mangled_str) = match item {
                     Item::Struct(s) => (&s.name, format!("{}{}", pkg_prefix, s.name)),
                     Item::Enum(e) => (&e.name, format!("{}{}", pkg_prefix, e.name)),
                     _ => continue
                 };
                 let mangled_ident = syn::Ident::new(&mangled_str, proc_macro2::Span::call_site());
                 let mut p = syn::punctuated::Punctuated::new();
                 p.push(mangled_ident);
                 self_imports.push(crate::grammar::ImportDecl { 
                     name: p, 
                     alias: Some(ident_name.clone()), 
                     group: None 
                 });
            }
        }
        self.imports_mut().extend(self_imports);
    }


    // --- LAZY REVOLUTION: The Recursive Context Switcher ---

    pub fn is_function_defined(&self, mangled_name: &str) -> bool {
        if self.defined_functions().contains(mangled_name) {

             return true;
        }
        if self.external_decls().contains(mangled_name) {

             return true;
        }
        false
    }

    /// [DEMAND-DRIVEN GENERATION] Ensure a local function is scheduled for generation.
    /// Used by emit_path when taking a function pointer, as this doesn't trigger
    /// the normal call-graph discovery.
    pub fn require_local_function(&self, mangled_name: &str) -> bool {
        // Check if already requested in the global registry
        if self.discovery.borrow().entity_registry.identity_map.contains(mangled_name) {
            return true;
        }

        let file = self.file.borrow();
        let current_pkg_prefix = if let Some(pkg) = &file.package {
             Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()) + "__"
        } else {
             String::new()
        };

        for item in &file.items {
            if let Item::Fn(f) = item {
                // Check if this function matches the mangled name
                let my_mangled = if f.attributes.iter().any(|a| a.name == "no_mangle") {
                    f.name.to_string()
                } else {
                    format!("{}{}", current_pkg_prefix, f.name)
                };

                if my_mangled == mangled_name {
                     // Found it! Schedule it.
                     
                     // Construct TypeKey
                     let path = if let Some(pkg) = &file.package {
                         pkg.name.iter().map(|id| id.to_string()).collect()
                     } else {
                         vec![]
                     };
                     
                     let identity = TypeKey {
                         path,
                         name: f.name.to_string(),
                         specialization: None, // Non-generic for now
                     };

                     // Create task
                     let task = MonomorphizationTask {
                         identity,
                         mangled_name: mangled_name.to_string(),
                         func: f.clone(),
                         concrete_tys: vec![],
                         self_ty: None,
                         imports: file.imports.clone(),
                         type_map: std::collections::BTreeMap::new(),
                     };
                     
                     // Push to worklist and mark seen
                     self.expansion.borrow_mut().pending_generations.push_back(task);
                     self.discovery.borrow_mut().entity_registry.identity_map.insert(mangled_name.to_string());
                     return true;
                }
            }
        }
        false
    }

    pub fn hydrate_specialization(&self, task: MonomorphizationTask) -> Result<(), String> {
        let mangled_name = task.mangled_name.clone();

        // 0. GENERICS CHECK: Backend cannot handle unresolved generics
        // If the specialization task contains Type::Generic, it's an invalid request
        // (likely a byproduct of inference or dead code). Skip it to avoid crashes.
        if task.type_map.values().any(|t| t.has_generics()) {
             return Ok(());
        }

        // 1. EXISTENCE CHECK: Use the absolute mangled name
        // If it's already in the defined_functions set, we've either finished it
        // or we are currently emitting it (handling recursion).
        // [FIX] Do NOT check external_decls here. Forward declarations (func.func private)
        // are in external_decls state, but we still need to emit the body for local functions.
        if self.defined_functions().contains(&mangled_name) {
             return Ok(());
        }

        // 1.5. EXTERN GUARD: Extern functions are FFI declarations (provided by runtime.c).
        // They must NOT be hydrated — doing so would add them to defined_functions (via the
        // recursion guard below), which would cause the assembly filter to suppress their
        // forward declarations. Call sites need those declarations to resolve.
        // NOTE: We must check BOTH external_decls AND empty body, because external_decls
        // is also populated by ensure_func_declared for forward-declared Salt functions.
        // True externs have empty SaltFn bodies (set by register_signatures wrapper).
        if self.external_decls().contains(&mangled_name) {
            if let Some((wrapper, _)) = self.generic_impls().get(&mangled_name) {
                if wrapper.body.stmts.is_empty() {
                    return Ok(());
                }
            }
        }

        // 2. PENDING REGISTRATION: Recursion Guard
        self.defined_functions_mut().insert(mangled_name.clone());

        // 3. CONTEXT SNAPSHOT: Preserve caller state
        // We use MANUAL context switching for maximum control, mirroring the "Senior Staff" advice.
        // Guards are good, but explicit snapshots are better for debugging complex recursions.
        let prev_type_map = self.current_type_map().clone();
        let prev_concrete = self.current_generic_args().clone();
        let prev_self = self.current_self_ty().clone();
        let prev_imports = self.imports().clone();
        let prev_ret_ty = self.current_ret_ty().clone();

        // 4. CONTEXT SWITCH: Load callee environment
        // [CANONICAL RESOLUTION] Canonicalize type_map entries before emission.
        // During hydration of std library methods (e.g., Box::new<T>), the type_map may contain
        // raw Struct("Node") instead of canonical Struct("main__Node"). This causes split MLIR
        // type aliases (e.g., !struct_Box_Node vs !struct_Box_main__Node). Canonicalizing here
        // ensures all downstream emission uses consistent FQN names.
        let mut canonical_type_map = task.type_map.clone();
        for (_key, ty) in canonical_type_map.iter_mut() {
            if let crate::types::Type::Struct(name) = ty {
                if !name.contains("__") {
                    let suffix = format!("__{}", name);
                    if let Some(canonical) = self.struct_templates().keys()
                        .find(|k| k.ends_with(&suffix))
                        .cloned()
                    {
                        *name = canonical;
                    }
                }
            }
        }
        *self.current_type_map_mut() = canonical_type_map.clone();
        *self.current_generic_args_mut() = task.concrete_tys.clone();
        *self.current_self_ty_mut() = task.self_ty.clone();
        *self.imports_mut() = task.imports.clone();
        *self.imports_mut() = task.imports.clone();

        // Switch Package Context
        let package_path: Vec<syn::Ident> = task.identity.path.iter()
            .map(|s| syn::Ident::new(s, proc_macro2::Span::call_site()))
            .collect();
        let new_package = if package_path.is_empty() {
            None
        } else {
             let mut p = syn::punctuated::Punctuated::new();
             for ident in package_path {
                 p.push(ident);
             }
             Some(crate::grammar::PackageDecl { name: p })
        };
        let prev_pkg = self.current_package.replace(new_package);

        // [MODULE GLOBALS FIX] Inject self-imports for module-level items (globals, structs, consts, fns).
        // Without this, hydrated functions can't resolve bare names like `COUNTER` or `GLOBAL_SCHED`
        // to their mangled forms (e.g., `test__module_globals__COUNTER`).
        {
            let pkg_prefix = if let Some(pkg) = self.current_package.borrow().as_ref() {
                Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()) + "__"
            } else {
                String::new()
            };

            if !pkg_prefix.is_empty() {
                let file = self.file.borrow();
                let mut self_imports = Vec::new();
                for item in &file.items {
                    let (ident_name, mangled_str) = match item {
                        Item::Struct(s) => (&s.name, format!("{}{}", pkg_prefix, s.name)),
                        Item::Enum(e) => (&e.name, format!("{}{}", pkg_prefix, e.name)),
                        Item::Fn(f) => (&f.name, if f.attributes.iter().any(|a| a.name == "no_mangle") { f.name.to_string() } else { format!("{}{}", pkg_prefix, f.name) }),
                        Item::ExternFn(e) => (&e.name, e.name.to_string()),
                        Item::Global(g) => (&g.name, format!("{}{}", pkg_prefix, g.name)),
                        Item::Const(c) => (&c.name, format!("{}{}", pkg_prefix, c.name)),
                        _ => continue
                    };
                    let mangled_ident = syn::Ident::new(&mangled_str, proc_macro2::Span::call_site());
                    let mut p = syn::punctuated::Punctuated::new();
                    p.push(mangled_ident);
                    self_imports.push(crate::grammar::ImportDecl {
                        name: p,
                        alias: Some(ident_name.clone()),
                        group: None
                    });
                }
                self.imports_mut().extend(self_imports);
            }
        }

        // 5. EMISSION: Generate the specialized MLIR body
        let emission_result = self.emit_function_definition(&task.func, &mangled_name);

        // 6. STATE RESTORATION: Return to caller's environment
        let expected_import_count = prev_imports.len();
        *self.current_type_map_mut() = prev_type_map;
        *self.current_generic_args_mut() = prev_concrete;
        *self.current_self_ty_mut() = prev_self;
        *self.imports_mut() = prev_imports;
        *self.current_ret_ty_mut() = prev_ret_ty;
        self.current_package.replace(prev_pkg);
        debug_assert_eq!(self.imports().len(), expected_import_count, 
            "IMPORT CLOBBER in hydrate_specialization for '{}': saved {} imports but restored {}",
            mangled_name, expected_import_count, self.imports().len());
        if self.imports().len() != expected_import_count {
            eprintln!("[BUG] IMPORT CLOBBER after hydrate_specialization '{}': expected {} imports, got {}",
                mangled_name, expected_import_count, self.imports().len());
        }

        // 7. HANDLE RESULT
        match emission_result {
            Ok(code) => {
                self.definitions_buffer_mut().push_str(&code);
                Ok(())
            },
            Err(e) => {
                // Remove from Defined so we can retry or just failing clean
                self.defined_functions_mut().remove(&mangled_name);
                Err(e)
            }
        }
    }

    pub fn emit_function_definition(&self, func: &SaltFn, mangled_name: &str) -> Result<String, String> {
        emit_fn(self, func, Some(mangled_name.to_string()))
    }

    pub fn ensure_external_declaration(&self, mangled_name: &str, arg_tys: &[Type], ret_ty: &Type) -> Result<(), String> {
         if self.is_function_defined(mangled_name) {

             return Ok(());
         }
         
         // If it starts with llvm., we assume it's an intrinsic that doesn't need explicit decl (or handled elsewhere)
         if mangled_name.starts_with("llvm.") {
             return Ok(());
         }
         
         // Generate Declaration
         // func.func private @name(args...) -> ret
         let mut args_code = Vec::new();
         for ty in arg_tys {
             args_code.push(self.resolve_mlir_type(&ty)?);
         }
         
         let ret_part = if *ret_ty == Type::Unit { "".to_string() } else { format!(" -> {}", self.resolve_mlir_type(&ret_ty)?) };
         
         // [PILLAR 2] Mark contract violation as cold+noreturn so LLVM
         // moves it off the hot path and optimizes branch prediction
         let attrs = if mangled_name == "__salt_contract_violation" {
             " attributes {passthrough = [\"cold\", \"noreturn\"]}"
         } else {
             ""
         };
         self.definitions_buffer_mut().push_str(&format!("  func.func private @{}({}){}{}\n", mangled_name, args_code.join(", "), ret_part, attrs));
         self.external_decls_mut().insert(mangled_name.to_string());
         
         Ok(())
    }

    pub fn resolve_global(&self, query_name: &str) -> Option<Type> {
        // 1. Resolve Aliases via Imports (explicit `use X as Y` or self-imports)
        let resolved_name = self.imports().iter().find_map(|imp| {
            if imp.alias.as_ref().map_or(false, |a| a == query_name) {
                 Some(Mangler::mangle(&imp.name.iter().map(|i| i.to_string()).collect::<Vec<_>>()))
            } else { None }
        }).unwrap_or(query_name.to_string());

        let mangled_name = &resolved_name;

        if let Some(ty) = self.globals().get(mangled_name) {
            return Some(ty.clone());
        }
        
        // 2. [FIX] Wildcard Import Expansion: Check `use X::*` imports
        // When import has no alias AND no group, it's a wildcard import from that module.
        // We look up the query_name in that module's symbols from Registry.
        if let Some(reg) = self.registry {
            for imp in self.imports().iter() {
                // Wildcard import: has path, no alias, no group
                let is_wildcard = imp.alias.is_none() && imp.group.is_none() && !imp.name.is_empty();
                if is_wildcard {
                    // Construct the module path from import name (e.g., ["std", "string"] -> "std.string")
                    let import_path: String = imp.name.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(".");
                    
                    if let Some(mod_info) = reg.modules.get(&import_path) {
                        // Check if query_name exists in this module's exports
                        let pkg_prefix = mod_info.package.replace(".", "__");
                        
                        // Check struct templates
                        if mod_info.struct_templates.contains_key(query_name) {
                            let fqn = format!("{}__{}", pkg_prefix, query_name);
                            if let Some(ty) = self.globals().get(&fqn) {
                                return Some(ty.clone());
                            }
                            // Return as struct type even if not in globals yet
                            return Some(Type::Struct(fqn));
                        }
                        
                        // Check concrete structs
                        if mod_info.structs.contains_key(query_name) {
                            let fqn = format!("{}__{}", pkg_prefix, query_name);
                            return Some(Type::Struct(fqn));
                        }
                        
                        // Check functions
                        if let Some((args, ret)) = mod_info.functions.get(query_name) {
                            return Some(Type::Fn(args.clone(), Box::new(ret.clone())));
                        }
                        
                        // Check enums
                        if mod_info.enum_templates.contains_key(query_name) {
                            let fqn = format!("{}__{}", pkg_prefix, query_name);
                            return Some(Type::Enum(fqn));
                        }
                    }
                }
            }
        }
        
        // 3. Fallback: Try current package prefix
        // This handles cases where imports are missing but we are in the correct package context
        let pkg_mangled = self.mangle_fn_name(mangled_name).to_string();
        if pkg_mangled != *mangled_name {
            if let Some(ty) = self.globals().get(&pkg_mangled) {
                return Some(ty.clone());
            }
        }

        if let Some(reg) = self.registry {
            for mod_info in reg.modules.values() {
                let pkg_mangled = mod_info.package.replace(".", "__");
                if mangled_name.starts_with(&pkg_mangled) {
                    let item = if mangled_name.len() > pkg_mangled.len() + 2 {
                        &mangled_name[pkg_mangled.len() + 2..]
                    } else {
                        &mangled_name[pkg_mangled.len()..]
                    };
                    if let Some(ty) = mod_info.globals.get(item) {
                        let t: Type = ty.clone();
                        return Some(t);
                    }
                    if let Some((args, ret)) = mod_info.functions.get(item) {
                        let a: Vec<Type> = args.clone();
                        let r: Type = ret.clone();
                        return Some(Type::Fn(a, Box::new(r)));
                    }
                }
            }
        }
        None
    }


    pub fn resolve_global_signature(&self, mangled_name: &str) -> Option<(String, Type)> {
        if let Some(ty) = self.globals().get(mangled_name) {
            return Some((mangled_name.to_string(), ty.clone()));
        }
        if let Some(reg) = self.registry {
            for mod_info in reg.modules.values() {
                // Use imported items or FQN logic
                let pkg_mangled = mod_info.package.replace(".", "__");
                if mangled_name.starts_with(&pkg_mangled) {
                    let item = if mangled_name.len() > pkg_mangled.len() + 2 {
                        &mangled_name[pkg_mangled.len() + 2..]
                    } else {
                        &mangled_name[pkg_mangled.len()..]
                    };
                    if let Some(ty) = mod_info.globals.get(item) {
                        return Some((mangled_name.to_string(), ty.clone()));
                    }
                    if let Some((args, ret)) = mod_info.functions.get(item) {
                        return Some((mangled_name.to_string(), Type::Fn(args.clone(), Box::new(ret.clone()))));
                    }
                }
            }
        }
        None
    }

    pub fn resolve_global_func(&self, name: &str) -> Option<(Type, String)> {
         if let Some(ty) = self.globals().get(name) {
             return Some((ty.clone(), name.to_string()));
         }
         // Fallback to resolve_global logic if needed, but globals should cover it
         self.resolve_global(name).map(|t| (t, name.to_string()))
    }

    pub fn init_registry_definitions(&self) {
        self.suppress_specialization.set(true);
        // Populate struct_registry from Registry (imported modules)
        if let Some(reg) = self.registry {
            for (_pkg, module_info) in &reg.modules {
                let pkg_prefix = module_info.package.replace(".", "__") + "__";
                let path: Vec<String> = module_info.package.split('.').map(|s| s.to_string()).collect();
                
                // Populate struct templates (generic structs)
                for (struct_name, struct_def) in &module_info.struct_templates {
                    let mangled = format!("{}{}", pkg_prefix, struct_name);
                    
                    // Also update the internal name of the struct definition!
                    let mut s_def = struct_def.clone();
                     s_def.name = syn::Ident::new(&mangled, struct_def.name.span());
                    self.struct_templates_mut().insert(mangled, s_def);
                }
                
                // Populate concrete structs
                for (struct_name, fields_vec) in &module_info.structs {
                    let mangled = format!("{}{}", pkg_prefix, struct_name);
                    
                    // Convert Vec<(String, Type)> to HashMap and field_order
                    let mut fields = std::collections::HashMap::new();
                    let mut field_order = Vec::new();
                    for (i, (fname, fty)) in fields_vec.iter().enumerate() {
                        fields.insert(fname.clone(), (i, fty.clone()));
                        field_order.push(fty.clone());
                    }
                    
                    // Construct TypeKey. 
                    // Note: We currently don't have specialization args for concrete structs in Registry.
                    // We register them as 'base' types for now, which allows direct lookup if name matches.
                    // e.g. "Vec_i32".
                    let key = TypeKey {
                        path: path.clone(),
                        name: struct_name.clone(),
                        specialization: None,
                    };
                    
                    self.struct_registry_mut().insert(key, StructInfo {
                        name: mangled,
                        fields,
                        field_order,
                        template_name: None,
                        specialization_args: vec![],
                    });
                }
                
                // Populate enum templates
                for (enum_name, enum_def) in &module_info.enum_templates {
                     let mangled = format!("{}{}", pkg_prefix, enum_name);
                     let mut e_def = enum_def.clone();
                     e_def.name = syn::Ident::new(&mangled, enum_def.name.span());
                     self.enum_templates_mut().insert(mangled, e_def);
                }
                
                // Populate concrete enums
                for (enum_name, info) in &module_info.enums {
                     let mangled = format!("{}{}", pkg_prefix, enum_name);
                     let mut new_info = info.clone();
                     new_info.name = mangled.clone();
                     
                     let key = TypeKey {
                        path: path.clone(),
                        name: enum_name.clone(),
                        specialization: None,
                     };
                     
                     self.enum_registry_mut().insert(key, new_info);
                }

                // ======================================================================
                // [SOVEREIGN FIX] Transitive Extern Collection
                // Collects extern fn declarations from ALL Registry modules.
                // This fixes the visibility hole where externs in library modules
                // (e.g., sys_write in BufferedWriter) weren't being emitted to MLIR.
                // ======================================================================
                for (name, export) in &module_info.exports {
                    if export.kind == crate::registry::SymbolKind::Intrinsic {
                        // Get type signature from functions map
                        if let Some((args, ret)) = module_info.functions.get(name) {
                            // Skip if already emitted (dedupe across modules)
                            if self.external_decls().contains(name) {
                                continue;
                            }
                            
                            // [SOVEREIGN FIX] Register in globals for type resolution
                            self.globals_mut().insert(
                                name.clone(),
                                crate::types::Type::Fn(args.clone(), Box::new(ret.clone()))
                            );
                            
                            // Mark as external declaration
                            self.external_decls_mut().insert(name.clone());
                            
                            // [SOVEREIGN FIX] Emit MLIR declaration to decl_out
                            // This is critical - without this, the extern is registered but
                            // the func.func private declaration never makes it to MLIR
                            let mut args_mlir = Vec::new();
                            for arg in args {
                                if let Ok(mlir_ty) = self.resolve_mlir_type(&arg) {
                                    args_mlir.push(mlir_ty);
                                }
                            }
                            let ret_mlir = if *ret == crate::types::Type::Unit {
                                "()".to_string()
                            } else if let Ok(mlir_ty) = self.resolve_mlir_type(&ret) {
                                mlir_ty
                            } else {
                                "()".to_string()
                            };
                            
                            let decl_str = format!("  func.func private @{}({}) -> {}\n", 
                                name, args_mlir.join(", "), ret_mlir);
                            self.pending_func_decls_mut().insert(name.clone(), decl_str);
                            

                        }
                    }
                }

            }
            
            // Pass 2: Populate Impls (Resolution Dependencies Resolved)
            for (_pkg, module_info) in &reg.modules {
                // Compute package path for this module
                let pkg_path: Vec<String> = module_info.package.split('.').map(|s| s.to_string()).collect();
                
                // Populate Impls (generic methods)
                for (impl_item, impl_imports) in &module_info.impls {
                    if let crate::grammar::SaltImpl::Methods { target_ty, methods, generics } = impl_item {
                        // Context Swap for Resolution with Self-Import Injection
                        let saved_imports = self.imports().clone();
                        let saved_map = self.current_type_map().clone();
                        
                        // Populate Type Map with Generic Params (e.g. SIZE) so resolution doesn't fail
                        if let Some(g) = generics {
                            for param in &g.params {
                                let name = match param {
                                    crate::grammar::GenericParam::Type { name, .. } => name,
                                    crate::grammar::GenericParam::Const { name, .. } => name,
                                };
                                self.current_type_map_mut().insert(name.to_string(), crate::types::Type::Struct(name.to_string()));
                            }
                        }

                        let mut combined_imports = impl_imports.clone();
                        
                        // Inject Self-Imports for the module (Same logic as in inject_self_imports/scan_defs)
                        // V3.0: Only inject non-generic templates - generic templates need explicit instantiation
                        {
                            let pkg_prefix = module_info.package.replace(".", "__") + "__";
                            // Recursively find module structs/enums locally
                            let mut self_imps = Vec::new();
                            
                            for (s_name, s_def) in &module_info.struct_templates {
                                 // V3.0: Skip generic templates - they need explicit instantiation
                                 let has_generics = s_def.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false);
                                 if has_generics {
                                     continue;
                                 }
                                 
                                 let mangled = format!("{}{}", pkg_prefix, s_name);
                                 let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                 let mut p = syn::punctuated::Punctuated::new();
                                 p.push(mangled_ident);
                                 self_imps.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                            }
                            // Also concrete structs (always add - they have no generics)
                            for (s_name, _) in &module_info.structs {
                                 let mangled = format!("{}{}", pkg_prefix, s_name);
                                 let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                 let mut p = syn::punctuated::Punctuated::new();
                                 p.push(mangled_ident);
                                 self_imps.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                            }
                            combined_imports.extend(self_imps);
                        }


                        *self.imports_mut() = combined_imports;
                        
                        if let Some(target) = crate::types::Type::from_syn(&target_ty) {
                            let resolved = self.bridge_resolve_codegen_type(&target);
                            let target_mangled = resolved.mangle_suffix();
                            


                            *self.current_self_ty_mut() = Some(resolved.clone());

                            // Calculate Template Key for Method Registry
                            // FIX: Use package path so methods resolve with full FQN
                            let mut impl_key = resolved.to_key().unwrap_or_else(|| {
                                TypeKey { path: pkg_path.clone(), name: resolved.mangle_suffix(), specialization: None }
                            });

                            // Ensure path is populated from package if to_key() returned empty
                            if impl_key.path.is_empty() && !pkg_path.is_empty() {
                                impl_key.path = pkg_path.clone();

                            }

                            
                            // If this is a generic implementation (which it usually is in registry),
                            // we strip specialization to register it as the Template.
                            if impl_key.specialization.as_ref().map_or(false, |s: &Vec<Type>| !s.is_empty()) {
                                // Check if implementation actually HAS generics?
                                // "generics" var tells us.
                                if generics.is_some() {
                                    impl_key.specialization = None;
                                }
                            }


                            for m in methods {
                                 let name = format!("{}__{}", target_mangled, m.name);
                                 
                                 // Register Global Fn Sig (Mangled)
                                 let ret_ty = if let Some(rt) = &m.ret_type {
                                     crate::types::Type::from_syn(rt).unwrap_or(crate::types::Type::Unit)
                                 } else {
                                      crate::types::Type::Unit
                                 };
                                 let args: Vec<crate::types::Type> = m.args.iter()
                                         .filter_map(|arg| arg.ty.as_ref().and_then(|t| crate::types::Type::from_syn(t)))
                                         .collect();
                                 self.globals_mut().insert(name.clone(), crate::types::Type::Fn(args.clone(), Box::new(ret_ty.clone())));
                                 
                                 // Store Template
                                 let mut m_clone = m.clone();
                                 if let Some(ig) = generics {
                                     if let Some(mg) = &mut m_clone.generics {
                                         let mut new_params = ig.params.clone();
                                         new_params.extend(mg.params.iter().cloned());
                                         mg.params = new_params;
                                     } else {
                                         m_clone.generics = Some(ig.clone());
                                     }
                                 }

                                 // Capture imports before mutating (both borrow discovery)
                                 let current_imports = self.imports().clone();
                                 self.generic_impls_mut().insert(name.clone(), (m_clone.clone(), current_imports.clone()));
                                 
                                 // [V4.0 SOVEREIGN] Register via TraitRegistry with signature extraction
                                 self.trait_registry_mut().register_simple(impl_key.clone(), m_clone, Some(resolved.clone()), current_imports);
                            }
                            *self.current_self_ty_mut() = None;
                        }
                        
                        *self.imports_mut() = saved_imports;
                        *self.current_type_map_mut() = saved_map;
                    }
                    // [V4.0 SOVEREIGN] Handle trait impl blocks: `impl Trait for Type { ... }`
                    // Flatten trait methods into the implementing type's method table
                    else if let crate::grammar::SaltImpl::Trait { trait_name, target_ty, methods, generics } = impl_item {
                        let saved_imports = self.imports().clone();
                        let saved_map = self.current_type_map().clone();
                        
                        // Populate Type Map with Generic Params
                        if let Some(g) = generics {
                            for param in &g.params {
                                let name = match param {
                                    crate::grammar::GenericParam::Type { name, .. } => name,
                                    crate::grammar::GenericParam::Const { name, .. } => name,
                                };
                                self.current_type_map_mut().insert(name.to_string(), crate::types::Type::Struct(name.to_string()));
                            }
                        }

                        let mut combined_imports = impl_imports.clone();
                        {
                            let pkg_prefix = module_info.package.replace(".", "__") + "__";
                            let mut self_imps = Vec::new();
                            for (s_name, s_def) in &module_info.struct_templates {
                                 let has_generics = s_def.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false);
                                 if has_generics { continue; }
                                 let mangled = format!("{}{}", pkg_prefix, s_name);
                                 let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                 let mut p = syn::punctuated::Punctuated::new();
                                 p.push(mangled_ident);
                                 self_imps.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                            }
                            for (s_name, _) in &module_info.structs {
                                 let mangled = format!("{}{}", pkg_prefix, s_name);
                                 let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                 let mut p = syn::punctuated::Punctuated::new();
                                 p.push(mangled_ident);
                                 self_imps.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                            }
                            combined_imports.extend(self_imps);
                        }

                        *self.imports_mut() = combined_imports;
                        
                        if let Some(target) = crate::types::Type::from_syn(&target_ty) {
                            let resolved = self.bridge_resolve_codegen_type(&target);
                            let target_mangled = resolved.mangle_suffix();
                            


                            *self.current_self_ty_mut() = Some(resolved.clone());

                            // Use TARGET TYPE key (String), not trait (Writer)
                            let mut impl_key = resolved.to_key().unwrap_or_else(|| {
                                TypeKey { path: pkg_path.clone(), name: resolved.mangle_suffix(), specialization: None }
                            });
                            if impl_key.path.is_empty() && !pkg_path.is_empty() {
                                impl_key.path = pkg_path.clone();
                            }
                            if impl_key.specialization.as_ref().map_or(false, |s: &Vec<Type>| !s.is_empty()) {
                                if generics.is_some() {
                                    impl_key.specialization = None;
                                }
                            }

                            for m in methods {
                                 // Register under TARGET TYPE (String), not trait (Writer)
                                 let name = format!("{}__{}", target_mangled, m.name);
                                 
                                 let ret_ty = if let Some(rt) = &m.ret_type {
                                     crate::types::Type::from_syn(rt).unwrap_or(crate::types::Type::Unit)
                                 } else {
                                      crate::types::Type::Unit
                                 };
                                 let args: Vec<crate::types::Type> = m.args.iter()
                                         .filter_map(|arg| arg.ty.as_ref().and_then(|t| crate::types::Type::from_syn(t)))
                                         .collect();
                                 self.globals_mut().insert(name.clone(), crate::types::Type::Fn(args.clone(), Box::new(ret_ty.clone())));
                                 
                                 let mut m_clone = m.clone();
                                 if let Some(ig) = generics {
                                     if let Some(mg) = &mut m_clone.generics {
                                         let mut new_params = ig.params.clone();
                                         new_params.extend(mg.params.iter().cloned());
                                         mg.params = new_params;
                                     } else {
                                         m_clone.generics = Some(ig.clone());
                                     }
                                 }

                                 let current_imports = self.imports().clone();
                                 self.generic_impls_mut().insert(name.clone(), (m_clone.clone(), current_imports.clone()));
                                 
                                 // Flatten trait method into type's method table
                                 self.trait_registry_mut().register_simple(impl_key.clone(), m_clone, Some(resolved.clone()), current_imports);

                            }
                            *self.current_self_ty_mut() = None;
                        }
                        
                        *self.imports_mut() = saved_imports;
                        *self.current_type_map_mut() = saved_map;
                    }
                }

            }
        }
        
        self.suppress_specialization.set(false);

    }

    pub fn scan_imports_from_file(&self, file: &SaltFile) {
        self.imports_mut().extend(file.imports.clone());
    }

    pub fn scan_defs_from_file(&self, file: &SaltFile) -> Result<(), String> {
        // Fix: Update current_package to match the file being scanned.
        // This ensures mangle_fn_name (used by emit_global_def) uses the correct prefix.
        let saved_pkg = self.current_package.replace(file.package.clone());

        let pkg_prefix = if let Some(pkg) = &file.package {
            Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()) + "__"
        } else {
            String::new()
        };
        let path = if let Some(pkg) = &file.package {
            pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()
        } else {
            vec![]
        };

        // Initialize imports for this file's context (crucial for local type resolution)
        self.imports_mut().clear();
        self.imports_mut().extend(file.imports.clone());
        
        // Inject self-imports for local Structs/Enums to ensure they resolve to the mangled package name
        if !pkg_prefix.is_empty() {
             // ... existing self-imports injection logic ...
             // Since we are rewriting the whole function, I need to include it.
            let mut self_imports = Vec::new();
            for item in &file.items {
                 let (ident_name, mangled_str) = match item {
                     Item::Struct(s) => (&s.name, format!("{}{}", pkg_prefix, s.name)),
                     Item::Enum(e) => (&e.name, format!("{}{}", pkg_prefix, e.name)),
                     Item::Fn(f) => (&f.name, if f.attributes.iter().any(|a| a.name == "no_mangle") { f.name.to_string() } else { format!("{}{}", pkg_prefix, f.name) }),
                     Item::ExternFn(e) => (&e.name, e.name.to_string()), // Extern functions always use C symbol name
                     Item::Global(g) => (&g.name, format!("{}{}", pkg_prefix, g.name)),
                     Item::Const(c) => (&c.name, format!("{}{}", pkg_prefix, c.name)),
                     _ => continue
                 };
                 
                 let mangled_ident = syn::Ident::new(&mangled_str, proc_macro2::Span::call_site());
                 let mut p = syn::punctuated::Punctuated::new();
                 p.push(mangled_ident);
                 
                 self_imports.push(crate::grammar::ImportDecl { 
                     name: p,
                     alias: Some(ident_name.clone()),
                     group: None
                 });
            }
            self.imports_mut().extend(self_imports);
        }

        for item in &file.items {
            if let Item::Global(g) = item {
                 let name = format!("{}{}", pkg_prefix, g.name);
                 let ty = self.bridge_resolve_type(&g.ty);
                 self.globals_mut().insert(name, ty);
                 
                 // FIX: Emit global definition immediately
                 let mut out = String::new();
                 if let Err(e) = self.bridge_emit_global_def(&mut out, g) {
                     return Err(format!("Error emitting global {}: {}", g.name, e));
                 } else {
                     self.decl_out_mut().push_str(&out);
                 }
            } else if let Item::Fn(f) = item {
                // ... (Fn logic mostly unchanged, except we don't register methods here)
                let is_extern = f.attributes.iter().any(|a| a.name == "extern");
                let is_no_mangle = f.attributes.iter().any(|a| a.name == "no_mangle");
                if is_extern {
                    self.external_decls_mut().insert(f.name.to_string());
                }

                let name = if is_no_mangle || is_extern {
                    f.name.to_string()
                } else {
                     format!("{}{}", pkg_prefix, f.name)
                };
                
                let ret_ty = if let Some(rt) = &f.ret_type {
                    self.bridge_resolve_type(rt)
                } else {
                     crate::types::Type::Unit
                };
                let args: Vec<crate::types::Type> = f.args.iter()
                                     .filter_map(|arg| arg.ty.as_ref().map(|t| self.bridge_resolve_type(t)))
                                     .collect();
                self.globals_mut().insert(name.clone(), crate::types::Type::Fn(args.clone(), Box::new(ret_ty.clone())));
                
                // Capture imports before mutating generic_impls (both borrow discovery)
                let current_imports = self.imports().clone();
                self.generic_impls_mut().insert(name.clone(), (f.clone(), current_imports));
                // LAZY REVOLUTION: Do NOT mark as defined here. 
                // We only register the template. Hydration will mark it defined when emitted.
            } else if let Item::Impl(i) = item {
                if let crate::grammar::SaltImpl::Methods { target_ty, methods, generics } = i {
                    let _saved_map = self.current_type_map().clone();
                    // Populate Type Map with Generic Params (e.g. SIZE)
                    if let Some(g) = generics {
                        for param in &g.params {
                            let name = match param {
                                crate::grammar::GenericParam::Type { name, .. } => name,
                                crate::grammar::GenericParam::Const { name, .. } => name,
                            };
                            self.current_type_map_mut().insert(name.to_string(), crate::types::Type::Struct(name.to_string()));
                        }
                    }

                    if let Some(target) = crate::types::Type::from_syn(&target_ty) {
                        let resolved = self.bridge_resolve_codegen_type(&target);
                        let target_mangled = resolved.mangle_suffix();
                        
                        *self.current_self_ty_mut() = Some(resolved.clone());

                        // FIX: Use package path for impl_key so methods resolve with full FQN
                        // This ensures Ptr::dangling in std.core.ptr resolves to std__core__ptr__Ptr
                        let mut impl_key = resolved.to_key().unwrap_or_else(|| {
                            TypeKey { path: path.clone(), name: resolved.mangle_suffix(), specialization: None }
                        });
                        // Also ensure path is populated if to_key() returned empty
                        if impl_key.path.is_empty() && !path.is_empty() {
                            impl_key.path = path.clone();
                        }
                        if generics.is_some() {
                             impl_key.specialization = None;
                        }


                        for m in methods {
                             let name = format!("{}__{}", target_mangled, m.name);

                             
                             let ret_ty = if let Some(rt) = &m.ret_type {
                                 crate::types::Type::from_syn(rt).unwrap_or(crate::types::Type::Unit)
                             } else {
                                  crate::types::Type::Unit
                             };
                             let args: Vec<crate::types::Type> = m.args.iter()
                                     .filter_map(|arg| arg.ty.as_ref().and_then(|t| crate::types::Type::from_syn(t)))
                                     .collect();
                             
                             self.globals_mut().insert(name.clone(), crate::types::Type::Fn(args.clone(), Box::new(ret_ty.clone())));
                             
                             let mut m_clone = m.clone();
                             if let Some(ig) = generics {
                                 if let Some(mg) = &mut m_clone.generics {
                                     let mut new_params = ig.params.clone();
                                     new_params.extend(mg.params.iter().cloned());
                                     mg.params = new_params;
                                 } else {
                                     m_clone.generics = Some(ig.clone());
                                 }
                             }

                             // Capture imports before mutating (both borrow discovery)
                             let current_imports = self.imports().clone();
                             self.generic_impls_mut().insert(name.clone(), (m_clone.clone(), current_imports.clone()));
                             // [V4.0 SOVEREIGN] Register via TraitRegistry with signature extraction
                             self.trait_registry_mut().register_simple(impl_key.clone(), m_clone, Some(resolved.clone()), current_imports);


                             // LAZY REVOLUTION: Do NOT mark as defined here.
                        }
                        
                        *self.current_self_ty_mut() = None;
                    }
                }
                // [SOVEREIGN V7.0] Handle `impl Trait for Type` blocks
                // The Sovereign Authority Rule: register these methods exactly like
                // SaltImpl::Methods, so they're available for demand-driven hydration.
                else if let crate::grammar::SaltImpl::Trait { trait_name, target_ty, methods, generics } = i {
                    let _saved_map = self.current_type_map().clone();
                    // Populate Type Map with Generic Params
                    if let Some(g) = generics {
                        for param in &g.params {
                            let name = match param {
                                crate::grammar::GenericParam::Type { name, .. } => name,
                                crate::grammar::GenericParam::Const { name, .. } => name,
                            };
                            self.current_type_map_mut().insert(name.to_string(), crate::types::Type::Struct(name.to_string()));
                        }
                    }

                    if let Some(target) = crate::types::Type::from_syn(&target_ty) {
                        let resolved = self.bridge_resolve_codegen_type(&target);
                        let target_mangled = resolved.mangle_suffix();


                        *self.current_self_ty_mut() = Some(resolved.clone());

                        let mut impl_key = resolved.to_key().unwrap_or_else(|| {
                            TypeKey { path: path.clone(), name: resolved.mangle_suffix(), specialization: None }
                        });
                        if impl_key.path.is_empty() && !path.is_empty() {
                            impl_key.path = path.clone();
                        }
                        if generics.is_some() {
                             impl_key.specialization = None;
                        }

                        // [SOVEREIGN V7.0] Register trait impl for coherence tracking
                        if let Err(e) = self.register_trait_impl(
                            target_mangled.clone(),
                            trait_name.to_string(),
                            pkg_prefix.trim_end_matches("__").to_string(),
                        ) {
                            eprintln!("Warning: {}", e);
                        }

                        for m in methods {
                             // Register under TARGET TYPE (String), not trait (Eq)
                             let name = format!("{}__{}", target_mangled, m.name);
                             
                             let ret_ty = if let Some(rt) = &m.ret_type {
                                 crate::types::Type::from_syn(rt).unwrap_or(crate::types::Type::Unit)
                             } else {
                                  crate::types::Type::Unit
                             };
                             let args: Vec<crate::types::Type> = m.args.iter()
                                     .filter_map(|arg| arg.ty.as_ref().and_then(|t| crate::types::Type::from_syn(t)))
                                     .collect();
                             
                             self.globals_mut().insert(name.clone(), crate::types::Type::Fn(args.clone(), Box::new(ret_ty.clone())));
                             
                             let mut m_clone = m.clone();
                             if let Some(ig) = generics {
                                 if let Some(mg) = &mut m_clone.generics {
                                     let mut new_params = ig.params.clone();
                                     new_params.extend(mg.params.iter().cloned());
                                     mg.params = new_params;
                                 } else {
                                     m_clone.generics = Some(ig.clone());
                                 }
                             }

                             // Capture imports before mutating (both borrow discovery)
                             let current_imports = self.imports().clone();
                             self.generic_impls_mut().insert(name.clone(), (m_clone.clone(), current_imports.clone()));
                             // [V4.0 SOVEREIGN] Register via TraitRegistry with signature extraction
                             self.trait_registry_mut().register_simple(impl_key.clone(), m_clone, Some(resolved.clone()), current_imports);

                             // LAZY REVOLUTION: Do NOT mark as defined here.
                        }
                        
                        *self.current_self_ty_mut() = None;
                    }
                }
            } else if let Item::ExternFn(e) = item {
                // Extern functions always use their C symbol name (never mangle)
                let mangled_name = e.name.to_string();

                // [SOVEREIGN FIX] Extern declaration emission is now handled by register_signatures.
                // This function only needs to ensure globals and defined_functions are populated.
                // Skip if already registered (handles dedupe across modules)
                if self.external_decls().contains(&mangled_name) {
                    continue;
                }
                self.external_decls_mut().insert(mangled_name.clone());
                
                let ret_ty = if let Some(rt) = &e.ret_type {
                    crate::types::Type::from_syn(rt).unwrap_or(crate::types::Type::Unit)
                } else { crate::types::Type::Unit };
                
                let args: Vec<crate::types::Type> = e.args.iter()
                    .filter_map(|arg| arg.ty.as_ref().and_then(|t| crate::types::Type::from_syn(t)))
                    .collect();
                    
                self.globals_mut().insert(mangled_name.clone(), crate::types::Type::Fn(args.clone(), Box::new(ret_ty.clone())));
                // NOTE: Extern functions are NOT added to defined_functions — they are
                // FFI declarations, not MLIR definitions. The BTreeMap assembly filter
                // must emit their forward declarations for call sites to resolve.
                // They are tracked separately in external_decls.
            } else if let Item::Const(c) = item {
                 let name = format!("{}{}", pkg_prefix, c.name);
                 let ty = self.bridge_resolve_type(&c.ty);
                 self.globals_mut().insert(name, ty);
                 
                 // FIX: Emit const definition (generates global/constant table entry)
                 let mut out = String::new();
                 if let Err(e) = self.bridge_emit_const(&mut out, c) {
                      return Err(format!("Error emitting const {}: {}", c.name, e));
                 } else {
                      self.decl_out_mut().push_str(&out);
                 }


            } else if let Item::Struct(s) = item {
                let name = format!("{}{}", pkg_prefix, s.name);
                if let Some(_generics) = &s.generics {
                    let mut s_mangled = s.clone();
                    s_mangled.name = syn::Ident::new(&name, s.name.span());
                    self.struct_templates_mut().insert(name.clone(), s_mangled);
                } else {
                    // Concrete Struct
                    // OPAQUE STUB: Register identity BEFORE resolving fields to break recursion
                    // This allows NodePtr<TrieNode> to specialize even if TrieNode isn't complete
                    let key = TypeKey {
                        path: path.clone(),
                        name: s.name.to_string(),
                        specialization: None,
                    };
                    self.struct_registry_mut().insert(key.clone(), StructInfo {
                        name: name.clone(),
                        fields: std::collections::HashMap::new(),  // Opaque: empty fields
                        field_order: vec![],
                        template_name: None,
                        specialization_args: vec![],
                    });
                    
                    // Now resolve fields - recursive types can reference the stub above
                    let mut fields: std::collections::HashMap<String, (usize, crate::types::Type)> = std::collections::HashMap::new();
                    let mut field_order = Vec::new();
                    for (i, f) in s.fields.iter().enumerate() {
                        let mut ty = self.bridge_resolve_type(&f.ty);
                        
                        // Handle @packed attribute
                        if f.attributes.iter().any(|a| a.name == "packed") {
                             if let crate::types::Type::Array(inner, len, _) = ty {
                                  ty = crate::types::Type::Array(inner, len, true);
                             } else {
                                  eprintln!("Warning: @packed attribute ignored on non-array field '{}' in struct '{}'", f.name, s.name);
                             }
                        }

                        fields.insert(f.name.to_string(), (i, ty.clone()));
                        field_order.push(ty);
                    }
                    
                    // HYDRATE: Replace stub with complete info
                    self.struct_registry_mut().insert(key, StructInfo {
                        name: name.clone(),
                        fields: fields.clone(),
                        field_order: field_order.clone(),
                        template_name: None,
                        specialization_args: vec![],
                    });

                    // =========================================================
                    // [FORMAL SHADOW] Z3 Alignment Verification for @atomic Fields
                    //
                    // If any field is tagged @atomic, verify that its byte offset
                    // within the struct is 16-byte aligned. This prevents hardware
                    // Alignment Check (#AC) faults and cache-line straddling on
                    // cmpxchg16b operations.
                    //
                    // The compiler REFUSES to emit the binary unless Z3 can
                    // mathematically prove alignment is invariant.
                    // =========================================================
                    {
                        use z3::ast::Ast;  // for _eq() and not()

                        let struct_reg = self.struct_registry();
                        let mut byte_offset: usize = 0;
                        for (i, f) in s.fields.iter().enumerate() {
                            let has_atomic = f.attributes.iter().any(|a| a.name == "atomic");
                            if has_atomic {
                                // Use Z3 to formally verify alignment via integer modular arithmetic
                                let z3_cfg = z3::Config::new();
                                let z3_ctx = z3::Context::new(&z3_cfg);
                                let solver = z3::Solver::new(&z3_ctx);

                                // Model: base address is an arbitrary non-negative integer
                                // that is 16-byte aligned (struct allocator contract)
                                let base = z3::ast::Int::new_const(&z3_ctx, "base_addr");
                                let sixteen = z3::ast::Int::from_i64(&z3_ctx, 16);
                                let zero = z3::ast::Int::from_i64(&z3_ctx, 0);

                                // Constraint: base >= 0 and base % 16 == 0
                                solver.assert(&base.ge(&zero));
                                solver.assert(&base.modulo(&sixteen)._eq(&zero));

                                // Compute field address: base + byte_offset
                                let offset_val = z3::ast::Int::from_i64(&z3_ctx, byte_offset as i64);
                                let field_addr = z3::ast::Int::add(&z3_ctx, &[&base, &offset_val]);

                                // Assert NEGATION: field_addr % 16 != 0
                                // UNSAT => always aligned (proof), SAT => misaligned (error)
                                solver.assert(&field_addr.modulo(&sixteen)._eq(&zero).not());

                                match solver.check() {
                                    z3::SatResult::Unsat => {
                                        // PROOF COMPLETE: Z3 proved alignment invariant.
                                        eprintln!(
                                            "[Formal Shadow] Z3 PROVED: @atomic field '{}' in struct '{}' \
                                             is 16-byte aligned at offset {} (z3_aligned)",
                                            f.name, s.name, byte_offset
                                        );
                                    }
                                    _ => {
                                        // COUNTEREXAMPLE: The compiler REFUSES to emit.
                                        drop(struct_reg);
                                        return Err(format!(
                                            "[Formal Shadow] ALIGNMENT VIOLATION: @atomic field '{}' \
                                             in struct '{}' is at byte offset {}, which is NOT \
                                             16-byte aligned. The Z3 SMT solver proved this layout \
                                             violates the hardware alignment contract for cmpxchg16b. \
                                             Fix: reorder fields or add padding so @atomic fields \
                                             start at offsets that are multiples of 16.",
                                            f.name, s.name, byte_offset
                                        ));
                                    }
                                }
                            }
                            // Advance byte offset by field size
                            let field_ty = &field_order[i];
                            byte_offset += field_ty.size_of(&*struct_reg);
                        }
                    }
                }
            } else if let Item::Enum(e) = item {
                 let name = format!("{}{}", pkg_prefix, e.name);
                 if let Some(_generics) = &e.generics {
                     self.enum_templates_mut().insert(name.clone(), e.clone());
                 } else {
                     // Concrete Enum
                     let mut variants = Vec::new();
                     let mut max_size = 0;
                     for (i, v) in e.variants.iter().enumerate() {
                         let p_ty = v.ty.as_ref().map(|t| self.bridge_resolve_type(t));
                         if let Some(ref ty) = p_ty {
                               let size = ty.size_of(&*self.struct_registry());
                               if size > max_size { max_size = size; }
                         }
                         variants.push((v.name.to_string(), p_ty, i as i32));
                     }
                     
                     let key = TypeKey {
                        path: path.clone(),
                        name: e.name.to_string(),
                        specialization: None,
                     };
                     
                     use crate::codegen::context::EnumInfo;
                     self.enum_registry_mut().insert(key, EnumInfo {
                         name,
                         variants,
                         max_payload_size: max_size,
                         template_name: None,
                         specialization_args: vec![],
                     });
                 }
            }
        }
        self.current_package.replace(saved_pkg);
        Ok(())
    }


    pub fn ensure_func_declared(&self, name: &str, arg_tys: &[Type], ret_ty: &Type) -> Result<(), String> {
        // Skip only if already queued in pending_func_decls, has a full body, or is a pending specialization.
        // Do NOT skip for external_decls — FFI functions need forward declarations.
        if self.emission.borrow().pending_func_decls.contains_key(name) || self.defined_functions().contains(name) || self.specializations().values().any(|v| v == name) {
            return Ok(());
        }

        
        // Don't redeclare if it's an intrinsic handled elsewhere
        if name.starts_with("llvm.") || name.starts_with("arith.") {
            return Ok(());
        }

        let mut arg_code = Vec::new();
        for t in arg_tys {
             let mut ty_str = self.resolve_mlir_type(&t)?;
             if matches!(t, Type::Reference(..) | Type::Owned(..) | Type::Fn(..)) {
                  ty_str.push_str(" {llvm.noalias}");
             }
             arg_code.push(ty_str);
        }
        let arg_str = arg_code.join(", ");
            
        // External declaration: func.func private
        let ret_str = if let Type::Unit = ret_ty { "()".to_string() } else { self.resolve_mlir_type(&ret_ty)? };
        // [PILLAR 2] Mark contract violation as cold+noreturn so LLVM
        // moves it off the hot path and optimizes branch prediction
        let attrs = if name == "__salt_contract_violation" {
            " attributes {passthrough = [\"cold\", \"noreturn\"]}"
        } else {
            ""
        };
        let decl = format!("  func.func private @{}({}) -> {}{}\n", name, arg_str, ret_str, attrs);
        self.pending_func_decls_mut().insert(name.to_string(), decl);
        self.external_decls_mut().insert(name.to_string());
        // Track type for addressof resolution
        self.globals_mut().insert(name.to_string(), Type::Fn(arg_tys.to_vec(), Box::new(ret_ty.clone())));
        Ok(())
    }

    pub fn ensure_global_declared(&self, name: &str, ty: &Type) -> Result<(), String> {
        if self.initialized_globals().contains(name) || self.external_decls().contains(name) {
            return Ok(());
        }

        if let Type::Fn(args, ret) = ty {
            return self.ensure_func_declared(name, args, ret);
        }

        let mlir_ty = self.resolve_mlir_type(&ty)?;
        self.decl_out_mut().push_str(&format!("  llvm.mlir.global external @{}() : {}\n", name, mlir_ty));
        self.external_decls_mut().insert(name.to_string());
        Ok(())
    }

    pub fn next_val(&self) -> usize {
        let mut n = self.val_counter_mut();
        *n += 1;
        *n
    }
    
    // Alias for backward compatibility if needed, or prefer next_val
    pub fn next_id(&self) -> usize {
        self.next_val()
    }

    pub fn next_metadata_id(&self) -> usize {
        let mut n = self.metadata_id_counter_mut();
        *n += 1;
        *n
    }

    pub fn get_yield_check_name(&self) -> String {
        "salt_yield_check".to_string()
    }

    /// Check if we are currently inside an affine.for context
    /// Used to decide whether to emit affine.load/store vs memref.load/store
    pub fn is_in_affine_context(&self) -> bool {
        *self.affine_depth() > 0
    }
    
    /// Enter an affine.for context
    pub fn enter_affine_context(&self) {
        *self.affine_depth_mut() += 1;
    }
    
    /// Exit an affine.for context
    pub fn exit_affine_context(&self) {
        *self.affine_depth_mut() -= 1;
    }

    // =========================================================================
    // MLIR Builder Pattern Helpers
    // =========================================================================

    pub fn emit_binop(&self, out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
        out.push_str(&format!("    {} = {} {}, {} : {}\n", res, op, lhs, rhs, ty));
    }
    
    /// [V7.5] Emit binary operation with fast-math attributes for vectorization.
    /// Only use for floating-point operations in reduction loops where reassociation is acceptable.
    /// Attributes: reassoc (allow reordering), contract (allow FMA contraction)
    pub fn emit_binop_fast(&self, out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
        out.push_str(&format!("    {} = {} {}, {} {{fastmath = #arith.fastmath<reassoc, contract>}} : {}\n", 
            res, op, lhs, rhs, ty));
    }

    pub fn emit_const_int(&self, out: &mut String, res: &str, val: i64, ty: &str) {
        out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val, ty));
    }

    pub fn emit_const_float(&self, out: &mut String, res: &str, val: f64, ty: &str) {
        let val_str = if val == 0.0 {
            "0.0".to_string()
        } else {
            format!("{:.17e}", val)
        };
        out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val_str, ty));
    }

    pub fn emit_load(&self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, ptr, ty));
    }

    pub fn emit_load_scoped(&self, out: &mut String, res: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
        if !self.emit_alias_scopes {
            // Fall back to plain load when alias scopes are disabled
            self.emit_load(out, res, ptr, ty);
            return;
        }
        // Use MLIR attribute syntax: { alias_scopes = [...], noalias = [...] }
        out.push_str(&format!("    {} = llvm.load {} {{ alias_scopes = [{}], noalias = [{}] }} : !llvm.ptr -> {}\n", res, ptr, scope, noalias, ty));
    }

    pub fn emit_load_logical(&self, out: &mut String, res: &str, ptr: &str, ty: &Type) -> Result<(), String> {
        self.emit_load_logical_with_scope(out, res, ptr, ty, None)
    }

    pub fn emit_load_logical_with_scope(&self, out: &mut String, res: &str, ptr: &str, ty: &Type, scopes: Option<(&str, &str)>) -> Result<(), String> {
        let storage_ty = self.resolve_mlir_storage_type(&ty)?;
        
        if *ty == Type::Bool {
            let load_res = format!("%b_load_{}", self.next_id());
            if let Some((s, n)) = scopes {
                self.emit_load_scoped(out, &load_res, ptr, &storage_ty, s, n);
            } else {
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", load_res, ptr, storage_ty));
            }
            self.emit_trunc(out, res, &load_res, "i8", "i1");
        } else if ty.k_is_ptr_type() {
             // [PROVENANCE FIX] Load pointers directly as !llvm.ptr
             // Previously used i64 storage + inttoptr which broke LLVM pointer provenance
             if let Some((s, n)) = scopes {
                 self.emit_load_scoped(out, res, ptr, "!llvm.ptr", s, n);
             } else {
                 out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !llvm.ptr\n", res, ptr));
             }
        } else {
            if let Some((s, n)) = scopes {
                self.emit_load_scoped(out, res, ptr, &storage_ty, s, n);
            } else {
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, ptr, storage_ty));
            }
        }
        Ok(())
    }

    pub fn emit_store(&self, out: &mut String, val: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", val, ptr, ty));
    }

    pub fn emit_store_scoped(&self, out: &mut String, val: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
        if !self.emit_alias_scopes {
            // Fall back to plain store when alias scopes are disabled
            self.emit_store(out, val, ptr, ty);
            return;
        }
        out.push_str(&format!("    llvm.store {}, {} {{ alias_scopes = [{}], noalias = [{}] }} : {}, !llvm.ptr\n", val, ptr, scope, noalias, ty));
    }

    pub fn emit_store_logical(&self, out: &mut String, val: &str, ptr: &str, ty: &Type) -> Result<(), String> {
        self.emit_store_logical_with_scope(out, val, ptr, ty, None)
    }

    pub fn emit_store_logical_with_scope(&self, out: &mut String, val: &str, ptr: &str, ty: &Type, scopes: Option<(&str, &str)>) -> Result<(), String> {
        let storage_ty = self.resolve_mlir_storage_type(&ty)?;
        if *ty == Type::Bool {
            let zext_res = format!("%b_zext_{}", self.next_id());
            // Boolean Law: i1 -> i8 via arith.extui
            out.push_str(&format!("    {} = arith.extui {} : i1 to i8\n", zext_res, val));
            if let Some((s, n)) = scopes {
                self.emit_store_scoped(out, &zext_res, ptr, &storage_ty, s, n);
            } else {
                out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", zext_res, ptr, storage_ty));
            }
        } else if ty.k_is_ptr_type() {
             // [PROVENANCE FIX] Store pointers directly as !llvm.ptr
             // Previously used ptrtoint + i64 storage which broke LLVM pointer provenance
             if let Some((s, n)) = scopes {
                 self.emit_store_scoped(out, val, ptr, "!llvm.ptr", s, n);
             } else {
                 out.push_str(&format!("    llvm.store {} , {} : !llvm.ptr, !llvm.ptr\n", val, ptr));
             }
        } else {
            if let Some((s, n)) = scopes {
                self.emit_store_scoped(out, val, ptr, &storage_ty, s, n);
            } else {
                out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", val, ptr, storage_ty));
            }
        }
        Ok(())
    }

    pub fn emit_alloca(&self, _out: &mut String, res: &str, ty: &str) {
        self.alloca_out_mut().push_str(&format!("    {} = llvm.alloca %c1_i64 x {} : (i64) -> !llvm.ptr\n", res, ty));
    }

    pub fn emit_gep_field(&self, out: &mut String, res: &str, base: &str, idx: usize, struct_ty: &str) {
        out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n", res, base, idx, struct_ty));
    }

    pub fn emit_gep(&self, out: &mut String, res: &str, base: &str, idx_var: &str, elem_ty: &str) {
        out.push_str(&format!("    {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", res, base, idx_var, elem_ty));
    }

    pub fn emit_extractvalue(&self, out: &mut String, res: &str, val: &str, idx: usize, ty: &str) {
        out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", res, val, idx, ty));
    }

    pub fn emit_extractvalue_logical(&self, out: &mut String, res: &str, val: &str, idx: usize, ty: &str, field_ty: &Type) -> Result<(), String> {
        if *field_ty == Type::Bool {
            let extract_res = format!("%b_extract_{}", self.next_id());
            // Extract as i8 (storage type)
            out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", extract_res, val, idx, ty));
            // Truncate to i1 (logical type)
            self.emit_trunc(out, res, &extract_res, "i8", "i1");
        } else {
            self.emit_extractvalue(out, res, val, idx, ty);
        }
        Ok(())
    }

    pub fn emit_insertvalue(&self, out: &mut String, res: &str, elem: &str, val: &str, idx: usize, ty: &str) {
        out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, elem, val, idx, ty));
    }

    pub fn emit_insertvalue_logical(&self, out: &mut String, res: &str, elem: &str, val: &str, idx: usize, ty: &str, field_ty: &Type) -> Result<(), String> {
        if *field_ty == Type::Bool {
             let zext_res = format!("%b_zext_ins_{}", self.next_id());
             // Promote i1 to i8
             self.emit_cast(out, &zext_res, "arith.extui", elem, "i1", "i8");
             // Insert the i8 into the struct
             out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, zext_res, val, idx, ty));
        } else {
             self.emit_insertvalue(out, res, elem, val, idx, ty);
        }
        Ok(())
    }

    pub fn emit_cmp(&self, out: &mut String, res: &str, cmp_op: &str, pred: &str, lhs: &str, rhs: &str, ty: &str) {
        let comma = if cmp_op == "llvm.icmp" || cmp_op == "llvm.fcmp" { "" } else { "," };
        out.push_str(&format!("    {} = {} \"{}\"{} {}, {} : {}\n", res, cmp_op, pred, comma, lhs, rhs, ty));
    }

    pub fn emit_cast(&self, out: &mut String, res: &str, op: &str, val: &str, from_ty: &str, to_ty: &str) {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, val, from_ty, to_ty));
    }

    pub fn emit_trunc(&self, out: &mut String, res: &str, val: &str, from_ty: &str, to_ty: &str) {
        out.push_str(&format!("    {} = arith.trunci {} : {} to {}\n", res, val, from_ty, to_ty));
    }

    pub fn emit_br(&self, out: &mut String, label: &str) {
        out.push_str(&format!("    llvm.br ^{}\n", label));
    }

    pub fn emit_cond_br(&self, out: &mut String, cond: &str, true_label: &str, false_label: &str) {
        out.push_str(&format!("    llvm.cond_br {}, ^{}, ^{}\n", cond, true_label, false_label));
    }

    pub fn emit_label(&self, out: &mut String, label: &str) {
        out.push_str(&format!("  ^{}:\n", label));
    }

    pub fn emit_return(&self, out: &mut String, val: &str, ty: &str) {
        out.push_str(&format!("    llvm.return {} : {}\n", val, ty));
    }

    pub fn emit_return_void(&self, out: &mut String) {
        out.push_str("    llvm.return\n");
    }

    pub fn emit_load_exclusive(&self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    {} = \"llvm.load\"({}) {{salt.access = \"exclusive\"}} : (!llvm.ptr) -> {}\n", res, ptr, ty));
    }

    pub fn emit_load_atomic(&self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        // Atomic load via atomicrmw or ptr, 0 — identity operation that returns current value.
        // This avoids MLIR version incompatibility with atomic_memory_order attributes on llvm.load.
        let zero = format!("%atomic_zero_{}", self.next_id());
        out.push_str(&format!("    {} = arith.constant 0 : {}\n", zero, ty));
        out.push_str(&format!("    {} = llvm.atomicrmw _or {}, {} seq_cst : !llvm.ptr, {}\n", res, ptr, zero, ty));
    }

    pub fn emit_store_atomic(&self, out: &mut String, val: &str, ptr: &str, ty: &str) {
        // Atomic store via atomicrmw xchg ptr, value — discards old value.
        // This avoids MLIR version incompatibility with atomic_memory_order attributes on llvm.store.
        let discard = format!("%atomic_discard_{}", self.next_id());
        out.push_str(&format!("    {} = llvm.atomicrmw xchg {}, {} seq_cst : !llvm.ptr, {}\n", discard, ptr, val, ty));
    }

    pub fn emit_atomicrmw(&self, out: &mut String, res: &str, op: &str, ptr: &str, val: &str, ty: &str) {
        out.push_str(&format!("    {} = llvm.atomicrmw {} {}, {} seq_cst : !llvm.ptr, {}\n", res, op, ptr, val, ty));
    }

    pub fn emit_call(&self, out: &mut String, res: Option<&str>, func_name: &str, args: &str, arg_tys: &str, ret_ty: &str) {
        let mangled_func_name = self.mangle_fn_name(func_name);
        if let Some(r) = res {
            out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", r, mangled_func_name, args, arg_tys, ret_ty));
        } else {
            out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled_func_name, args, arg_tys));
        }
    }

    pub fn emit_addressof(&self, out: &mut String, res: &str, name: &str) -> Result<(), String> {
        // Check if the symbol is a function (Local, External, or from Registry)
        let is_func = self.defined_functions().contains(name) 
            || self.external_decls().contains(name)
            || matches!(self.resolve_global(name), Some(Type::Fn(_, _)));

        if is_func {
            // Retrieve type to construct signature
            let ty = self.resolve_global(name).unwrap_or(Type::Unit);
            if let Type::Fn(args, ret) = ty {
                 let mut arg_code = Vec::new();
                 for t in args {
                     arg_code.push(self.resolve_mlir_type(&t)?);
                 }
                 let arg_str = arg_code.join(", ");
                 let ret_str = if let Type::Unit = *ret { "()".to_string() } else { self.resolve_mlir_type(&ret)? };
                 let signature = format!("({}) -> {}", arg_str, ret_str);
                 
                 let tmp = format!("{}__fn", res);
                 out.push_str(&format!("    {} = func.constant @{} : {}\n", tmp, name, signature));
                 out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : {} to !llvm.ptr\n", res, tmp, signature));
            } else {
                 // Function is declared but type not in globals (e.g., assembly-only extern fn).
                 // Use func.constant with () -> () signature (safe for addressof + ptrtoint).
                 let tmp = format!("{}__fn", res);
                 out.push_str(&format!("    {} = func.constant @{} : () -> ()\n", tmp, name));
                 out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : () -> () to !llvm.ptr\n", res, tmp));
            }
        } else {
             // For global variables, use llvm.mlir.addressof
             out.push_str(&format!("    {} = llvm.mlir.addressof @{} : !llvm.ptr\n", res, name));
        }
        Ok(())
    }

    pub fn emit_inttoptr(&self, out: &mut String, res: &str, val: &str, from_ty: &str) {
        out.push_str(&format!("    {} = llvm.inttoptr {} : {} to !llvm.ptr\n", res, val, from_ty));
    }

    pub fn emit_verify(&self, out: &mut String, cond: &str, _msg: &str) {
        // Lower to standard MLIR: scf.if with inverted condition + panic
        let true_const = format!("%verify_true_{}", self.next_id());
        let violated = format!("%verify_violated_{}", self.next_id());
        out.push_str(&format!("    {} = arith.constant true\n", true_const));
        out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", violated, cond, true_const));
        out.push_str(&format!("    scf.if {} {{\n", violated));
        out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
        out.push_str("      scf.yield\n");
        out.push_str("    }\n");
    }
    pub fn ensure_struct_exists(&self, base_name: &str, params: &[Type]) -> Result<String, String> {
        if base_name == "GlobalSlabAlloc" {
             eprintln!("CRITICAL ERROR: 'GlobalSlabAlloc' Short Name detected in ensure_struct_exists!");
             panic!("Short Name Leak detected!");
        }
        let full_params = params.to_vec();
        let key = (base_name.to_string(), full_params.clone());
        if let Some(mangled) = self.specializations().get(&key) {
            let m_res: String = mangled.clone();
            return Ok(m_res);
        }

        // Delegate to type_bridge specialized logic which handles template instantiation
        Ok(self.specialize_template(base_name, params, false)?.mangle())
    }

    pub fn ensure_enum_exists(&self, base_name: &str, params: &[Type]) -> Result<String, String> {
        let full_params = params.to_vec();
        let key = (base_name.to_string(), full_params.clone());
        if let Some(mangled) = self.specializations().get(&key) {
            let m_res: String = mangled.clone();
            return Ok(m_res);
        }

        Ok(self.specialize_template(base_name, params, true)?.mangle())
    }

    pub fn get_tensor_layout(&self, ty: &Type) -> Result<TensorLayout, String> {
        if let Some(layout) = self.tensor_layout_cache().get(ty) {
            return Ok(layout.clone());
        }
        if let Type::Tensor(_, shape) = ty {
            let mut strides = vec![1; shape.len()];
            for i in (0..shape.len() - 1).rev() {
                strides[i] = strides[i+1] * shape[i+1];
            }
            let layout = TensorLayout {
                shape: shape.clone(),
                strides,
                is_row_major: true,
            };
            self.tensor_layout_cache_mut().insert(ty.clone(), layout.clone());
            Ok(layout)
        } else {
            Err(format!("Type {:?} is not a tensor", ty))
        }
    }

    pub fn emit_linalg_generic(
        &self,
        out: &mut String,
        inputs: Vec<String>,
        outputs: Vec<String>,
        indexing_maps: Vec<String>,
        iterator_types: Vec<String>,
    ) -> Result<String, String> {
        let res = format!("%linalg_res_{}", self.next_id());
        let ins = inputs.join(", ");
        let outs = outputs.join(", ");
        let maps = indexing_maps.iter().map(|s| format!("affine_map<{}>", s)).collect::<Vec<_>>().join(", ");
        let iter_tys = iterator_types.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
        
        out.push_str(&format!("    {} = linalg.generic {{indexing_maps = [{}], iterator_types = [{}]}} ins({}) outs({}) \n", 
            res, maps, iter_tys, ins, outs));
        Ok(res)
    }

    pub fn emit_linalg_matmul(
        &self,
        out: &mut String,
        lhs: &str,
        _lhs_ty: &str,
        rhs: &str,
        _rhs_ty: &str,
        acc: &str,
        acc_ty: &str,
    ) -> Result<String, String> {
        let res = format!("%matmul_res_{}", self.next_id());
        out.push_str(&format!("    {} = linalg.matmul ins({}, {} : {}, {}) outs({} : {}) -> {}\n", 
            res, lhs, rhs, _lhs_ty, _rhs_ty, acc, acc_ty, acc_ty));
        Ok(res)
    }

    pub fn emit_noalias_metadata(&self, _out: &mut String, region_name: &str) -> (String, String) {
        let id = self.next_metadata_id();
        let scope_domain = format!("@alias_domain_{}", id);
        let scope_id = format!("@alias_scope_{}_{}", region_name, id);
        
        // This is a simplification. Real MLIR would need these in the metadata section.
        // For now, we'll store them in CodegenContext to be emitted later if needed,
        // or emit them as LLVM IR metadata if we were targeting LLVM directly.
        // In MLIR, these are often dialect attributes.
        (scope_id, scope_domain)
    }

    // =========================================================================
    // Verification & Symbolic Analysis
    // =========================================================================

    /* [SOVEREIGN V3] Disabled Z3 methods
    pub fn mk_int(&self, val: i64) -> z3::ast::Int<'a> {
        z3::ast::Int::from_i64(self.z3_ctx, val)
    }

    pub fn mk_var(&self, name: &str) -> z3::ast::Int<'a> {
        z3::ast::Int::new_const(self.z3_ctx, name)
    }

    pub fn push_solver(&self) {
        self.z3_solver.borrow_mut().push();
    }

    pub fn pop_solver(&self) {
        self.z3_solver.borrow_mut().pop(1);
    }

    pub fn add_assertion(&self, expr: &z3::ast::Bool<'a>) {
        self.z3_solver.borrow_mut().assert(expr);
    }

    pub fn is_provably_safe(&self, violation: &z3::ast::Bool<'a>) -> bool {
        *self.total_checks.borrow_mut() += 1;
        self.z3_solver.borrow_mut().push();
        self.add_assertion(violation);
        
        let res = self.z3_solver.borrow_mut().check();
        self.pop_solver();

        let safe = matches!(res, z3::SatResult::Unsat);
        if safe {
            *self.elided_checks.borrow_mut() += 1;
        }
        safe
    }

    pub fn register_symbolic_int(&self, ssa_name: String, val: z3::ast::Int<'a>) {
        self.symbolic_tracker.borrow_mut().insert(ssa_name, val);
    }

    pub fn get_symbolic_int(&self, ssa_name: &str) -> Option<z3::ast::Int<'a>> {
        self.symbolic_tracker.borrow().get(ssa_name).cloned()
    }
    */

     pub fn lookup_global_type(&self, key: &TypeKey) -> Option<Type> {
         let mut module_path = key.path.join(".");
         
         if module_path.is_empty() {
             // Fallback 1: Try current function's package (Mangled Name)
             let fn_name = self.current_fn_name();
             if fn_name.contains("__") {
                 let parts: Vec<&str> = fn_name.split("__").collect();
                 // Valid package name is everything except the last part (function name)
                 // e.g. std__core__slab_alloc__alloc -> std.core.slab_alloc
                 if parts.len() > 1 {
                     let pkg_parts = &parts[..parts.len()-1];
                     // Check if this looks like a package (starts with std or user pkg)
                     // Reconstruct strict path
                     module_path = pkg_parts.join(".");
                 }
             }

             // Fallback 2: Current file package (if not mangled or failed)
             if module_path.is_empty() {
                if let Some(pkg) = &self.file.borrow().package {
                    module_path = pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(".");
                }
             }
         }



         if let Some(reg) = self.registry {
              if let Some(module) = reg.modules.get(&module_path) {
                   if let Some(ty) = module.globals.get(&key.name) {
                        // Ensure type is qualified with the module path where it was found
                        // This bridges the gap between Local-Name-In-Module and Fully-Qualified-Key-In-Registry
                        let prefix = module_path.replace(".", "__");
                        let qualified_ty = match ty {
                            Type::Struct(n) if !n.contains("__") => Type::Struct(format!("{}__{}", prefix, n)),
                            Type::Enum(n) if !n.contains("__") => Type::Enum(format!("{}__{}", prefix, n)),
                            Type::Concrete(n, args) if !n.contains("__") => Type::Concrete(format!("{}__{}", prefix, n), args.clone()),
                            _ => ty.clone()
                        };
                        return Some(qualified_ty);
                   } else {

                   }
              } else {

              }
         }
         None
    }
}
