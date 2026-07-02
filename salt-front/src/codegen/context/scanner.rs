use std::collections::HashMap;
use crate::codegen::context::CodegenContext;
use crate::common::mangling::Mangler;
use crate::grammar::{ConstDef, EnumDef, ExternFnDecl, GenericParam, GlobalDef, ImportDecl, Item, SaltFile, SaltFn, SaltImpl, StructDef};
use crate::types::{Type, TypeKey};
use proc_macro2::Span;

impl<'a> CodegenContext<'a> {

    pub fn scan_imports_from_file(&self, file: &SaltFile) {
        self.imports_mut().extend(file.imports.clone());
    }

    /// Inject self-imports so that items in the current file can resolve
    /// unqualified names to their package-mangled forms during scanning.
    /// This is a renamed variant of context::inject_self_imports that
    /// accepts a pre-computed pkg_prefix for use during scan_defs_from_file.
    fn inject_scan_self_imports(&self, file: &SaltFile, pkg_prefix: &str) {
        let mut self_imports = Vec::new();
        for item in &file.items {
            let (ident_name, mangled_str) = match item {
                Item::Struct(s) => (&s.name, format!("{}{}", pkg_prefix, s.name)),
                Item::Enum(e) => (&e.name, format!("{}{}", pkg_prefix, e.name)),
                Item::Fn(f) => {
                    let m = if f.attributes.iter().any(|a| a.name == "no_mangle") {
                        f.name.to_string()
                    } else {
                        format!("{}{}", pkg_prefix, f.name)
                    };
                    (&f.name, m)
                }
                Item::ExternFn(e) => (&e.name, e.name.to_string()),
                Item::Global(g) => (&g.name, format!("{}{}", pkg_prefix, g.name)),
                Item::Const(c) => (&c.name, format!("{}{}", pkg_prefix, c.name)),
                _ => continue,
            };
            let mangled_ident = syn::Ident::new(&mangled_str, Span::call_site());
            let mut p = syn::punctuated::Punctuated::new();
            p.push(mangled_ident);
            self_imports.push(ImportDecl {
                name: p,
                alias: Some(ident_name.clone()),
                group: None,
            });
        }
        self.imports_mut().extend(self_imports);
    }

    pub fn scan_defs_from_file(&self, file: &SaltFile, is_main_file: bool) -> Result<(), String> {
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

        self.imports_mut().clear();
        self.imports_mut().extend(file.imports.clone());

        if !pkg_prefix.is_empty() {
            self.inject_scan_self_imports(file, &pkg_prefix);
        }

        for item in &file.items {
            match item {
                Item::Global(g) => self.scan_def_global(g, &pkg_prefix, is_main_file)?,
                Item::Fn(f) => self.scan_def_fn(f, &pkg_prefix),
                Item::Impl(i) => self.scan_def_impl(i, &pkg_prefix, &path),
                Item::ExternFn(e) => self.scan_def_extern_fn(e)?,
                Item::Const(c) => self.scan_def_const(c, &pkg_prefix)?,
                Item::Struct(s) => self.scan_def_struct(s, &pkg_prefix, &path)?,
                Item::Enum(e) => self.scan_def_enum(e, &pkg_prefix, &path),
                _ => {}
            }
        }
        self.current_package.replace(saved_pkg);
        Ok(())
    }

    fn scan_def_global(&self, g: &GlobalDef, pkg_prefix: &str, is_main_file: bool) -> Result<(), String> {
        let name = format!("{}{}", pkg_prefix, g.name);
        let ty = self.bridge_resolve_type(&g.ty);
        self.globals_mut().insert(name, ty);

        if is_main_file {
            let mut out = String::new();
            if let Err(e) = self.bridge_emit_global_def(&mut out, g) {
                return Err(format!("Error emitting global {}: {}", g.name, e));
            } else {
                self.decl_out_mut().push_str(&out);
            }
        }
        Ok(())
    }

    fn scan_def_fn(&self, f: &SaltFn, pkg_prefix: &str) {
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
            Type::Unit
        };
        let args: Vec<Type> = f.args.iter()
            .filter_map(|arg| arg.ty.as_ref().map(|t| self.bridge_resolve_type(t)))
            .collect();
        self.globals_mut().insert(name.clone(), Type::Fn(args.clone(), Box::new(ret_ty.clone())));

