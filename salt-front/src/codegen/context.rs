use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use crate::grammar::{SaltFile, SaltFn, Item, ImportDecl};
use crate::registry::{Registry, EnumInfo};
use crate::types::{Type, TypeKey};
use crate::evaluator::Evaluator;
use crate::common::mangling::Mangler;
use crate::codegen::collector::MonomorphizationTask;
use crate::codegen::emit_fn;

pub struct StringInterner {
    pool: HashSet<Rc<str>>,
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
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

pub mod fstring;
pub mod resolver;
pub mod guards;
pub mod registry_init;
pub mod scanner;
pub mod accessors;
pub mod emit_helpers;
pub mod emit_helpers_ctx;
pub mod struct_lookup;
pub mod bridge;
pub mod raii;
pub use fstring::FStringSegment;
pub use guards::GenericContextGuard;
pub use guards::ImportContextGuard;
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
/// # Phased Organization
/// State is grouped by compiler phase:
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
    pub z3_ctx: &'a crate::z3_shim::Context,
    pub z3_solver: RefCell<crate::z3_shim::Solver<'a>>,
    pub symbolic_tracker: RefCell<HashMap<String, crate::z3_shim::ast::Int<'a>>>,
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
    
    // === Proof-Hint Engine ===
    /// Proof hints generated by verify_struct_alignments() for @align(N) fields.
    /// Each entry is ("StructName_FieldName", hash_combine(struct_id, offset, align)).
    pub proof_hints: RefCell<Vec<(String, u64)>>,

    // === Interprocedural Free Analysis ===
    /// Set of function names that call `free` (directly or transitively)
    pub freeing_functions: HashSet<String>,
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
    pub freeing_functions: &'a std::collections::HashSet<String>,
    /// When true, integer arithmetic emits overflow checks calling __salt_overflow_panic. Default: true in debug builds.
    pub debug_overflow_checks: bool,
}

