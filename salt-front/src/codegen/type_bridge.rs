use crate::types::{Type, TypeKey};
use crate::codegen::context::LoweringContext;
use crate::registry::{StructInfo, EnumInfo};
use crate::evaluator::ConstValue;
use std::collections::HashMap;
pub use super::type_casts::cast_numeric;

pub use crate::codegen::types::numeric::get_numeric_idx;
pub use crate::codegen::types::numeric::PromotionTable;
pub use crate::codegen::types::numeric::PROMOTION_OPS;

pub use crate::codegen::types::numeric::get_arith_op;
pub use crate::codegen::types::numeric::get_comparison_pred;
pub use crate::codegen::types::numeric::promote_numeric;
pub(crate) use crate::codegen::types::numeric::get_bit_width;


// to_mlir_type impl moved to crate::codegen::types::mlir

// ============================================================================
// Pointer flattening and layout validation
// ============================================================================

/// Extracts the inner type from mangled pointer names.
pub use crate::codegen::types::layout::extract_ptr_inner;
pub use crate::codegen::types::layout::flatten_nested_ptr;
pub use crate::codegen::types::layout::prove_layout_compatibility;
pub use crate::codegen::types::layout::prove_layout_compatibility_ctx;

pub use crate::codegen::types::substitution::substitute_generics;
pub use crate::codegen::types::substitution::substitute_generics_ctx;
pub use crate::codegen::types::mlir::to_mlir_type;


