pub mod context;
pub mod phases;
pub mod abi;
pub mod type_bridge;
pub mod type_casts;
pub mod expr;
pub mod stmt;
pub mod intrinsics;
pub mod module_loader;
pub mod collector;
pub mod seeker;
pub mod seeker_resolve;
pub mod tracer;
pub mod verification;
pub mod const_eval;
pub mod struct_deriver;
pub mod trait_registry;  // Signature-aware method resolution
pub mod types;
pub mod interleaved_gen;  // FFB: Fused Forward-Backward codegen
pub mod passes;           // KeuOS: Pulse injection, yield injection, sync verification
pub mod generic_resolver;
pub mod generic_unify;
pub mod shader;           // Metal Shading Language codegen for @shader functions
pub mod emit_hir;         // HIR-to-MLIR emitter for async lowering Items
mod scan_types;
#[cfg(test)]
mod tests_ptr_and_comparison;
#[cfg(test)]
mod tests_generic_enum_match;
mod tests_result_monomorphization;
#[cfg(test)]
mod tests_ptr_field_access;
#[cfg(test)]
mod tests_malloc_tracking;
#[cfg(test)]
mod tests_type_promotion;
#[cfg(test)]
mod tests_stack_array;
#[cfg(test)]
mod tests_pointer_safety;
#[cfg(test)]
mod tests_pointer_truthiness;
#[cfg(test)]
mod tests_iterator_protocol;
#[cfg(test)]
mod tests_iterator_combinators;
#[cfg(test)]
mod tests_generic_resolver;
#[cfg(test)]
mod tests_match_destructuring;
#[cfg(test)]
mod tests_shader;
#[cfg(test)]
mod tests_main_entry;
#[cfg(test)]
mod tests_fast_math_reduction;
#[cfg(test)]
mod tests_spill_elimination;
#[cfg(test)]
mod tests_cross_module_struct;
#[cfg(test)]
mod tests_kernel_halt;
#[cfg(test)]
mod tests_ffi;
#[cfg(test)]
mod tests_datalayout;
#[cfg(test)]
mod tests_method_receiver;
#[cfg(test)]
mod tests_forward_ref;
#[cfg(test)]
mod tests_salt_atomic;
#[cfg(test)]
mod tests_fn_ptr;
#[cfg(test)]
mod tests_cf_br_backedge;
#[cfg(test)]
mod tests_z3_alignment;
#[cfg(test)]
mod tests_sip_safety;
#[cfg(test)]
mod tests_packed_struct;
#[cfg(test)]
mod tests_mixed_width_struct;
#[cfg(test)]
mod tests_z3_loop_verification;
#[cfg(test)]
mod tests_pmm_aba;
#[cfg(test)]
mod tests_kernel_unsafe;
#[cfg(test)]
mod tests_proof_hint;
#[cfg(test)]
mod tests_postcondition;
#[cfg(test)]
mod tests_ptr_null_comparison;
#[cfg(test)]
mod tests_malloc_arg_escape;
#[cfg(test)]
mod tests_static_mut;
#[cfg(test)]
mod tests_struct_ref_pass;
#[cfg(test)]
mod tests_nested_ptr_access;
#[cfg(test)]
mod tests_simd_v128;
#[cfg(test)]
mod tests_chase_lev_verification;
#[cfg(test)]
mod tests_syn_cookie_verification;
#[cfg(test)]
mod tests_ebr_verification;
pub mod sir;
#[cfg(test)]
mod tests_sir_emit;
#[cfg(test)]
mod tests_netd_integration;
#[cfg(test)]
mod tests_sir_ast_extraction;
#[cfg(test)]
mod tests_ring_abi;
#[cfg(test)]
mod tests_negative_verification;
use crate::grammar::{SaltFile, Item, SaltFn, SaltImpl, ExternFnDecl, SaltConcept, SaltTrait};
use crate::codegen::context::CodegenContext;
use crate::codegen::stmt::emit_block;
use crate::codegen::module_loader::ModuleLoader;
use crate::common::mangling::Mangler;
use crate::types::Type;
use crate::registry::Registry;
use std::collections::{HashMap, HashSet};
    #[allow(clippy::too_many_arguments)]
    // REASON: all 10 parameters are independently meaningful; bundling would obscure intent
    pub fn emit_mlir(file: &mut SaltFile, release_mode: bool, _registry: Option<&Registry>, _skip_scan: bool, no_verify: bool, disable_alias_scopes: bool, lib_mode: bool, sip_mode: bool, debug_info: bool, source_file: &str) -> Result<String, String> {
        let (mut loader, loader_registry) = load_modules(file)?;
        resolve_names(file, &mut loader)?;
        // Use the combined AST (all imported modules merged) as the
        // compilation unit. This links all module bodies into one MLIR
        // output, resolving cross-module function calls.
        // Carry the entry file's package declaration so the unsafe policy
        // and name mangling work correctly.
        let mut combined = loader.combined_ast.clone();
        combined.package = file.package.clone();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&combined, release_mode, Some(&loader_registry), &z3_ctx);
        initialize_context(&mut ctx, &combined, &loader, no_verify, disable_alias_scopes, lib_mode, sip_mode, debug_info, source_file);
        crate::codegen::expr::memory::clear_field_axioms_cache();
        register_all_templates_and_signatures(&ctx, &combined, &loader)?;
        scan_definitions(&mut ctx, &combined, &loader)?;
        let call_graph_analyzer = run_call_graph_analysis(&combined, release_mode);
        run_pulse_analysis(&mut ctx, &combined, &call_graph_analyzer, release_mode);
        run_liveness_analysis(&mut ctx, &combined, release_mode);
        lower_state_machines(&mut ctx, &combined);
        ctx.drive_codegen()
    }

    fn load_modules(file: &SaltFile) -> Result<(ModuleLoader, Registry), String> {
        let mut loader_registry = Registry::new();
        let mut loader = ModuleLoader::new(vec![
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            std::path::PathBuf::from("."),
            std::path::PathBuf::from(".."),
            std::path::PathBuf::from("../std"),
            std::path::PathBuf::from("../../std"),
        ]);
        for imp in &file.get_use_namespaces() {
            let _ = loader.load_module(imp, &mut loader_registry);
        }
        Ok((loader, loader_registry))
    }

    fn resolve_names(file: &mut SaltFile, loader: &mut ModuleLoader) -> Result<(), String> {
        let mut global_types = HashSet::new();
        let collect_globals = |f: &SaltFile, globals: &mut HashSet<String>| {
            let pkg_prefix = if let Some(pkg) = &f.package {
                Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>())
            } else {
                String::new()
            };
            for item in &f.items {
                let name = match item {
                    Item::Struct(s) => Some(&s.name),
                    Item::Enum(e) => Some(&e.name),
                    Item::Trait(t) => Some(&t.name),
                    Item::Concept(c) => Some(&c.name),
                    _ => None,
                };
                if let Some(id) = name {
                    let fqn = if pkg_prefix.is_empty() { id.to_string() } else { format!("{}__{}", pkg_prefix, id) };
                    globals.insert(fqn);
                }
            }
        };
        
        let comp_order = loader.get_compilation_order()?;
        for ns in &comp_order {
            if let Some(ast) = loader.loaded_files.get(ns) {
                collect_globals(ast, &mut global_types);
            }
        }
        collect_globals(file, &mut global_types);

        for ns in &comp_order {
            if let Some(ast) = loader.loaded_files.get_mut(ns) {
                crate::codegen::phases::resolution::name_resolver::NameResolver::resolve_file(ast, &global_types);
            }
        }
        crate::codegen::phases::resolution::name_resolver::NameResolver::resolve_file(file, &global_types);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    // REASON: all 9 parameters are independently meaningful; bundling would obscure intent
    fn initialize_context<'a>(ctx: &mut CodegenContext<'a>, file: &SaltFile, loader: &ModuleLoader, no_verify: bool, disable_alias_scopes: bool, lib_mode: bool, sip_mode: bool, debug_info: bool, source_file: &str) {
        let mut all_files = Vec::new();
        let comp_order = loader.get_compilation_order().unwrap_or_default();
        for ns in &comp_order {
            if let Some(ast) = loader.loaded_files.get(ns) {
                all_files.push(ast);
            }
        }
        all_files.push(file);
        ctx.freeing_functions = crate::codegen::phases::purity::PurityAnalyzer::analyze(&all_files);

        ctx.emit_alias_scopes = !disable_alias_scopes;
        ctx.no_verify = no_verify;
        ctx.lib_mode = lib_mode;
        ctx.sip_mode = sip_mode;
        ctx.debug_info = debug_info;
        ctx.source_file = source_file.to_string();
        ctx.register_builtins();
    }

    fn register_all_templates_and_signatures(ctx: &CodegenContext, file: &SaltFile, loader: &ModuleLoader) -> Result<(), String> {
        let order = loader.get_compilation_order()?;
        for ns in &order {
            if let Some(ast) = loader.loaded_files.get(ns) {
                register_templates(ctx, ast)?;
            }
        }
        register_templates(ctx, file)?;

        for ns in &order {
            if let Some(ast) = loader.loaded_files.get(ns) {
                register_signatures(ctx, ast)?;
            }
        }
        register_signatures(ctx, file)?;
        Ok(())
    }

    fn scan_definitions(ctx: &mut CodegenContext, file: &SaltFile, loader: &ModuleLoader) -> Result<(), String> {
        let dep_order = loader.get_compilation_order()?;
        ctx.init_registry_definitions();
        for ns in &dep_order {
            if let Some(f) = loader.loaded_files.get(ns) {
                 ctx.scan_defs_from_file(f, false)?;
            }
        }
        ctx.scan_defs_from_file(file, true)?;
        Ok(())
    }

    fn run_call_graph_analysis(file: &SaltFile, _release_mode: bool) -> passes::call_graph::CallGraphAnalyzer {
        use passes::call_graph::CallGraphAnalyzer;
        let mut cg = CallGraphAnalyzer::new();
        let _call_graph_analysis = cg.analyze(file);
        cg
    }

    fn run_pulse_analysis(ctx: &mut CodegenContext, file: &SaltFile, call_graph_analyzer: &passes::call_graph::CallGraphAnalyzer, _release_mode: bool) {
        use passes::pulse_injection::PulseInjectionContext;
        let mut pulse_ctx = PulseInjectionContext::new();
        pulse_ctx.analyze_with_call_graph(file, call_graph_analyzer);
        
        for info in pulse_ctx.pulse_info {
            ctx.register_pulse_function(&info.name, info.frequency_hz, info.tier);
        }
    }

    fn run_liveness_analysis(ctx: &mut CodegenContext, file: &SaltFile, _release_mode: bool) {
        use passes::liveness::CrossYieldAnalyzer;
        for item in &file.items {
            if let Item::Fn(func) = item {
                let mut analyzer = CrossYieldAnalyzer::new();
                let result = analyzer.analyze(func);
                if result.needs_transform {
                    let name = func.name.to_string();
                    ctx.register_liveness(name, result);
                }
            }
        }
    }

    fn lower_state_machines(ctx: &mut CodegenContext, file: &SaltFile) {
        use crate::hir::lower::LoweringContext;
        use crate::hir::async_lower::{lower_async_fn_cfg, VarInfo};

        for item in &file.items {
            if let Item::Fn(func) = item {
                let name = func.name.to_string();
                
                if let Some(liveness) = ctx.get_liveness(&name) {
                    if liveness.needs_transform {
                        let mut lctx = LoweringContext::new();
                        if let Some(crate::hir::items::Item { kind: crate::hir::items::ItemKind::Fn(hir_func), .. }) = lctx.lower_item(item) {
                            
                            let mut crossing_var_infos = Vec::new();
                            for frame_member in &liveness.frame_members {
                                if let Some(&var_id) = lctx.var_name_map.get(&frame_member.name) {
                                    crossing_var_infos.push(VarInfo {
                                        var_id,
                                        name: frame_member.name.clone(),
                                        ty: crate::hir::types::Type::I64,
                                    });
                                }
                            }

                            let next_var_id = (lctx.var_name_map.len() + 100) as u32;
                            let lowered_items = lower_async_fn_cfg(
                                &name,
                                &hir_func,
                                &crossing_var_infos,
                                next_var_id,
                            );

                            ctx.register_hir_async(&name, lowered_items);
                        }
                    }
                }
            }
        }
    }

