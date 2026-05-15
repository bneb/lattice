use crate::types::{Type, TypeKey};
use crate::codegen::context::LoweringContext;
use crate::evaluator::ConstValue;
use crate::common::mangling::Mangler;

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    pub fn request_explicit_specialization(&mut self, func_name: &str, override_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        let self_ty = self_ty.map(|mut ty| {
            while let Type::Reference(inner, _) = ty {
                ty = *inner;
            }
            ty
        });
        
        let mangled = override_name.to_string();
        
        if let Some(existing) = self.discovery.specializations.get(&(func_name.to_string(), concrete_tys.clone())) {
            let defined = self.emission.defined_functions.contains(existing);
            let pending = self.expansion.pending_generations.iter().any(|task| task.mangled_name == *existing);

            if !defined && !pending {
                 // Fall through to queue logic!
            } else {
                 return existing.clone();
            }
        }

        self.discovery.specializations.insert((func_name.to_string(), concrete_tys.clone()), mangled.clone());
        
        let file = self.file;
        let found = if let Some(st) = &self_ty {
             let (st_base, method_name) = if let Some((base, method)) = func_name.rsplit_once("__") {
                 (base.to_string(), method.to_string())
             } else {
                 ("".to_string(), func_name.to_string())
             };
             
            let template_name = if let Type::Struct(name) = st {
                 self.discovery.struct_registry.values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Enum(name) = st {
                 self.discovery.enum_registry.values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Pointer { .. } = st {
                 "std__core__ptr__Ptr".to_string()
             } else {
                 st_base
             };
             self.discovery.trait_registry.find_method_by_name(&template_name, &method_name, st)
        } else {
             file.items.iter().find_map(|item| {
                 if let crate::grammar::Item::Fn(f) = item {
                     if f.name == func_name { return Some((f.clone(), None, self.discovery.imports.clone())); }
                 }
                 None
             })
        };
        
        if let Some((func, s_ty, imports)) = found {
            let spec_map;
            {
                let old_imports = self.discovery.imports.clone();
                self.discovery.imports = imports.clone();
                let old_map = self.expansion.current_type_map.clone();
                let old_args = self.expansion.current_generic_args.clone();
                let old_self = self.expansion.current_self_ty.clone();
                let mut old_const_vals = Vec::new();
                
                self.expansion.current_generic_args = concrete_tys.clone();
                self.expansion.current_self_ty = s_ty.clone();

                if let Some(st) = &s_ty {
                    let template_name = if let Type::Struct(name) = st {
                        self.discovery.struct_registry.values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
                    } else if let Type::Enum(name) = st {
                        self.discovery.enum_registry.values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
                    } else if let Type::Concrete(name, _) = st {
                        name.clone()
                    } else if let Type::Pointer { .. } = st {
                        "std__core__ptr__Ptr".to_string()
                    } else {
                        "".to_string()
                    };
                    
                     if !template_name.is_empty() {
                         let gen_params = if let Some(s) = self.discovery.struct_templates.get(&template_name) {
                             s.generics.as_ref().map(|g| g.params.clone())
                         } else if let Some(e) = self.discovery.enum_templates.get(&template_name) {
                             e.generics.as_ref().map(|g| g.params.clone())
                         } else { None };
                         
                         if let Some(params) = gen_params {
                              for (i, param) in params.iter().enumerate() {
                                   let pname = match param { crate::grammar::GenericParam::Type { name, .. } => name.to_string(), crate::grammar::GenericParam::Const { name, .. } => name.to_string() };
                                   if let Type::Concrete(_, args) = &st {
                                        if let Some(arg) = args.get(i) {
                                            self.expansion.current_type_map.insert(pname, arg.clone());
                                        }
                                   } else if let Type::Pointer { element, .. } = &st {
                                        if i == 0 {
                                            self.expansion.current_type_map.insert(pname, (**element).clone());
                                        }
                                   } else if let Some(arg) = concrete_tys.get(i) {
                                       self.expansion.current_type_map.insert(pname, arg.clone());
                                   }
                              }
                         }
                     }
                }
                
                if let Some(fn_generics) = &func.generics {
                    let mut struct_generic_names = std::collections::HashSet::new();
                    if let Some(t) = self_ty.as_ref() {
                        let type_name = match t {
                            Type::Struct(name) | Type::Concrete(name, _) => Some(name.clone()),
                            _ => None
                        };
                        if let Some(ref tname) = type_name {
                            let gen_params = if let Some(s) = self.discovery.struct_templates.get(tname) {
                                s.generics.as_ref().map(|g| g.params.clone())
                            } else {
                                self.discovery.enum_templates.get(tname).and_then(|e| e.generics.as_ref()).map(|g| g.params.clone())
                            };
                            if let Some(params) = gen_params {
                                for p in &params {
                                    let name = match p {
                                        crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                        crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                    };
                                    struct_generic_names.insert(name);
                                }
                            }
                        }
                    }
                    
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
                        self.map_generics(&Some(method_only_generics), &method_args, &func.name.to_string(), &mut old_const_vals);
                    }
                }

                spec_map = self.expansion.current_type_map.clone();

                self.expansion.current_type_map = old_map;
                self.expansion.current_generic_args = old_args;
                self.expansion.current_self_ty = old_self;
                self.discovery.imports = old_imports;
            }

            let path_segments: Vec<String> = if func_name.contains("__") {
                 func_name.split("__").map(|s| s.to_string()).collect()
            } else {
                 vec![]
            };
            let pkg_path = if path_segments.len() > 1 {
                path_segments[0..path_segments.len()-1].to_vec()
            } else {
                vec![]
            };

            let task = crate::codegen::collector::MonomorphizationTask {
                identity: crate::types::TypeKey { 
                    path: pkg_path, 
                    name: func.name.to_string(), 
                    specialization: None 
                },
                mangled_name: mangled.clone(),
                func: func.clone(),
                concrete_tys: concrete_tys.clone(),
                self_ty: s_ty.clone(),
                imports: imports.clone(),
                type_map: spec_map,
            };

            self.expansion.pending_generations.push_back(task);
        } else {
             eprintln!("Error: Function '{}' not found for specialization.", func_name);
        }
        
        mangled
    }

    pub fn request_specialization(&mut self, func_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        let mut final_concrete_tys = concrete_tys.clone();
        if let Some(ref st) = self_ty {
            if let Type::Concrete(_, args) = st {
                 let mut merged = args.clone();
                 merged.extend(concrete_tys);
                 final_concrete_tys = merged;
            } else if let Type::Pointer { element, .. } = st {
                 let mut merged = vec![(**element).clone()];
                 merged.extend(concrete_tys);
                 final_concrete_tys = merged;
            }
        }
        
        let mangled = Mangler::mangle_specialized(func_name, &final_concrete_tys);
        self.request_explicit_specialization(func_name, &mangled, final_concrete_tys, self_ty)
    }

    pub fn specialize_template(&mut self, base_name: &str, concrete_tys: &[Type], is_enum: bool) -> Result<TypeKey, String> {
        let parts: Vec<&str> = base_name.split("__").collect();
        let (path, name) = if parts.len() > 1 {
             (parts[..parts.len()-1].iter().map(|s| s.to_string()).collect::<Vec<_>>(), parts.last().ok_or_else(|| "parts.len() > 1".to_string())?.to_string())
        } else {
             (vec![], base_name.to_string())
        };
        let key = TypeKey {
             path,
             name,
             specialization: Some(concrete_tys.to_vec()),
        };
        
        let mangled = key.mangle();

        let exists = if is_enum {
            self.discovery.enum_registry.contains_key(&key)
        } else {
            self.discovery.struct_registry.contains_key(&key)
        };

        if !exists && !self.expansion.monomorphizer.pending_set.contains(&mangled) {
            self.expansion.monomorphizer.pending_set.insert(mangled.clone());
            self.expansion.monomorphizer.work_queue.push_back(crate::codegen::context::SpecializationTask {
                template_name: base_name.to_string(),
                args: concrete_tys.to_vec(),
                mangled_name: mangled.clone(),
                is_enum,
            });
        }
        
        Ok(key)
    }

    pub fn drain_work_queue(&mut self) {
        while let Some(task) = self.expansion.monomorphizer.work_queue.pop_front() {
            let old_self = self.expansion.current_self_ty.clone();
            let self_type = if task.is_enum { Type::Enum(task.mangled_name.clone()) } else { Type::Struct(task.mangled_name.clone()) };
            self.expansion.current_self_ty = Some(self_type);

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

            if task.is_enum {
                if let Ok(info) = self.expand_enum_structure(&task.template_name, &task.args) {
                    if let Some(entry) = self.discovery.enum_registry.get_mut(&key) {
                        *entry = info;
                    }
                }
            } else {
                if let Ok(info) = self.expand_template_structure(&task.template_name, &task.args) {
                    if let Some(entry) = self.discovery.struct_registry.get_mut(&key) {
                        *entry = info;
                    }
                }
            };

            self.expansion.current_self_ty = old_self;
        }
    }

    pub fn map_generics(&mut self, generics: &Option<crate::grammar::Generics>, args: &[Type], template_name: &str, old_const_vals: &mut Vec<(String, Option<ConstValue>)>) {
        if let Some(g) = generics {
            for (i, param) in g.params.iter().enumerate() {
                let pname = match param {
                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                };
                if let Some(arg) = args.get(i) {
                    self.expansion.current_type_map.insert(pname.clone(), arg.clone());
                    if let Type::Struct(val_str) = arg {
                         if let Ok(val) = val_str.parse::<i64>() {
                             old_const_vals.push((pname.clone(), self.discovery.evaluator.set_const(&pname, ConstValue::Integer(val))));
                         }
                    }
                } else {
                    eprintln!("WARNING: Missing generic argument for {} in template {}", pname, template_name);
                }
            }
        }
    }
}