pub use crate::codegen::types::traits::{check_trait_constraint, validate_trait_constraints, has_unresolved_type_params};
pub use crate::codegen::types::resolution::{resolve_codegen_type, resolve_type, infer_expr_type, type_to_type_key};

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    
    
    pub(crate) fn populate_explicit_specialization_map(
        &mut self,
        func: &crate::grammar::SaltFn,
        concrete_tys: &[Type],
        st: &Type,
        old_const_vals: &mut Vec<(String, Option<crate::evaluator::ConstValue>)>,
    ) {
        let template_name = if let Type::Struct(name) = st {
            self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
        } else if let Type::Enum(name) = st {
            self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
        } else if let Type::Concrete(name, _) = st {
            name.clone()
        } else if let Type::Pointer { .. } = st {
            "std__core__ptr__Ptr".to_string()
        } else {
            "".to_string()
        };
        
        if !template_name.is_empty() {
            let gen_params = if let Some(s) = self.struct_templates().get(&template_name) {
                s.generics.as_ref().map(|g| g.params.clone())
            } else if let Some(e) = self.enum_templates().get(&template_name) {
                e.generics.as_ref().map(|g| g.params.clone())
            } else { None };
            
            if let Some(params) = gen_params {
                for (i, param) in params.iter().enumerate() {
                    let pname = match param { crate::grammar::GenericParam::Type { name, .. } => name.to_string(), crate::grammar::GenericParam::Const { name, .. } => name.to_string() };
                    if let Type::Concrete(_, args) = &st {
                        if let Some(arg) = args.get(i) {
                            self.current_type_map_mut().insert(pname, arg.clone());
                        }
                    } else if let Type::Pointer { element, .. } = &st {
                        if i == 0 {
                            self.current_type_map_mut().insert(pname, (**element).clone());
                        }
                    } else if let Some(arg) = concrete_tys.get(i) {
                        self.current_type_map_mut().insert(pname, arg.clone());
                    }
                }
            }
        }
        
        if let Some(fn_generics) = &func.generics {
            let struct_generic_names: std::collections::HashSet<String> = {
                let mut names = std::collections::HashSet::new();
                let type_name = match st {
                    Type::Struct(name) | Type::Concrete(name, _) => Some(name.clone()),
                    _ => None
                };
                if let Some(ref tname) = type_name {
                    let gen_params = {
                        let templates = self.struct_templates();
                        if let Some(s) = templates.get(tname) {
                            s.generics.as_ref().map(|g| g.params.clone())
                        } else {
                            let _ = templates;
                            let etemplates = self.enum_templates();
                            etemplates.get(tname).and_then(|e| e.generics.as_ref()).map(|g| g.params.clone())
                        }
                    };
                    if let Some(params) = gen_params {
                        for p in &params {
                            let name = match p {
                                crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                            };
                            names.insert(name);
                        }
                    }
                }
                names
            };
            
            let struct_generic_count = struct_generic_names.len();
            let method_args: Vec<Type> = concrete_tys.iter().skip(struct_generic_count).cloned().collect();
            
            if !method_args.is_empty() {
                let method_only_params: syn::punctuated::Punctuated<_, syn::token::Comma> = fn_generics.params.iter()
                    .filter(|p| {
                        let name = match p {
                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                        };
                        !struct_generic_names.contains(&name)
                    })
                    .cloned()
                    .collect();
                
                let method_only_generics = crate::grammar::Generics {
                    params: method_only_params,
                };
                self.map_generics(&Some(method_only_generics), &method_args, &func.name.to_string(), old_const_vals);
            }
        }
    }
    #[allow(clippy::too_many_arguments)] // All 8 parameters needed to construct MonomorphizationTask
    pub(crate) fn enqueue_monomorphization_task(
        &mut self,
        func_name: &str,
        mangled: &str,
        func: crate::grammar::SaltFn,
        concrete_tys: Vec<Type>,
        s_ty: Option<Type>,
        imports: Vec<crate::grammar::ImportDecl>,
        spec_map: std::collections::BTreeMap<String, Type>,
    ) {
        let mut pkg_path = Vec::new();
        if let Some((t_name, _method)) = func_name.rsplit_once("__") {
            if let Some(pkg) = self.discovery.type_origins.get(t_name) {
                pkg_path = pkg.split('.').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
            }
        }
        
        if pkg_path.is_empty() {
            let path_segments: Vec<String> = if func_name.contains("__") {
                 func_name.split("__").map(|s| s.to_string()).collect()
            } else {
                 vec![]
            };
            pkg_path = if path_segments.len() > 1 {
                path_segments[0..path_segments.len()-1].to_vec()
            } else {
                vec![]
            };
        }

        let task = crate::codegen::collector::MonomorphizationTask {
            identity: crate::types::TypeKey { 
                path: pkg_path, 
                name: func.name.to_string(), 
                specialization: None 
            },
            mangled_name: mangled.to_string(),
            func,
            concrete_tys,
            self_ty: s_ty,
            imports,
            type_map: spec_map,
        };
        self.expansion.pending_generations.push_back(task);
    }

    pub fn request_explicit_specialization(&mut self, func_name: &str, override_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        // Always strip Reference wrappers from self_ty.
        let self_ty = self_ty.map(|mut ty| {
            while let Type::Reference(inner, _) = ty {
                ty = *inner;
            }
            ty
        });
        
        let mangled = override_name.to_string();
        
        // Check strict map
        if let Some(existing) = self.specializations().get(&(func_name.to_string(), concrete_tys.clone())) {

            // If it exists in map but isn't defined or pending, queue it
            let defined = self.defined_functions().contains(existing);
            let pending = self.pending_generations().iter().any(|task| task.mangled_name == *existing);
            


            if !defined && !pending {

                 // Fall through to queue logic!
            } else {
                 return existing.clone();
            }
        }

        self.specializations_mut().insert((func_name.to_string(), concrete_tys.clone()), mangled.clone());
        
        let file = &self.config.file;
        // Search logic duplicated from request_specialization
        let found = if let Some(st) = &self_ty {
             let (st_base, method_name) = if let Some((base, method)) = func_name.rsplit_once("__") {
                 (base.to_string(), method.to_string())
             } else {
                 ("".to_string(), func_name.to_string())
             };
             
            let template_name = if let Type::Struct(name) = st {
                 self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Enum(name) = st {
                 self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             // Handle Type::Pointer method lookup with fully-qualified template name
             } else if let Type::Pointer { .. } = st {
                 "std__core__ptr__Ptr".to_string()
             } else {
                 st_base
             };
             // Use TraitRegistry for method lookup
             self.trait_registry().find_method_by_name(&template_name, &method_name, st)
        } else {
             file.items.iter().find_map(|item| {
                 if let crate::grammar::Item::Fn(f) = item {
                     if f.name == func_name { return Some((f.clone(), None, self.imports().clone())); }
                 }
                 None
             })
        };
        
        if let Some((func, s_ty, imports)) = found {

            let spec_map;
            {
                let old_imports = self.imports().clone();
                *self.imports_mut() = imports.clone();
                let old_map = self.current_type_map().clone();
                let old_args = self.current_generic_args().clone();
                let old_self = self.current_self_ty().clone();
                let mut old_const_vals = Vec::new();
                
                *self.current_generic_args_mut() = concrete_tys.clone();
                *self.current_self_ty_mut() = s_ty.clone();

                if let Some(st) = &s_ty {
                    self.populate_explicit_specialization_map(&func, &concrete_tys, st, &mut old_const_vals);
                }

                spec_map = self.current_type_map().clone();

                *self.current_type_map_mut() = old_map;
                *self.current_generic_args_mut() = old_args;
                *self.current_self_ty_mut() = old_self;
                *self.imports_mut() = old_imports;
            }

            self.enqueue_monomorphization_task(func_name, &mangled, func.clone(), concrete_tys.clone(), s_ty.clone(), imports.clone(), spec_map);
        }

        mangled
    }




    pub fn request_specialization(&mut self, func_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        // Always strip Reference wrappers from self_ty.
        // The self_ty identity should be the naked base type (e.g., Result), not Reference(Result).
        // This ensures correct type mangling and Self resolution during hydration.
        let self_ty = self_ty.map(|mut ty| {
            while let Type::Reference(inner, _) = ty {
                ty = *inner;
            }
            ty
        });

        // Prevent recursive specialization
        // Recursively flatten nested pointer wrappers
        let concrete_tys: Vec<Type> = concrete_tys.into_iter().enumerate().map(|(i, ty)| {
            let debug_ctx = format!("{}[arg {}]", func_name, i);
            flatten_nested_ptr(&ty, 0, &debug_ctx)
        }).collect();

        // Security check: ensure no generics leak into the monomorphization queue
        // Check for both Generic("T") and Struct("F") where F is not a known struct/enum
        if concrete_tys.iter().any(|t| has_unresolved_type_params(self, t)) {

             return func_name.to_string();
        }
        if let Some(sty) = &self_ty {
            if has_unresolved_type_params(self, sty) {

                 return func_name.to_string();
            }
        }

        // Derive suffix from concrete_tys, OR from self_ty's specialization args if concrete_tys is empty
        // This ensures method specializations like Ptr<u8>::offset get suffix "_u8"

        let suffix = if !concrete_tys.is_empty() {
            concrete_tys.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_")
        } else if let Some(Type::Concrete(_, args)) = &self_ty {
            args.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_")
        } else {
            String::new()
        };
        let mangled = if suffix.is_empty() { func_name.to_string() } else { format!("{}_{}", func_name, suffix) };
        
        if let Some(existing) = self.specializations().get(&(func_name.to_string(), concrete_tys.clone())) {
            let s_res: String = existing.clone();
            return s_res;
        }
        self.specializations_mut().insert((func_name.to_string(), concrete_tys.clone()), mangled.clone());
        
        let file = &self.config.file;
        let found = if let Some(st) = &self_ty {
             // Method lookup
             let (st_base, method_name) = if let Some((base, method)) = func_name.rsplit_once("__") {
                 (base.to_string(), method.to_string())
             } else {
                 ("".to_string(), func_name.to_string())
             };
             
             // If st_base is a specialized name, resolve it to template name
             let template_name = if let Type::Struct(name) = st {
                 self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Enum(name) = st {
                 self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else {
                 st_base
             };
             // Use TraitRegistry for method lookup
             self.trait_registry().find_method_by_name(&template_name, &method_name, st)
        } else {
             // Function lookup
             file.items.iter().find_map(|item| {
                 if let crate::grammar::Item::Fn(f) = item {
                     if f.name == func_name { return Some((f.clone(), None, self.imports().clone())); }
                 }
                 None
             })
        };

        if let Some((func, s_ty, imports)) = found {
            // Validate trait constraints before specialization
            let _ = validate_trait_constraints(self, &func.generics, &concrete_tys);

            // Scan specialized function for new dependencies (e.g. return types, local vars)
            // This prevents "Frozen Emission" panics by discovering deps during Expansion phase.
            let spec_map;
            {
                let old_imports = self.imports().clone();
                *self.imports_mut() = imports.clone();
                
                let old_map = self.current_type_map().clone();
                let old_args = self.current_generic_args().clone();
                let old_self = self.current_self_ty().clone();
                let mut old_const_vals = Vec::new();
                
                *self.current_generic_args_mut() = concrete_tys.clone();
                *self.current_self_ty_mut() = s_ty.clone();

                // Map Generics
                if let Some(st) = &s_ty {
                    // Extract concrete args from Type::Concrete for struct generics
                    let (template_name, struct_concrete_args) = if let Type::Struct(name) = st {
                        let tname = self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone());
                        (tname, vec![])
                    } else if let Type::Enum(name) = st {
                        let tname = self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone());
                        (tname, vec![])
                    } else if let Type::Concrete(name, args) = st {
                        // The args here are the concrete types for the struct generics

                        (name.clone(), args.clone())
                    } else if let Type::Pointer { element, .. } = st {
                        let canonical_element = crate::codegen::type_bridge::resolve_codegen_type(self, element);
                        ("std__core__ptr__Ptr".to_string(), vec![canonical_element])
                    } else {
                        ("".to_string(), vec![])
                    };
                    
                    if !template_name.is_empty() {
                         let gen_params = if let Some(s) = self.struct_templates().get(&template_name) {
                             s.generics.clone()
                         } else if let Some(e) = self.enum_templates().get(&template_name) {
                             e.generics.clone()
                         } else { None };
                          

                          // Use struct_concrete_args when available, fallback to concrete_tys
                          let args_to_map = if struct_concrete_args.is_empty() { &concrete_tys[..] } else { &struct_concrete_args[..] };

                          self.map_generics(&gen_params, args_to_map, &template_name, &mut old_const_vals);
                    }
                } else {
                    // Global Fn
                    if !concrete_tys.is_empty() {
                         self.map_generics(&func.generics, &concrete_tys, &func.name.to_string(), &mut old_const_vals);
                    }
                }
                
                // Method-level generics (e.g., mmap<T> on File struct)
                // CRITICAL: func.generics.params includes BOTH impl-level and method-level params.
                // Only method-level ones must be mapped (skip struct_generic_count from func.generics).
                if let Some(fn_generics) = &func.generics {
                    // Use the CALLER's self_ty for correct struct_generic_count
                    let struct_generic_count = self_ty.as_ref()
                        .and_then(|t| match t {
                            Type::Struct(name) | Type::Concrete(name, _) => {
                                self.struct_templates().get(name)
                                    .and_then(|s| s.generics.as_ref())
                                    .map(|g| g.params.len())
                                    .or_else(|| self.enum_templates().get(name)
                                        .and_then(|e| e.generics.as_ref())
                                        .map(|g| g.params.len()))
                            }
                            Type::Pointer { .. } => Some(1),
                            _ => None
                        })
                        .unwrap_or(0);
                    
                    let method_args: Vec<Type> = concrete_tys.iter().skip(struct_generic_count).cloned().collect();

                    if !method_args.is_empty() {
                        // Create method-only generics by skipping impl-level params
                        let method_only_generics = crate::grammar::Generics {
                            params: fn_generics.params.iter().skip(struct_generic_count).cloned().collect(),
                        };
                        self.map_generics(&Some(method_only_generics), &method_args, &func.name.to_string(), &mut old_const_vals);
                    }
                }
                
                // Scan!

                // Scan for new dependencies discovered during specialization
                let _ = self.scan_types_in_fn_lctx(&func);
                
                // Capture the specialized map before restoring context
                spec_map = self.current_type_map().clone();

                *self.imports_mut() = old_imports;
                *self.current_type_map_mut() = old_map;
                *self.current_generic_args_mut() = old_args;
                *self.current_self_ty_mut() = old_self;
                
                // Restore consts
                for (name, old_val) in old_const_vals.into_iter().rev() {
                    if let Some(v) = old_val {
                        self.evaluator.constant_table.insert(name, v);
                    } else {
                        self.evaluator.constant_table.remove(&name);
                    }
                }
            }

            self.enqueue_monomorphization_task(func_name, &mangled, func.clone(), concrete_tys.clone(), s_ty.clone(), imports.clone(), spec_map);
        };

        mangled
    }
    pub fn specialize_template(&mut self, base_name: &str, concrete_tys: &[Type], is_enum: bool) -> Result<TypeKey, String> {
        // Canonicalize concrete_tys before constructing the TypeKey.
        // Without this, Struct("Node") produces "Box_Node" while Struct("main__Node") produces
        // "Box_main__Node", creating duplicate specializations. By canonicalizing here, all
        // specializations consistently use FQN names.
        let concrete_tys: Vec<Type> = concrete_tys.iter().map(|ty| {
            if let Type::Struct(name) = ty {
                if !name.contains("__") {
                    let suffix = format!("__{}", name);
                    if let Some(canonical) = self.struct_templates().keys()
                        .find(|k| k.ends_with(&suffix))
                        .cloned()
                    {
                        return Type::Struct(canonical);
                    }
                    if let Some(canonical) = self.struct_registry().keys()
                        .find(|k| k.name == *name || k.name.ends_with(&suffix))
                        .map(|k| k.mangle())
                    {
                        return Type::Struct(canonical);
                    }
                }
            } else if let Type::Enum(name) = ty {
                if !name.contains("__") {
                    let suffix = format!("__{}", name);
                    if let Some(canonical) = self.enum_templates().keys()
                        .find(|k| k.ends_with(&suffix))
                        .cloned()
                    {
                        return Type::Enum(canonical);
                    }
                    if let Some(canonical) = self.enum_registry().keys()
                        .find(|k| k.name == *name || k.name.ends_with(&suffix))
                        .map(|k| k.mangle())
                    {
                        return Type::Enum(canonical);
                    }
                }
            }
            ty.clone()
        }).collect();
        let concrete_tys = &concrete_tys;
        
        // Construct TypeKey

        let parts: Vec<&str> = base_name.split("__").collect();
        let (path, name) = if parts.len() > 1 {
             (parts[..parts.len()-1].iter().map(|s| s.to_string()).collect::<Vec<_>>(), parts.last().expect("parts.len() > 1").to_string())
        } else {
             (vec![], base_name.to_string())
        };
        let key = TypeKey {
             path,
             name,
             specialization: if concrete_tys.is_empty() { None } else { Some(concrete_tys.to_vec()) },
        };
        
        let mangled = key.mangle();

        // 1. Check Registry (Existence = Done or In Progress)
        let exists = if is_enum {
            self.enum_registry().contains_key(&key)
        } else {
            self.struct_registry().contains_key(&key)
        };

        if exists { return Ok(key); }

        // 1.5. Generic Guard: Do NOT specialize (expand) if args are still generic
        // After substitute_generics, self-referential {I: Struct("I")} → Generic("I")
        let substituted_tys: Vec<Type> = concrete_tys.iter()
            .map(|t| substitute_generics_ctx(self, t))
            .collect();
        if substituted_tys.iter().any(|t| t.has_generics()) {
             return Ok(key);
        }

        // 2. Check Pending Set
        let is_queued = self.monomorphizer().pending_set.contains(&mangled);
        if is_queued { return Ok(key); }

        // 3. Frozen Check (Provenance Safety)
        if self.monomorphizer().is_frozen {
            // WARNING: Late specialization during emission.
            // Allowed via iterative drainage.
        }

        // 4. Self-Identity Guard (If inside the struct being simplified)
        if let Some(Type::Struct(self_name)) = self.current_self_ty() {
            if *self_name == mangled { return Ok(key); }
        }
        if let Some(Type::Enum(self_name)) = self.current_self_ty() {
             if *self_name == mangled { return Ok(key); }
        }

        // 5. Protected Name Check
        if Type::is_protected_name(&mangled) {
             return Ok(key); 
        }

        // 6. Atomic Registration (Placeholder)
        // Insert empty info to prevent recursive re-entry if registry lookup happens (redundant with pending_set but safe)
        if is_enum {
             let reg = self.enum_registry_mut();
             reg.insert(key.clone(), EnumInfo {
                 name: mangled.clone(), variants: Vec::new(), max_payload_size: 0,
                 template_name: if concrete_tys.is_empty() { None } else { Some(base_name.to_string()) },
                 specialization_args: concrete_tys.to_vec(),
             });
        } else {
             let reg = self.struct_registry_mut();
             reg.insert(key.clone(), StructInfo {
                 name: mangled.clone(), fields: HashMap::new(), field_order: Vec::new(), field_alignments: Vec::new(),
                 template_name: if concrete_tys.is_empty() { None } else { Some(base_name.to_string()) },
                 specialization_args: concrete_tys.to_vec(),
             });
        }

        // 7. Recursive expansion: process immediately to ensure
        // dependencies are sized before dependents
        {
            self.monomorphizer_mut().pending_set.insert(mangled.clone());
        }

        // EXPAND
        if is_enum {
             let res = self.expand_enum_structure(base_name, concrete_tys);
             match res {
                 Ok(info) => { self.enum_registry_mut().insert(key.clone(), info); }
                 Err(e) => {
                     self.enum_registry_mut().remove(&key);
                     self.monomorphizer_mut().pending_set.remove(&mangled);
                     return Err(e);
                 }
             }
        } else {
             let res = self.expand_template_structure(base_name, concrete_tys);
             match res {
                 Ok(info) => { 
                     self.struct_registry_mut().insert(key.clone(), info); 
                 }
                 Err(e) => {
                     self.struct_registry_mut().remove(&key);
                     self.monomorphizer_mut().pending_set.remove(&mangled);
                     return Err(e);
                 }
             }
        };

        // HOISTING (Immediate)
        let full_ty = if is_enum { crate::types::Type::Enum(mangled.clone()) } else { crate::types::Type::Struct(mangled.clone()) };
        if let Ok(mlir_def) = full_ty.to_mlir_storage_type(self) {
             if mlir_def.contains(", (") || mlir_def.contains(", ()") {
                let dummy_name = format!("__typedef_{}", mangled);
                let d = self.decl_out_mut();
                d.push_str(&format!("  llvm.mlir.global private @{}() : {} {{\n", dummy_name, mlir_def));
                d.push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_def));
                d.push_str(&format!("    llvm.return %0 : {}\n", mlir_def));
                d.push_str("  }\n");
             }
        }

        self.monomorphizer_mut().pending_set.remove(&mangled);

        Ok(key)
    }

    pub fn drain_work_queue(&mut self) {
        while let Some(task) = self.monomorphizer_mut().work_queue.pop_front() {
            // Setup Context for Self-Resolution
            let old_self = self.current_self_ty().clone();
            let self_type = if task.is_enum { Type::Enum(task.mangled_name.clone()) } else { Type::Struct(task.mangled_name.clone()) };
            *self.current_self_ty_mut() = Some(self_type);

            // Construct Key for Registry Access
            let base_name = &task.template_name;
            let parts: Vec<&str> = base_name.split("__").collect();
            let (path, name) = if parts.len() > 1 {
                 (parts[..parts.len()-1].iter().map(|s| s.to_string()).collect::<Vec<_>>(), parts.last().expect("parts.len() > 1").to_string())
            } else {
                 (vec![], base_name.to_string())
            };
            let key = TypeKey {
                 path,
                 name,
                 specialization: Some(task.args.clone()),
            };

            // EXPAND (No Registry Borrow Here, only Read Templates + Request Spec)
            if task.is_enum {
                if let Ok(info) = self.expand_enum_structure(&task.template_name, &task.args) {
                    // Commit to Registry
                    if let Some(entry) = self.enum_registry_mut().get_mut(&key) {
                        *entry = info;
                    }
                }
            } else if let Ok(info) = self.expand_template_structure(&task.template_name, &task.args) {
                // Commit to Registry
                if let Some(entry) = self.struct_registry_mut().get_mut(&key) {
                    *entry = info;
                }
            };

            // Restore Context
            *self.current_self_ty_mut() = old_self;

            // Mark as Done (Removing from pending_set is optional if registry is checked first, but good for cleanup)
            self.monomorphizer_mut().pending_set.remove(&task.mangled_name);

            // Emit the struct/enum definition into decl_out immediately after
            // specialization so the type is defined before any function body uses it.
            let full_ty = if task.is_enum { crate::types::Type::Enum(task.mangled_name.clone()) } else { crate::types::Type::Struct(task.mangled_name.clone()) };
            
            // Generate the full body definition string (e.g. !llvm.struct<"Vec_u8", (...)>)
            // to_mlir_storage_type triggers the registry lookup and body formatting.
            if let Ok(mlir_def) = full_ty.to_mlir_storage_type(self) {
                // Only hoist if the returned string contains a body definition (i.e. has fields or explicitly empty body).
                // If it returns an opaque reference (e.g. !llvm.struct<"Foo">), it means it was already emitted elsewhere.
                if mlir_def.contains(", (") || mlir_def.contains(", ()") {
                    let dummy_name = format!("__typedef_{}", task.mangled_name);
                    let d = self.decl_out_mut();
                    d.push_str(&format!("  llvm.mlir.global private @{}() : {} {{\n", dummy_name, mlir_def));
                    d.push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_def));
                    d.push_str(&format!("    llvm.return %0 : {}\n", mlir_def));
                    d.push_str("  }\n");
                }
            }
        }
        
        // Finalize (Freeze)
        self.monomorphizer_mut().is_frozen = true;
    }

    pub fn map_generics(&mut self, generics: &Option<crate::grammar::Generics>, args: &[Type], template_name: &str, old_const_vals: &mut Vec<(String, Option<ConstValue>)>) {

         if let Some(gen) = generics {
             for (i, param) in gen.params.iter().enumerate() {
                 if let Some(concrete) = args.get(i) {
                     let c_t: Type = concrete.clone();
                     let name = match param {
                         crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                         crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                     };
                     if Type::is_protected_name(&name) {
                           panic!("Shadowing Guard: Generic parameter '{}' covers a protected type name in template '{}'", name, template_name);
                      }
                     self.current_type_map_mut().insert(name.clone(), c_t.clone());

                     
                     // Const Generic Injection
                     if let Type::Struct(val_str) = &c_t {
                         if let Ok(int_val) = val_str.parse::<i64>() {
                             let old = self.evaluator.constant_table.insert(name.clone(), ConstValue::Integer(int_val));
                             old_const_vals.push((name, old));
                         }
                     }
                 }
             }
         }
    }

    /// Performs the structural expansion of a template by mapping generic
    /// parameters to concrete arguments and resolving field types.
    /// This is side-effect free w.r.t the struct registry.
    pub fn expand_template_structure(&mut self,
        template_name: &str,
        args: &[Type],
    ) -> Result<StructInfo, String> {
        // 1. Transactional Read: Extract Template Data
        // generics and fields are cloned to free struct_templates for the next level of recursion.
        let templates = self.struct_templates();
        let template = match templates.get(template_name) {
            Some(t) => t.clone(),
            None => return Err(format!("Template '{}' not found in registry.", template_name)),
        };
        let generics = template.generics.clone();
        let fields = template.fields.clone();

        // Fix: Context Swap to Template Definition Scope to prevent Key Drift
        // This makes sure that field resolution (e.g. "GlobalSlabAlloc") happens in the std lib context, NOT the user context.
        let mut _import_guard = None;
        if let Some(registry) = self.config.registry {
             let parts: Vec<&str> = template_name.split("__").collect();
             if parts.len() > 1 {
                 for (pkg_name, mod_info) in &registry.modules {
                      let pkg_mangled = pkg_name.replace(".", "__");
                      let prefix = format!("{}__", pkg_mangled);
                      if template_name.starts_with(&prefix) {
                           let mut combined_imports = mod_info.imports.clone();
                           // Synthesize self-imports ONLY for non-generic types
                           // Generic types (like Vec<T>, SlabCache<SIZE>) should be resolved
                           // via their categorical export metadata which preserves generic_params.
                           {
                                let pkg_prefix_ident = format!("{}__", pkg_mangled);
                                
                                // Only add non-generic struct templates as simple aliases
                                for (s_name, s_def) in &mod_info.struct_templates {
                                     // Skip generic templates - they need explicit instantiation
                                     let has_generics = s_def.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false);
                                     if has_generics {
                                         continue;
                                     }
                                     
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                                
                                // Concrete (non-template) structs can be aliased directly
                                for s_name in mod_info.structs.keys() {
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                           }
                           // Direct import swap (ImportContextGuard expects CodegenContext)
                           let old_imports = std::mem::replace(&mut *self.imports_mut(), combined_imports);
                           _import_guard = Some(old_imports);
                           break; 
                      }
                 }
             }

        }

        // 2. Validate Argument Count
        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
            // Instead of hard error, return placeholder for deferred expansion
            // This handles cases like Vec<T> inside String definition - the T will be
            // substituted later when the actual specialization is requested with concrete args.
            // Only log for debugging, don't fail compilation.

            // Restore imports if they were swapped for template definition scope
            if let Some(old_imports) = _import_guard {
                *self.imports_mut() = old_imports;
            }
            
            // Return a stub StructInfo with the template name - indicates "unspecialized"
            return Ok(StructInfo {
                name: template_name.to_string(),
                fields: std::collections::HashMap::new(),
                field_order: vec![],
                field_alignments: vec![],
                template_name: Some(template_name.to_string()),
                specialization_args: vec![],
            });
        }



        // 3. State Snapshot: Prepare new type mapping
        let old_map = self.current_type_map().clone();
        let old_generic_args = self.current_generic_args().clone();

        let mut type_map = old_map.clone();
        
        if let Some(gen) = &generics {
            for (param, arg) in gen.params.iter().zip(args.iter()) {
                 let name = match param {
                     crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                     crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                 };
                 type_map.insert(name, arg.clone());
            }
        }

        // 4. Transactional Update: Set the recursion context
        {
            *self.current_type_map_mut() = type_map;
            *self.current_generic_args_mut() = args.to_vec();
        }

        // 5. Recursive Discovery: Map fields in the new context
        let mut resolved_fields = HashMap::new();
        let mut field_order = Vec::new();
        let mut field_alignments = Vec::new();

        for (i, field) in fields.iter().enumerate() {
            // resolve_type is recursive and might access struct_templates/current_type_map
            let mut field_ty = resolve_type(self, &field.ty);

            // Handle @packed attribute
            if field.attributes.iter().any(|a| a.name == "packed") {
                 if let Type::Array(inner, len, _) = field_ty {
                      field_ty = Type::Array(inner, len, true);
                 }
            }
            
            let align = crate::grammar::attr::extract_align(&field.attributes);

            resolved_fields.insert(field.name.to_string(), (i, field_ty.clone()));
            field_order.push(field_ty);
            field_alignments.push(align);
        }
        
        // 6. Transactional Restore: Roll back the context
        {
            *self.current_type_map_mut() = old_map;
            *self.current_generic_args_mut() = old_generic_args;
        }
        // Restore imports that were swapped for template definition scope.
        // Without this, the caller's import context is permanently clobbered
        // with the template's module imports (e.g., Slice's 1-import context
        // overwrites main's 21-import context).
        if let Some(old_imports) = _import_guard {
            *self.imports_mut() = old_imports;
        }

        // Phase B: API Surface Discovery (Eager Method Registration)
        let methods = self.find_methods_for_template(template_name);
        for method_name in methods {
             // Skip generic methods. They require inference/turbofish at call site.
             // Registry stores full mangled name in 'name' field with empty path for Struct types.
             let key = crate::types::TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
             
             if let Some((func, _, _)) = self.trait_registry().get_legacy(&key, &method_name) {
                 if let Some(g) = &func.generics {
                     if !g.params.is_empty() {
                         continue; 
                     }
                 }
             } 

             let full_name = format!("{}__{}", template_name, method_name);
             let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
             let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }


        // 7. Return Metadata
        Ok(StructInfo {
            name: self.specialize_template(template_name, args, false)?.mangle(),
            fields: resolved_fields,
            field_order,
            field_alignments,
            template_name: Some(template_name.to_string()),
            specialization_args: args.to_vec(),
        })
    }

    pub fn expand_enum_structure(&mut self,
        template_name: &str,
        args: &[Type],
    ) -> Result<EnumInfo, String> {
         // 1. Transactional Read: Extract Enum Template Data
        let (generics, variants) = {
            let templates = self.enum_templates();
            let template = templates.get(template_name)
                .cloned()
                .ok_or_else(|| format!("Enum Template '{}' not found", template_name))?;
            (template.generics.clone(), template.variants.clone())
        };

        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
             return Err(format!("Generic mismatch for enum {}", template_name));
        }

        // 3. State Snapshot
        let old_map = self.current_type_map().clone();
        let old_generic_args = self.current_generic_args().clone();

        let mut type_map = old_map.clone();
        if let Some(gen) = &generics {
            for (param, arg) in gen.params.iter().zip(args.iter()) {
                 let name = match param {
                     crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                     crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                 };
                 type_map.insert(name, arg.clone());
            }
        }

        // 4. Transactional Update: Set recursion context
        {
            *self.current_type_map_mut() = type_map;
            *self.current_generic_args_mut() = args.to_vec();
        }
        
        let mut resolved_variants = Vec::new();
        let mut max_payload_size = 0;
        
        // 5. Recursive Discovery
        for (idx, v) in variants.iter().enumerate() {
             let p_ty = v.ty.as_ref().map(|sy| crate::codegen::type_bridge::resolve_type(self, sy));
             if let Some(ref ty) = p_ty {
                 let size = ty.size_of(self.struct_registry());
                 if size > max_payload_size { max_payload_size = size; }
             }
             resolved_variants.push((v.name.to_string(), p_ty, idx as i32));
        }

        // 6. Transactional Restore
        {
            *self.current_type_map_mut() = old_map;
            *self.current_generic_args_mut() = old_generic_args;
        }

        // Phase B: API Surface Discovery
        let methods = self.find_methods_for_template(template_name);
        for method_name in methods {
             // Skip generic methods. They require inference/turbofish at call site.
             // Registry stores full mangled name in 'name' field with empty path for Struct types.
             let key = crate::types::TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
             

             if let Some((func, _, _)) = self.trait_registry().get_legacy(&key, &method_name) {
                 if let Some(g) = &func.generics {
                     if !g.params.is_empty() {

                         continue; 
                     }
                 }
             }

             let full_name = format!("{}__{}", template_name, method_name);
             let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
             let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }


        Ok(EnumInfo {
            name: self.specialize_template(template_name, args, true)?.mangle(),
            variants: resolved_variants,
            max_payload_size,
            template_name: Some(template_name.to_string()),
            specialization_args: args.to_vec(),
        })
    }

}


