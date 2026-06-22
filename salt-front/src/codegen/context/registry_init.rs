use std::collections::HashMap;
use crate::codegen::context::CodegenContext;
use crate::grammar::{GenericParam, ImportDecl, SaltFn, SaltImpl};
use crate::registry::{ModuleInfo, StructInfo};
use crate::types::{Type, TypeKey};
use proc_macro2::Span;

impl<'a> CodegenContext<'a> {

    pub fn init_registry_definitions(&self) {
        self.suppress_specialization.set(true);
        if let Some(reg) = self.registry {
            for module_info in reg.modules.values() {
                self.populate_struct_templates(module_info);
                self.populate_concrete_structs(module_info);
                self.populate_enum_templates(module_info);
                self.populate_concrete_enums(module_info);
                self.init_registry_exports(module_info);
            }

            // Pass 2: Populate Impls (Resolution Dependencies Resolved)
            for module_info in reg.modules.values() {
                self.init_registry_impls(module_info);
            }
        }
        self.suppress_specialization.set(false);
    }

    fn populate_struct_templates(&self, module_info: &ModuleInfo) {
        let pkg_prefix = module_info.package.replace(".", "__") + "__";
        for (struct_name, struct_def) in &module_info.struct_templates {
            let mangled = format!("{}{}", pkg_prefix, struct_name);
            let mut s_def = struct_def.clone();
            s_def.name = syn::Ident::new(&mangled, struct_def.name.span());
            self.struct_templates_mut().insert(mangled, s_def);
        }
    }

    fn populate_concrete_structs(&self, module_info: &ModuleInfo) {
        let pkg_prefix = module_info.package.replace(".", "__") + "__";
        let path: Vec<String> = module_info.package.split('.').map(|s| s.to_string()).collect();
        for (struct_name, fields_vec) in &module_info.structs {
            let mangled = format!("{}{}", pkg_prefix, struct_name);
            let mut fields = HashMap::new();
            let mut field_order = Vec::new();
            for (i, (fname, fty)) in fields_vec.iter().enumerate() {
                fields.insert(fname.clone(), (i, fty.clone()));
                field_order.push(fty.clone());
            }
            let key = TypeKey {
                path: path.clone(),
                name: struct_name.clone(),
                specialization: None,
            };
            self.struct_registry_mut().insert(key, StructInfo {
                name: mangled,
                fields,
                field_order: field_order.clone(),
                field_alignments: vec![None; field_order.len()],
                template_name: None,
                specialization_args: vec![],
            });
        }
    }

    fn populate_enum_templates(&self, module_info: &ModuleInfo) {
        let pkg_prefix = module_info.package.replace(".", "__") + "__";
        for (enum_name, enum_def) in &module_info.enum_templates {
            let mangled = format!("{}{}", pkg_prefix, enum_name);
            let mut e_def = enum_def.clone();
            e_def.name = syn::Ident::new(&mangled, enum_def.name.span());
            self.enum_templates_mut().insert(mangled, e_def);
        }
    }

    fn populate_concrete_enums(&self, module_info: &ModuleInfo) {
        let pkg_prefix = module_info.package.replace(".", "__") + "__";
        let path: Vec<String> = module_info.package.split('.').map(|s| s.to_string()).collect();
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
    }

    fn intrinsic_mlir_decl(&self, name: &str, args: &[Type], ret: &Type) -> String {
        let args_mlir: Vec<String> = args.iter()
            .filter_map(|arg| self.resolve_mlir_type(arg).ok())
            .collect();
        let ret_mlir = if *ret == Type::Unit {
            "()".to_string()
        } else if let Ok(mlir_ty) = self.resolve_mlir_type(ret) {
            mlir_ty
        } else {
            "()".to_string()
        };
        format!("  func.func private @{}({}) -> {}\n", name, args_mlir.join(", "), ret_mlir)
    }

    fn init_registry_exports(&self, module_info: &ModuleInfo) {
        for (name, export) in &module_info.exports {
            if export.kind != crate::registry::SymbolKind::Intrinsic {
                continue;
            }
            if self.external_decls().contains(name) {
                continue;
            }
            if let Some((args, ret)) = module_info.functions.get(name) {
                self.globals_mut().insert(
                    name.clone(),
                    Type::Fn(args.clone(), Box::new(ret.clone())),
                );
                self.external_decls_mut().insert(name.clone());
                let decl_str = self.intrinsic_mlir_decl(name, args, ret);
                self.pending_func_decls_mut().insert(name.clone(), decl_str);
            }
        }
    }

    fn init_registry_impls(&self, module_info: &ModuleInfo) {
        let pkg_path: Vec<String> = module_info.package.split('.').map(|s| s.to_string()).collect();

        for (impl_item, impl_imports) in &module_info.impls {
            match impl_item {
                SaltImpl::Methods { target_ty, methods, generics } => {
                    self.register_impl_methods(module_info, &pkg_path, impl_imports, target_ty, methods, generics);
                }
                SaltImpl::Trait { target_ty, methods, generics, .. } => {
                    // Flatten trait methods into the implementing type's method table
                    self.register_impl_methods(module_info, &pkg_path, impl_imports, target_ty, methods, generics);
                }
                SaltImpl::Concept { .. } => {}
            }
        }
    }