impl<'a> CodegenContext<'a> {

    pub fn drive_codegen(&mut self) -> Result<String, String> {
        self.verify_struct_alignments()?;

        if self.lib_mode {
            self.seed_library_mode()?;
        } else {
            self.seed_executable_mode()?;
        }
        
        self.hydrate_all()?;

        self.assemble_module()
    }

    fn seed_library_mode(&mut self) -> Result<(), String> {
        let mut tasks = Vec::new();
        for item in &self.file.borrow().items {
            match item {
                Item::Fn(f) => tasks.extend(self.collect_fn_tasks(f)),
                Item::Impl(imp) => {
                    match imp {
                        SaltImpl::Methods { target_ty, methods, generics } => {
                            tasks.extend(self.collect_methods_impl_tasks(target_ty, methods, generics)?);
                        }
                        SaltImpl::Trait { target_ty, methods, generics, .. } => {
                            tasks.extend(self.collect_trait_impl_tasks(target_ty, methods, generics)?);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        for task in tasks { self.hydrate_specialization(task)?; }
        Ok(())
    }

    fn collect_fn_tasks(&self, f: &SaltFn) -> Vec<crate::codegen::collector::MonomorphizationTask> {
        // Hydrate any function with contracts so requires/ensures are
        // verified at call sites, regardless of visibility.
        let has_contracts = !f.requires.is_empty() || !f.ensures.is_empty();
        if !f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export") && !f.is_pub && !has_contracts {
            return vec![];
        }
        self.create_main_task(&f.name.to_string()).map(|t| vec![t]).unwrap_or_default()
    }

    fn collect_impl_tasks(
        &self, target_ty: &crate::grammar::SynType, methods: &[SaltFn],
        generics: &Option<crate::grammar::Generics>, only_pub: bool,
    ) -> Result<Vec<crate::codegen::collector::MonomorphizationTask>, String> {
        if generics.is_some() { return Ok(vec![]); }
        let Some(parsed_ty) = crate::types::Type::from_syn(target_ty) else { return Ok(vec![]); };
        let target = self.bridge_resolve_codegen_type(&parsed_ty).mangle_suffix();
        let pkg_path = match &self.file.borrow().package {
            Some(pkg) => pkg.name.iter().map(|id| id.to_string()).collect(),
            None => vec![],
        };
        let imports = CodegenContext::compute_full_imports(&self.file.borrow());
        let mut tasks = Vec::new();
        for m in methods {
            if (only_pub && !m.is_pub) || m.generics.is_some() { continue; }
            let name = m.name.to_string();
            tasks.push(crate::codegen::collector::MonomorphizationTask {
                identity: crate::types::TypeKey { path: pkg_path.clone(), name: name.clone(), specialization: None },
                mangled_name: Mangler::mangle(&[&target, &name]),
                func: m.clone(), concrete_tys: vec![],
                self_ty: Some(parsed_ty.clone()), imports: imports.clone(),
                type_map: std::collections::BTreeMap::new(),
            });
        }
        Ok(tasks)
    }

    fn collect_methods_impl_tasks(
        &self, target_ty: &crate::grammar::SynType, methods: &[SaltFn],
        generics: &Option<crate::grammar::Generics>,
    ) -> Result<Vec<crate::codegen::collector::MonomorphizationTask>, String> {
        self.collect_impl_tasks(target_ty, methods, generics, true)
    }

    fn collect_trait_impl_tasks(
        &self, target_ty: &crate::grammar::SynType, methods: &[SaltFn],
        generics: &Option<crate::grammar::Generics>,
    ) -> Result<Vec<crate::codegen::collector::MonomorphizationTask>, String> {
        self.collect_impl_tasks(target_ty, methods, generics, false)
    }

    fn seed_executable_mode(&mut self) -> Result<(), String> {
        let main_task = self.create_main_task("main")
            .or_else(|| self.create_main_task("main_salt"))
            .or_else(|| self.create_main_task("kmain"))
            .ok_or_else(|| "Entry point 'main', 'main_salt', or 'kmain' not found in root file.".to_string())?;
        self.hydrate_specialization(main_task)?;
        Ok(())
    }

    fn hydrate_all(&mut self) -> Result<(), String> {
        loop {
            let task = {
                let mut q = self.pending_generations_mut();
                q.pop_front()
            };
            
            if let Some(t) = task {
                 self.hydrate_specialization(t)?;
            } else {
                 break;
            }
        }
        Ok(())
    }

    fn assemble_module(&mut self) -> Result<String, String> {
        self.emitted_types_mut().clear();
        self.mlir_type_cache_mut().clear();
        
        let mut structure_defs = String::new();
        self.emit_structure_defs(&mut structure_defs);

        let mut out = String::new();
        out.push_str(&structure_defs);

        if self.debug_info && !self.source_file.is_empty() {
            self.assemble_debug_info(&mut out);
        }

        self.assemble_module_attributes(&mut out);
        
        {
            let emission = self.emission.borrow();
            // Sort global declarations for deterministic output.
            // HashMap-based import scanning emits globals in non-deterministic order.
            let sorted_decls = Self::sort_global_decls(&emission.decl_out);
            out.push_str(&sorted_decls);
            for (name, decl) in &emission.pending_func_decls {
                if !emission.defined_functions.contains(name) {
                    out.push_str(decl);
                }
            }
        }
        
        self.assemble_bootstrap_patches(&mut out);
        self.assemble_externals(&mut out);
        self.assemble_string_literals(&mut out);
        
        let bodies = self.definitions_buffer();
        out.push_str(&bodies);
        
        out.push_str("}\n");

        // Report Z3 verification coverage
        let elided = *self.elided_checks.borrow();
        let total = *self.total_checks.borrow();
        eprintln!("Z3: {}/{} checks proven ({}%), {} deferred to runtime",
            elided, total,
            if total > 0 { (elided * 100) / total } else { 0 },
            total - elided);
        Ok(out)
    }

    fn assemble_debug_info(&self, out: &mut String) {
        let source_path = std::path::Path::new(&self.source_file);
        let filename = source_path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| self.source_file.clone());
        let directory = source_path.parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        out.push_str(&format!(
            "#di_file = #llvm.di_file<\"{}\" in \"{}\">\n",
            filename, directory
        ));
        out.push_str(
            "#di_compile_unit = #llvm.di_compile_unit<id = distinct[100]<>, sourceLanguage = DW_LANG_C, file = #di_file, producer = \"Salt Compiler\", isOptimized = true, emissionKind = Full>\n"
        );
        out.push_str(
            "#di_subroutine_type = #llvm.di_subroutine_type<>\n"
        );
    }

    fn assemble_module_attributes(&self, out: &mut String) {
        let sip_attr = if self.sip_mode { ", \"salt.sip_verified\" = true" } else { "" };
        let proof_hints = self.proof_hints.borrow();
        let proof_hints_attr = if proof_hints.is_empty() {
            String::new()
        } else {
            let entries: Vec<String> = proof_hints.iter()
                .map(|(key, val)| format!("\"{}\" = {}", key, val))
                .collect();
            format!(", \"salt.proof_hints\" = {{{}}}", entries.join(", "))
        };
        out.push_str(&format!("module attributes {{llvm.data_layout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\", llvm.target_triple = \"x86_64-unknown-none-elf\"{}{}}} {{\n", sip_attr, proof_hints_attr));
    }

    /// Sort global declarations for deterministic output.
    /// The scanner emits globals and consts in HashMap-dependent order
    /// (imported files' consts are interleaved with the main file's items).
    /// This function extracts all `llvm.mlir.global` blocks, sorts them by
    /// symbol name, and reassembles the output.
    fn sort_global_decls(decl_out: &str) -> String {
        let mut result = String::with_capacity(decl_out.len());
        let mut pending_global: Option<String> = None;
        let mut globals: Vec<String> = Vec::new();
        let mut non_global_lines = String::new();

        for line in decl_out.lines() {
            if line.trim().starts_with("llvm.mlir.global") {
                // Start of a global declaration
                if let Some(g) = pending_global.take() {
                    // Multi-line global (region-based zero init): capture until '}'
                    globals.push(g);
                }
                if line.contains('{') && !line.contains('}') {
                    // Multi-line: region-based init
                    let mut global_text = line.to_string();
                    global_text.push('\n');
                    pending_global = Some(global_text);
                } else if line.trim().ends_with('{') {
                    // Multi-line variant
                    let mut global_text = line.to_string();
                    global_text.push('\n');
                    pending_global = Some(global_text);
                } else {
                    // Single-line global
                    globals.push(line.to_string());
                }
            } else if pending_global.is_some() {
                // Continue or end a multi-line global
                let mut g = pending_global.take().unwrap();
                g.push_str(line);
                if line.trim() == "}" {
                    // End of multi-line global
                    g.push('\n');
                    globals.push(g);
                } else {
                    g.push('\n');
                    pending_global = Some(g);
                }
            } else {
                // Non-global line (e.g. comments, func.func declarations, etc.)
                non_global_lines.push_str(line);
                non_global_lines.push('\n');
            }
        }
        // Flush any remaining pending global
        if let Some(g) = pending_global.take() {
            globals.push(g);
        }

        // Sort globals by their symbol name (extracted from @name pattern)
        globals.sort_by(|a, b| {
            let extract_name = |s: &str| {
                s.lines().next()
                    .and_then(|first| {
                        let trimmed = first.trim();
                        if let Some(at_pos) = trimmed.find('@') {
                            let after_at = &trimmed[at_pos + 1..];
                            if let Some(paren_pos) = after_at.find('(') {
                                Some(after_at[..paren_pos].to_string())
                            } else {
                                Some(after_at.split_whitespace().next()?.to_string())
                            }
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default()
            };
            extract_name(a).cmp(&extract_name(b))
        });

        result.push_str(&non_global_lines);
        for g in &globals {
            result.push_str(g);
            if !g.ends_with('\n') {
                result.push('\n');
            }
        }
        result
    }

    fn assemble_bootstrap_patches(&mut self, out: &mut String) {
        let patches = self.pending_bootstrap_patches();
        if !patches.is_empty() {
            out.push_str("  // Salt Bootstrap Runtime - patches global initializers\n");
            out.push_str("  func.func @__salt_bootstrap_runtime() {\n");
            
            for (patch_idx, patch) in patches.iter().enumerate() {
                let patch_id = format!("p{}", patch_idx);
                
                let target_ptr = format!("%target_{}", patch_id);
                out.push_str(&format!("    {} = llvm.mlir.addressof @{} : !llvm.ptr\n", 
                    target_ptr, patch.target_symbol));
                
                let mut current_ptr = format!("%global_{}", patch_id);
                out.push_str(&format!("    {} = llvm.mlir.addressof @{} : !llvm.ptr\n",
                    current_ptr, patch.global_name));
                
                for (level, idx) in patch.field_path.iter().enumerate() {
                    let next_ptr = format!("%field_{}_{}", patch_id, level);
                    let struct_ty = patch.struct_types.get(level)
                        .map(|s| s.as_str())
                        .unwrap_or("!llvm.struct<()>");
                    out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n",
                        next_ptr, current_ptr, idx, struct_ty));
                    current_ptr = next_ptr;
                }
                
                if !patch.field_path.is_empty() {
                    out.push_str(&format!("    llvm.store {}, {} : !llvm.ptr, !llvm.ptr\n",
                        target_ptr, current_ptr));
                } else {
                    out.push_str(&format!("    llvm.store {}, %global_{} : !llvm.ptr, !llvm.ptr\n",
                        target_ptr, patch_id));
                }
            }
            
            out.push_str("    func.return\n");
            out.push_str("  }\n");
        }
        drop(patches);
    }

    fn assemble_externals(&self, out: &mut String) {
        let hooks = self.entity_registry().get_active_hooks();
        for hook in &hooks {
            let sig = match hook.as_str() {
                "__salt_print_literal" => "(!llvm.ptr, i64) -> ()",
                "__salt_print_i64" | "__salt_print_u64" | "__salt_print_ptr" => "(i64) -> ()",
                "__salt_print_f64" => "(f64) -> ()",
                "__salt_print_bool" => "(i8) -> ()",
                "__salt_print_char" => "(i32) -> ()",
                "putchar" => "(i32) -> i32",
                "__salt_fmt_f64_to_buf" => "(!llvm.ptr, f64, i64) -> i64",
                "free" => "(!llvm.ptr) -> ()",
                "malloc" => "(i64) -> !llvm.ptr",
                "memchr" => "(!llvm.ptr, i32, i64) -> !llvm.ptr",
                _ => "() -> ()",
            };
            out.push_str(&format!("  func.func private @{}{}\n", hook, sig));
        }
    }

    fn assemble_string_literals(&self, out: &mut String) {
        let string_lits = self.string_literals();
        for (name, content, _len) in string_lits.iter() {
            let escaped = content
                .replace('\\', "\\\\")
                .replace('\0', "\\00")
                .replace('\n', "\\n")
                .replace('\r', "\\0D")
                .replace('\t', "\\t")
                .replace('"', "\\\"");
            out.push_str(&format!("  llvm.mlir.global internal constant @{}(\"{}\\00\") {{addr_space = 0 : i32}} : !llvm.array<{} x i8>\n", 
                name, escaped, content.len() + 1));
        }
        drop(string_lits);
    }

    /// Verify all struct alignment constraints:
    ///   - @atomic fields: 16-byte alignment for cmpxchg16b
    ///   - @align(N) fields: N-byte alignment (cache-line isolation)
    ///   - @atomic structs: stride alignment (sizeof % 16 == 0)
    ///   - @packed structs: zero implicit padding
    fn verify_struct_alignments(&self) -> Result<(), String> {
        let structs: Vec<_> = {
            let file = self.file.borrow();
            file.items.iter().filter_map(|item| {
                if let Item::Struct(s) = item {
                    if s.generics.is_none() {
                        return Some(s.clone());
                    }
                }
                None
            }).collect()
        };

        for s in &structs {
            let mut byte_offset: usize = 0;
            let s_name_str = s.name.to_string();
            
            for f in &s.fields {
                self.verify_field_atomic(&s_name_str, f, byte_offset)?;
                byte_offset = self.verify_field_align(&s_name_str, f, byte_offset)?;

                let field_ty = self.bridge_resolve_type(&f.ty);
                let struct_reg = self.struct_registry();
                byte_offset += field_ty.size_of(&struct_reg);
            }

            self.verify_struct_atomic(&s_name_str, &s.attributes, byte_offset)?;
            self.verify_struct_packed(&s_name_str, &s.attributes, byte_offset, &s.fields)?;
        }
        Ok(())
    }

    fn verify_field_atomic(&self, s_name: &str, f: &crate::grammar::FieldDef, byte_offset: usize) -> Result<(), String> {
        use crate::z3_shim::ast::Ast;
        let has_atomic = f.attributes.iter().any(|a| a.name == "atomic");
        if !has_atomic { return Ok(()); }

        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);

        let base = crate::z3_shim::ast::Int::new_const(&z3_ctx, "base_addr");
        let sixteen = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 16);
        let zero = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 0);

        solver.assert(&base.ge(&zero));
        solver.assert(&base.modulo(&sixteen)._eq(&zero));

        let offset_val = crate::z3_shim::ast::Int::from_i64(&z3_ctx, byte_offset as i64);
        let field_addr = crate::z3_shim::ast::Int::add(&z3_ctx, &[&base, &offset_val]);

        solver.assert(&field_addr.modulo(&sixteen)._eq(&zero).not());

        match solver.check() {
            crate::z3_shim::SatResult::Unsat => {
                Ok(())
            }
            _ => Err(format!("[Formal Shadow] ALIGNMENT VIOLATION: @atomic field '{}' in struct '{}' is at byte offset {}, which is NOT 16-byte aligned. The Z3 SMT solver proved this layout violates the hardware alignment contract for cmpxchg16b. Fix: reorder fields or add padding so @atomic fields start at offsets that are multiples of 16.", f.name, s_name, byte_offset))
        }
    }

    fn verify_field_align(&self, s_name: &str, f: &crate::grammar::FieldDef, mut byte_offset: usize) -> Result<usize, String> {
        use crate::z3_shim::ast::Ast;
        let align_value = crate::grammar::attr::extract_align(&f.attributes);
        if let Some(n) = align_value {
            if n == 0 || (n & (n - 1)) != 0 {
                return Err(format!("[Formal Shadow] ALIGNMENT ERROR: @align({}) on field '{}' in struct '{}' is not a power of 2. Alignment values must be powers of 2 (e.g., 1, 2, 4, 8, 16, 32, 64).", n, f.name, s_name));
            }

            let align_n = n as usize;
            byte_offset = (byte_offset + align_n - 1) & !(align_n - 1);

            let z3_cfg = crate::z3_shim::Config::new();
            let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
            let solver = crate::z3_shim::Solver::new(&z3_ctx);

            let base = crate::z3_shim::ast::Int::new_const(&z3_ctx, "base_addr");
            let align_const = crate::z3_shim::ast::Int::from_i64(&z3_ctx, n as i64);
            let zero = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 0);

            solver.assert(&base.ge(&zero));
            solver.assert(&base.modulo(&align_const)._eq(&zero));

            let offset_val = crate::z3_shim::ast::Int::from_i64(&z3_ctx, byte_offset as i64);
            let field_addr = crate::z3_shim::ast::Int::add(&z3_ctx, &[&base, &offset_val]);

            solver.assert(&field_addr.modulo(&align_const)._eq(&zero).not());

            match solver.check() {
                crate::z3_shim::SatResult::Unsat => {
                    let struct_id = crate::codegen::verification::proof_hint::struct_name_to_id(s_name);
                    let hint = crate::codegen::verification::proof_hint::hash_combine(struct_id, byte_offset as u64, n as u64);
                    self.proof_hints.borrow_mut().push((format!("{}_{}", s_name, f.name), hint));
                }
                _ => {
                    return Err(format!("[Formal Shadow] ALIGNMENT VIOLATION: @align({}) field '{}' in struct '{}' is at byte offset {}, which is NOT {}-byte aligned. The Z3 SMT solver proved this layout violates the cache-line isolation contract. Fix: reorder fields or adjust alignment so @align({}) fields start at offsets that are multiples of {}.", n, f.name, s_name, byte_offset, n, n, n));
                }
            }
        }
        Ok(byte_offset)
    }

