pub mod context;
pub mod phases;
pub mod abi;
pub mod type_bridge;
pub mod expr;
pub mod stmt;
pub mod intrinsics;
pub mod module_loader;
pub mod collector;
pub mod seeker;
pub mod tracer;
pub mod verification;
pub mod const_eval;
pub mod struct_deriver;
pub mod trait_registry;  // V4.0: Signature-aware method resolution
pub mod types;
pub mod interleaved_gen;  // V2.0 FFB: Fused Forward-Backward codegen
pub mod passes;           // V2.0 KeuOS: Pulse injection, yield injection, sync verification
pub mod generic_resolver; // Consolidated generic type resolution
pub mod shader;           // [FACET L1] Metal Shading Language codegen for @shader functions
pub mod emit_hir;         // [PHASE 11] HIR-to-MLIR emitter for async lowering Items
#[cfg(test)]
mod tests_ptr_and_comparison;
#[cfg(test)]
mod tests_generic_enum_match;
mod tests_result_monomorphization;
#[cfg(test)]
mod tests_bidir_inference;
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
use crate::codegen::context::{CodegenContext, GenericContextGuard};
use crate::codegen::stmt::emit_block;
use crate::codegen::module_loader::ModuleLoader;
use crate::common::mangling::Mangler;

use crate::types::Type;
use crate::registry::Registry;
use std::collections::{HashMap, HashSet};
    pub fn emit_mlir(file: &mut SaltFile, release_mode: bool, _registry: Option<&Registry>, _skip_scan: bool, no_verify: bool, disable_alias_scopes: bool, lib_mode: bool, sip_mode: bool, debug_info: bool, source_file: &str) -> Result<String, String> {
        let (mut loader, loader_registry) = load_modules(file)?;
        
        resolve_names(file, &mut loader)?;

        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        
        let mut ctx = CodegenContext::new(file, release_mode, Some(&loader_registry), &z3_ctx);

        initialize_context(&mut ctx, file, &loader, no_verify, disable_alias_scopes, lib_mode, sip_mode, debug_info, source_file);

        register_all_templates_and_signatures(&ctx, file, &loader)?;

        scan_definitions(&mut ctx, file, &loader)?;

        let call_graph_analyzer = run_call_graph_analysis(file, release_mode);

        run_pulse_analysis(&mut ctx, file, &call_graph_analyzer, release_mode);

        run_liveness_analysis(&mut ctx, file, release_mode);

        lower_state_machines(&mut ctx, file);

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
            if let Err(e) = loader.load_module(imp, &mut loader_registry) {
                eprintln!("Warning: Failed to load module '{}': {}", imp, e);
            }
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
        
        for (_, ast) in &loader.loaded_files {
            collect_globals(ast, &mut global_types);
        }
        collect_globals(file, &mut global_types);

        for (_, ast) in loader.loaded_files.iter_mut() {
            crate::codegen::phases::resolution::name_resolver::NameResolver::resolve_file(ast, &global_types);
        }
        crate::codegen::phases::resolution::name_resolver::NameResolver::resolve_file(file, &global_types);
        Ok(())
    }

    fn initialize_context<'a>(ctx: &mut CodegenContext<'a>, file: &SaltFile, loader: &ModuleLoader, no_verify: bool, disable_alias_scopes: bool, lib_mode: bool, sip_mode: bool, debug_info: bool, source_file: &str) {
        let mut all_files = Vec::new();
        for (_, ast) in &loader.loaded_files {
            all_files.push(ast);
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
        for (_, ast) in &loader.loaded_files {
            register_templates(ctx, ast)?;
        }
        register_templates(ctx, file)?;

        for (_, ast) in &loader.loaded_files {
            register_signatures(ctx, ast)?;
        }
        register_signatures(ctx, file)?;
        Ok(())
    }

    fn scan_definitions(ctx: &mut CodegenContext, file: &SaltFile, loader: &ModuleLoader) -> Result<(), String> {
        let dep_order = loader.get_compilation_order().map_err(|e| e)?;
        ctx.init_registry_definitions();
        for ns in &dep_order {
            if let Some(f) = loader.loaded_files.get(ns) {
                 ctx.scan_defs_from_file(f, false)?;
            }
        }
        ctx.scan_defs_from_file(file, true)?;
        Ok(())
    }

    fn run_call_graph_analysis(file: &SaltFile, release_mode: bool) -> passes::call_graph::CallGraphAnalyzer {
        use passes::call_graph::CallGraphAnalyzer;
        let mut cg = CallGraphAnalyzer::new();
        let call_graph_analysis = cg.analyze(file);

        if !release_mode {
            let blocking: Vec<&str> = call_graph_analysis.fn_attributes.iter()
                .filter(|(_, a)| a.is_blocking)
                .map(|(n, _)| n.as_str())
                .collect();
            if !blocking.is_empty() {
                eprintln!("[KeuOS] Blocking functions detected: {:?}", blocking);
            }
        }

        for v in &call_graph_analysis.violations {
            eprintln!(
                "[KeuOS] WARNING: @pulse function '{}' transitively calls blocking '{}'\n  chain: {}",
                v.pulse_fn, v.blocking_fn, v.call_chain.join(" → ")
            );
        }
        cg
    }

    fn run_pulse_analysis(ctx: &mut CodegenContext, file: &SaltFile, call_graph_analyzer: &passes::call_graph::CallGraphAnalyzer, release_mode: bool) {
        use passes::pulse_injection::PulseInjectionContext;
        let mut pulse_ctx = PulseInjectionContext::new();
        pulse_ctx.analyze_with_call_graph(file, call_graph_analyzer);
        
        if !release_mode && !pulse_ctx.pulse_info.is_empty() {
            eprintln!("[KeuOS] Found {} @pulse functions:", pulse_ctx.pulse_info.len());
            for info in &pulse_ctx.pulse_info {
                eprintln!("  - {} @ {}Hz (Tier {})", info.name, info.frequency_hz, info.tier);
            }
        }
        
        for info in pulse_ctx.pulse_info {
            ctx.register_pulse_function(&info.name, info.frequency_hz, info.tier);
        }
    }

    fn run_liveness_analysis(ctx: &mut CodegenContext, file: &SaltFile, release_mode: bool) {
        use passes::liveness::CrossYieldAnalyzer;

        for item in &file.items {
            if let Item::Fn(func) = item {
                let mut analyzer = CrossYieldAnalyzer::new();
                let result = analyzer.analyze(func);
                if result.needs_transform {
                    let name = func.name.to_string();
                    if !release_mode {
                        eprintln!(
                            "[KeuOS] @yielding function '{}': {} yield points, {} frame members",
                            name, result.yield_points.len(), result.frame_members.len()
                        );
                    }
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
                Item::Fn(f) => {
                    let is_no_mangle = f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" );
                    let is_pub = f.is_pub;
                    if is_no_mangle || is_pub {
                        if let Some(task) = self.create_main_task(&f.name.to_string()) {
                            tasks.push(task);
                        }
                    }
                }
                Item::Impl(imp) => {
                    if let SaltImpl::Methods { target_ty, methods, generics } = imp {
                        if generics.is_none() {
                            if let Some(parsed_ty) = crate::types::Type::from_syn(target_ty) {
                                let target_name_full = self.bridge_resolve_codegen_type(&parsed_ty).mangle_suffix();
                                let pkg_path = if let Some(pkg) = &self.file.borrow().package {
                                    pkg.name.iter().map(|id| id.to_string()).collect()
                                } else {
                                    vec![]
                                };
                                for m in methods {
                                    if m.is_pub && m.generics.is_none() {
                                        let m_name_str = m.name.to_string();
                                        let mangled_name = Mangler::mangle(&[target_name_full.as_str(), m_name_str.as_str()]);
                                        tasks.push(crate::codegen::collector::MonomorphizationTask {
                                            identity: crate::types::TypeKey { path: pkg_path.clone(), name: m_name_str, specialization: None },
                                            mangled_name,
                                            func: m.clone(),
                                            concrete_tys: vec![],
                                            self_ty: Some(parsed_ty.clone()),
                                            imports: CodegenContext::compute_full_imports(&self.file.borrow()),
                                            type_map: std::collections::BTreeMap::new(),
                                        });
                                    }
                                }
                            }
                        }
                    } else if let SaltImpl::Trait { trait_name: _, target_ty, methods, generics } = imp {
                        if generics.is_none() {
                            if let Some(parsed_ty) = crate::types::Type::from_syn(target_ty) {
                                let target_name_full = self.bridge_resolve_codegen_type(&parsed_ty).mangle_suffix();
                                let pkg_path = if let Some(pkg) = &self.file.borrow().package {
                                    pkg.name.iter().map(|id| id.to_string()).collect()
                                } else {
                                    vec![]
                                };
                                for m in methods {
                                    if m.generics.is_none() {
                                        let m_name_str = m.name.to_string();
                                        let mangled_name = Mangler::mangle(&[target_name_full.as_str(), m_name_str.as_str()]);
                                        tasks.push(crate::codegen::collector::MonomorphizationTask {
                                            identity: crate::types::TypeKey { path: pkg_path.clone(), name: m_name_str, specialization: None },
                                            mangled_name,
                                            func: m.clone(),
                                            concrete_tys: vec![],
                                            self_ty: Some(parsed_ty.clone()),
                                            imports: CodegenContext::compute_full_imports(&self.file.borrow()),
                                            type_map: std::collections::BTreeMap::new(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        for task in tasks {
            self.hydrate_specialization(task)?;
        }
        Ok(())
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
            out.push_str(&emission.decl_out);
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
                    out.push_str(&format!("    llvm.store {}, {} : !llvm.ptr, !llvm.ptr\n",
                        target_ptr, format!("%global_{}", patch_id)));
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

    /// [FORMAL SHADOW] Verify all struct alignment constraints:
    ///   - @atomic fields: 16-byte alignment for cmpxchg16b
    ///   - @align(N) fields: N-byte alignment (cache-line isolation)
    ///   - @atomic structs: stride alignment (sizeof % 16 == 0)
    ///   - @packed structs: zero implicit padding
    /// Uses Z3 integer modular arithmetic to prove alignment is invariant.


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
                byte_offset += field_ty.size_of(&*struct_reg);
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
                eprintln!("[Formal Shadow] Z3 PROVED: @atomic field '{}' in struct '{}' is 16-byte aligned at offset {} (z3_aligned)", f.name, s_name, byte_offset);
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
                    eprintln!("[Formal Shadow] Z3 PROVED: @align({}) field '{}' in struct '{}' is {}-byte aligned at offset {} (z3_align_verified)", n, f.name, s_name, n, byte_offset);
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
                eprintln!("[Formal Shadow] Z3 PROVED: @atomic struct '{}' has size {} bytes, which is 16-byte stride-safe for cmpxchg16b arrays (z3_stride_aligned)", s_name, byte_offset);
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
            resolved_types.iter().map(|ty: &crate::types::Type| ty.size_of(&*struct_reg)).collect()
        };

        let mut abi_offset: usize = 0;
        let mut max_align: usize = 1;

        for &field_size in &field_sizes {
            let field_align = field_size.min(8).max(1);
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
                eprintln!("[Formal Shadow] Z3 PROVED: @packed struct '{}' has {} bytes with ZERO implicit padding (z3_packed_verified)", s_name, abi_total);
                Ok(())
            }
            _ => Err(format!("[Formal Shadow] PACKED VIOLATION: @packed struct '{}' has implicit padding. ABI layout = {} bytes, but raw field sum = {} bytes ({} bytes of hidden padding). The Z3 SMT solver proved this layout violates the zero-padding contract. Fix: reorder fields or add explicit padding fields to eliminate gaps.", s_name, abi_total, unpadded_sum, abi_total - unpadded_sum))
        }
    }
    
    fn create_main_task(&self, name: &str) -> Option<crate::codegen::collector::MonomorphizationTask> {
        // 1. Check current file
        for item in &self.file.borrow().items {
            if let Item::Fn(f) = item {
                if f.name.to_string() == name {
                    let pkg_path = if let Some(pkg) = &self.file.borrow().package {
                        pkg.name.iter().map(|id| id.to_string()).collect()
                    } else {
                        vec![]
                    };
                    
                    // [KEUOS FIX] In lib mode, mangle function names with package prefix
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

    // Legacy method - kept to satisfy trait if any, but effectively dead or helper
    #[allow(dead_code)]
    fn absorb_pending_to_registry(&self) {
        // No-op in lazy flow
    }
    
    // finalize_module removed/merged into drive_codegen

    fn emit_structure_defs(&self, out: &mut String) {
        // [KEY-EXTRACTION PATTERN] Clone registry data into owned collections
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
        let mut adj: HashMap<crate::types::TypeKey, Vec<crate::types::TypeKey>> = HashMap::new();
        
        for key in &all_keys {
            let mut deps = Vec::new();
            
            if let Some(info) = struct_map.get(key) {
                for field_ty in &info.field_order {
                    self.collect_dependencies(field_ty, &mut deps);
                }
            }
            // Enums: no dependencies (opaque byte-array payload)
            adj.insert(key.clone(), deps);
        }
        
        // 2. Topological Sort (DFS Post-Order)
        let mut sorted_keys = Vec::new();
        let mut temp_mark = HashSet::new();
        let mut perm_mark = HashSet::new();
        
        // Deterministic iteration for stability
        let mut sorted_starts = all_keys.clone();
        for _k in &all_keys {
        }
        sorted_starts.sort_by(|a, b| a.mangle().cmp(&b.mangle()));
        
        for key in &sorted_starts {
            self.topo_visit(key, &adj, &mut temp_mark, &mut perm_mark, &mut sorted_keys);
        }
        
        // 3. Emit in Sorted Order (no Ref guards alive — safe to call resolve_mlir_storage_type)
        let enum_names: HashSet<String> = enum_map.values()
            .map(|e| e.name.clone()).collect();
        let mut emitted_canonical_aliases: HashSet<String> = HashSet::new();
        for key in sorted_keys {
            let _mangled = key.mangle();
            if let Some(info) = struct_map.get(&key) {
                // [GHOST STRUCT FILTER] Skip struct type aliases whose fields contain
                // unresolved generic parameters.
                let has_ghost_fields = info.field_order.iter().any(|ty| {
                    !ty.is_fully_concrete(&struct_map, &enum_names)
                });
                if has_ghost_fields {
                    continue;
                }

                let mut type_str = format!("!llvm.struct<\"{}\", (", info.name);
                for (i, ty) in info.field_order.iter().enumerate() {
                    if i > 0 { type_str.push_str(", "); }
                    match self.resolve_mlir_storage_type(&ty) {
                        Ok(s) => type_str.push_str(&s),
                        Err(_) => type_str.push_str("!llvm.ptr"),
                    }
                }
                type_str.push_str(")>");
                out.push_str(&format!("!struct_{} = {}\n", info.name, type_str));
                
                // [CANONICAL ALIAS] Emit canonical alias for all namespace-prefixed types
                let canonical_name = Type::Struct(info.name.clone()).to_canonical_name();

                if canonical_name != info.name && !emitted_canonical_aliases.contains(&canonical_name) {
                    emitted_canonical_aliases.insert(canonical_name.clone());
                    out.push_str(&format!("!struct_{} = !struct_{}\n", canonical_name, info.name));
                }
            } else if let Some(info) = enum_map.get(&key) {
                // Emit Enum Definition
                let mut type_str = format!("!llvm.struct<\"{}\", (i32", info.name);
                if info.max_payload_size > 0 {
                    type_str.push_str(&format!(", !llvm.array<{} x i8>", info.max_payload_size));
                }
                type_str.push_str(")>");
                out.push_str(&format!("!struct_{} = {}\n", info.name, type_str));

                // [CANONICAL ALIAS] Emit canonical alias for all namespace-prefixed enums
                let canonical_name = Type::Enum(info.name.clone()).to_canonical_name();
                if canonical_name != info.name && !emitted_canonical_aliases.contains(&canonical_name) {
                    emitted_canonical_aliases.insert(canonical_name.clone());
                    out.push_str(&format!("!struct_{} = !struct_{}\n", canonical_name, info.name));
                }
            }
        }

        // [SENTINEL] Always emit StringView type alias.
        let sv_name = "std__core__str__StringView";
        let sv_already_emitted = struct_map.values().any(|info| info.name == sv_name);
        if !sv_already_emitted {
            out.push_str(&format!(
                "!struct_{} = !llvm.struct<\"{}\", (!llvm.ptr, i64)>\n",
                sv_name, sv_name
            ));
        }
    }

    fn collect_dependencies(&self, ty: &Type, deps: &mut Vec<crate::types::TypeKey>) {
        // if ty.k_is_ptr_type() { return; } // Pointers break dependencies - REMOVED for Ptr struct info
        match ty {
            Type::Struct(name) | Type::Enum(name) => {
                // Find Key from Mangled Name (Reverse Lookup or assume we can derive it?)
                // Registry is keyed by TypeKey. We have the mangled name.
                // We need to match it to a Key.
                // Only if it exists in struct_registry.
                if let Some((k, _)) = self.struct_registry().iter().find(|(_, v)| v.name == *name) {
                    deps.push(k.clone());
                } else if let Some((k, _)) = self.enum_registry().iter().find(|(_, v)| v.name == *name) {
                    deps.push(k.clone());
                }
            },
            Type::Concrete(_base, _params) => {
                 // Should be resolved to mangled name by now in storage type?
                 // But here we are analyzing Type structure from registry which might store Concrete.
                 // Actually expand_template_structure stores Concrete types in fields?
                 // Most likely fields are resolved to Type::Struct(mangled) or Type::Concrete(resolved).
                 // We need to find the registry entry.
                 // For Concrete, we construct a key.
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
            // If we found a cycle via non-pointers, it's an Infinite Size type (compile error usually).
            // We ignore for now and proceed.
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


#[allow(dead_code)]
fn emit_item(ctx: &CodegenContext, item: &Item) -> Result<String, String> {
    match item {
        Item::Fn(f) => {
            if f.generics.is_none() {
                emit_fn(ctx, f, None)
            } else {
                Ok(String::new())
            }
        }
        Item::Impl(i) => emit_impl(ctx, i),
        Item::ExternFn(e) => emit_extern_fn(ctx, e),
        Item::Const(c) => {
            let mut out = String::new();
            ctx.bridge_emit_const(&mut out, c)?;
            Ok(out)
        }
        Item::Global(g) => {
            let mut out = String::new();
            ctx.bridge_emit_global_def(&mut out, g)?;
            Ok(out)
        }
        Item::Concept(c) => emit_concept(ctx, c),
        Item::Trait(t) => emit_trait(ctx, t),  // [V4.0] Trait definitions
        _ => Ok(String::new()),
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

/// [V4.0] Emit a trait definition - registers trait in TraitRegistry
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
        args_code.push(ctx.resolve_mlir_type(&ty)?);
    }
    
    // Extern functions always use their original C symbol name (never mangle)
    let name = decl.name.to_string();
    
    ctx.external_decls_mut().insert(name.clone());
    
    let ret_ty = if let Some(rt) = &decl.ret_type { ctx.bridge_resolve_type(rt) } else { Type::Unit };

    let ret_part = if ret_ty == Type::Unit { "()".to_string() } else { 
        ctx.resolve_mlir_type(&ret_ty)?
    };

    Ok(format!("  func.func private @{}({}) -> {}\n", name, args_code.join(", "), ret_part))
}

/// [KEUOS V2.0] Emit a @yielding function as a state machine.
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

    let old_no_yield = *ctx.no_yield();
    let old_pulse = ctx.current_pulse().clone();
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

    let terminator = ctx.with_lowering_ctx(|lctx| crate::codegen::stmt::emit_block(lctx, &mut body_out, &func.body.stmts, &mut local_vars))?;
    
    ctx.emission.borrow_mut().in_fast_math_fn = old_fast_math_fn;
    ctx.emission.borrow_mut().in_trusted_fn = old_trusted_fn;
    ctx.emission.borrow_mut().in_dynamic_check_fn = old_dynamic_check_fn;
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
        } else if arg.name.to_string() == "self" {
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
        let is_not_main = func.name.to_string() != "main";
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
    let has_trusted = func.attributes.iter().any(|a| a.name == "trusted");
    if !has_trusted {
        for req in &func.requires {
            let proven = ctx.with_lowering_ctx(|lctx| {
                let sym_ctx = crate::codegen::verification::SymbolicContext::new(lctx.z3_ctx);
                let z3_result = crate::codegen::expr::translate_bool_to_z3(lctx, req, local_vars, &sym_ctx);
                if let Ok(z3_req) = z3_result {
                    lctx.z3_solver.push();
                    lctx.z3_solver.assert(&z3_req.not());
                    let result = lctx.z3_solver.check();
                    lctx.z3_solver.pop(1);
                    let is_proven = matches!(result, crate::z3_shim::SatResult::Unsat);
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
    }
    Ok(())
}

fn emit_fn_cleanup(ctx: &CodegenContext, func: &crate::grammar::SaltFn, out: &mut String, local_vars: &std::collections::HashMap<String, (Type, crate::codegen::context::LocalKind)>, terminator: bool, ret_ty: &Type) -> Result<(), String> {
    if !terminator {
        ctx.pop_and_emit_cleanup(out)?;
        ctx.with_lowering_ctx(|lctx| crate::codegen::stmt::emit_cleanup_for_return(lctx, out, local_vars))?;
        
        if *ret_ty == Type::Unit {
            out.push_str("    func.return\n");
        } else if func.name.to_string() == "main" && *ret_ty == Type::I32 {
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


#[allow(dead_code)]
fn emit_impl(ctx: &CodegenContext, imp: &SaltImpl) -> Result<String, String> {
    let mut out = String::new();
    match imp {
        SaltImpl::Methods { target_ty, methods, generics: _ } => {
            let parsed_ty = crate::types::Type::from_syn(target_ty).ok_or_else(|| "Failed to parse type from syntax in impl block".to_string())?;
            let target_name_full = ctx.bridge_resolve_codegen_type(&parsed_ty).mangle_suffix();
            let _target_base_name = match &parsed_ty {
                Type::Struct(name) | Type::Enum(name) => name.clone(),
                Type::Concrete(name, _) => name.clone(),
                _ => target_name_full.clone(),
            };
            
            // Set current self type for methods
            let old_self = ctx.current_self_ty().clone();
            *ctx.current_self_ty_mut() = Some(parsed_ty.clone());
            
            for m in methods {
                let key = parsed_ty.to_key().ok_or_else(|| format!("Failed to derive TypeKey for impl target {}", target_name_full))?;
                // [V4.0 KEUOS] Register via TraitRegistry with signature extraction
                ctx.trait_registry_mut().register_simple(key, m.clone(), Some(parsed_ty.clone()), ctx.imports().clone());
                // Only emit immediately if NOT a generic struct/enum and NOT a generic method
                if (parsed_ty.is_numeric() || matches!(parsed_ty, Type::Bool | Type::Unit)) || (!matches!(parsed_ty, Type::Concrete(..)) && m.generics.is_none()) {
                    let m_name_str = m.name.to_string();
                    let mangled_name = Mangler::mangle(&[target_name_full.as_str(), m_name_str.as_str()]);
                    out.push_str(&emit_fn(ctx, m, Some(mangled_name))?);
                }
            }
            
            // Restore
            *ctx.current_self_ty_mut() = old_self;
        }
        // [COUNCIL V2] Trait impl blocks — emit method bodies (e.g. impl Display for Point { fn fmt })
        SaltImpl::Trait { trait_name: _, target_ty, methods, generics: _ } => {
            let parsed_ty = crate::types::Type::from_syn(target_ty).ok_or_else(|| "Failed to parse type from syntax in trait impl block".to_string())?;
            let target_name_full = ctx.bridge_resolve_codegen_type(&parsed_ty).mangle_suffix();
            
            // Set current self type for methods
            let old_self = ctx.current_self_ty().clone();
            *ctx.current_self_ty_mut() = Some(parsed_ty.clone());
            
            for m in methods {
                let key = parsed_ty.to_key().ok_or_else(|| format!("Failed to derive TypeKey for trait impl target {}", target_name_full))?;
                ctx.trait_registry_mut().register_simple(key, m.clone(), Some(parsed_ty.clone()), ctx.imports().clone());
                // Emit the method with mangled name: TypeName__method
                if !matches!(parsed_ty, Type::Concrete(..)) || m.generics.is_none() {
                    let m_name_str = m.name.to_string();
                    let mangled_name = Mangler::mangle(&[target_name_full.as_str(), m_name_str.as_str()]);
                    out.push_str(&emit_fn(ctx, m, Some(mangled_name))?);
                }
            }
            
            // Restore
            *ctx.current_self_ty_mut() = old_self;
        }
        _ => {}
    }
    Ok(out)
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
    
    // [V4.0] Mark comptime as ready now that std discovery is complete
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
        } else if path.extension().map_or(false, |ext| ext == "salt") {
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

    // [KEUOS V7.0] Derive the module package for Home registration
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
                // [KEUOS V7.0] Register this struct's KeuOS Home
                ctx.register_type_home(mangled, module_package.clone());
            }
            Item::Enum(e) => {
                let mangled = if pkg_name.is_empty() { e.name.to_string() } else { Mangler::mangle(&[&pkg_name, &e.name.to_string()]) };
                let mut e_mangled = e.clone();
                e_mangled.name = syn::Ident::new(&mangled, e.name.span());
                ctx.enum_templates_mut().insert(mangled.clone(), e_mangled);
                // [KEUOS V7.0] Register this enum's KeuOS Home
                ctx.register_type_home(mangled, module_package.clone());
            }
            Item::Trait(t) => {
                // [KEUOS V7.0] Register this trait's KeuOS Home
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
        if let Ok(mlir_ty) = ctx.resolve_mlir_type(&arg) {
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
            Type::Struct(name) => {
                 // TOP MINDS: Check for 'Self' keyword - resolve to current impl target
                 if name == "Self" {
                     if let Some(self_ty) = ctx.current_self_ty().as_ref() {
                         return self_ty.clone();
                     }
                     eprintln!("CRITICAL: Use of 'Self' outside of an implementation block");
                     return Type::Concrete("Unknown_Self".to_string(), vec![]);
                 }
                 
                 // [CROSS-MODULE STRUCT] Split qualified names like "addr::PhysAddr" into
                 // segments ["addr", "PhysAddr"] so bridge_resolve_package_prefix can match
                 // the module alias against the import table.
                 let segments: Vec<String> = name.split("::").map(|s| s.to_string()).collect();
                 // RESOLVE TO FQN
                 let resolved_name = if let Some((pkg, item)) = ctx.bridge_resolve_package_prefix(&segments) {
                     let r = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { Mangler::mangle(&[&pkg, &item]) };
                     r
                 } else {
                     // TOP MINDS: Check if this is the impl target struct (Self-referential scope injection)
                     // This handles cases like `impl Point { fn sum(self: &Point) }` where Point isn't imported
                     if let Some(self_ty) = ctx.current_self_ty().as_ref() {
                         if let Type::Struct(self_name) = self_ty {
                             let self_short = self_name.rsplit("__").next().unwrap_or(self_name);
                             if name == self_short || name == *self_name {
                                 let _ = ctx.ensure_struct_exists(self_name, &[]);
                                 return Type::Struct(self_name.clone());
                             }
                         }
                     }
                     eprintln!("CRITICAL: resolve_type_safe failed to resolve '{}'. Imports: {:?}", name, ctx.imports().iter().map(|i| i.alias.as_ref().map(|a| a.to_string()).unwrap_or("?".to_string())).collect::<Vec<_>>());
                     return Type::Concrete(format!("Unknown_{}", name), vec![]);
                 };
                 
                 let _ = ctx.ensure_struct_exists(&resolved_name, &[]); 
                 Type::Struct(resolved_name)
            },
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
                // Safely recurse for references
                if let crate::grammar::SynType::Reference(inner_syn, _) = ty {
                    Type::Reference(Box::new(resolve_type_safe(ctx, inner_syn)), is_mut)
                } else {
                    Type::Reference(inner, is_mut)
                }
            }
            Type::Concrete(base, params) => {
                 // [CROSS-MODULE STRUCT] Split qualified names like "addr::PhysAddr" into segments
                 let segments: Vec<String> = base.split("::").map(|s| s.to_string()).collect();
                 let resolved_base = if let Some((pkg, item)) = ctx.bridge_resolve_package_prefix(&segments) {
                     if item.is_empty() { pkg } else { format!("{}__{}", pkg, item) }
                 } else {
                     base
                 };
                 
                 // Recurse params? resolve_type_safe doesn't take params list in signature but returns Type.
                 // Need to map params.
                 // But params in Type::Concrete are already Types.
                 // We should resolve them too!
                 let resolved_params: Vec<Type> = params.iter().map(|p| {
                     // We don't have the original syn for params here easily unless we fetch from ty...
                     // But Type::Concrete params are Types. We can't map Type -> Syn -> Type.
                     // But we can recurse on Type if we had a "resolve_type_safe_inner(Type)".
                     // For now, assume params are primitives or simple? 
                     // Or better: resolve_package_prefix logic on Type::Struct params.
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
                 
                 // Check if it's an Enum Template
                 if ctx.enum_templates().contains_key(&resolved_base) {
                     let _ = ctx.ensure_enum_exists(&resolved_base, &resolved_params);
                 } else {
                     let _ = ctx.ensure_struct_exists(&resolved_base, &resolved_params);
                 }
                 
                 Type::Concrete(resolved_base, resolved_params)
            },
            _ => {
                // Primitives and other simple types are safe to resolve fully
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

// CV-3 FIX: Removed unsafe transmute.
// Instead of transmuting to extend lifetime, we now:
// 1. Store the package as an owned clone (not a reference to the file)
// 2. This function is only called where the file lifetime is properly bounded
// 3. The caller is responsible for ensuring the file outlives the context
//
// NOTE: This function is marked dead_code and only used in scan_dependencies_in_file.
// For a full Arena-Owner refactor, SaltFile would be allocated in a typed arena,
// but this minimal fix removes the UB while preserving existing behavior.
#[allow(dead_code)]
fn setup_file_context_safe(ctx: &CodegenContext, f: &SaltFile) {
    // We don't store the file reference anymore - instead we just use it for reading
    // The context already has a file reference from its construction
    ctx.current_package.replace(f.package.clone());
    
    ctx.imports_mut().clear();
    
    // Inject self-imports for local resolution
    ctx.inject_self_imports(f);
    
    // Inject File-Specific Imports
    ctx.scan_imports_from_file(f);
}

#[allow(dead_code)]
fn scan_dependencies_in_file(ctx: &CodegenContext, f: &SaltFile) -> Result<(), String> {

    setup_file_context_safe(ctx, f);
    let _pkg_prefix = if let Some(pkg) = f.package.as_ref() {
        Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()) + "__"
    } else { String::new() };

    for item in &f.items {
         match item {
             // Structs/Enums are already registered templates, but concrete fields might trigger deps?
             // No, fields are resolved on instantiation.
             Item::Fn(func) => {
                 if func.generics.is_none() {
                     // Scan signature: Args
                     for arg in &func.args {
                         if let Some(ty) = &arg.ty {
                             ctx.bridge_resolve_type(ty);
                         }
                     }
                     // Scan signature: Return
                     if let Some(ret) = &func.ret_type {
                         ctx.bridge_resolve_type(ret);
                     }
                     scan_types_in_fn(ctx, func)?;
                 }
             }
             Item::Impl(imp) => {
                 if let SaltImpl::Methods { target_ty, methods, generics } = imp {
                     if generics.is_some() { 

                        continue; 
                    }
                    // Also check if target_ty implies generics (e.g. impl Vec<T>)
                    let parsed_ty = crate::types::Type::from_syn(target_ty).ok_or_else(|| "Failed to parse type from syntax in impl block during scan".to_string())?;
                    let substituted = crate::codegen::type_bridge::substitute_generics(&ctx.current_type_map(), &parsed_ty);
                    if substituted.has_generics() {

                         continue;
                    }

                     let parsed_ty = crate::types::Type::from_syn(target_ty).ok_or_else(|| "Failed to parse type from syntax in impl block during scan".to_string())?;
                     // Resolve target (might trigger specialization!)
                     let resolved_target = ctx.bridge_resolve_codegen_type(&parsed_ty);
                     
                     let old_self = ctx.current_self_ty().clone();
                     *ctx.current_self_ty_mut() = Some(resolved_target.clone());
                     
                     // Scan methods
                     for m in methods {
                         if m.generics.is_none() {
                             for arg in &m.args {
                                 if let Some(ty) = &arg.ty {
                                     ctx.bridge_resolve_type(ty);
                                 }
                             }
                             if let Some(ret) = &m.ret_type {
                                 ctx.bridge_resolve_type(ret);
                             }
                             scan_types_in_fn(ctx, m)?;
                         }
                     }
                     
                     *ctx.current_self_ty_mut() = old_self;
                 }
             }
             _ => {}
         }
    }
    Ok(())
}

pub fn scan_types_in_fn(ctx: &CodegenContext, func: &SaltFn) -> Result<(), String> {

    // Scan arguments
    for arg in &func.args {
        if let Some(ty) = &arg.ty {
            ctx.bridge_resolve_type(ty);
        }
    }
    
    // Scan return type
    if let Some(ret) = &func.ret_type {
        ctx.bridge_resolve_type(ret);
    }

    // Scan body for types (Locals, Casts, SizeOfs, etc.)
    for stmt in &func.body.stmts {
        scan_stmt(ctx, stmt)?;
    }
    Ok(())
}



fn scan_stmt(ctx: &CodegenContext, stmt: &crate::grammar::Stmt) -> Result<(), String> {
    match stmt {
        crate::grammar::Stmt::Syn(s) => match s {
            syn::Stmt::Local(l) => {
                if let syn::Pat::Type(pt) = &l.pat {
                    ctx.bridge_resolve_type(&crate::grammar::SynType::from_std(*pt.ty.clone()).map_err(|e| e.to_string())?);
                }
                if let Some(init) = &l.init { scan_expr(ctx, &init.expr)?; }
                Ok(())
            }
            syn::Stmt::Expr(e, _) => scan_expr(ctx, e),
            _ => Ok(())
        },
        crate::grammar::Stmt::If(f) => {
            scan_expr(ctx, &f.cond)?;
            for s in &f.then_branch.stmts { scan_stmt(ctx, s)?; }
             if let Some(eb) = &f.else_branch {
                match eb.as_ref() {
                    crate::grammar::SaltElse::Block(b) => { for s in &b.stmts { scan_stmt(ctx, s)?; } }
                    crate::grammar::SaltElse::If(nested) => { scan_stmt(ctx, &crate::grammar::Stmt::If(nested.as_ref().clone()))?; }
                }
            }
            Ok(())
        },
        crate::grammar::Stmt::While(w) => {
            scan_expr(ctx, &w.cond)?;
             for s in &w.body.stmts { scan_stmt(ctx, s)?; }
             Ok(())
        },
        crate::grammar::Stmt::For(f) => {
            if let syn::Expr::Range(r) = &f.iter {
                 if let Some(s) = &r.start { scan_expr(ctx, s)?; }
                 if let Some(e) = &r.end { scan_expr(ctx, e)?; }
            }
            for s in &f.body.stmts { scan_stmt(ctx, s)?; }
            Ok(())
        }
        crate::grammar::Stmt::Expr(e, _) => scan_expr(ctx, e),
        crate::grammar::Stmt::Return(e) => {
            if let Some(expr) = e { scan_expr(ctx, expr)?; }
            Ok(())
        },
        crate::grammar::Stmt::MapWindow { addr, body, .. } => {
            scan_expr(ctx, addr)?;
            for s in &body.stmts { scan_stmt(ctx, s)?; }
            Ok(())
        }
        crate::grammar::Stmt::Unsafe(b) => {
            for s in &b.stmts { scan_stmt(ctx, s)?; }
            Ok(())
        }
        _ => Ok(())
    }
}

fn scan_expr(ctx: &CodegenContext, expr: &syn::Expr) -> Result<(), String> {
    match expr {
        syn::Expr::Call(c) => scan_expr_call(ctx, c),

        syn::Expr::Struct(s) => {
             // Check if it is a specialized struct instantiation
            // Resolve the path type!
             let ty_syn = syn::Type::Path(syn::TypePath { qself: None, path: s.path.clone() });
             ctx.bridge_resolve_type(&crate::grammar::SynType::from_std(ty_syn).map_err(|e| e.to_string())?);
             
             for f in &s.fields { scan_expr(ctx, &f.expr)?; }
             Ok(())
        }
        syn::Expr::Cast(c) => {
            scan_expr(ctx, &c.expr)?;
            ctx.bridge_resolve_type(&crate::grammar::SynType::from_std(*c.ty.clone()).map_err(|e| e.to_string())?);
            Ok(())
        }
        syn::Expr::Binary(b) => {
            scan_expr(ctx, &b.left)?;
            scan_expr(ctx, &b.right)?;
            Ok(())
        }
        syn::Expr::Unary(u) => scan_expr(ctx, &u.expr),
        syn::Expr::Paren(p) => scan_expr(ctx, &p.expr),
        syn::Expr::MethodCall(m) => scan_expr_method_call(ctx, m),
        syn::Expr::Block(b) => {
            for s in &b.block.stmts { scan_stmt(ctx, &crate::grammar::Stmt::Syn(s.clone()))?; }
            Ok(())
        }
        _ => Ok(())
    }
}


fn resolve_scan_expr_call_fqn(ctx: &CodegenContext, p: &syn::ExprPath, target_key: &crate::types::TypeKey, generic_args: &[crate::types::Type]) -> Result<(), String> {
    let full_mangled = target_key.mangle();

    let parts: Vec<&str> = full_mangled.split("__").collect();
    if parts.len() >= 2 {
         let base_name = Mangler::mangle(&parts[..parts.len()-1]);
         let method_name = parts.last().ok_or_else(|| "Failed to get method name from mangled path".to_string())?;
         
         if ctx.struct_templates().contains_key(&base_name) || ctx.enum_templates().contains_key(&base_name) {
             let template_key = base_name.clone();
             let is_generic_struct = {
                 if let Some(t) = ctx.struct_templates().get(&template_key) {
                     t.generics.as_ref().map_or(false, |g| !g.params.is_empty())
                 } else if let Some(e) = ctx.enum_templates().get(&template_key) {
                     e.generics.as_ref().map_or(false, |g| !g.params.is_empty())
                 } else { false }
             };
             
             let base_parts: Vec<&str> = base_name.split("__").collect();
             let (b_path, b_name) = if base_parts.len() > 1 {
                 (base_parts[..base_parts.len()-1].iter().map(|s| s.to_string()).collect::<Vec<_>>(), base_parts.last().ok_or_else(|| "Failed to get base name".to_string())?.to_string())
             } else {
                 (vec![], base_name.clone())
             };
             let key_obj = crate::types::TypeKey { path: b_path, name: b_name, specialization: None };
             
             let fn_item_opt = ctx.trait_registry().get_legacy(&key_obj, &method_name);
             
             if let Some((f, _, _)) = fn_item_opt {
                 let old_map = ctx.current_type_map().clone();
                 
                 if is_generic_struct {
                     let map_params = |params: &syn::punctuated::Punctuated<crate::grammar::GenericParam, syn::Token![,]>| {
                          for (i, param) in params.iter().enumerate() {
                              let name = match param {
                                  crate::grammar::GenericParam::Type { name, .. } => name,
                                  crate::grammar::GenericParam::Const { name, .. } => name,
                              };
                              if let Some(arg) = generic_args.get(i) {
                                  let val: crate::types::Type = arg.clone();
                                ctx.current_type_map_mut().insert(name.to_string(), val);
                              }
                          }
                     };
                     if let Some(template) = ctx.struct_templates().get(&template_key) {
                          if let Some(generics) = &template.generics { map_params(&generics.params); }
                     } else if let Some(template) = ctx.enum_templates().get(&template_key) {
                          if let Some(generics) = &template.generics { map_params(&generics.params); } 
                     }
                 } else {
                      if let Some(generics) = &f.generics {
                         for (i, param) in generics.params.iter().enumerate() {
                              let name = match param {
                                  crate::grammar::GenericParam::Type { name, .. } => name,
                                  crate::grammar::GenericParam::Const { name, .. } => name,
                              };
                              if let Some(arg) = generic_args.get(i) {
                                  let val: crate::types::Type = arg.clone();
                                ctx.current_type_map_mut().insert(name.to_string(), val);
                              }
                         }
                      }
                 }

                 if let Some(rt) = &f.ret_type { let _ = ctx.bridge_resolve_type(rt); }
                 let unknown_ty = crate::grammar::SynType::Other("UnknownSelf".to_string());
                 for a in &f.args { let _ = ctx.bridge_resolve_type(a.ty.as_ref().unwrap_or(&unknown_ty)); }
                 
                 *ctx.current_type_map_mut() = old_map;
                 
                 let segments_len = p.path.segments.len();
                 if segments_len >= 2 {
                     let mut base_path = p.path.clone();
                     let method_seg = base_path.segments.pop().ok_or_else(|| "Failed to pop method segment".to_string())?.into_value();
                     
                     let base_ty_syn = syn::Type::Path(syn::TypePath { qself: None, path: base_path });
                     let self_ty = ctx.bridge_resolve_type(&crate::grammar::SynType::from_std(base_ty_syn).map_err(|e| e.to_string())?);
                     
                     let mut concrete_tys = Vec::new();
                     
                     if let crate::types::Type::Concrete(_, args) = &self_ty {
                         concrete_tys.extend(args.clone());
                     } 
                     
                     if let syn::PathArguments::AngleBracketed(args) = &method_seg.arguments {
                         for arg in &args.args {
                             match arg {
                                 syn::GenericArgument::Type(ty) => {
                                     concrete_tys.push(ctx.bridge_resolve_type(&crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?));
                                 }
                                 syn::GenericArgument::Const(expr) => {
                                      if let Ok(crate::evaluator::ConstValue::Integer(val)) = ctx.evaluator.borrow_mut().eval_expr(expr) {
                                          concrete_tys.push(crate::types::Type::Struct(val.to_string()));
                                      } else {
                                          concrete_tys.push(crate::types::Type::Struct("0".to_string()));
                                      }
                                 }
                                 _ => {}
                             }
                         }
                     }
                     
                     let method_generic_count = f.generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
                     let turbofish_count = if let syn::PathArguments::AngleBracketed(args) = &method_seg.arguments {
                         args.args.len()
                     } else { 0 };
                     
                     if turbofish_count >= method_generic_count {
                          let _ = ctx.request_specialization(&full_mangled, concrete_tys, Some(self_ty));
                     }
                 }
             }
         }
    } 
    
    if let Some((_, ret_ty)) = ctx.resolve_global_signature(&full_mangled) { 
         if let crate::types::Type::Fn(_, box_ret) = ret_ty {
             let _ = ctx.bridge_resolve_codegen_type(&box_ret);
         }
    }
    
    Ok(())
}

fn scan_expr_call(ctx: &CodegenContext, c: &syn::ExprCall) -> Result<(), String>  {
            scan_expr(ctx, &c.func)?;
            for a in &c.args { scan_expr(ctx, a)?; }
            
            // FQN UPGRADE: Resolve Return Type & Static Methods using Canonical Key
            if let syn::Expr::Path(p) = &*c.func {
                 // 1. Extract Concrete Args (Generics)
                 let mut generic_args = Vec::new();
                 for seg in &p.path.segments {
                     if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                         for arg in &args.args {
                             match arg {
                                 syn::GenericArgument::Type(ty) => {
                                     generic_args.push(ctx.bridge_resolve_type(&crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?));
                                 }
                                 syn::GenericArgument::Const(expr) => {
                                     if let Ok(crate::evaluator::ConstValue::Integer(val)) = ctx.evaluator.borrow_mut().eval_expr(expr) {
                                         generic_args.push(crate::types::Type::Struct(val.to_string()));
                                     } else {
                                         generic_args.push(crate::types::Type::Struct("0".to_string()));
                                     }
                                 }
                                 _ => {}
                             }
                         }
                     }
                 }
                
                // 2. Resolve to FQN Identity
                if let Ok(target_key) = ctx.resolve_path_to_fqn(&p.path) {
                    let _ = resolve_scan_expr_call_fqn(ctx, p, &target_key, &generic_args);
                }
            }
            Ok(())
        }

fn scan_expr_method_call(ctx: &CodegenContext, m: &syn::ExprMethodCall) -> Result<(), String>  {
            scan_expr(ctx, &m.receiver)?;
            for a in &m.args { scan_expr(ctx, a)?; }
            
            // Try to resolve receiver type to trigger specialization of the method
            if let Some(recv_ty) = resolve_receiver_scan_helper(ctx, &m.receiver) {
                let method_name = m.method.to_string();
                
                // 1. Check for Method Generics (Turbofish)
                let mut method_generics = Vec::new();
                if let Some(turbofish) = &m.turbofish {
                    for arg in &turbofish.args {
                        if let syn::GenericArgument::Type(ty) = arg {
                            method_generics.push(ctx.bridge_resolve_type(&crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?));
                        }
                    }
                }

                // 2. Resolve Method via Context
                // [V4.0 KEUOS] Use TraitRegistry for method lookup with receiver type matching
                let method_result: Option<(crate::grammar::SaltFn, Option<crate::types::Type>, Vec<crate::grammar::ImportDecl>)> = {
                    // Try to resolve via TraitRegistry using the receiver type
                    if let Some(recv_key) = recv_ty.to_key() {
                        ctx.trait_registry().get_legacy(&recv_key, &method_name)
                            .or_else(|| {
                                // Try template key (without specialization)
                                let template_key = recv_key.to_template();
                                ctx.trait_registry().get_legacy(&template_key, &method_name)
                            })
                    } else {
                        None
                    }
                };
                if let Some((func_def, impl_ty, _imports)) = method_result
                {
                     // Found generic impl or static method?
                     // We need to request specialization if:
                     // A. Receiver is specialized (recv_ty has args)
                     // B. Method has generics (method_generics not empty)
                     
                     // Construct concrete_tys combining Impl args + Method args
                     let mut concrete_tys = Vec::new();
                     
                     // A. Impl Args (from Receiver)
                     if let crate::types::Type::Concrete(_, args) = &recv_ty {
                         concrete_tys.extend(args.clone());
                     }
                     // B. Method Args
                     concrete_tys.extend(method_generics);
                     
                     // [KEUOS FIX] Check if method generics are fully satisfied by turbofish.
                     // scan_expr cannot do argument inference. If inference is needed, we SKIP creating the task.
                     // emit_method_call will create the correct task with inference later.
                     let method_generic_count = func_def.generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
                     let turbofish_count = if let Some(t) = &m.turbofish { t.args.len() } else { 0 };
                     


                     if turbofish_count < method_generic_count {


                     } else if !concrete_tys.is_empty() {

                         // Determine Template Name
                         let template_name = if let crate::types::Type::Concrete(bx, _) = &recv_ty {
                             bx.clone()
                         } else if let crate::types::Type::Struct(bx) = &recv_ty {
                             bx.clone()
                         } else if let Some(it) = impl_ty {
                             it.mangle_suffix()
                         } else {
                             // Fallback
                             if let crate::types::Type::Struct(n) = &recv_ty { n.clone() }
                             else if let crate::types::Type::Concrete(n, _) = &recv_ty { n.clone() }
                             else { recv_ty.mangle_suffix() }
                         };
                         
                         let func_name = format!("{}__{}", template_name, method_name);
                         
                         // [GRAYDON FIX] Substitute generic placeholders in concrete_tys using current_type_map
                         // This fixes internal method calls inside with_capacity where recv_ty is Concrete(HashMap, [Struct(K), Struct(V)])
                         // We need to convert [Struct(K), Struct(V)] → [I64, I64] using the active type_map
                         let current_map = ctx.current_type_map().clone();

                         let substituted_tys: Vec<crate::types::Type> = concrete_tys.iter()
                             .map(|t| t.substitute(&current_map))
                             .collect();
                         let substituted_recv = recv_ty.substitute(&current_map);

                         
                         let _ = ctx.request_specialization(&func_name, substituted_tys, Some(substituted_recv));
                } 
                // Fallback: Check Global dispatch if method syntax used for global? (Rare in Salt)
            }
            } // Close if let Some(recv_ty)
            Ok(())
        }


// Helper to resolve simple receiver chains like `self.field` or `Struct::new()` to trigger Lazy Specialization
fn resolve_receiver_scan_helper(ctx: &CodegenContext, expr: &syn::Expr) -> Option<crate::types::Type> {
     match expr {
         syn::Expr::Path(p) => {
             if let Some(ident) = p.path.get_ident() {
                 let name = ident.to_string();
                 if name == "self" {
                     return ctx.current_self_ty().clone();
                 }
                 // Check Globals
                 let segments = vec![name.clone()];
                 if let Some((pkg, item)) = ctx.bridge_resolve_package_prefix(&segments) {
                      let mangled = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) };
                      if let Some(ty) = ctx.resolve_global(&mangled) {

                          return Some(ty);
                      }
                 }
                 
                 // Try just name (if local/same package global)
                 if let Some(ty) = ctx.resolve_global(&name) {

                     return Some(ty);
                 }
                 
                 // Try current package prefix manually
                 if let Some(pkg) = &*ctx.current_package.borrow() {
                     let pkg_name = Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                     let local_global = format!("{}__{}", pkg_name, name);
                     if let Some(ty) = ctx.resolve_global(&local_global) {

                         return Some(ty);
                     }
                 }
             }
             None
         },
         syn::Expr::Field(f) => {
             if let Some(base_ty) = resolve_receiver_scan_helper(ctx, &f.base) {
                  let inner = if let crate::types::Type::Reference(inner, _) | crate::types::Type::Owned(inner) = base_ty { *inner } else { base_ty };
                  // Handle both Struct and Concrete
                  let name_opt = match &inner {
                      crate::types::Type::Struct(n) => Some(n.clone()),
                      crate::types::Type::Concrete(_n, _) => None,
                      _ => None
                  };
                  
                  if let Some(name) = name_opt {
                      if let Some(info) = ctx.struct_registry().values().find(|i| i.name == name).cloned() {
                          let fname = if let syn::Member::Named(n) = &f.member { n.to_string() } else { return None };
                          if let Some((_, fty)) = info.fields.get(&fname) {

                              return Some(fty.clone());
                          }
                      }
                  } else if let crate::types::Type::Concrete(base, params) = inner {
                       let key = crate::types::TypeKey { path: vec![], name: base.clone(), specialization: Some(params.clone()) };
                       if let Some(info) = ctx.struct_registry().get(&key).cloned() {
                            let fname = if let syn::Member::Named(n) = &f.member { n.to_string() } else { return None };
                            if let Some((_, fty)) = info.fields.get(&fname) {

                                return Some(fty.clone());
                            }
                       } else {

                       }
                  }
             }
             None
         },
         syn::Expr::Paren(p) => resolve_receiver_scan_helper(ctx, &p.expr),
         _ => None
     }
}


fn resolve_localized_self_ty<'a>(
    ctx: &CodegenContext<'a>,
    st: &Type,
    concrete_tys: &[Type],
    new_type_map: &mut std::collections::BTreeMap<String, Type>,
    old_const_vals: &mut Vec<(String, Option<crate::evaluator::ConstValue>)>,
    arg_idx: &mut usize
) -> Option<Type> {
        let base_name = match st {
            Type::Struct(name) | Type::Enum(name) => Some(name.clone()),
            Type::Concrete(name, _) => Some(name.clone()),
            _ => None,
        };

        if let Some(base) = base_name {
             let mut template_key = base.clone();
             if !ctx.struct_templates().contains_key(&base) && !ctx.enum_templates().contains_key(&base) {
                  if let Some(info) = ctx.struct_registry().values().find(|i| i.name == base).cloned() {
                      if let Some(tn) = &info.template_name { template_key = tn.clone(); }
                  } else if let Some(info) = ctx.enum_registry().values().find(|i| i.name == base).cloned() {
                      if let Some(tn) = &info.template_name { template_key = tn.clone(); }
                  }
             }

             let gen_params = if let Some(s) = ctx.struct_templates().get(&template_key) {
                 s.generics.as_ref().map(|g| g.params.clone())
             } else if let Some(e) = ctx.enum_templates().get(&template_key) {
                 e.generics.as_ref().map(|g| g.params.clone())
             } else { None };

             if let Some(params) = gen_params {
                  let source_args = if let Type::Concrete(_, c_args) = st { c_args } else { concrete_tys };
                  let use_shared_idx = !matches!(st, Type::Concrete(..));

                  for (i, param) in params.iter().enumerate() {
                      let current_arg = if use_shared_idx { concrete_tys.get(*arg_idx) } else { source_args.get(i) };
                      
                      if let Some(arg) = current_arg {
                          let name = match param {
                              crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                              crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                          };
                          new_type_map.insert(name.clone(), arg.clone());
                          
                          if let Type::Struct(val_str) = &arg {
                              if let Ok(int_val) = val_str.parse::<i64>() {
                                  use crate::evaluator::ConstValue;
                                  let old = ctx.evaluator.borrow_mut().constant_table.insert(name.clone(), ConstValue::Integer(int_val));
                                  old_const_vals.push((name, old));
                              }
                          }
                          if use_shared_idx { *arg_idx += 1; }
                      }
                  }
             }

             if !concrete_tys.is_empty() && (ctx.struct_templates().contains_key(&template_key) || ctx.enum_templates().contains_key(&template_key)) {
                 let specialized_key = crate::types::TypeKey {
                     path: vec![],
                     name: template_key.clone(),
                     specialization: Some(concrete_tys.to_vec()), 
                 };
                 let mangled_type = specialized_key.mangle();
                 if ctx.enum_templates().contains_key(&template_key) {
                     Some(Type::Enum(mangled_type))
                 } else {
                     Some(Type::Struct(mangled_type))
                 }
             } else {
                 Some(st.clone())
             }
        } else {
            Some(st.clone())
        }
}

#[allow(dead_code)]
fn emit_specialized_generation(
    ctx: &CodegenContext, 
    func: &SaltFn, 
    concrete_tys: Vec<Type>, 
    self_ty: Option<Type>, 
    mangled_name: String
) -> Result<String, String> {
    // 3. RAII GUARD: Ensure all lookups for 'Self' inside this body resolution
    //    We need to ensure 'self' maps to the specialized struct name (e.g. "RawVec_u8")
    //    not the template (e.g. "RawVec").
    
    // Construct new type map for generic args
    let mut new_type_map = ctx.current_type_map().clone();
    let mut old_const_vals = Vec::new(); // Keep track for manual restore if needed (Evaluator doesn't support RAII easily yet)
    let mut arg_idx = 0;

    let localized_self_ty = if let Some(st) = &self_ty {
        resolve_localized_self_ty(ctx, st, &concrete_tys, &mut new_type_map, &mut old_const_vals, &mut arg_idx)
    } else {
        None
    };

    // Map Function Generics
    if let Some(generics) = &func.generics {
        for param in &generics.params {
            if let Some(arg) = concrete_tys.get(arg_idx).cloned() {
                let name = match param {
                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                };
                new_type_map.insert(name.clone(), arg.clone());
                 if let Type::Struct(val_str) = &arg {
                    if let Ok(int_val) = val_str.parse::<i64>() {
                        use crate::evaluator::ConstValue;
                        let old = ctx.evaluator.borrow_mut().constant_table.insert(name.clone(), ConstValue::Integer(int_val));
                        old_const_vals.push((name, old));
                    }
                }
                arg_idx += 1;
            }
        }
    }

    // 3. RAII GUARD: Enable the Guard!
    // The guard will automatically restore 'current_type_map' and 'current_self_ty' when it goes out of scope.
    // 3. RAII GUARD: Enable the Guard!
    // The guard will automatically restore 'current_type_map', 'current_self_ty', and 'current_generic_args'
    let _guard = GenericContextGuard::new(ctx, new_type_map, localized_self_ty.unwrap_or(Type::Unit), concrete_tys);

    // FIX: Guard Package Context (Restore package from mangled name e.g. std__core__alloc -> std.core)
    let old_pkg = ctx.current_package.borrow().clone();
    let parts: Vec<&str> = mangled_name.split("__").collect();
    if parts.len() > 1 {
        // Assume last part is function name, rest is package path
        // BUT for methods (Struct__Method), package is Struct? No.
        // Mangled name IS FQN of the function instance.
        // std__core__vec__Vec__push.
        // Package: std.core.vec.
        // If we treat it as package, resolve_global uses it to prefix lookups.
        // We should construct a path.
        // However, resolve_global matches mangled names against module globals.
        // globals keys are Mangled FQNs.
        // But resolve_variable uses `current_package` to prepend to `name`?
        // Yes line 1150: format!("{}{}", pkg_prefix, g.name).
        // So we need pkg_prefix to be "std__core__slab_alloc__".
        // Let's reconstruct it.
        // We take all parts except the last one?
        // For Global Function: std__core__slab_alloc__alloc -> Base: std__core__slab_alloc.
        // For Method: std__vec__Vec__push -> Base: std__vec__Vec.
        // Does resolve_variable expect Struct in package prefix?
        // No, resolve_global logic usually checks module level.
        // But if `GLOBAL_ALLOC` is module level.
        // If current_package is "std__core__slab_alloc".
        // lookup "GLOBAL_ALLOC" -> "std__core__slab_alloc__GLOBAL_ALLOC".
        // This matches registry key.
        // So generic strategy: Take all but last.
        
        // Wait. For Method `Vec::push`, base is `Vec`.
        // If `Vec` accesses global?
        // It should match `std__core__vec`.
        // If mangled is `std__core__vec__Vec__push`.
        // All-but-last is `std__core__vec__Vec`.
        // `prefix` = `std__core__vec__Vec__`.
        // lookup "GLOBAL" -> `std__core__vec__Vec__GLOBAL`.
        // Incorrect.
        
        // Strategy: Iterate parts. Match against `reg.modules`.
        // Longest match wins.
        let mut best_pkg = old_pkg.clone();
        for i in (1..parts.len()).rev() {
             let candidate = parts[0..i].join(".");
             let exists = ctx.registry.as_ref().map_or(false, |r| r.modules.contains_key(&candidate));
             if mangled_name.contains("alloc") {

             }
             if exists {
                 let pkg_str = format!("package {};", candidate);
                 if let Ok(pkg) = syn::parse_str::<crate::grammar::PackageDecl>(&pkg_str) {
                     best_pkg = Some(pkg);
                     break;
                 } else {

                 }
             }
        }
        *ctx.current_package.borrow_mut() = best_pkg;
    }



    // 4. EMIT
    let res = emit_fn(ctx, func, Some(mangled_name));

    // Manual Cleanup for fields not in Guard

    *ctx.current_package.borrow_mut() = old_pkg;
    
    // Restore Consts
    let mut eval = ctx.evaluator.borrow_mut();
    for (k, v_opt) in old_const_vals.drain(..).rev() {
        if let Some(v) = v_opt {
            eval.constant_table.insert(k, v);
        } else {
            eval.constant_table.remove(&k);
        }
    }

    res
}
