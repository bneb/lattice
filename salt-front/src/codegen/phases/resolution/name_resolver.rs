use crate::grammar::*;
use crate::grammar::pattern::{Pattern, PatternField};
use proc_macro2::Ident;
use std::collections::{HashMap, HashSet};
use crate::common::mangling::Mangler;

pub struct NameResolver {
    import_map: HashMap<String, String>, // Alias/Base -> FQN
    local_generics: HashSet<String>, // T, U, etc. (do not qualify these)
    available_global_types: HashSet<String>, // All FQNs across the project
    current_pkg_prefix: String,
}

impl NameResolver {
    pub fn resolve_file(file: &mut SaltFile, global_types: &HashSet<String>) {
        let mut resolver = NameResolver {
            import_map: HashMap::new(),
            local_generics: HashSet::new(),
            available_global_types: global_types.clone(),
            current_pkg_prefix: if let Some(pkg) = &file.package {
                Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>())
            } else {
                String::new()
            },
        };

        resolver.build_import_map(file);

        // Add all local types defined in the file to the import map
        // clone items temporarily to avoid borrow check issues on file
        let items_copy = file.items.clone();
        for item in &items_copy {
            match item {
                Item::Struct(s) => {
                    let fqn = if resolver.current_pkg_prefix.is_empty() { s.name.to_string() } else { format!("{}__{}", resolver.current_pkg_prefix, s.name) };
                    resolver.import_map.insert(s.name.to_string(), fqn);
                }
                Item::Enum(e) => {
                    let fqn = if resolver.current_pkg_prefix.is_empty() { e.name.to_string() } else { format!("{}__{}", resolver.current_pkg_prefix, e.name) };
                    resolver.import_map.insert(e.name.to_string(), fqn);
                }
                Item::Trait(t) => {
                    let fqn = if resolver.current_pkg_prefix.is_empty() { t.name.to_string() } else { format!("{}__{}", resolver.current_pkg_prefix, t.name) };
                    resolver.import_map.insert(t.name.to_string(), fqn);
                }
                Item::Concept(c) => {
                    let fqn = if resolver.current_pkg_prefix.is_empty() { c.name.to_string() } else { format!("{}__{}", resolver.current_pkg_prefix, c.name) };
                    resolver.import_map.insert(c.name.to_string(), fqn);
                }
                _ => {}
            }
        }

