use crate::codegen::context::LoweringContext;
use crate::codegen::collector::MonomorphizationTask;
use crate::types::{Type, TypeKey};
use std::collections::BTreeMap;
use crate::common::mangling::Mangler;

use crate::codegen::seeker::Seeker;

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {

    pub fn resolve_method_to_task(
        &mut self,
        receiver_ty: &Type,
        method_name: &str,
        generics: Vec<Type>,
    ) -> Result<MonomorphizationTask, String> {
        let (func, trait_ty, imports) = self.resolve_method(receiver_ty, method_name)?;

        let mut type_map = BTreeMap::new();
        let mut self_ty = if let Some(t) = trait_ty.as_ref() {
            t.clone()
        } else {
            receiver_ty.clone()
        };
        while let Type::Reference(inner, _) = self_ty {
            self_ty = *inner;
        }

        // 1. Hydrate Impl Scope (e.g., T -> bool)
        let mut base_ty = receiver_ty;
        while let Type::Reference(inner, _) = base_ty {
            base_ty = inner;
        }
        self.hydrate_impl_scope(base_ty, &mut type_map);

        // 2. Hydrate Method Scope
        if let Some(g) = &func.generics {
            for (i, param) in g.params.iter().enumerate() {
                let name = match param {
                    crate::grammar::GenericParam::Type { name, .. } => name,
                    crate::grammar::GenericParam::Const { name, .. } => name,
                };
                if let Some(arg) = generics.get(i) {
                    type_map.insert(name.to_string(), arg.clone());
                }
            }
        }

        // 3. Substitute generics in self_ty
        let concrete_self = substitute_generics(&self_ty, &type_map);

        let mangled_name = Seeker::mangle_method_name(
            &concrete_self.mangle_suffix(),
            method_name,
            &generics,
        );

        let identity = TypeKey {
            path: vec![],
            name: mangled_name.clone(),
            specialization: None,
        };

        Ok(MonomorphizationTask {
            identity,
            mangled_name,
            func,
            concrete_tys: generics,
            self_ty: Some(concrete_self),
            imports,
            type_map,
        })
    }

    /// Hydrate the impl-level generic scope: map struct/enum generic params
    /// to their concrete arguments from the receiver type.
    fn hydrate_impl_scope(&mut self, base_ty: &Type, type_map: &mut BTreeMap<String, Type>) {
        let (name, args) = match base_ty {
            Type::Concrete(n, a) => (n, a.as_slice()),
            Type::Struct(n) => {
                // Fallback: pull mapping from current context's self_ty
                self.hydrate_from_current_self(n, type_map);
                return;
            }
            _ => return,
        };
        let params = self.get_generic_params(name);
        for (i, p) in params.iter().enumerate() {
            let p_name = match p {
                crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
            };
            if let Some(arg) = args.get(i) {
                type_map.insert(p_name, arg.clone());
            }
        }
    }

    fn hydrate_from_current_self(&self, name: &str, type_map: &mut BTreeMap<String, Type>) {
        let current_self = match self.current_self_ty().as_ref() {
            Some(s) => s,
            None => return,
        };
        let mut curr_base = current_self;
        while let Type::Reference(inner, _) = curr_base {
            curr_base = inner;
        }
        let (curr_name, curr_args) = match curr_base {
            Type::Concrete(n, a) => (n, a.as_slice()),
            _ => return,
        };
        if curr_name != name {
            return;
        }
        let params = self.get_generic_params(name);
        for (i, p) in params.iter().enumerate() {
            let p_name = match p {
                crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
            };
            if let Some(arg) = curr_args.get(i) {
                type_map.insert(p_name, arg.clone());
            }
        }
    }

    fn get_generic_params(&self, name: &str) -> Vec<crate::grammar::GenericParam> {
        if let Some(s) = self.struct_templates().get(name) {
            s.generics.as_ref().map(|g| g.params.iter().cloned().collect())
        } else if let Some(e) = self.enum_templates().get(name) {
            e.generics.as_ref().map(|g| g.params.iter().cloned().collect())
        } else {
            None
        }
        .unwrap_or_default()
    }

    pub fn resolve_global_to_task(
        &mut self,
        key: &TypeKey,
        concrete_args: Vec<Type>,
    ) -> Option<MonomorphizationTask> {
        let module_path = key.path.join(".");

        if let Some(reg) = self.config.registry {
            if let Some(module) = reg.modules.get(&module_path) {
                return self.resolve_via_module(key, module, concrete_args);
            } else if module_path.is_empty() {
                return self.resolve_via_local_file(key, concrete_args);
            }
        }
        None
    }

    fn resolve_via_module(
        &self,
        key: &TypeKey,
        module: &crate::registry::ModuleInfo,
        concrete_args: Vec<Type>,
    ) -> Option<MonomorphizationTask> {
        let func = module.function_templates.get(&key.name)?;
        let type_map = Self::build_type_map(&func.generics, &concrete_args);
        let func_name = func.name.to_string();
        let mangled_name = Self::mangle_with_args(&module_path_str(&key.path), &func_name, &concrete_args);
        Some(MonomorphizationTask {
            identity: key.clone(),
            mangled_name,
            func: func.clone(),
            concrete_tys: concrete_args,
            self_ty: None,
            imports: module.imports.clone(),
            type_map,
        })
    }

    fn resolve_via_local_file(
        &self,
        key: &TypeKey,
        concrete_args: Vec<Type>,
    ) -> Option<MonomorphizationTask> {
        for item in &self.config.file.items {
            let f = match item {
                crate::grammar::Item::Fn(f) if f.name == key.name => f,
                _ => continue,
            };
            let pkg_prefix = self.config.file.package.as_ref().map(|pkg| {
                Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()) + "__"
            }).unwrap_or_default();

            let mut task_imports = self.config.file.imports.clone();
            self.inject_self_imports(&pkg_prefix, &mut task_imports);

            let type_map = Self::build_type_map(&f.generics, &concrete_args);
            let fn_name = f.name.to_string();
            let mangled_name = Self::mangle_with_args(&pkg_prefix, &fn_name, &concrete_args);

            return Some(MonomorphizationTask {
                identity: key.clone(),
                mangled_name,
                func: f.clone(),
                concrete_tys: concrete_args,
                self_ty: None,
                imports: task_imports,
                type_map,
            });
        }
        None
    }

    fn inject_self_imports(&self, pkg_prefix: &str, task_imports: &mut Vec<crate::grammar::ImportDecl>) {
        if pkg_prefix.is_empty() {
            return;
        }
        for item in &self.config.file.items {
            let ident_name = match item {
                crate::grammar::Item::Struct(s) => &s.name,
                crate::grammar::Item::Enum(e) => &e.name,
                _ => continue,
            };
            let mangled_str = format!("{}{}", pkg_prefix, ident_name);
            let mangled_ident = syn::Ident::new(&mangled_str, proc_macro2::Span::call_site());
            let mut p = syn::punctuated::Punctuated::new();
            p.push(mangled_ident);
            task_imports.push(crate::grammar::ImportDecl {
                name: p,
                alias: Some(ident_name.clone()),
                group: None,
            });
        }
    }

    fn build_type_map(
        generics: &Option<crate::grammar::Generics>,
        concrete_args: &[Type],
    ) -> BTreeMap<String, Type> {
        let mut map = BTreeMap::new();
        if let Some(g) = generics {
            for (i, p) in g.params.iter().enumerate() {
                let p_name = match p {
                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                };
                if let Some(arg) = concrete_args.get(i) {
                    map.insert(p_name, arg.clone());
                }
            }
        }
        map
    }

    fn mangle_with_args(prefix: &str, func_name: &str, concrete_args: &[Type]) -> String {
        if concrete_args.is_empty() {
            format!("{}{}", prefix, func_name)
        } else {
            let mut s = format!("{}{}", prefix, func_name);
            for arg in concrete_args {
                s.push('_');
                s.push_str(&arg.mangle_suffix());
            }
            s
        }
    }

    pub fn scan_function_for_calls(
        &mut self,
        func: &crate::grammar::SaltFn,
    ) -> Result<Vec<MonomorphizationTask>, String> {
        let mut tasks = Vec::new();
        let mut locals = BTreeMap::new();
        for arg in &func.args {
            if let Some(ty) = &arg.ty {
                locals.insert(
                    arg.name.to_string(),
                    crate::codegen::type_bridge::resolve_type(self, ty),
                );
            }
        }
        let mut seeker = Seeker::new(self);
        for stmt in &func.body.stmts {
            seeker.walk_stmt(stmt, &mut tasks, &mut locals)?;
        }
        Ok(tasks)
    }
}

