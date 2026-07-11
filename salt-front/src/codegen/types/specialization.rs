use crate::types::Type;
use crate::codegen::context::LoweringContext;

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    
    
    pub(crate) fn populate_explicit_specialization_map(
        &mut self,
        func: &crate::grammar::SaltFn,
        concrete_tys: &[Type],
        st: &Type,
        old_const_vals: &mut Vec<(String, Option<crate::evaluator::ConstValue>)>,
    ) {
        let template_name = if let Type::Struct(name) = st {
            self.struct_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
        } else if let Type::Enum(name) = st {
            self.enum_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
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
                 self.struct_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Enum(name) = st {
                 self.enum_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
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




}