    fn verify_struct_atomic(&self, s_name: &str, attributes: &[crate::grammar::attr::Attribute], byte_offset: usize) -> Result<(), String> {
        use crate::z3_shim::ast::Ast;
        let has_struct_atomic = attributes.iter().any(|a| a.name == "atomic");
        if !has_struct_atomic { return Ok(()); }

        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);

        let size = crate::z3_shim::ast::Int::from_i64(&z3_ctx, byte_offset as i64);
        let sixteen = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 16);
        let zero = crate::z3_shim::ast::Int::from_i64(&z3_ctx, 0);

        solver.assert(&size.modulo(&sixteen)._eq(&zero).not());

        match solver.check() {
            crate::z3_shim::SatResult::Unsat => {
                Ok(())
            }
            _ => Err(format!("[Formal Shadow] STRIDE VIOLATION: @atomic struct '{}' has size {} bytes. {} % 16 != 0, so array elements would NOT be 16-byte aligned. The Z3 SMT solver proved this layout violates the hardware alignment contract for cmpxchg16b. Fix: ensure sizeof(@atomic struct) is a multiple of 16 bytes.", s_name, byte_offset, byte_offset))
        }
    }

    fn verify_struct_packed(&self, s_name: &str, attributes: &[crate::grammar::attr::Attribute], byte_offset: usize, fields: &[crate::grammar::FieldDef]) -> Result<(), String> {
        use crate::z3_shim::ast::Ast;
        let has_packed = attributes.iter().any(|a| a.name == "packed");
        if !has_packed { return Ok(()); }

        let unpadded_sum = byte_offset;
        let resolved_types: Vec<_> = fields.iter().map(|f| self.bridge_resolve_type(&f.ty)).collect();
        let field_sizes: Vec<usize> = {
            let struct_reg = self.struct_registry();
            resolved_types.iter().map(|ty: &crate::types::Type| ty.size_of(&struct_reg)).collect()
        };

        let mut abi_offset: usize = 0;
        let mut max_align: usize = 1;

        for &field_size in &field_sizes {
            let field_align = field_size.clamp(1, 8);
            let padding = (field_align - (abi_offset % field_align)) % field_align;
            abi_offset += padding;
            abi_offset += field_size;
            if field_align > max_align { max_align = field_align; }
        }

        let tail_padding = (max_align - (abi_offset % max_align)) % max_align;
        let abi_total = abi_offset + tail_padding;

        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let solver = crate::z3_shim::Solver::new(&z3_ctx);

        let abi_size = crate::z3_shim::ast::Int::from_i64(&z3_ctx, abi_total as i64);
        let raw_sum = crate::z3_shim::ast::Int::from_i64(&z3_ctx, unpadded_sum as i64);

        solver.assert(&abi_size._eq(&raw_sum).not());

        match solver.check() {
            crate::z3_shim::SatResult::Unsat => {
                Ok(())
            }
            _ => Err(format!("[Formal Shadow] PACKED VIOLATION: @packed struct '{}' has implicit padding. ABI layout = {} bytes, but raw field sum = {} bytes ({} bytes of hidden padding). The Z3 SMT solver proved this layout violates the zero-padding contract. Fix: reorder fields or add explicit padding fields to eliminate gaps.", s_name, abi_total, unpadded_sum, abi_total - unpadded_sum))
        }
    }
    
    fn create_main_task(&self, name: &str) -> Option<crate::codegen::collector::MonomorphizationTask> {
        // 1. Check current file
        for item in &self.file.borrow().items {
            if let Item::Fn(f) = item {
                if f.name == name {
                    let pkg_path = if let Some(pkg) = &self.file.borrow().package {
                        pkg.name.iter().map(|id| id.to_string()).collect()
                    } else {
                        vec![]
                    };
                    
                    // In lib mode, mangle function names with package prefix
                    // to avoid symbol collisions between modules (e.g., multiple `init` functions).
                    // @no_mangle functions retain their bare names.
                    let is_no_mangle = f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" );
                    // Future(Phase 4): Remove `main_salt` hardcode once all legacy tests use
                    // `@no_mangle fn main_salt()`. The @no_mangle attribute already exists
                    // in the grammar and handles this generically for any FFI boundary.
                    let mangled = if name == "main" || name == "main_salt" {
                        // ENTRY POINT: fn main/main_salt — never mangle.
                        // The linker expects `_main` / `_main_salt`, not `_main__main`.
                        name.to_string()
                    } else if !is_no_mangle && !pkg_path.is_empty() {
                        format!("{}__{}", pkg_path.join("__"), name)
                    } else {
                        name.to_string()
                    };

                    return Some(crate::codegen::collector::MonomorphizationTask {
                        identity: crate::types::TypeKey { path: pkg_path, name: name.to_string(), specialization: None },
                        mangled_name: mangled,
                        func: f.clone(),
                        concrete_tys: vec![],
                        self_ty: None,
                        imports: CodegenContext::compute_full_imports(&self.file.borrow()),
                        type_map: std::collections::BTreeMap::new(),
                    });
                }
            }
        }
        None
    }
    
    // finalize_module removed/merged into drive_codegen

    fn emit_structure_defs(&self, out: &mut String) {
        // Clone registry data into owned collections
        // to drop the RefCell Ref guards before calling resolve_mlir_storage_type,
        // which needs with_lowering_ctx → discovery.borrow_mut().
        let (struct_entries, enum_entries, all_keys) = {
            let registry = self.struct_registry();
            let enum_registry = self.enum_registry();
            let struct_entries: Vec<_> = registry.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let enum_entries: Vec<_> = enum_registry.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let mut all_keys: Vec<_> = registry.keys().cloned().collect();
            all_keys.extend(enum_registry.keys().cloned());
            (struct_entries, enum_entries, all_keys)
            // Ref guards dropped here
        };

        let struct_map: HashMap<_, _> = struct_entries.into_iter().collect();
        let enum_map: HashMap<_, _> = enum_entries.into_iter().collect();

        // 1. Build Dependency Graph
        let adj = self.build_struct_dep_graph(&all_keys, &struct_map);

        // 2. Topological Sort (DFS Post-Order)
        let mut sorted_keys = Vec::new();
        let mut temp_mark = HashSet::new();
        let mut perm_mark = HashSet::new();

        let mut sorted_starts = all_keys.clone();
        sorted_starts.sort_by_key(|a| a.mangle());

        for key in &sorted_starts {
            self.topo_visit(key, &adj, &mut temp_mark, &mut perm_mark, &mut sorted_keys);
        }

        // 3. Emit in Sorted Order
        self.emit_sorted_type_defs(out, &sorted_keys, &struct_map, &enum_map);

        // Always emit StringView type alias.
        let sv_name = "std__core__str__StringView";
        let sv_already_emitted = struct_map.values().any(|info| info.name == sv_name);
        if !sv_already_emitted {
            out.push_str(&format!(
                "!struct_{} = !llvm.struct<\"{}\", (!llvm.ptr, i64)>\n",
                sv_name, sv_name
            ));
        }
    }

    fn build_struct_dep_graph(
        &self, all_keys: &[crate::types::TypeKey],
        struct_map: &HashMap<crate::types::TypeKey, crate::registry::StructInfo>,
    ) -> HashMap<crate::types::TypeKey, Vec<crate::types::TypeKey>> {
        let mut adj = HashMap::new();
        for key in all_keys {
            let mut deps = Vec::new();
            if let Some(info) = struct_map.get(key) {
                for field_ty in &info.field_order {
                    self.collect_dependencies(field_ty, &mut deps);
                }
            }
            adj.insert(key.clone(), deps);
        }
        adj
    }

    fn emit_sorted_type_defs(&self, out: &mut String, sorted_keys: &[crate::types::TypeKey], struct_map: &HashMap<crate::types::TypeKey, crate::registry::StructInfo>, enum_map: &HashMap<crate::types::TypeKey, crate::registry::EnumInfo>) {
        let et: HashSet<String> = enum_map.values().map(|e| e.name.clone()).collect();
        let mut emitted: HashSet<String> = HashSet::new();
        for key in sorted_keys {
            if let Some(info) = struct_map.get(key) {
                if info.field_order.iter().any(|ty| !ty.is_fully_concrete(struct_map, &et)) { continue; }
                let mut ts = format!("!llvm.struct<\"{}\", (", info.name);
                for (i, ty) in info.field_order.iter().enumerate() {
                    if i > 0 { ts.push_str(", "); }
                    ts.push_str(&self.resolve_mlir_storage_type(ty).unwrap_or_else(|_| "!llvm.ptr".to_string()));
                }
                ts.push_str(")>");
                out.push_str(&format!("!struct_{} = {}\n", info.name, ts));
                let cn = Type::Struct(info.name.clone()).to_canonical_name();
                if cn != info.name && !emitted.contains(&cn) {
                    emitted.insert(cn.clone());
                    out.push_str(&format!("!struct_{} = !struct_{}\n", cn, info.name));
                }
            } else if let Some(info) = enum_map.get(key) {
                let payload = if info.max_payload_size > 0 { format!(", !llvm.array<{} x i8>", info.max_payload_size) } else { String::new() };
                let ts = format!("!llvm.struct<\"{}\", (i32{})>", info.name, payload);
                out.push_str(&format!("!struct_{} = {}\n", info.name, ts));
                let cn = Type::Enum(info.name.clone()).to_canonical_name();
                if cn != info.name && !emitted.contains(&cn) { emitted.insert(cn.clone()); out.push_str(&format!("!struct_{} = !struct_{}\n", cn, info.name)); }
            }
        }
    }

    fn collect_dependencies(&self, ty: &Type, deps: &mut Vec<crate::types::TypeKey>) {
        // if ty.k_is_ptr_type() { return; } // Pointers break dependencies - REMOVED for Ptr struct info
        match ty {
            Type::Struct(name) | Type::Enum(name) => {
                // Find Key from Mangled Name (Reverse Lookup or derive from mangled name?)
                // Registry is keyed by TypeKey. The mangled name is available.
                // The name must be matched to a Key.
                // Only if it exists in struct_registry.
                if let Some((k, _)) = self.struct_registry().iter().find(|(_, v)| v.name == *name) {
                    deps.push(k.clone());
                } else if let Some((k, _)) = self.enum_registry().iter().find(|(_, v)| v.name == *name) {
                    deps.push(k.clone());
                }
            },
            Type::Concrete(_base, _params) => {
                 // Should be resolved to mangled name by now in storage type?
                 // But here the analysis is on the Type structure from the registry which might store Concrete.
                 // Actually expand_template_structure stores Concrete types in fields?
                 // Most likely fields are resolved to Type::Struct(mangled) or Type::Concrete(resolved).
                 // The registry entry must be found.
                 // For Concrete, a key is constructed.
                 // (Simplified: rely on mangled name lookup)
                 if let Some((k, _)) = self.struct_registry().iter().find(|(k, _)| k.mangle() == ty.mangle_suffix()) {
                     deps.push(k.clone());
                 } else if let Some((k, _)) = self.enum_registry().iter().find(|(k, _)| k.mangle() == ty.mangle_suffix()) {
                     deps.push(k.clone());
                 }
            },
            Type::Array(inner, _, _) => self.collect_dependencies(inner, deps),
            Type::Tuple(elems) => {
                for e in elems { self.collect_dependencies(e, deps); }
            },
            _ => {}
        }
    }

    fn topo_visit(&self, key: &crate::types::TypeKey, adj: &HashMap<crate::types::TypeKey, Vec<crate::types::TypeKey>>, temp: &mut HashSet<crate::types::TypeKey>, perm: &mut HashSet<crate::types::TypeKey>, result: &mut Vec<crate::types::TypeKey>) {
        if perm.contains(key) { return; }
        if temp.contains(key) {
            // Cycle detected! Break cycle by emitting current.
            // In Salt, struct cycles must be via pointers.
            // A cycle via non-pointers indicates an Infinite Size type (compile error usually).
            // This is ignored; proceed.
            return;
        }
        
        temp.insert(key.clone());
        
        if let Some(deps) = adj.get(key) {
            for dep in deps {
                self.topo_visit(dep, adj, temp, perm, result);
            }
        }
        
        temp.remove(key);
        perm.insert(key.clone());
        result.push(key.clone());
    }
}

