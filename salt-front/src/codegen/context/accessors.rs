use crate::types::Type;
use crate::types::TypeKey;
use crate::codegen::context::{LoweringContext, CodegenContext};
use crate::codegen::collector::MonomorphizationTask;
use crate::grammar::{SaltFn, ImportDecl, StructDef, EnumDef};
use crate::registry::{StructInfo, EnumInfo};

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
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
    pub fn current_ensures(&self) -> &Vec<syn::Expr> { &self.expansion.current_ensures }
    pub fn current_ensures_mut(&mut self) -> &mut Vec<syn::Expr> { &mut self.expansion.current_ensures }
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

    pub fn is_dynamic_check_block(&self) -> &bool { &self.control_flow.is_dynamic_check_block }
    pub fn is_dynamic_check_block_mut(&mut self) -> &mut bool { &mut self.control_flow.is_dynamic_check_block }
    pub fn no_yield(&self) -> &bool { &self.control_flow.no_yield }
    pub fn no_yield_mut(&mut self) -> &mut bool { &mut self.control_flow.no_yield }
    pub fn current_pulse(&self) -> &Option<u32> { &self.control_flow.current_pulse }
    pub fn current_pulse_mut(&mut self) -> &mut Option<u32> { &mut self.control_flow.current_pulse }
    pub fn is_hot_path(&self) -> &bool { &self.control_flow.is_hot_path }
    pub fn is_hot_path_mut(&mut self) -> &mut bool { &mut self.control_flow.is_hot_path }
}

impl<'a> CodegenContext<'a> {
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

    // Signature-aware method resolution - the ONLY method lookup path
    pub fn trait_registry(&self) -> std::cell::Ref<'_, crate::codegen::trait_registry::TraitRegistry> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.trait_registry)
    }
    pub fn trait_registry_mut(&self) -> std::cell::RefMut<'_, crate::codegen::trait_registry::TraitRegistry> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.trait_registry)
    }
    // String prefix handlers for comptime string processing
    pub fn string_prefix_handlers(&self) -> std::cell::Ref<'_, std::collections::HashMap<String, String>> {
        std::cell::Ref::map(self.discovery.borrow(), |d| &d.string_prefix_handlers)
    }
    pub fn string_prefix_handlers_mut(&self) -> std::cell::RefMut<'_, std::collections::HashMap<String, String>> {
        std::cell::RefMut::map(self.discovery.borrow_mut(), |d| &mut d.string_prefix_handlers)
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
    pub fn current_ensures(&self) -> std::cell::Ref<'_, Vec<syn::Expr>> {
        std::cell::Ref::map(self.expansion.borrow(), |e| &e.current_ensures)
    }
    pub fn current_ensures_mut(&self) -> std::cell::RefMut<'_, Vec<syn::Expr>> {
        std::cell::RefMut::map(self.expansion.borrow_mut(), |e| &mut e.current_ensures)
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
    
    // TypeID Registry accessors
    pub fn type_id_registry(&self) -> std::cell::Ref<'_, crate::codegen::types::TypeIDRegistry> {
        std::cell::Ref::map(self.emission.borrow(), |e| &e.type_id_registry)
    }
    pub fn type_id_registry_mut(&self) -> std::cell::RefMut<'_, crate::codegen::types::TypeIDRegistry> {
        std::cell::RefMut::map(self.emission.borrow_mut(), |e| &mut e.type_id_registry)
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

}