        let current_imports = self.imports().clone();
        self.generic_impls_mut().insert(name.clone(), (f.clone(), current_imports));
    }

    fn scan_def_impl(&self, i: &SaltImpl, pkg_prefix: &str, path: &[String]) {
        match i {
            SaltImpl::Methods { target_ty, methods, generics } => {
                self.scan_def_impl_methods(target_ty, methods, generics, path);
            }
            SaltImpl::Trait { trait_name, target_ty, methods, generics } => {
                self.scan_def_impl_trait(trait_name, target_ty, methods, generics, pkg_prefix, path);
            }
            _ => {}
        }
    }

    /// Shared body for scan_def_impl_methods and scan_def_impl_trait.
    /// Sets up generics, registers all method signatures, restores state.
    /// Returns the mangled target name for additional use by the trait variant.
    fn scan_impl_body(
        &self,
        target_ty: &crate::grammar::SynType,
        methods: &[SaltFn],
        generics: &Option<crate::grammar::Generics>,
        path: &[String],
    ) -> Option<String> {
        let _saved_map = self.current_type_map().clone();
        self.hydrate_generic_placeholders(generics);

        let target = Type::from_syn(target_ty)?;
        let resolved = self.bridge_resolve_codegen_type(&target);
        let target_mangled = resolved.mangle_suffix();

        *self.current_self_ty_mut() = Some(resolved.clone());

        let mut impl_key = resolved.to_key().unwrap_or_else(|| {
            TypeKey { path: path.to_vec(), name: resolved.mangle_suffix(), specialization: None }
        });
        if impl_key.path.is_empty() && !path.is_empty() {
            impl_key.path = path.to_vec();
        }
        if generics.is_some() {
            impl_key.specialization = None;
        }

        self.register_methods_in_impl(methods, generics, &target_mangled, &resolved, &impl_key);

        *self.current_self_ty_mut() = None;
        Some(target_mangled)
    }

    fn hydrate_generic_placeholders(&self, generics: &Option<crate::grammar::Generics>) {
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

    fn register_methods_in_impl(
        &self,
        methods: &[SaltFn],
        generics: &Option<crate::grammar::Generics>,
        target_mangled: &str,
        resolved: &Type,
        impl_key: &TypeKey,
    ) {
        for m in methods {
            let name = format!("{}__{}", target_mangled, m.name);
            let ret_ty = m.ret_type.as_ref()
                .and_then(Type::from_syn)
                .unwrap_or(Type::Unit);
            let args: Vec<Type> = m.args.iter()
                .filter_map(|arg| arg.ty.as_ref().and_then(Type::from_syn))
                .collect();
            self.globals_mut().insert(
                name.clone(),
                Type::Fn(args.clone(), Box::new(ret_ty.clone())),
            );
            let m_clone = self.merge_method_generics(m, generics);
            let current_imports = self.imports().clone();
            self.generic_impls_mut().insert(
                name.clone(),
                (m_clone.clone(), current_imports.clone()),
            );
            self.trait_registry_mut().register_simple(
                impl_key.clone(),
                m_clone,
                Some(resolved.clone()),
                current_imports,
            );
        }
    }

    fn scan_def_impl_methods(
        &self,
        target_ty: &crate::grammar::SynType,
        methods: &[SaltFn],
        generics: &Option<crate::grammar::Generics>,
        path: &[String],
    ) {
        self.scan_impl_body(target_ty, methods, generics, path);
    }

    fn scan_def_impl_trait(
        &self,
        trait_name: &syn::Ident,
        target_ty: &crate::grammar::SynType,
        methods: &[SaltFn],
        generics: &Option<crate::grammar::Generics>,
        pkg_prefix: &str,
        path: &[String],
    ) {
        let target_mangled = match self.scan_impl_body(target_ty, methods, generics, path) {
            Some(m) => m,
            None => return,
        };
        let _ = self.register_trait_impl(
            target_mangled,
            trait_name.to_string(),
            pkg_prefix.trim_end_matches("__").to_string(),
        );
    }

    fn scan_def_extern_fn(&self, e: &ExternFnDecl) -> Result<(), String> {
        let mangled_name = e.name.to_string();
        if self.external_decls().contains(&mangled_name) {
            return Ok(());
        }
        self.external_decls_mut().insert(mangled_name.clone());

        let ret_ty = if let Some(rt) = &e.ret_type {
            Type::from_syn(rt).unwrap_or(Type::Unit)
        } else {
            Type::Unit
        };

        if !ret_ty.is_ffi_safe() {
            return Err(format!(
                "Extern function `{}` has return type `{:?}` which is not FFI-safe.",
                e.name, ret_ty,
            ));
        }

        let mut args = Vec::new();
        for arg in &e.args {
            let t = arg.ty.as_ref().and_then(Type::from_syn).unwrap_or(Type::Unit);
            if !t.is_ffi_safe() {
                return Err(format!(
                    "Extern function `{}` argument `{}` has type `{:?}` which is not FFI-safe.",
                    e.name, arg.name, t,
                ));
            }
            args.push(t);
        }

        self.globals_mut().insert(mangled_name.clone(), Type::Fn(args, Box::new(ret_ty.clone())));
        Ok(())
    }

    fn scan_def_const(&self, c: &ConstDef, pkg_prefix: &str) -> Result<(), String> {
        let name = format!("{}{}", pkg_prefix, c.name);
        let ty = self.bridge_resolve_type(&c.ty);
        self.globals_mut().insert(name, ty);

        let mut out = String::new();
        if let Err(e) = self.bridge_emit_const(&mut out, c) {
            return Err(format!("Error emitting const {}: {}", c.name, e));
        } else {
            self.decl_out_mut().push_str(&out);
        }
        Ok(())
    }

    fn merge_method_generics(&self, m: &SaltFn, generics: &Option<crate::grammar::Generics>) -> SaltFn {
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
        m_clone
    }

    fn resolve_packed_field_type(&self, f: &crate::grammar::FieldDef, ty: &Type) -> Type {
        if !f.attributes.iter().any(|a| a.name == "packed") {
            return ty.clone();
        }
        if let Type::Array(inner, len, _) = ty {
            return Type::Array(inner.clone(), *len, true);
        }
        ty.clone()
    }

    #[allow(clippy::type_complexity)]
    fn build_struct_fields(
        &self,
        s: &StructDef,
    ) -> (HashMap<String, (usize, Type)>, Vec<Type>, Vec<Option<u32>>) {
        let mut fields = HashMap::new();
        let mut field_order = Vec::new();
        let mut field_alignments = Vec::new();
        for (i, f) in s.fields.iter().enumerate() {
            let ty = self.resolve_packed_field_type(f, &self.bridge_resolve_type(&f.ty));
            let align = crate::grammar::attr::extract_align(&f.attributes);
            fields.insert(f.name.to_string(), (i, ty.clone()));
            field_order.push(ty);
            field_alignments.push(align);
        }
        (fields, field_order, field_alignments)
    }

    fn scan_def_struct(&self, s: &StructDef, pkg_prefix: &str, path: &[String]) -> Result<(), String> {
        let name = format!("{}{}", pkg_prefix, s.name);
        if let Some(_generics) = &s.generics {
            let mut s_mangled = s.clone();
            s_mangled.name = syn::Ident::new(&name, s.name.span());
            self.struct_templates_mut().insert(name.clone(), s_mangled);
            return Ok(());
        }

        let key = TypeKey {
            path: path.to_vec(),
            name: s.name.to_string(),
            specialization: None,
        };

        let (fields, field_order, field_alignments) = self.build_struct_fields(s);

        self.struct_registry_mut().insert(key, crate::registry::StructInfo {
            name: name.clone(),
            fields,
            field_order,
            field_alignments,
            template_name: None,
            specialization_args: vec![],
        });

        self.verify_struct_alignment(s)?;
        Ok(())
    }

    fn check_field_atomic_alignment(
        &self,
        f: &crate::grammar::FieldDef,
        s_name: &str,
        byte_offset: usize,
    ) -> Result<(), String> {
        let has_atomic = f.attributes.iter().any(|a| a.name == "atomic");
        if !has_atomic {
            return Ok(());
        }
        z3_prove_atomic_alignment(&f.name.to_string(), s_name, byte_offset)
    }

    fn verify_struct_alignment(&self, s: &StructDef) -> Result<(), String> {
        let mut byte_offset: usize = 0;
        for f in s.fields.iter() {
            self.check_field_atomic_alignment(f, &s.name.to_string(), byte_offset)?;
            let ty = self.resolve_packed_field_type(f, &self.bridge_resolve_type(&f.ty));
            byte_offset += ty.size_of(&self.struct_registry());
        }
        Ok(())
    }

    fn scan_def_enum(&self, e: &EnumDef, pkg_prefix: &str, path: &[String]) {
        let name = format!("{}{}", pkg_prefix, e.name);
        if let Some(_generics) = &e.generics {
            self.enum_templates_mut().insert(name.clone(), e.clone());
        } else {
            let mut variants = Vec::new();
            let mut max_size = 0;
            for (i, v) in e.variants.iter().enumerate() {
                let p_ty: Option<Type> = if v.tys.is_empty() {
                    None
                } else if v.tys.len() == 1 {
                    Some(self.bridge_resolve_type(&v.tys[0]))
                } else {
                    let types: Vec<Type> = v.tys.iter().map(|t| self.bridge_resolve_type(t)).collect();
                    Some(Type::Tuple(types))
                };
                let size = p_ty.as_ref()
                    .map(|ty| ty.size_of(&self.struct_registry()))
                    .unwrap_or(0);
                max_size = max_size.max(size);
                variants.push((v.name.to_string(), p_ty, i as i32));
            }

            let key = TypeKey {
                path: path.to_vec(),
                name: e.name.to_string(),
                specialization: None,
            };

            self.enum_registry_mut().insert(key, crate::registry::EnumInfo {
                name,
                variants,
                max_payload_size: max_size,
                template_name: None,
                specialization_args: vec![],
            });
        }
    }
}

fn z3_prove_atomic_alignment(
    field_name: &str,
    struct_name: &str,
    byte_offset: usize,
) -> Result<(), String> {
    use crate::z3_shim::ast::Ast;
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
        _ => Err(format!(
            "[Formal Shadow] ALIGNMENT VIOLATION: @atomic field '{}' in struct '{}' \
             is at byte offset {}, which is NOT 16-byte aligned.",
            field_name, struct_name, byte_offset,
        )),
    }
}