pub fn emit_concept(ctx: &CodegenContext, concept: &SaltConcept) -> Result<String, String> {
    if concept.generics.is_some() {
        return Ok(String::new()); // Generic concepts are purely compile-time for now
    }

    let fn_name = concept.name.to_string();
    ctx.defined_functions_mut().insert(fn_name.clone());
    *ctx.current_fn_name_mut() = fn_name.clone();
    
    ctx.consumed_vars_mut().clear();
    ctx.consumption_locs_mut().clear();
    ctx.devoured_vars_mut().clear();
    ctx.mutated_vars_mut().clear();
    
    let arg_name = concept.param.to_string();
    let arg_ty = ctx.bridge_resolve_type(&concept.param_ty);
    let mlir_arg_ty = ctx.resolve_mlir_type(&arg_ty)?;
    
    let ssa_name = format!("%arg_{}", arg_name);
    let mut local_vars = HashMap::new();
    local_vars.insert(arg_name.clone(), (arg_ty.clone(), crate::codegen::context::LocalKind::SSA(ssa_name.clone())));
    
    // Register Symbolic Var
    let z3_var = ctx.mk_var(&arg_name);
    ctx.register_symbolic_int(ssa_name.clone(), z3_var);

    let mut out = String::new();
    out.push_str(&format!("  func.func private @{}({}: {}) -> i1 {{\n", fn_name, ssa_name, mlir_arg_ty));
    
    let (val, ty) = ctx.with_lowering_ctx(|lctx| crate::codegen::expr::emit_expr(lctx, &mut out, &concept.requires, &mut local_vars, Some(&Type::Bool)))?;
    
    if ty != Type::Bool {
         return Err(format!("Concept {} requires clause must return Bool, got {:?}", fn_name, ty));
    }
    
    out.push_str(&format!("    return {}\n", val));
    out.push_str("  }\n");
    
    Ok(out)
}

