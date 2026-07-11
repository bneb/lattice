use crate::types::{Type, TypeKey};
use crate::codegen::context::LoweringContext;
use crate::registry::{StructInfo, EnumInfo};
use std::collections::HashMap;
use crate::codegen::types::layout::flatten_nested_ptr;
use crate::codegen::types::substitution::substitute_generics_ctx;
use crate::codegen::types::traits::{has_unresolved_type_params, validate_trait_constraints};

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
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
                 self.struct_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Enum(name) = st {
                 self.enum_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
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
                        let tname = self.struct_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone());
                        (tname, vec![])
                    } else if let Type::Enum(name) = st {
                        let tname = self.enum_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone());
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

}