pub use crate::codegen::types::zero_attr::zero_attr;
pub use crate::codegen::types::emit::{emit_const, emit_global_def};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::context::CodegenContext;
    use crate::registry::EnumInfo;
    use crate::grammar::SaltFile;

    #[test]
    fn test_enum_payload_packing() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let name = "PackingEnum".to_string();
        let variants = vec![
            ("A".to_string(), Some(Type::U8), 0),
            ("B".to_string(), Some(Type::Array(Box::new(Type::F64), 8, false)), 1),
        ];

        let info = EnumInfo {
            name: name.clone(),
            variants,
            max_payload_size: 64,
            template_name: None,
            specialization_args: vec![],
        };
        let key = TypeKey { path: vec![], name: name.clone(), specialization: None };
        ctx.enum_registry_mut().insert(key, info);

        let ty = Type::Enum(name);
        let mlir = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();
        // After enum type resolution fix: registered enums return their type alias
        // The inline struct definition with payload is emitted separately in type definitions
        assert_eq!(mlir, "!struct_PackingEnum", "Registered enum should use type alias");
    }

    // =========================================================================
    // TDD: Usize (MLIR index) ↔ I64 type conversion
    // =========================================================================
    // Bug context: The compiler generates MLIR `index` for `usize` params but
    // tracks them as `I64` in local_vars, causing `as i64` casts to be no-ops.
    // These tests ensure the conversion functions correctly emit arith.index_cast.

    #[test]
    fn test_usize_and_i64_are_distinct_types() {
        // CRITICAL: Type::Usize and Type::I64 must NOT be equal.
        // If they were, emit_cast's `if ty == target_ty` check would skip
        // the arith.index_cast, leaving index-typed values in i64 operations.
        assert_ne!(Type::Usize, Type::I64,
            "Type::Usize and Type::I64 must be distinct types");
        assert_ne!(Type::Usize, Type::U64,
            "Type::Usize and Type::U64 must be distinct types");
    }

    #[test]
    fn test_promote_numeric_usize_to_i64_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%arg_len", &Type::Usize, &Type::I64));

        assert!(result.is_ok(), "promote_numeric(Usize, I64) should succeed");
        assert!(out.contains("arith.index_cast"),
            "Usize→I64 must emit arith.index_cast, got: {}", out);
        assert!(out.contains("index to i64"),
            "Cast should be 'index to i64', got: {}", out);
    }

    #[test]
    fn test_promote_numeric_i64_to_usize_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%val", &Type::I64, &Type::Usize));

        assert!(result.is_ok(), "promote_numeric(I64, Usize) should succeed");
        assert!(out.contains("arith.index_cast"),
            "I64→Usize must emit arith.index_cast, got: {}", out);
        assert!(out.contains("i64 to index"),
            "Cast should be 'i64 to index', got: {}", out);
    }

    #[test]
    fn test_cast_numeric_usize_to_i64_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| cast_numeric(lctx, &mut out, "%arg_len", &Type::Usize, &Type::I64));

        assert!(result.is_ok(), "cast_numeric(Usize, I64) should succeed");
        assert!(out.contains("arith.index_cast"),
            "cast_numeric(Usize, I64) must emit arith.index_cast, got: {}", out);
    }

    #[test]
    fn test_usize_identity_does_not_emit_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg2 = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg2);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%val", &Type::Usize, &Type::Usize));

        assert!(result.is_ok(), "promote_numeric(Usize, Usize) should succeed");
        assert!(out.is_empty(),
            "Usize→Usize should be identity (no MLIR emitted), got: {}", out);
    }

    // =========================================================================
    // TDD: Atomic<T> Type Emission — The Slab Memory Leak Root Cause
    // =========================================================================
    // Bug: Atomic<i32> globals emitted as `!llvm.ptr` with `null` init instead
    // of `i32` with `0 : i32` init. This causes LLVM Translation to reject the
    // MLIR with: "Global variable initializer type does not match global variable type!"
    //
    // Call graph layers to fix:
    //   Layer 0: to_mlir_type_simple(Atomic<T>) → T's MLIR type  [already works]
    //   Layer 1: zero_attr(Atomic<T>) → recurse to inner T
    //   Layer 2: to_mlir_storage_type_simple(Atomic<T>) → T's storage type
    //   Layer 3: emit_global_def sees Atomic<T> → unwraps to T for init_val

    // --- Layer 0: to_mlir_type_simple (already correct, assert for safety) ---
    #[test]
    fn test_atomic_i32_mlir_type_simple() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.to_mlir_type_simple(), "i32",
            "Atomic<i32> MLIR type should be 'i32', not '!llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_mlir_type_simple() {
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.to_mlir_type_simple(), "i64",
            "Atomic<u64> MLIR type should be 'i64'");
    }

    // --- Layer 1: zero_attr should recurse into inner type ---
    #[test]
    fn test_atomic_i32_zero_attr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ty = Type::Atomic(Box::new(Type::I32));
        let result = ctx.with_lowering_ctx(|lctx| zero_attr(lctx, &ty));
        assert!(result.is_ok(), "zero_attr(Atomic<i32>) should succeed");
        assert_eq!(result.unwrap(), "0 : i32",
            "zero_attr(Atomic<i32>) must be '0 : i32', not 'null : !llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_zero_attr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ty = Type::Atomic(Box::new(Type::U64));
        let result = ctx.with_lowering_ctx(|lctx| zero_attr(lctx, &ty));
        assert!(result.is_ok(), "zero_attr(Atomic<u64>) should succeed");
        assert_eq!(result.unwrap(), "0 : i64",
            "zero_attr(Atomic<u64>) must be '0 : i64', not 'null : !llvm.ptr'");
    }

    // --- Layer 2: to_mlir_storage_type_simple should unwrap to inner type ---
    #[test]
    fn test_atomic_i32_storage_type_simple() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.to_mlir_storage_type_simple(), "i32",
            "Atomic<i32> storage type should be 'i32', not '!llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_storage_type_simple() {
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.to_mlir_storage_type_simple(), "i64",
            "Atomic<u64> storage type should be 'i64'");
    }

    // --- Layer 3: k_is_ptr_type should NOT match Atomic ---
    #[test]
    fn test_atomic_is_not_ptr_type() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert!(!ty.k_is_ptr_type(),
            "Atomic<i32> is NOT a pointer type — it is a scalar wrapper");
    }

    // --- Layer 4: size_of should reflect inner type, not pointer ---
    #[test]
    fn test_atomic_i32_size_of() {
        let reg = std::collections::HashMap::new();
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.size_of(&reg), 4,
            "Atomic<i32> should be 4 bytes, not 8 (pointer size)");
    }

    #[test]
    fn test_atomic_u64_size_of() {
        let reg = std::collections::HashMap::new();
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.size_of(&reg), 8,
            "Atomic<u64> should be 8 bytes");
    }
}