/// Emit a trait definition - registers trait in TraitRegistry
pub fn emit_trait(ctx: &CodegenContext, trait_def: &SaltTrait) -> Result<String, String> {
    let trait_name = trait_def.name.to_string();
    
    // Register the trait definition in TraitRegistry
    ctx.trait_registry_mut().register_trait_def(
        trait_name.clone(),
        trait_def.generics.clone(),
        trait_def.methods.iter().map(|m| m.name.to_string()).collect(),
    );
    
    // Trait definitions don't emit MLIR directly - they're purely compile-time
    Ok(String::new())
}

pub fn emit_extern_fn(ctx: &CodegenContext, decl: &ExternFnDecl) -> Result<String, String> {
    let mut args_code = Vec::new();
    for arg in &decl.args {
        let ty = ctx.bridge_resolve_type(arg.ty.as_ref().ok_or_else(|| "Extern function argument missing type".to_string())?);
        if !ty.is_ffi_safe() {
            return Err(format!("Extern function `{}` argument `{}` has type `{:?}` which is not FFI-safe.", decl.name, arg.name, ty));
        }
        args_code.push(ctx.resolve_mlir_type(&ty)?);
    }
    
    // Extern functions always use their original C symbol name (never mangle)
    let name = decl.name.to_string();
    
    ctx.external_decls_mut().insert(name.clone());
    
    let ret_ty = if let Some(rt) = &decl.ret_type { ctx.bridge_resolve_type(rt) } else { Type::Unit };
    if !ret_ty.is_ffi_safe() {
        return Err(format!("Extern function `{}` has return type `{:?}` which is not FFI-safe.", decl.name, ret_ty));
    }

    let ret_part = if ret_ty == Type::Unit { "()".to_string() } else { 
        ctx.resolve_mlir_type(&ret_ty)?
    };

    Ok(format!("  func.func private @{}({}) -> {}\n", name, args_code.join(", "), ret_part))
}

/// Emit a @yielding function as a state machine.
/// Splits the function body at yield points, emits each segment via emit_block(),
/// and wraps them in state machine infrastructure (TaskFrame, jump table, dispatch hub).
fn emit_async_fn(
    ctx: &CodegenContext,
    func: &SaltFn,
    liveness: &crate::codegen::passes::liveness::LivenessResult,
) -> Result<String, String> {
    use crate::codegen::passes::async_to_state::{StateMachineEmitter, StateMachineConfig};

    let fn_name = if func.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" ) {
        func.name.to_string()
    } else {
        ctx.mangle_fn_name(&func.name.to_string()).to_string()
    };

    // === Body Splitting ===
    // Partition func.body.stmts at yield point positions.
    // YieldPointInfo.position maps directly to statement indices
    // (from CrossYieldAnalyzer.walk_block incrementing per stmt).
    let stmts = &func.body.stmts;
    let mut yield_positions: Vec<usize> = liveness.yield_points.iter()
        .map(|yp| yp.position)
        .collect();
    yield_positions.sort();

    // Generate per-state body MLIR by calling emit_block on each slice
    let num_states = liveness.yield_points.len() + 1;
    let mut state_bodies: Vec<String> = Vec::with_capacity(num_states);
    let mut local_vars = std::collections::HashMap::new();

    // Register function parameters as local variables
    for arg in &func.args {
        if let Some(ty) = &arg.ty {
            let resolved = ctx.bridge_resolve_type(ty);
            let _mlir_ty = ctx.resolve_mlir_type(&resolved)?;
            local_vars.insert(
                arg.name.to_string(),
                (resolved, crate::codegen::context::LocalKind::SSA(format!("%{}", arg.name))),
            );
        }
    }

    for state_idx in 0..num_states {
        let start = if state_idx == 0 {
            0
        } else {
            // Resume after yield point — skip the yield statement itself
            (yield_positions[state_idx - 1] + 1).min(stmts.len())
        };

        let end = if state_idx < yield_positions.len() {
            yield_positions[state_idx].min(stmts.len())
        } else {
            stmts.len()
        };

        let mut body_out = String::new();
        if start < end {
            let slice = &stmts[start..end];
            // Reset per-state emission counters
            *ctx.val_counter_mut() = 0;
            let _has_terminator = ctx.with_lowering_ctx(|lctx| emit_block(lctx, &mut body_out, slice, &mut local_vars))?;
        } else {
            body_out.push_str("      // (empty state segment)\n");
        }

        state_bodies.push(body_out);
    }

    // === State Machine Emission ===
    let config = StateMachineConfig {
        fn_name,
        ..Default::default()
    };
    let emitter = StateMachineEmitter::new(config);
    Ok(emitter.emit_full_async_mlir_with_bodies(liveness, &state_bodies))
}