/// LoweringContext: A "view struct" holding direct &mut references to phase structs, eliminating RefCell runtime panics via compile-time borrow checking.
pub struct LoweringContext<'a, 'ctx> {
    pub discovery: &'a mut crate::codegen::phases::DiscoveryState,
    pub expansion: &'a mut crate::codegen::phases::ExpansionState,
    pub emission: &'a mut crate::codegen::phases::EmissionState,
    pub control_flow: &'a mut crate::codegen::phases::ControlFlowState,
    pub z3_ctx: &'ctx crate::z3_shim::Context,
    pub z3_solver: &'a mut crate::z3_shim::Solver<'ctx>,
    pub symbolic_tracker: &'a mut std::collections::HashMap<String, crate::z3_shim::ast::Int<'ctx>>,
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
        let sym = crate::z3_shim::ast::Int::new_const(self.z3_ctx, ssa_name);
        self.symbolic_tracker.insert(ssa_name.to_string(), sym);
    }

    pub fn z3_try_prove_positive(&self, ssa_name: &str) -> bool {
        if let Some(sym) = self.symbolic_tracker.get(ssa_name) {
            let zero = crate::z3_shim::ast::Int::from_i64(self.z3_ctx, 0);
            let cond = sym.ge(&zero);
            self.z3_solver.push();
            self.z3_solver.assert(&cond.not());
            let result = self.z3_solver.check();
            self.z3_solver.pop(1);
            result == crate::z3_shim::SatResult::Unsat
        } else {
            false
        }
    }

    pub fn mark_released(&mut self, var_name: &str) {
        let z3_solver_ptr = self.z3_solver as *const crate::z3_shim::Solver;
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

    // --- Verification Phase ---
    pub fn get_symbolic_int(&self, ssa_name: &str) -> Option<crate::z3_shim::ast::Int<'ctx>> { self.symbolic_tracker.get(ssa_name).cloned() }

    // =========================================================================
    // Complex Methods (zero RefCell)
    // =========================================================================

    pub fn ensure_struct_exists(&mut self, base_name: &str, params: &[Type]) -> Result<String, String> {
        let key = (base_name.to_string(), params.to_vec());
        if let Some(mangled) = self.expansion.specializations.get(&key) {
            return Ok(mangled.clone());
        }
        // Check struct_registry for non-generic structs,
        // then fall back to specialize_template for generic specializations.
        // This mirrors CodegenContext::ensure_struct_exists which delegates to
        // specialize_template for on-demand struct registration.
        if params.is_empty() {
            for tk in self.discovery.struct_registry.keys() {
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
        // Check enum_registry for non-generic enums,
        // then fall back to specialize_template for generic specializations.
        if params.is_empty() {
            for tk in self.discovery.enum_registry.keys() {
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
                let _fields: Vec<(String, Type)> = info.fields.iter()
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

    #[allow(clippy::too_many_arguments)] // REASON: all 7 params independently meaningful for MLIR matmul emission
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
                    if (info.name == *name || key.name == *name)
                        && info.variants.iter().any(|v| v.0 == "Some") && info.variants.iter().any(|v| v.0 == "None") {
                            return Some(info.clone());
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
        // Function symbols must NOT be emitted as llvm.mlir.global.
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
        self.emission.decl_out.push_str(&format!("  llvm.mlir.global external @{}() : {}\n", name, mlir_ty));
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
    pub fn mk_int(&self, val: i64) -> crate::z3_shim::ast::Int<'ctx> {
        crate::z3_shim::ast::Int::from_i64(self.z3_ctx, val)
    }

    pub fn mk_var(&self, name: &str) -> crate::z3_shim::ast::Int<'ctx> {
        crate::z3_shim::ast::Int::new_const(self.z3_ctx, name)
    }

    pub fn is_provably_safe(&self, violation: &crate::z3_shim::ast::Bool<'ctx>) -> bool {
        self.z3_solver.push();
        self.z3_solver.assert(violation);
        let result = self.z3_solver.check();
        self.z3_solver.pop(1);
        result == crate::z3_shim::SatResult::Unsat
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
        if spec.is_some() {
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
        code.push('}');
        code
    }

    pub fn native_hex_expand(&self, content: &str) -> String {
        let clean_hex: String = content.chars().filter(|c| !c.is_whitespace()).collect();
        if !clean_hex.len().is_multiple_of(2) {
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
        code.push('}');
        code
    }

    // --- Resolve Method (complex delegation) ---
    pub fn resolve_method(&self, receiver_ty: &Type, method_name: &str) -> Result<(crate::grammar::SaltFn, Option<Type>, Vec<crate::grammar::ImportDecl>), String> {
        // Extract the receiver type's base name to match against method keys.
        // This prevents Slice::offset from shadowing Ptr::offset when called on a Ptr receiver.
        // Recursively peel all Reference wrappers to reach the base type.
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
                // Both the full prefix and the basename must be checked.
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
        let old_self = self.expansion.current_self_ty.replace(self_ty);
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

    // --- Specialization queue ---
    // Push to expansion.pending_generations — the same queue
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
            discovery: &mut discovery,
            expansion: &mut expansion,
            emission:  &mut emission,
            control_flow: &mut control_flow,
            z3_ctx: self.z3_ctx,
            z3_solver: &mut z3_solver,
            symbolic_tracker: &mut symbolic_tracker,
            ownership_tracker: &mut ownership_tracker,
            elided_checks: &mut elided_checks,
            total_checks: &mut total_checks,
            evaluator: &mut evaluator,
            malloc_tracker: &mut malloc_tracker,
            pointer_tracker: &mut pointer_tracker,
            arena_escape_tracker: &mut arena_escape_tracker,
            pending_malloc_result: &mut pending_malloc_result,
            pending_pointer_state: &mut pending_pointer_state,
            current_package: &mut current_package,
            suppress_specialization: &self.suppress_specialization,
            config: CodegenConfig {
                file: &file,
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
                freeing_functions: &self.freeing_functions,
                debug_overflow_checks: !self.release_mode,
            },
        };

        f(&mut lctx)
    }

    pub fn compute_full_imports(file: &SaltFile) -> Vec<crate::grammar::ImportDecl> {
        file.imports.clone()
    }

    pub fn new(file: &'a SaltFile, release_mode: bool, registry: Option<&'a Registry>, z3_ctx: &'a crate::z3_shim::Context) -> Self {
        Self {
            // Phased state containers
            discovery: RefCell::new(crate::codegen::phases::DiscoveryState::new(file)),
            expansion: RefCell::new(crate::codegen::phases::ExpansionState::new()),
            emission: RefCell::new(crate::codegen::phases::EmissionState::new()),
            control_flow: RefCell::new(crate::codegen::phases::ControlFlowState::new()),
            
            // Verification state (has lifetime)
            z3_ctx,
            z3_solver: RefCell::new(crate::z3_shim::Solver::new(z3_ctx)),
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
            emit_alias_scopes: false, // temporarily disabled to fix llvm bug
            no_verify: false, // default: verification enabled
            lib_mode: false,
            sip_mode: false,
            debug_info: false,
            source_file: String::new(),
            freeing_functions: HashSet::new(),
            
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
            
            proof_hints: RefCell::new(Vec::new()),
        }
    }

    pub fn with_registry(mut self, registry: &'a Registry) -> Self {
        self.registry = Some(registry);
        self
    }

    // === Field Accessors (delegate to phased structs) ===
    // These provide backward-compatible access while state is organized by phase.
    

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
    
    /// Check if comptime is ready (std discovery complete)
    pub fn is_comptime_ready(&self) -> bool {
        self.discovery.borrow().comptime_ready
    }
    
    /// Mark comptime as ready after std library discovery
    pub fn set_comptime_ready(&self) {
        self.discovery.borrow_mut().comptime_ready = true;
    }
    
    /// Register a pulse function discovered during analysis
    pub fn register_pulse_function(&self, name: &str, frequency_hz: u32, tier: u8) {
        self.discovery.borrow_mut().pulse_functions.insert(name.to_string(), (frequency_hz, tier));
    }
    
    /// Check if a function is a pulse function and get its tier
    pub fn get_pulse_info(&self, name: &str) -> Option<(u32, u8)> {
        self.discovery.borrow().pulse_functions.get(name).copied()
    }
    
    /// Check if a function requires yield injection (is pulse function)
    pub fn is_pulse_function(&self, name: &str) -> bool {
        self.discovery.borrow().pulse_functions.contains_key(name)
    }

    /// Register a type's KeuOS Home module.
    pub fn register_type_home(&self, type_name: String, module_package: String) {
        self.discovery.borrow_mut().register_type_home(type_name, module_package);
    }

    /// Register a trait's home module.
    pub fn register_trait_home(&self, trait_name: String, module_package: String) {
        self.discovery.borrow_mut().register_trait_home(trait_name, module_package);
    }

    /// Check if this module owns the type.
    pub fn is_type_home(&self, type_name: &str, current_module: &str) -> bool {
        self.discovery.borrow().is_type_home(type_name, current_module)
    }

    /// Check if this module owns the trait.
    pub fn is_trait_home(&self, trait_name: &str, current_module: &str) -> bool {
        self.discovery.borrow().is_trait_home(trait_name, current_module)
    }

    /// Register a trait impl and check for duplicates.
    pub fn register_trait_impl(&self, type_name: String, trait_name: String, module_package: String) -> Result<(), String> {
        self.discovery.borrow_mut().register_trait_impl(type_name, trait_name, module_package)
    }

    /// Validate coherence of all trait implementations.
    pub fn validate_coherence(&self) -> Result<(), String> {
        self.discovery.borrow().validate_coherence()
    }

    /// Register liveness analysis result for a @yielding function
    pub fn register_liveness(&self, fn_name: String, result: crate::codegen::passes::liveness::LivenessResult) {
        self.discovery.borrow_mut().liveness_results.insert(fn_name, result);
    }

    /// Get liveness result for a function (None if synchronous)
    pub fn get_liveness(&self, fn_name: &str) -> Option<crate::codegen::passes::liveness::LivenessResult> {
        self.discovery.borrow().liveness_results.get(fn_name).cloned()
    }

    /// Register HIR items for an async function that has been lowered
    /// via lower_async_fn_cfg. The items (struct + step fn) are cached so emit_fn
    /// can bypass AST codegen and delegate directly to emit_hir_items.
    pub fn register_hir_async(&self, fn_name: &str, items: Vec<crate::hir::items::Item>) {
        self.discovery.borrow_mut().hir_async_items.insert(fn_name.to_string(), items);
    }

    /// Get HIR items for a function (None if not lowered via HIR path)
    pub fn get_hir_async_items(&self, fn_name: &str) -> Option<Vec<crate::hir::items::Item>> {
        self.discovery.borrow().hir_async_items.get(fn_name).cloned()
    }

    /// Get the I/O backend for the current target platform.
    /// Returns a boxed trait object implementing platform-specific I/O MLIR emission.
    pub fn io_backend(&self) -> Box<dyn crate::codegen::passes::io_backend::IoBackend> {
        crate::codegen::passes::io_backend::backend_for_target(self.target_platform)
    }
    
    
    
    
    
    

    pub fn invalidate_type_cache(&self) {
        *self.struct_type_cache_mut() = None;
    }

    pub fn find_methods_for_template(&self, template_name: &str) -> Vec<String> {
        // Delegate to TraitRegistry
        self.trait_registry().find_methods_for_type(template_name)
    }

    /// Unified Pointer Peeling
    /// Instead of checking Reference/Owned/NativePtr separately, the
    /// first-class Type::Pointer variant is peeled.
    pub fn resolve_method(&self, receiver_ty: &Type, method_name: &str) -> Result<(SaltFn, Option<Type>, Vec<ImportDecl>), String> {
        let mut current_ty = receiver_ty.clone();
        let mut depth = 0;
        
        loop {
            if depth > 10 { break; }
            depth += 1;

            // Lookup via TraitRegistry signature-aware resolution
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

            // 2. Dereference through pointer wrappers
            // This replaces legacy Reference/Owned/NativePtr branches.
            if let Type::Pointer { element, .. } = current_ty {
                current_ty = (*element).clone();
                continue;
            }

            break; 
        }

        Err(format!("Method '{}' not found for type '{}' (peeled depth: {})", method_name, receiver_ty.mangle_suffix(), depth))
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
        out: &mut String, // Assumes writing to string buffer usually; or return value?
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
        // A dummy Type::Struct/Concrete can be constructed from the key to get the mlir type string
        let _dummy_ty = if let Some(args) = &key.specialization {
             Type::Concrete(key.mangle(), args.clone()) // This might be circular if mangle uses args?
             // Actually Type::Concrete expects "BaseName".
             // The Type should be reconstructed from Key.
        } else {
             Type::Struct(key.mangle())
        };
        // Use TypeKey to reconstruct Type properly for to_mlir_type lookup?
        // Actually to_mlir_type uses registry lookup.
        // The mangled name can be used for the explicit struct type in GEP?
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
}


impl<'a> CodegenContext<'a> {

    pub fn get_struct_types(&self) -> HashMap<String, Vec<Type>> {
        if let Some(cache) = self.struct_type_cache().as_ref() {
            return cache.clone();
        }
        let mut map = HashMap::new();
        // Iterate over struct_registry (keyed by TypeKey)
        // info.name is used for the string map key (mangled name)
        for info in self.struct_registry().values() {
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
             // The first part of the FQN is assumed to be the namespace path,
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
        
        let size = ty.size_of(&self.struct_registry());
        let align = ty.align_of(&self.struct_registry());
        
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

        self.globals_mut().insert("sys_write".to_string(), Type::Fn(vec![Type::U64, Type::U64, Type::U64], Box::new(Type::U64)));
        self.globals_mut().insert("sys_read".to_string(), Type::Fn(vec![Type::U64, Type::U64, Type::U64], Box::new(Type::U64)));
        self.globals_mut().insert("sys_exit".to_string(), Type::Fn(vec![Type::I32], Box::new(Type::Unit)));
        // Note: Do NOT add these to defined_functions — they are FFI functions
        // without emitted bodies. Adding them to defined_functions would suppress
        // their func.func private forward declarations in the MLIR output.
    }

    // --- Unified Driver Extensions ---

    pub fn should_flatten_type(&self, task: &crate::codegen::collector::MonomorphizationTask) -> bool {
        // Check if the return type or self type is a single-field wrapper
        // that should be erased for structural identity (e.g. Ptr<T> → i64).

        // Check Self Type
        if let Some(Type::Concrete(base, _)) = &task.self_ty {
                 if let Some(def) = self.struct_templates().get(base) {
                     if def.fields.len() == 1 {
                         // Check if field is primitive (this is hard without mapping args, 
                         // but for now Ptr is the main target).
                         // k_is_ptr_type covers the Ptr case.
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
                 let exists = self.registry.as_ref().is_some_and(|r| r.modules.contains_key(&candidate));
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


    /// Removed legacy Ptr bootstrap.
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


    // --- Recursive context switcher ---

    pub fn is_function_defined(&self, mangled_name: &str) -> bool {
        if self.defined_functions().contains(mangled_name) {

             return true;
        }
        if self.external_decls().contains(mangled_name) {

             return true;
        }
        false
    }

    /// Ensure a local function is scheduled for generation.
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

        if task.type_map.values().any(|t| t.has_generics()) {
             return Ok(());
        }

        if self.defined_functions().contains(&mangled_name) {
             return Ok(());
        }

        if self.external_decls().contains(&mangled_name) {
            if let Some((wrapper, _)) = self.generic_impls().get(&mangled_name) {
                if wrapper.body.stmts.is_empty() {
                    return Ok(());
                }
            }
        }

        self.defined_functions_mut().insert(mangled_name.clone());

        let prev_type_map = self.current_type_map().clone();
        let prev_concrete = self.current_generic_args().clone();
        let prev_self = self.current_self_ty().clone();
        let prev_imports = self.imports().clone();
        let prev_ret_ty = self.current_ret_ty().clone();
        let prev_ensures = self.current_ensures().clone();

        let canonical_type_map = self.canonicalize_type_map(&task.type_map);
        *self.current_type_map_mut() = canonical_type_map;
        *self.current_generic_args_mut() = task.concrete_tys.clone();
        *self.current_self_ty_mut() = task.self_ty.clone();
        *self.imports_mut() = task.imports.clone();

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

        self.inject_module_globals();

        let emission_result = self.emit_function_definition(&task.func, &mangled_name);

        let expected_import_count = prev_imports.len();
        *self.current_type_map_mut() = prev_type_map;
        *self.current_generic_args_mut() = prev_concrete;
        *self.current_self_ty_mut() = prev_self;
        *self.imports_mut() = prev_imports;
        *self.current_ret_ty_mut() = prev_ret_ty;
        *self.current_ensures_mut() = prev_ensures;
        self.current_package.replace(prev_pkg);
        debug_assert_eq!(self.imports().len(), expected_import_count,
            "IMPORT CLOBBER in hydrate_specialization for '{}': saved {} imports but restored {}",
            mangled_name, expected_import_count, self.imports().len());

        match emission_result {
            Ok(code) => {
                self.definitions_buffer_mut().push_str(&code);
                Ok(())
            },
            Err(e) => {
                self.defined_functions_mut().remove(&mangled_name);
                Err(e)
            }
        }
    }

    fn canonicalize_type_map(&self, type_map: &std::collections::BTreeMap<String, Type>) -> std::collections::BTreeMap<String, Type> {
        let mut canonical_type_map = type_map.clone();
        for ty in canonical_type_map.values_mut() {
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
        canonical_type_map
    }

    fn inject_module_globals(&self) {
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


    pub fn emit_function_definition(&self, func: &SaltFn, mangled_name: &str) -> Result<String, String> {
        emit_fn(self, func, Some(mangled_name.to_string()))
    }

    pub fn ensure_external_declaration(&self, mangled_name: &str, arg_tys: &[Type], ret_ty: &Type) -> Result<(), String> {
         if self.is_function_defined(mangled_name) {

             return Ok(());
         }
         
         // If it starts with llvm., it is assumed to be an intrinsic that doesn't need explicit decl (or handled elsewhere)
         if mangled_name.starts_with("llvm.") {
             return Ok(());
         }
         
         // Generate Declaration
         // func.func private @name(args...) -> ret
         let mut args_code = Vec::new();
         for ty in arg_tys {
             args_code.push(self.resolve_mlir_type(ty)?);
         }
         
         let ret_part = if *ret_ty == Type::Unit { "".to_string() } else { format!(" -> {}", self.resolve_mlir_type(ret_ty)?) };
         
         // Mark contract violation as cold+noreturn so LLVM
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
            if imp.alias.as_ref().is_some_and(|a| a == query_name) {
                 Some(Mangler::mangle(&imp.name.iter().map(|i| i.to_string()).collect::<Vec<_>>()))
            } else { None }
        }).unwrap_or(query_name.to_string());

        let mangled_name = &resolved_name;

        if let Some(ty) = self.globals().get(mangled_name) {
            return Some(ty.clone());
        }
        
        // 2. Wildcard Import Expansion: Check `use X::*` imports
        // When import has no alias AND no group, it's a wildcard import from that module.
        // query_name is looked up in that module's symbols from Registry.
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
        // This handles cases where imports are missing but the correct package context applies
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
             let mut ty_str = self.resolve_mlir_type(t)?;
             if matches!(t, Type::Reference(..) | Type::Owned(..) | Type::Fn(..)) {
                  ty_str.push_str(" {llvm.noalias}");
             }
             arg_code.push(ty_str);
        }
        let arg_str = arg_code.join(", ");
            
        // External declaration: func.func private
        let ret_str = if let Type::Unit = ret_ty { "()".to_string() } else { self.resolve_mlir_type(ret_ty)? };
        // Mark contract violation as cold+noreturn so LLVM
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

        let mlir_ty = self.resolve_mlir_type(ty)?;
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

    /// Check if currently inside an affine.for context
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

    // =========================================================================
    // Verification & Symbolic Analysis
    // =========================================================================

    /* Disabled Z3 methods
    pub fn mk_int(&self, val: i64) -> crate::z3_shim::ast::Int<'a> {
        crate::z3_shim::ast::Int::from_i64(self.z3_ctx, val)
    }

    pub fn mk_var(&self, name: &str) -> crate::z3_shim::ast::Int<'a> {
        crate::z3_shim::ast::Int::new_const(self.z3_ctx, name)
    }

    pub fn push_solver(&self) {
        self.z3_solver.borrow_mut().push();
    }

    pub fn pop_solver(&self) {
        self.z3_solver.borrow_mut().pop(1);
    }

    pub fn add_assertion(&self, expr: &crate::z3_shim::ast::Bool<'a>) {
        self.z3_solver.borrow_mut().assert(expr);
    }

    pub fn is_provably_safe(&self, violation: &crate::z3_shim::ast::Bool<'a>) -> bool {
        *self.total_checks.borrow_mut() += 1;
        self.z3_solver.borrow_mut().push();
        self.add_assertion(violation);
        
        let res = self.z3_solver.borrow_mut().check();
        self.pop_solver();

        let safe = matches!(res, crate::z3_shim::SatResult::Unsat);
        if safe {
            *self.elided_checks.borrow_mut() += 1;
        }
        safe
    }

    pub fn register_symbolic_int(&self, ssa_name: String, val: crate::z3_shim::ast::Int<'a>) {
        self.symbolic_tracker.borrow_mut().insert(ssa_name, val);
    }

    pub fn get_symbolic_int(&self, ssa_name: &str) -> Option<crate::z3_shim::ast::Int<'a>> {
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
                   } 
              } 
         }
         None
    }
}