    fn build_combined_imports(
        &self,
        module_info: &ModuleInfo,
        impl_imports: &[ImportDecl],
    ) -> Vec<ImportDecl> {
        let mut combined = impl_imports.to_vec();
        let pkg_prefix = module_info.package.replace(".", "__") + "__";
        let mut self_imps = Vec::new();

        for (s_name, s_def) in &module_info.struct_templates {
            let has_generics = s_def.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false);
            if has_generics {
                continue;
            }
            let mangled = format!("{}{}", pkg_prefix, s_name);
            let mangled_ident = syn::Ident::new(&mangled, Span::call_site());
            let mut p = syn::punctuated::Punctuated::new();
            p.push(mangled_ident);
            self_imps.push(ImportDecl {
                name: p,
                alias: Some(syn::Ident::new(s_name, Span::call_site())),
                group: None,
            });
        }
        for s_name in module_info.structs.keys() {
            let mangled = format!("{}{}", pkg_prefix, s_name);
            let mangled_ident = syn::Ident::new(&mangled, Span::call_site());
            let mut p = syn::punctuated::Punctuated::new();
            p.push(mangled_ident);
            self_imps.push(ImportDecl {
                name: p,
                alias: Some(syn::Ident::new(s_name, Span::call_site())),
                group: None,
            });
        }
        combined.extend(self_imps);
        combined
    }

    fn register_single_impl_method(
        &self,
        m: &SaltFn,
        generics: &Option<crate::grammar::Generics>,
        target_mangled: &str,
        resolved: &Type,
        impl_key: &TypeKey,
    ) {
        let name = format!("{}__{}", target_mangled, m.name);

        let ret_ty = if let Some(rt) = &m.ret_type {
            Type::from_syn(rt).unwrap_or(Type::Unit)
        } else {
            Type::Unit
        };
        let args: Vec<Type> = m.args.iter()
            .filter_map(|arg| arg.ty.as_ref().and_then(Type::from_syn))
            .collect();
        self.globals_mut().insert(
            name.clone(),
            Type::Fn(args.clone(), Box::new(ret_ty.clone())),
        );

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
        self.generic_impls_mut()
            .insert(name.clone(), (m_clone.clone(), current_imports.clone()));
        self.trait_registry_mut()
            .register_simple(impl_key.clone(), m_clone, Some(resolved.clone()), current_imports);
    }

    fn register_impl_methods(
        &self,
        module_info: &ModuleInfo,
        pkg_path: &[String],
        impl_imports: &[ImportDecl],
        target_ty: &crate::grammar::SynType,
        methods: &[SaltFn],
        generics: &Option<crate::grammar::Generics>,
    ) {
        let saved_imports = self.imports().clone();
        let saved_map = self.current_type_map().clone();
        self.hydrate_impl_generics(generics);
        *self.imports_mut() = self.build_combined_imports(module_info, impl_imports);
        self.register_impl_methods_inner(target_ty, methods, generics, pkg_path);
        *self.imports_mut() = saved_imports;
        *self.current_type_map_mut() = saved_map;
    }

    fn hydrate_impl_generics(&self, generics: &Option<crate::grammar::Generics>) {
        let Some(g) = generics else { return };
        for param in &g.params {
            let name = match param {
                GenericParam::Type { name, .. } => name,
                GenericParam::Const { name, .. } => name,
            };
            self.current_type_map_mut()
                .insert(name.to_string(), Type::Struct(name.to_string()));
        }
    }

    fn register_impl_methods_inner(
        &self,
        target_ty: &crate::grammar::SynType,
        methods: &[SaltFn],
        generics: &Option<crate::grammar::Generics>,
        pkg_path: &[String],
    ) {
        let target = match Type::from_syn(target_ty) {
            Some(t) => t,
            None => return,
        };
        let resolved = self.bridge_resolve_codegen_type(&target);
        let target_mangled = resolved.mangle_suffix();
        *self.current_self_ty_mut() = Some(resolved.clone());
        let impl_key = self.build_impl_key(&resolved, pkg_path, generics);
        for m in methods {
            self.register_single_impl_method(m, generics, &target_mangled, &resolved, &impl_key);
        }
        *self.current_self_ty_mut() = None;
    }

    fn build_impl_key(
        &self,
        resolved: &Type,
        pkg_path: &[String],
        generics: &Option<crate::grammar::Generics>,
    ) -> TypeKey {
        let mut key = resolved.to_key().unwrap_or_else(|| TypeKey {
            path: pkg_path.to_vec(),
            name: resolved.mangle_suffix(),
            specialization: None,
        });
        if key.path.is_empty() && !pkg_path.is_empty() {
            key.path = pkg_path.to_vec();
        }
        if key.specialization.as_ref().is_some_and(|s: &Vec<Type>| !s.is_empty())
            && generics.is_some()
        {
            key.specialization = None;
        }
        key
    }
}