pub fn emit_fn(ctx: &CodegenContext, func: &crate::grammar::SaltFn, override_name: Option<String>) -> Result<String, String> {
    if let Some(early_ret) = check_early_returns(ctx, func)? {
        return Ok(early_ret);
    }

    let fn_name = override_name.unwrap_or_else(|| {
        if func.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" ) {
            func.name.to_string()
        } else {
            ctx.mangle_fn_name(&func.name.to_string()).to_string()
        }
    });
    
    if check_generic_guards(ctx, func, &fn_name)? {
        return Ok(String::new());
    }
    
    *ctx.current_fn_name_mut() = fn_name.clone();
    ctx.defined_functions_mut().insert(fn_name.clone());
    
    let saved_alloca = ctx.alloca_out().clone();
    
    ctx.consumed_vars_mut().clear();
    ctx.consumption_locs_mut().clear();
    ctx.devoured_vars_mut().clear();
    *ctx.mutated_vars_mut() = crate::codegen::stmt::collect_mutations(&func.body.stmts);

    // Record array stores (arr[i] = val) for postcondition verification.
    // Previously only called inside for-loops; this enables ensures forall
    // on function bodies with direct array writes.
    crate::codegen::verification::array_tracker::process_array_stores_in_body(&func.body.stmts);

    let mut local_vars = std::collections::HashMap::new();
    let mut args_code = Vec::new();
    
    ctx.control_flow.borrow_mut().clear_arg_scopes();
    let prev_func_lvn = ctx.emission.borrow_mut().global_lvn.set_current_function(fn_name.clone());
    ctx.emission.borrow_mut().global_lvn.clear_current_func_cache();
    
    process_fn_arguments(ctx, func, &mut local_vars, &mut args_code)?;

    let ret_ty_raw = if let Some(rt) = &func.ret_type { ctx.bridge_resolve_type(rt) } else { Type::Unit };
    let ret_ty = ret_ty_raw.substitute(&ctx.current_type_map());
    *ctx.current_ret_ty_mut() = Some(ret_ty.clone());
    *ctx.current_ensures_mut() = func.ensures.clone();
    let ret_part = if ret_ty == Type::Unit { "".to_string() } else { format!(" -> {}", ctx.resolve_mlir_type(&ret_ty)?) };
    
    let fn_attrs = build_fn_attributes(ctx, func, &fn_name, &ret_ty);
    let loc_annotation = build_loc_annotation(ctx, func, &fn_name);

    let is_main = fn_name == "main";
    let is_no_mangle = func.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" );
    
    if is_no_mangle {
        for arg in &func.args {
            let ty = ctx.bridge_resolve_type(arg.ty.as_ref().unwrap()).substitute(&ctx.current_type_map());
            if !ty.is_ffi_safe() {
                return Err(format!("Exported function `{}` argument `{}` has type `{:?}` which is not FFI-safe.", fn_name, arg.name, ty));
            }
        }
        if !ret_ty.is_ffi_safe() {
            return Err(format!("Exported function `{}` has return type `{:?}` which is not FFI-safe.", fn_name, ret_ty));
        }
    }
    let visibility_keyword = if func.is_pub || is_no_mangle || is_main { "public" } else { "private" };

    let mut out = format!("  func.func {} @{}({}){}{} {{\n", visibility_keyword, fn_name, args_code.join(", "), ret_part, fn_attrs);
    out.push_str("    %c0 = arith.constant 0 : i32\n");
    out.push_str("    %c1_i64 = arith.constant 1 : i64\n");
    
    if is_main && !ctx.pending_bootstrap_patches().is_empty() {
        out.push_str("    // Warm boot: initialize global allocators\n");
        out.push_str("    func.call @__salt_bootstrap_runtime() : () -> ()\n");
    }
    
    ctx.alloca_out_mut().clear();
    let mut body_out = String::new();
    
    promote_mutated_args(ctx, func, &mut body_out, &mut local_vars)?;

    let saved_ownership = ctx.ownership_tracker.replace(crate::codegen::verification::Z3StateTracker::new(ctx.z3_ctx));
    let saved_malloc_tracker = ctx.malloc_tracker.replace(crate::codegen::verification::MallocTracker::new());
    let saved_arena_escape = ctx.arena_escape_tracker.replace(crate::codegen::verification::ArenaEscapeTracker::new());

    *ctx.pending_malloc_result.borrow_mut() = None;

    for arg in &func.args {
        ctx.arena_escape_tracker.borrow_mut().register_arg(&arg.name.to_string());
    }

    ctx.push_solver();
    
    emit_requires_verification(ctx, func, &mut body_out, &mut local_vars)?;

    // Push caller preconditions so callee verification benefits from them
    if !func.requires.is_empty() {
        for req in &func.requires { ctx.emission.borrow_mut().caller_preconditions.push(req.clone()); }
    }

    let old_no_yield = *ctx.no_yield();
    let old_pulse = *ctx.current_pulse();
    let pulse = crate::grammar::attr::extract_yielding_pulse(&func.attributes);
    *ctx.no_yield_mut() = pulse.is_none();
    *ctx.current_pulse_mut() = pulse;

    ctx.push_cleanup_scope();

    let has_fast_math = crate::grammar::attr::is_fast_math(&func.attributes);
    let old_fast_math_fn = ctx.emission.borrow().in_fast_math_fn;
    ctx.emission.borrow_mut().in_fast_math_fn = has_fast_math;

    let has_trusted = func.attributes.iter().any(|a| a.name == "trusted");
    let old_trusted_fn = ctx.emission.borrow().in_trusted_fn;
    ctx.emission.borrow_mut().in_trusted_fn = has_trusted;

    let has_dynamic_check = func.attributes.iter().any(|a| a.name == "dynamic_check");
    let old_dynamic_check_fn = ctx.emission.borrow().in_dynamic_check_fn;
    ctx.emission.borrow_mut().in_dynamic_check_fn = has_dynamic_check;

    let has_checked = func.attributes.iter().any(|a| a.name == "checked");
    let old_checked_fn = ctx.emission.borrow().in_checked_fn;
    ctx.emission.borrow_mut().in_checked_fn = has_checked;

    let terminator = ctx.with_lowering_ctx(|lctx| crate::codegen::stmt::emit_block(lctx, &mut body_out, &func.body.stmts, &mut local_vars))?;
    // Pop caller preconditions after body
    for _ in 0..func.requires.len() { ctx.emission.borrow_mut().caller_preconditions.pop(); }
    ctx.emission.borrow_mut().in_fast_math_fn = old_fast_math_fn;
    ctx.emission.borrow_mut().in_trusted_fn = old_trusted_fn;
    ctx.emission.borrow_mut().in_dynamic_check_fn = old_dynamic_check_fn;
    ctx.emission.borrow_mut().in_checked_fn = old_checked_fn;
    *ctx.no_yield_mut() = old_no_yield;
    *ctx.current_pulse_mut() = old_pulse;
    
    out.push_str(&ctx.alloca_out());
    out.push_str(&body_out);
    
    emit_fn_cleanup(ctx, func, &mut out, &local_vars, terminator, &ret_ty)?;
    
    if !ctx.no_verify {
        ctx.ownership_tracker.borrow().verify_leak_free(&ctx.z3_solver.borrow())?;
        ctx.malloc_tracker.borrow().verify()?;
    }
    
    ctx.ownership_tracker.replace(saved_ownership);
    ctx.malloc_tracker.replace(saved_malloc_tracker);
    ctx.arena_escape_tracker.replace(saved_arena_escape);
    
    *ctx.alloca_out_mut() = saved_alloca;

    if let Some(prev) = prev_func_lvn {
        ctx.emission.borrow_mut().global_lvn.set_current_function(prev);
    } else {
        ctx.emission.borrow_mut().global_lvn.clear_current_function();
    }
    
    out.push_str(&format!("  }}{}\n\n", loc_annotation));
    ctx.pop_solver();
    Ok(out)
}

fn check_early_returns(ctx: &CodegenContext, func: &crate::grammar::SaltFn) -> Result<Option<String>, String> {
    if let Some(hir_items) = ctx.get_hir_async_items(&func.name.to_string()) {
        return Ok(Some(crate::codegen::emit_hir::emit_hir_items(&hir_items)?));
    }
    if let Some(liveness) = ctx.get_liveness(&func.name.to_string()) {
        return Ok(Some(emit_async_fn(ctx, func, &liveness)?));
    }
    if crate::grammar::attr::has_attribute(&func.attributes, "shader") {
        return Ok(Some(ctx.with_lowering_ctx(|lctx| crate::codegen::shader::emit_shader_fn(lctx, func))?));
    }
    if ctx.external_decls().contains(&func.name.to_string()) && func.body.stmts.is_empty() {
        return Ok(Some(String::new()));
    }
    Ok(None)
}