fn module_path_str(path: &[String]) -> String {
    path.join(".")
}

fn substitute_generics(ty: &Type, map: &BTreeMap<String, Type>) -> Type {
    match ty {
        Type::Struct(name) => map.get(name).cloned().unwrap_or_else(|| Type::Struct(name.clone())),
        Type::Concrete(name, args) => {
            let new_args = args.iter().map(|a| substitute_generics(a, map)).collect();
            Type::Concrete(name.clone(), new_args)
        }
        Type::Reference(inner, m) => {
            Type::Reference(Box::new(substitute_generics(inner, map)), *m)
        }
        Type::Window(inner, r) => {
            Type::Window(Box::new(substitute_generics(inner, map)), r.clone())
        }
        Type::Array(inner, len, packed) => {
            Type::Array(Box::new(substitute_generics(inner, map)), *len, *packed)
        }
        Type::Tuple(elems) => {
            Type::Tuple(elems.iter().map(|e| substitute_generics(e, map)).collect())
        }
        _ => ty.clone(),
    }
}

pub(crate) fn is_task_concrete(task: &MonomorphizationTask) -> bool {
    let name = &task.mangled_name;
    !(name.contains("_T") || name.contains("_E") || name.contains("_SIZE") || name.contains("_U"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_path_str_empty() {
        assert_eq!(module_path_str(&[]), "");
    }

    #[test]
    fn test_module_path_str_single() {
        assert_eq!(module_path_str(&["core".to_string()]), "core");
    }

    #[test]
    fn test_module_path_str_multiple() {
        let path = vec!["std".to_string(), "collections".to_string(), "hashmap".to_string()];
        assert_eq!(module_path_str(&path), "std.collections.hashmap");
    }

    #[test]
    fn test_substitute_generics_struct_replaced() {
        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::I32);
        let ty = Type::Struct("T".to_string());
        let result = substitute_generics(&ty, &map);
        assert_eq!(result, Type::I32);
    }

    #[test]
    fn test_substitute_generics_struct_unchanged() {
        let map = BTreeMap::new();
        let ty = Type::Struct("U".to_string());
        let result = substitute_generics(&ty, &map);
        assert_eq!(result, Type::Struct("U".to_string()));
    }

    #[test]
    fn test_substitute_generics_concrete_nested() {
        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::F32);
        let ty = Type::Concrete("Vec".to_string(), vec![Type::Struct("T".to_string())]);
        let result = substitute_generics(&ty, &map);
        assert_eq!(result, Type::Concrete("Vec".to_string(), vec![Type::F32]));
    }

    #[test]
    fn test_substitute_generics_reference() {
        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::U64);
        let ty = Type::Reference(Box::new(Type::Struct("T".to_string())), false);
        let result = substitute_generics(&ty, &map);
        assert_eq!(result, Type::Reference(Box::new(Type::U64), false));
    }

    #[test]
    fn test_substitute_generics_array() {
        let mut map = BTreeMap::new();
        map.insert("T".to_string(), Type::I8);
        let ty = Type::Array(Box::new(Type::Struct("T".to_string())), 16, false);
        let result = substitute_generics(&ty, &map);
        assert_eq!(result, Type::Array(Box::new(Type::I8), 16, false));
    }

    #[test]
    fn test_substitute_generics_tuple() {
        let mut map = BTreeMap::new();
        map.insert("A".to_string(), Type::Bool);
        map.insert("B".to_string(), Type::F64);
        let ty = Type::Tuple(vec![Type::Struct("A".to_string()), Type::Struct("B".to_string())]);
        let result = substitute_generics(&ty, &map);
        assert_eq!(result, Type::Tuple(vec![Type::Bool, Type::F64]));
    }

    #[test]
    fn test_substitute_generics_primitive_unchanged() {
        let map = BTreeMap::new();
        assert_eq!(substitute_generics(&Type::I32, &map), Type::I32);
        assert_eq!(substitute_generics(&Type::Bool, &map), Type::Bool);
        assert_eq!(substitute_generics(&Type::Unit, &map), Type::Unit);
    }

    #[test]
    fn test_is_task_concrete_simple_name() {
        let task = MonomorphizationTask {
            identity: TypeKey { path: vec![], name: "foo".to_string(), specialization: None },
            mangled_name: "foo".to_string(),
            func: crate::grammar::SaltFn {
                attributes: vec![],
                is_pub: false,
                name: syn::Ident::new("foo", proc_macro2::Span::call_site()),
                generics: None,
                args: syn::punctuated::Punctuated::new(),
                ret_type: None,
                requires: vec![],
                ensures: vec![],
                body: crate::grammar::SaltBlock { stmts: vec![] },
            },
            concrete_tys: vec![],
            self_ty: None,
            imports: vec![],
            type_map: BTreeMap::new(),
        };
        assert!(is_task_concrete(&task));
    }

    #[test]
    fn test_is_task_concrete_with_generic_type_param() {
        let task = MonomorphizationTask {
            identity: TypeKey { path: vec![], name: "foo_T".to_string(), specialization: None },
            mangled_name: "foo_T".to_string(),
            func: crate::grammar::SaltFn {
                attributes: vec![],
                is_pub: false,
                name: syn::Ident::new("foo", proc_macro2::Span::call_site()),
                generics: None,
                args: syn::punctuated::Punctuated::new(),
                ret_type: None,
                requires: vec![],
                ensures: vec![],
                body: crate::grammar::SaltBlock { stmts: vec![] },
            },
            concrete_tys: vec![],
            self_ty: None,
            imports: vec![],
            type_map: BTreeMap::new(),
        };
        assert!(!is_task_concrete(&task));
    }

    #[test]
    fn test_is_task_concrete_with_size_param() {
        let task = MonomorphizationTask {
            identity: TypeKey { path: vec![], name: "foo_SIZE".to_string(), specialization: None },
            mangled_name: "foo_SIZE".to_string(),
            func: crate::grammar::SaltFn {
                attributes: vec![],
                is_pub: false,
                name: syn::Ident::new("foo", proc_macro2::Span::call_site()),
                generics: None,
                args: syn::punctuated::Punctuated::new(),
                ret_type: None,
                requires: vec![],
                ensures: vec![],
                body: crate::grammar::SaltBlock { stmts: vec![] },
            },
            concrete_tys: vec![],
            self_ty: None,
            imports: vec![],
            type_map: BTreeMap::new(),
        };
        assert!(!is_task_concrete(&task));
    }
}