        resolver.visit_file(file);
    }

    fn build_import_map(&mut self, file: &SaltFile) {
        // Built-ins (never qualify)
        let builtins = vec!["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "usize", "Self", "LlvmPtr", "Tensor"];
        for b in builtins {
            self.import_map.insert(b.to_string(), b.to_string());
        }

        for imp in &file.imports {
            let pkg_path: Vec<String> = imp.name.iter().map(|id| id.to_string()).collect();
            let base_pkg = Mangler::mangle(&pkg_path);

            match (&imp.alias, &imp.group, pkg_path.last()) {
                (Some(alias), _, _) => {
                    self.import_map.insert(alias.to_string(), base_pkg.clone());
                }
                (_, Some(group), _) => {
                    group.iter().for_each(|g| {
                        self.import_map.insert(g.to_string(), format!("{}__{}", base_pkg, g));
                    });
                }
                (_, _, Some(last)) => {
                    self.import_map.insert(last.clone(), base_pkg.clone());
                }
                _ => {}
            }
        }
    }

    fn visit_file(&mut self, file: &mut SaltFile) {
        for item in &mut file.items {
            self.visit_item(item);
        }
    }

    fn visit_item(&mut self, item: &mut Item) {
        match item {
            Item::Struct(s) => self.visit_struct_item(s),
            Item::Enum(e) => self.visit_enum_item(e),
            Item::Fn(f) => self.visit_fn_item(f),
            Item::ExternFn(f) => self.visit_extern_fn_item(f),
            Item::Impl(i) => self.visit_impl_item(i),
            Item::Global(g) => self.visit_syn_type(&mut g.ty),
            Item::Const(c) => self.visit_syn_type(&mut c.ty),
            Item::Trait(t) => self.visit_trait_item(t),
            Item::Concept(c) => self.visit_concept_item(c),
        }
    }

    fn visit_struct_item(&mut self, s: &mut StructDef) {
        self.with_generics(&s.generics, |this| {
            for field in &mut s.fields {
                this.visit_syn_type(&mut field.ty);
            }
        });
    }

    fn visit_enum_item(&mut self, e: &mut EnumDef) {
        self.with_generics(&e.generics, |this| {
            for variant in &mut e.variants {
                for ty in &mut variant.tys { this.visit_syn_type(ty); }
            }
        });
    }

    fn visit_fn_item(&mut self, f: &mut SaltFn) {
        self.with_generics(&f.generics, |this| {
            for arg in f.args.iter_mut() {
                let Some(ty) = &mut arg.ty else { continue; };
                this.visit_syn_type(ty);
            }
            if let Some(ty) = &mut f.ret_type {
                this.visit_syn_type(ty);
            }
            this.visit_block(&mut f.body);
        });
    }

    fn visit_extern_fn_item(&mut self, f: &mut ExternFnDecl) {
        for arg in f.args.iter_mut() {
            if let Some(ty) = &mut arg.ty {
                self.visit_syn_type(ty);
            }
        }
        if let Some(ty) = &mut f.ret_type {
            self.visit_syn_type(ty);
        }
    }

    fn visit_impl_item(&mut self, i: &mut SaltImpl) {
        match i {
            SaltImpl::Methods { target_ty, methods, generics } => {
                self.visit_impl_methods_body(target_ty, methods, generics);
            }
            SaltImpl::Trait { target_ty, methods, generics, .. } => {
                self.visit_impl_methods_body(target_ty, methods, generics);
            }
            SaltImpl::Concept { target_ty, .. } => {
                self.visit_syn_type(target_ty);
            }
        }
    }

    fn visit_impl_methods_body(&mut self, target_ty: &mut SynType, methods: &mut Vec<SaltFn>, generics: &Option<Generics>) {
        self.with_generics(generics, |this| {
            this.visit_syn_type(target_ty);
            for m in methods {
                this.visit_single_impl_method(m);
            }
        });
    }

    fn visit_single_impl_method(&mut self, m: &mut SaltFn) {
        self.with_generics(&m.generics, |this| {
            for arg in m.args.iter_mut() {
                let Some(ty) = &mut arg.ty else { continue; };
                this.visit_syn_type(ty);
            }
            if let Some(ty) = &mut m.ret_type {
                this.visit_syn_type(ty);
            }
            this.visit_block(&mut m.body);
        });
    }

    fn visit_trait_item(&mut self, t: &mut SaltTrait) {
        self.with_generics(&t.generics, |this| {
            for m in &mut t.methods {
                this.visit_trait_method_sig(m);
            }
        });
    }

    fn visit_trait_method_sig(&mut self, m: &mut TraitMethodSig) {
        for arg in m.args.iter_mut() {
            let Some(ty) = &mut arg.ty else { continue; };
            self.visit_syn_type(ty);
        }
        if let Some(ty) = &mut m.ret_type {
            self.visit_syn_type(ty);
        }
    }

    fn visit_concept_item(&mut self, c: &mut SaltConcept) {
        self.with_generics(&c.generics, |this| {
            this.visit_syn_type(&mut c.param_ty);
        });
    }

    fn visit_block(&mut self, block: &mut SaltBlock) {
        for stmt in &mut block.stmts {
            self.visit_stmt(stmt);
        }
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::LetElse(l) => {
                self.visit_pattern(&mut l.pattern);
                self.visit_block(&mut l.else_block);
            }
            Stmt::While(w) => self.visit_block(&mut w.body),
            Stmt::For(f) => self.visit_block(&mut f.body),
            Stmt::If(i) => self.visit_if_stmt(i),
            Stmt::Match(m) => self.visit_match_stmt(m),
            Stmt::MapWindow { body, .. } => self.visit_block(body),
            Stmt::WithRegion { body, .. } => self.visit_block(body),
            Stmt::Unsafe(b) => self.visit_block(b),
            Stmt::Loop(b) => self.visit_block(b),
            #[allow(clippy::collapsible_match)]
            Stmt::Syn(s) => self.visit_syn_stmt(s),
            _ => {}
        }
    }

    fn visit_if_stmt(&mut self, i: &mut SaltIf) {
        self.visit_block(&mut i.then_branch);
        if let Some(eb) = &mut i.else_branch {
            self.visit_else(eb);
        }
    }

    fn visit_match_stmt(&mut self, m: &mut SaltMatch) {
        for arm in &mut m.arms {
            self.visit_pattern(&mut arm.pattern);
            self.visit_block(&mut arm.body);
        }
    }

    fn visit_syn_stmt(&mut self, s: &mut syn::Stmt) {
        if let syn::Stmt::Local(_l) = s {
            // Cannot mutate syn::Type here without parsing to SynType and converting back.
            // Salt compiler ignores type annotations in Stmt::Syn except when doing from_syn
            // in statements.rs. This is why `let res: Result<i32> = ...` was unresolved.
        }
    }

    fn visit_else(&mut self, el: &mut SaltElse) {
        match el {
            SaltElse::Block(b) => self.visit_block(b),
            SaltElse::If(i) => self.visit_if_stmt(i),
        }
    }

    fn visit_pattern(&mut self, pat: &mut Pattern) {
        match pat {
            Pattern::Variant { path, fields } => self.resolve_variant_pattern(path, fields),
            Pattern::Struct { name, fields } => self.resolve_struct_pattern(name, fields),
            Pattern::Tuple(fields) => self.visit_patterns(fields),
            Pattern::Or(alts) => self.visit_patterns(alts),
            _ => {}
        }
    }

    fn visit_patterns(&mut self, pats: &mut Vec<Pattern>) {
        for p in pats {
            self.visit_pattern(p);
        }
    }

    fn resolve_variant_pattern(&mut self, path: &mut Vec<Ident>, fields: &mut Option<Vec<Pattern>>) {
        if let Some(first_ident) = path.first() {
            let first = first_ident.to_string();
            if let Some(fqn) = self.import_map.get(&first).or_else(|| self.available_global_types.get(&first)) {
                let new_path: Vec<Ident> = std::iter::once(syn::Ident::new(fqn, path[0].span()))
                    .chain(path[1..].iter().cloned())
                    .collect();
                *path = new_path;
            }
        }
        if let Some(fields) = fields {
            for f in fields {
                self.visit_pattern(f);
            }
        }
    }

    fn resolve_struct_pattern(&mut self, name: &mut Ident, fields: &mut Vec<PatternField>) {
        let base = name.to_string();
        if let Some(fqn) = self.import_map.get(&base).or_else(|| self.available_global_types.get(&base)) {
            *name = syn::Ident::new(fqn, name.span());
        }
        for f in fields {
            if let Some(p) = &mut f.pattern {
                self.visit_pattern(p);
            }
        }
    }

    fn with_generics<F>(&mut self, generics: &Option<Generics>, f: F)
    where F: FnOnce(&mut Self) {
        let mut added = Vec::new();
        if let Some(g) = generics {
            for param in &g.params {
                self.register_generic_param(param, &mut added);
            }
        }

        f(self);

        for a in added {
            self.local_generics.remove(&a);
        }
    }

    fn register_generic_param(&mut self, param: &GenericParam, added: &mut Vec<String>) {
        if let GenericParam::Type { name, .. } = param {
            if self.local_generics.insert(name.to_string()) {
                added.push(name.to_string());
            }
        }
    }

    fn visit_syn_type(&mut self, ty: &mut SynType) {
        match ty {
            SynType::Pointer(inner) => self.visit_syn_type(inner),
            SynType::Reference(inner, _) => self.visit_syn_type(inner),
            SynType::Array(inner, _) => self.visit_syn_type(inner),
            SynType::Tuple(t) => self.visit_syn_tuple(t),
            SynType::FnPtr(args, ret) => self.visit_fn_ptr_types(args, ret),
            SynType::ShapedTensor { element, .. } => self.visit_syn_type(element),
            SynType::Path(p) => self.resolve_syn_type_path(p),
            SynType::Other(_) => {}
        }
    }

    fn visit_syn_tuple(&mut self, t: &mut SynTuple) {
        for e in &mut t.elems {
            self.visit_syn_type(e);
        }
    }

    fn visit_fn_ptr_types(&mut self, args: &mut Vec<SynType>, ret: &mut Option<Box<SynType>>) {
        for a in args {
            self.visit_syn_type(a);
        }
        if let Some(r) = ret {
            self.visit_syn_type(r);
        }
    }

    fn resolve_syn_type_path(&mut self, p: &mut SynPath) {
        for seg in &mut p.segments {
            for arg in &mut seg.args {
                self.visit_syn_type(arg);
            }
        }
        if p.segments.len() == 1 {
            let name = p.segments[0].ident.to_string();
            if self.local_generics.contains(&name) {
                return;
            }
            if let Some(fqn) = self.import_map.get(&name) {
                p.segments[0].ident = proc_macro2::Ident::new(fqn, p.segments[0].ident.span());
            } else if !self.import_map.contains_key(&name) {
                self.suffix_fallback_resolve(p);
            }
        } else if p.segments.len() > 1 {
            self.resolve_multi_segment_path(p);
        }
    }

    fn suffix_fallback_resolve(&mut self, p: &mut SynPath) {
        let name = p.segments[0].ident.to_string();
        let mut matches = Vec::new();
        for gt in &self.available_global_types {
            if gt == &name || gt.ends_with(&format!("__{}", name)) {
                matches.push(gt.clone());
            }
        }
        if matches.len() == 1 {
            p.segments[0].ident = proc_macro2::Ident::new(&matches[0], p.segments[0].ident.span());
        }
    }

    fn resolve_multi_segment_path(&mut self, p: &mut SynPath) {
        let first = p.segments[0].ident.to_string();
        let pkg_fqn = match self.import_map.get(&first) {
            Some(fqn) => fqn,
            None => return,
        };
        let rest = p.segments[1..].iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("__");
        let full = format!("{}__{}", pkg_fqn, rest);
        let mut args = Vec::new();
        for seg in &mut p.segments {
            args.append(&mut seg.args);
        }
        p.segments = vec![SynPathSegment {
            ident: proc_macro2::Ident::new(&full, p.segments[0].ident.span()),
            args,
        }];
    }
}