fn check_generic_guards(ctx: &CodegenContext, func: &crate::grammar::SaltFn, fn_name: &str) -> Result<bool, String> {
    let has_unresolved_generics = fn_name.ends_with("_T_E") 
        || fn_name.ends_with("_T")
        || fn_name.contains("_T_") && !fn_name.contains("_Tensor_")
        || fn_name.contains("_Ptr_T")
        || fn_name.contains("_E_") && fn_name.contains("Result");
    
    if has_unresolved_generics {
        return Ok(true);
    }
    
    if let Some(ref generics) = func.generics {
        if !generics.params.is_empty() && ctx.current_type_map().is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn process_fn_arguments(ctx: &CodegenContext, func: &crate::grammar::SaltFn, local_vars: &mut std::collections::HashMap<String, (Type, crate::codegen::context::LocalKind)>, args_code: &mut Vec<String>) -> Result<(), String> {
    for arg in &func.args {
        let ty = if let Some(t) = &arg.ty {
            ctx.bridge_resolve_type(t)
        } else if arg.name == "self" {
             if let Some(self_ty) = &*ctx.current_self_ty() {
                 self_ty.clone()
             } else {
                 return Err("Found 'self' argument outside of impl block".to_string());
             }
        } else {
            return Err(format!("Argument '{}' missing type annotation", arg.name));
        };
        
        let arg_name = arg.name.to_string();
        let mlir_ty = ctx.resolve_mlir_type(&ty)?;
        let ssa_name = format!("%arg_{}", arg_name);
        
        let is_ptr = matches!(ty, Type::Reference(..) | Type::Owned(..) | Type::Fn(..) | Type::Pointer { .. });
        if is_ptr {
            ctx.control_flow.borrow_mut().register_arg_scope(&ssa_name);
        }
        
        let attrs = if is_ptr { " {llvm.noalias}" } else { "" };
        args_code.push(format!("%arg_{}: {}{}", arg_name, mlir_ty, attrs));
        local_vars.insert(arg_name.clone(), (ty.clone(), crate::codegen::context::LocalKind::SSA(ssa_name.clone())));
        
        if matches!(ty, Type::I32 | Type::I64 | Type::Usize) {
            let z3_var = ctx.mk_var(&arg_name);
            ctx.register_symbolic_int(ssa_name.clone(), z3_var);
        }
        
        if matches!(ty, Type::Pointer { .. } | Type::Reference(..) | Type::Owned(..)) {
            ctx.pointer_tracker.borrow_mut().mark_valid(&arg_name);
        }
    }
    Ok(())
}

fn build_fn_attributes(ctx: &CodegenContext, func: &crate::grammar::SaltFn, _fn_name: &str, ret_ty: &Type) -> String {
    let has_inline = func.attributes.iter().any(|a| a.name == "inline");
    let has_noinline = func.attributes.iter().any(|a| a.name == "noinline");
    let is_no_mangle = func.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" );
    
    let is_auto_leaf = !has_noinline && {
        let stmt_count = func.body.stmts.len();
        let is_small = stmt_count <= 2;
        let is_small_return = matches!(ret_ty, 
            Type::F32 | Type::F64 | Type::I8 | Type::I16 | Type::I32 | Type::I64 |
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::Bool | Type::Usize | Type::Unit |
            Type::Reference(..) | Type::Owned(..) | Type::Pointer { .. }
        );
        let has_no_io = !func.body.stmts.iter().any(|s| {
            let s_str = format!("{:?}", s);
            s_str.contains("print") || s_str.contains("open") || s_str.contains("write") || s_str.contains("mmap")
        });
        let is_not_main = func.name != "main";
        is_small && is_small_return && has_no_io && is_not_main
    };
    
    let is_generic_instantiation = !ctx.current_type_map().is_empty() || !ctx.current_generic_args().is_empty();
    let mut attr_dict = Vec::new();
    
    if is_generic_instantiation {
        attr_dict.push("linkage = #llvm.linkage<internal>".to_string());
    }

    if has_inline || has_noinline || is_no_mangle || is_auto_leaf {
        let mut pt_items = Vec::new();
        if has_inline || is_auto_leaf { pt_items.push("\"alwaysinline\"".to_string()); }
        if has_noinline { pt_items.push("\"noinline\"".to_string()); }
        if is_no_mangle {
             pt_items.push("[\"frame-pointer\", \"non-leaf\"]".to_string());
             let target_cpu = if ctx.lib_mode { "x86-64" } else { "apple-m4" };
             pt_items.push(format!("[\"target-cpu\", \"{}\"]", target_cpu));
             pt_items.push("[\"stack-alignment\", \"16\"]".to_string());
             attr_dict.push("llvm.emit_c_interface".to_string());
        }
        if !pt_items.is_empty() {
            attr_dict.push(format!("passthrough = [ {} ]", pt_items.join(", ")));
        }
    }
    
    if !attr_dict.is_empty() {
        format!(" attributes {{ {} }}", attr_dict.join(", "))
    } else {
        "".to_string()
    }
}

fn build_loc_annotation(ctx: &CodegenContext, func: &crate::grammar::SaltFn, fn_name: &str) -> String {
    if ctx.debug_info && !ctx.source_file.is_empty() {
        let span = func.name.span();
        let line = span.start().line;
        let col = span.start().column;
        let fn_display_name = fn_name.trim_start_matches('"').trim_end_matches('"');
        format!(
            " loc(fused<#llvm.di_subprogram<compileUnit = #di_compile_unit, \
             scope = #di_file, name = \"{}\", file = #di_file, line = {}, \
             scopeLine = {}, subprogramFlags = \"Definition\", \
             type = #di_subroutine_type>>[\"{}\": {} : {}])",
            fn_display_name, line, line, ctx.source_file, line, col
        )
    } else {
        String::new()
    }
}

fn promote_mutated_args(ctx: &CodegenContext, func: &crate::grammar::SaltFn, body_out: &mut String, local_vars: &mut std::collections::HashMap<String, (Type, crate::codegen::context::LocalKind)>) -> Result<(), String> {
    let mutated = ctx.mutated_vars().clone();
    let mut promotions = Vec::new();
    for arg in &func.args {
        let arg_name = arg.name.to_string();
        if arg.is_mut || mutated.contains(&arg_name) {
            if let Some((ty, crate::codegen::context::LocalKind::SSA(ssa_name))) = local_vars.get(&arg_name).cloned() {
                promotions.push((arg_name, ty, ssa_name));
            }
        }
    }
    for (arg_name, ty, ssa_name) in promotions {
        let mlir_ty = ctx.resolve_mlir_type(&ty)?;
        let alloca_name = format!("%mut_arg_{}", arg_name);
        ctx.emit_alloca(body_out, &alloca_name, &mlir_ty);
        body_out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", ssa_name, alloca_name, mlir_ty));
        local_vars.insert(arg_name, (ty, crate::codegen::context::LocalKind::Ptr(alloca_name)));
    }
    Ok(())
}

fn emit_requires_verification(ctx: &CodegenContext, func: &crate::grammar::SaltFn, body_out: &mut String, local_vars: &mut std::collections::HashMap<String, (Type, crate::codegen::context::LocalKind)>) -> Result<(), String> {
    // Collect parameter names constrained by requires clauses for Ptr<T> bounds proofs
    let mut req_params: Vec<String> = Vec::new();
    for req in &func.requires {
        collect_path_idents(req, &mut req_params);
    }
    crate::codegen::verification::loop_bounds::set_requires_params(req_params);

    for req in &func.requires {
        let proven = ctx.with_lowering_ctx(|lctx| {
            let sym_ctx = crate::codegen::verification::SymbolicContext::new(lctx.z3_ctx);
            let z3_result = crate::codegen::expr::translate_bool_to_z3(lctx, req, local_vars, &sym_ctx);
            if let Ok(z3_req) = z3_result {
                *lctx.total_checks += 1;
                lctx.z3_solver.push();
                lctx.z3_solver.assert(&z3_req.not());
                let result = lctx.z3_solver.check();
                lctx.z3_solver.pop(1);
                let is_proven = matches!(result, crate::z3_shim::SatResult::Unsat);
                if is_proven { *lctx.elided_checks += 1; }
                lctx.z3_solver.assert(&z3_req);
                is_proven
            } else {
                false
            }
        });

        if !proven && !ctx.no_verify {
            let (req_val, _) = ctx.with_lowering_ctx(|lctx| crate::codegen::expr::emit_expr(lctx, body_out, req, local_vars, Some(&Type::Bool)))?;
            let true_const = format!("%contract_true_{}", ctx.next_id());
            let violated = format!("%contract_violated_{}", ctx.next_id());
            body_out.push_str(&format!("    {} = arith.constant true\n", true_const));
            body_out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", violated, req_val, true_const));
            ctx.ensure_external_declaration("__salt_contract_violation", &[], &Type::Unit)?;
            body_out.push_str(&format!("    scf.if {} {{\n", violated));
            body_out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
            body_out.push_str("      scf.yield\n");
            body_out.push_str("    }\n");
        }
    }
    Ok(())
}

/// Recursively collect all Path identifiers from a syn::Expr (for requires clause analysis).
fn collect_path_idents(expr: &syn::Expr, out: &mut Vec<String>) {
    match expr {
        syn::Expr::Path(p) => {
            if let Some(ident) = p.path.get_ident() {
                out.push(ident.to_string());
            }
        }
        syn::Expr::Binary(b) => {
            collect_path_idents(&b.left, out);
            collect_path_idents(&b.right, out);
        }
        syn::Expr::Unary(u) => { collect_path_idents(&u.expr, out); }
        syn::Expr::Paren(p) => { collect_path_idents(&p.expr, out); }
        _ => {}
    }
}

fn emit_fn_cleanup(ctx: &CodegenContext, func: &crate::grammar::SaltFn, out: &mut String, local_vars: &std::collections::HashMap<String, (Type, crate::codegen::context::LocalKind)>, terminator: bool, ret_ty: &Type) -> Result<(), String> {
    if !terminator {
        ctx.pop_and_emit_cleanup(out)?;
        ctx.with_lowering_ctx(|lctx| crate::codegen::stmt::emit_cleanup_for_return(lctx, out, local_vars))?;
        
        if *ret_ty == Type::Unit {
            out.push_str("    func.return\n");
        } else if func.name == "main" && *ret_ty == Type::I32 {
            let c0 = format!("%c0_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.constant 0 : i32\n", c0));
            out.push_str(&format!("    func.return {} : i32\n", c0));
        } else {
            out.push_str("    llvm.unreachable\n");
        }
    } else {
        let _ = ctx.cleanup_stack_mut().pop();
    }
    Ok(())
}

pub fn pre_scan_workspace(ctx: &CodegenContext) -> Result<(), String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut root = current_dir.clone();
    
    // Find KeuOS project root
    for _ in 0..5 {
        if root.join("kernel").exists() || root.join("salt-front").exists() {
            break;
        }
        if let Some(parent) = root.parent() {
            root = parent.to_path_buf();
        } else {
            break;
        }
    }

    // Pass 1: Register all templates (Structs/Enums)
    scan_dir(ctx, &root, true)?;
    // Pass 2: Register signatures (Functions/Globals)
    scan_dir(ctx, &root, false)?;
    
    // Mark comptime as ready now that std discovery is complete
    // This enables Salt-native string prefix handlers to be used
    ctx.set_comptime_ready();
    
    Ok(())
}

fn scan_dir(ctx: &CodegenContext, dir: &std::path::Path, pass1: bool) -> Result<(), String> {
    if !dir.is_dir() { return Ok(()); }
    
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name == ".git" || name == "build" || name == "qemu_build" {
                continue;
            }
            scan_dir(ctx, &path, pass1)?;
        } else if path.extension().is_some_and(|ext| ext == "salt") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let processed = crate::preprocess(&content);
                if let Ok(file) = syn::parse_str::<SaltFile>(&processed) {
                    if pass1 {
                        register_templates(ctx, &file)?;
                    } else {
                        register_signatures(ctx, &file)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn register_templates(ctx: &CodegenContext, file: &SaltFile) -> Result<(), String> {
    let pkg_name = if let Some(pkg) = &file.package {
        Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>())
    } else {
        String::new()
    };

    // Derive the module package for Home registration
    let module_package = if let Some(pkg) = &file.package {
        pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(".")
    } else {
        String::new()
    };

    for item in &file.items {
        match item {
            Item::Struct(s) => {
                let mangled = if pkg_name.is_empty() { s.name.to_string() } else { Mangler::mangle(&[&pkg_name, &s.name.to_string()]) };
                let mut s_mangled = s.clone();
                s_mangled.name = syn::Ident::new(&mangled, s.name.span());
                ctx.struct_templates_mut().insert(mangled.clone(), s_mangled);
                // Register this struct's KeuOS Home
                ctx.register_type_home(mangled, module_package.clone());
            }
            Item::Enum(e) => {
                let mangled = if pkg_name.is_empty() { e.name.to_string() } else { Mangler::mangle(&[&pkg_name, &e.name.to_string()]) };
                let mut e_mangled = e.clone();
                e_mangled.name = syn::Ident::new(&mangled, e.name.span());
                ctx.enum_templates_mut().insert(mangled.clone(), e_mangled);
                // Register this enum's KeuOS Home
                ctx.register_type_home(mangled, module_package.clone());
            }
            Item::Trait(t) => {
                // Register this trait's KeuOS Home
                let trait_mangled = if pkg_name.is_empty() { t.name.to_string() } else { Mangler::mangle(&[&pkg_name, &t.name.to_string()]) };
                ctx.register_trait_home(trait_mangled, module_package.clone());
            }
            _ => {}
        }
    }
    Ok(())
}

fn register_signatures(ctx: &CodegenContext, file: &SaltFile) -> Result<(), String> {
    let pkg_name = if let Some(pkg) = &file.package {
        Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>())
    } else {
        String::new()
    };

    let old_pkg = ctx.current_package.borrow().clone();
    *ctx.current_package.borrow_mut() = file.package.clone();

    let old_imports = ctx.imports().clone();
    *ctx.imports_mut() = file.imports.clone();
    ctx.inject_self_imports(file);

    for item in &file.items {
        match item {
            Item::Fn(f) => register_fn_signature(ctx, f, &pkg_name)?,
            Item::ExternFn(ef) => register_extern_fn_signature(ctx, ef)?,
            Item::Concept(c) => register_concept_signature(ctx, c, &pkg_name)?,
            Item::Global(g) => register_global_signature(ctx, g, &pkg_name)?,
            Item::Const(c) => register_const_signature(ctx, c, &pkg_name)?,
            Item::Impl(imp) => register_impl_signatures(ctx, imp)?,
            _ => {}
        }
    }

    *ctx.imports_mut() = old_imports;
    *ctx.current_package.borrow_mut() = old_pkg;

    Ok(())
}

fn register_fn_signature(ctx: &CodegenContext, f: &crate::grammar::SaltFn, pkg_name: &str) -> Result<(), String> {
    let name = f.name.to_string();
    let mangled = if name == "main" { "main".to_string() }
                 else if pkg_name.is_empty() { name.clone() }
                 else { Mangler::mangle(&[pkg_name, &name]) };
    
    for attr in &f.attributes {
        if attr.name == "string_prefix" {
            if let Some(prefix) = &attr.string_arg {
                ctx.string_prefix_handlers_mut().insert(prefix.clone(), mangled.clone());
            }
        }
    }
    
    let ret = if let Some(rt) = &f.ret_type { resolve_type_safe(ctx, rt) } else { Type::Unit };
    let unknown_ty = crate::grammar::SynType::Other("UnknownSelf".to_string());
    let args = f.args.iter().map(|a| resolve_type_safe(ctx, a.ty.as_ref().unwrap_or(&unknown_ty))).collect();
    ctx.globals_mut().insert(mangled, Type::Fn(args, Box::new(ret)));
    Ok(())
}

fn register_extern_fn_signature(ctx: &CodegenContext, ef: &crate::grammar::ExternFnDecl) -> Result<(), String> {
    let name = ef.name.to_string();
    let mangled = name.clone();
    
    if ctx.external_decls().contains(&name) {
        return Ok(());
    }
    
    ctx.external_decls_mut().insert(name.clone());
                 
    let ret = if let Some(rt) = &ef.ret_type { resolve_type_safe(ctx, rt) } else { Type::Unit };
    let unknown_ty = crate::grammar::SynType::Other("UnknownSelf".to_string());
    let args: Vec<Type> = ef.args.iter().map(|a| resolve_type_safe(ctx, a.ty.as_ref().unwrap_or(&unknown_ty))).collect();
    ctx.globals_mut().insert(mangled.clone(), Type::Fn(args.clone(), Box::new(ret.clone())));

    let mut args_mlir = Vec::new();
    for arg in &args {
        if let Ok(mlir_ty) = ctx.resolve_mlir_type(arg) {
            args_mlir.push(mlir_ty);
        }
    }
    let ret_mlir = if ret == Type::Unit {
        "()".to_string()
    } else if let Ok(mlir_ty) = ctx.resolve_mlir_type(&ret) {
        mlir_ty
    } else {
        "()".to_string()
    };
    let decl_str = format!("  func.func private @{}({}) -> {}\n", 
        name, args_mlir.join(", "), ret_mlir);
    ctx.pending_func_decls_mut().insert(name.clone(), decl_str);

    let wrapper = crate::grammar::SaltFn {
        attributes: ef.attributes.clone(),
        is_pub: ef.is_pub,
        name: syn::Ident::new(&name, proc_macro2::Span::call_site()),
        generics: None,
        args: ef.args.clone(),
        ret_type: ef.ret_type.clone(),
        body: crate::grammar::SaltBlock { stmts: vec![] },
        requires: ef.requires.clone(),
        ensures: ef.ensures.clone(),
    };
    ctx.generic_impls_mut().insert(name.clone(), (wrapper, vec![]));
    Ok(())
}

fn register_concept_signature(ctx: &CodegenContext, c: &crate::grammar::SaltConcept, pkg_name: &str) -> Result<(), String> {
    if c.generics.is_none() {
        let name = c.name.to_string();
        let mangled = if pkg_name.is_empty() { name } else { Mangler::mangle(&[pkg_name, &name]) };
        
        let arg_ty = resolve_type_safe(ctx, &c.param_ty);
        let sig = Type::Fn(vec![arg_ty], Box::new(Type::Bool));
        ctx.globals_mut().insert(mangled, sig);
    }
    Ok(())
}

fn register_global_signature(ctx: &CodegenContext, g: &crate::grammar::GlobalDef, pkg_name: &str) -> Result<(), String> {
    let name = g.name.to_string();
    let mangled = if pkg_name.is_empty() { name }
                 else { format!("{}__{}", pkg_name, name) };
    let ty = resolve_type_safe(ctx, &g.ty);
    ctx.globals_mut().insert(mangled, ty);
    Ok(())
}

fn register_const_signature(ctx: &CodegenContext, c: &crate::grammar::ConstDef, pkg_name: &str) -> Result<(), String> {
    let name = c.name.to_string();
    let mangled = if pkg_name.is_empty() { name.clone() }
                 else { format!("{}__{}", pkg_name, name) };
    let ty = resolve_type_safe(ctx, &c.ty);
    ctx.globals_mut().insert(mangled.clone(), ty.clone());
    
    let mut eval = ctx.evaluator.borrow_mut();
    if let Ok(val) = eval.eval_expr(&c.value) {
        match &val {
            crate::evaluator::ConstValue::Integer(_) |
            crate::evaluator::ConstValue::Bool(_) |
            crate::evaluator::ConstValue::Float(_) |
            crate::evaluator::ConstValue::String(_) => {
                eval.constant_table.insert(mangled.clone(), val);
            }
            _ => {}
        }
    }
    Ok(())
}

fn register_impl_signatures(ctx: &CodegenContext, imp: &SaltImpl) -> Result<(), String> {
    if let SaltImpl::Methods { target_ty, methods, generics } = imp {
        let parsed_ty = resolve_type_safe(ctx, target_ty);
        let _target_name = match &parsed_ty {
            Type::Struct(name) | Type::Enum(name) => name.clone(),
            Type::Concrete(name, _) => name.clone(),
            _ => parsed_ty.mangle_suffix(),
        };
        
        let mut key = parsed_ty.to_key().ok_or_else(|| format!("Failed to derive TypeKey for impl target {}", _target_name))?;
        if generics.is_some() {
            key.specialization = None;
        }

        for m in methods {
            let current_imports = ctx.imports().clone();
            ctx.trait_registry_mut().register_simple(key.clone(), m.clone(), Some(parsed_ty.clone()), current_imports);
        }
    } else if let SaltImpl::Trait { trait_name: _, target_ty, methods, generics } = imp {
        let parsed_ty = resolve_type_safe(ctx, target_ty);
        
        let mut key = parsed_ty.to_key().unwrap_or_else(|| {
            crate::types::TypeKey { path: vec![], name: parsed_ty.mangle_suffix(), specialization: None }
        });
        
        if generics.is_some() {
            key.specialization = None;
        }

        for m in methods {
            let current_imports = ctx.imports().clone();
            ctx.trait_registry_mut().register_simple(key.clone(), m.clone(), Some(parsed_ty.clone()), current_imports);
        }
    }
    Ok(())
}



/// A non-panicking version of resolve_type for pre-scanning.
/// This version avoids any logic that triggers specialization (ensure_struct_exists).
fn resolve_type_safe(ctx: &CodegenContext, ty: &crate::grammar::SynType) -> Type {
    if let Some(parsed_ty) = crate::types::Type::from_syn(ty) {
        match parsed_ty {
            Type::Struct(name) => resolve_type_safe_struct(ctx, &name),
            Type::Enum(name) => {
                let segments = vec![name.clone()];
                let resolved_name = if let Some((pkg, item)) = ctx.bridge_resolve_package_prefix(&segments) {
                    if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) }
                } else {
                    name
                };
                let _ = ctx.ensure_enum_exists(&resolved_name, &[]);
                Type::Enum(resolved_name)
            },
            Type::Reference(inner, is_mut) => {
                if let crate::grammar::SynType::Reference(inner_syn, _) = ty {
                    Type::Reference(Box::new(resolve_type_safe(ctx, inner_syn)), is_mut)
                } else {
                    Type::Reference(inner, is_mut)
                }
            }
            Type::Concrete(base, params) => resolve_type_safe_concrete(ctx, &base, &params),
            _ => {
                if parsed_ty.is_numeric() || matches!(parsed_ty, Type::Bool | Type::Unit) {
                    ctx.bridge_resolve_type(ty)
                } else {
                    parsed_ty
                }
            }
        }
    } else {
        Type::Unit
    }
}

fn resolve_type_safe_struct(ctx: &CodegenContext, name: &str) -> Type {
    if name == "Self" {
        if let Some(self_ty) = ctx.current_self_ty().as_ref() { return self_ty.clone(); }
        return Type::Concrete("Unknown_Self".to_string(), vec![]);
    }
    let segments: Vec<String> = name.split("::").map(|s| s.to_string()).collect();
    let resolved_name = if let Some((pkg, item)) = ctx.bridge_resolve_package_prefix(&segments) {
        if item.is_empty() { pkg } else if pkg.is_empty() { item } else { Mangler::mangle(&[&pkg, &item]) }
    } else {
        if let Some(Type::Struct(self_name)) = ctx.current_self_ty().as_ref() {
            let self_short = self_name.rsplit("__").next().unwrap_or(self_name);
            if name == self_short || name == *self_name {
                let _ = ctx.ensure_struct_exists(self_name, &[]);
                return Type::Struct(self_name.clone());
            }
        }
        return Type::Concrete(format!("Unknown_{}", name), vec![]);
    };
    let _ = ctx.ensure_struct_exists(&resolved_name, &[]);
    Type::Struct(resolved_name)
}

fn resolve_type_safe_concrete(ctx: &CodegenContext, base: &str, params: &[Type]) -> Type {
    let segments: Vec<String> = base.split("::").map(|s| s.to_string()).collect();
    let resolved_base = if let Some((pkg, item)) = ctx.bridge_resolve_package_prefix(&segments) {
        if item.is_empty() { pkg } else { format!("{}__{}", pkg, item) }
    } else {
        base.to_string()
    };
    let resolved_params: Vec<Type> = params.iter().map(|p| {
        match p {
            Type::Struct(n) => {
                let segs = vec![n.clone()];
                if let Some((pkg, item)) = ctx.bridge_resolve_package_prefix(&segs) {
                    let fqn = if item.is_empty() { pkg } else { format!("{}__{}", pkg, item) };
                    Type::Struct(fqn)
                } else { p.clone() }
            },
            _ => p.clone()
        }
    }).collect();
    if ctx.enum_templates().contains_key(&resolved_base) {
        let _ = ctx.ensure_enum_exists(&resolved_base, &resolved_params);
    } else {
        let _ = ctx.ensure_struct_exists(&resolved_base, &resolved_params);
    }
    Type::Concrete(resolved_base, resolved_params)
}


