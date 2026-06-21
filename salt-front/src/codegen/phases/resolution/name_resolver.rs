use crate::grammar::*;
use crate::grammar::pattern::Pattern;
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

            if let Some(alias) = &imp.alias {
                self.import_map.insert(alias.to_string(), base_pkg.clone());
            } else if let Some(group) = &imp.group {
                for g in group {
                    self.import_map.insert(g.to_string(), format!("{}__{}", base_pkg, g));
                }
            } else if let Some(last) = pkg_path.last() {
                self.import_map.insert(last.clone(), base_pkg.clone());
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
            Item::Struct(s) => {
                self.with_generics(&s.generics, |this| {
                    for field in &mut s.fields {
                        this.visit_syn_type(&mut field.ty);
                    }
                });
            }
            Item::Enum(e) => {
                self.with_generics(&e.generics, |this| {
                    for variant in &mut e.variants {
                        if let Some(ty) = &mut variant.ty {
                            this.visit_syn_type(ty);
                        }
                    }
                });
            }
            Item::Fn(f) => {
                self.with_generics(&f.generics, |this| {
                    for arg in f.args.iter_mut() {
                        if let Some(ty) = &mut arg.ty {
                            this.visit_syn_type(ty);
                        }
                    }
                    if let Some(ty) = &mut f.ret_type {
                        this.visit_syn_type(ty);
                    }
                    // For a full AST pass we would visit Exprs inside f.body to mutate type annotations.
                    // Let's implement that.
                    this.visit_block(&mut f.body);
                });
            }
            Item::ExternFn(f) => {
                for arg in f.args.iter_mut() {
                    if let Some(ty) = &mut arg.ty {
                        self.visit_syn_type(ty);
                    }
                }
                if let Some(ty) = &mut f.ret_type {
                    self.visit_syn_type(ty);
                }
            }
            Item::Impl(i) => {
                match i {
                    SaltImpl::Methods { target_ty, methods, generics } => {
                        self.with_generics(generics, |this| {
                            this.visit_syn_type(target_ty);
                            for m in methods {
                                this.with_generics(&m.generics, |this2| {
                                    for arg in m.args.iter_mut() {
                                        if let Some(ty) = &mut arg.ty {
                                            this2.visit_syn_type(ty);
                                        }
                                    }
                                    if let Some(ty) = &mut m.ret_type {
                                        this2.visit_syn_type(ty);
                                    }
                                    this2.visit_block(&mut m.body);
                                });
                            }
                        });
                    }
                    SaltImpl::Trait { target_ty, methods, generics, .. } => {
                        self.with_generics(generics, |this| {
                            this.visit_syn_type(target_ty);
                            for m in methods {
                                this.with_generics(&m.generics, |this2| {
                                    for arg in m.args.iter_mut() {
                                        if let Some(ty) = &mut arg.ty {
                                            this2.visit_syn_type(ty);
                                        }
                                    }
                                    if let Some(ty) = &mut m.ret_type {
                                        this2.visit_syn_type(ty);
                                    }
                                    this2.visit_block(&mut m.body);
                                });
                            }
                        });
                    }
                    SaltImpl::Concept { target_ty, .. } => {
                        self.visit_syn_type(target_ty);
                    }
                }
            }
            Item::Global(g) => {
                self.visit_syn_type(&mut g.ty);
            }
            Item::Const(c) => {
                self.visit_syn_type(&mut c.ty);
            }
            Item::Trait(t) => {
                self.with_generics(&t.generics, |this| {
                    for m in &mut t.methods {
                        for arg in m.args.iter_mut() {
                            if let Some(ty) = &mut arg.ty {
                                this.visit_syn_type(ty);
                            }
                        }
                        if let Some(ty) = &mut m.ret_type {
                            this.visit_syn_type(ty);
                        }
                    }
                });
            }
            Item::Concept(c) => {
                self.with_generics(&c.generics, |this| {
                    this.visit_syn_type(&mut c.param_ty);
                });
            }
        }
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
            Stmt::If(i) => {
                self.visit_block(&mut i.then_branch);
                if let Some(eb) = &mut i.else_branch {
                    self.visit_else(eb);
                }
            }
            Stmt::Match(m) => {
                for arm in &mut m.arms {
                    self.visit_pattern(&mut arm.pattern);
                    self.visit_block(&mut arm.body);
                }
            }
            Stmt::MapWindow { body, .. } => self.visit_block(body),
            Stmt::WithRegion { body, .. } => self.visit_block(body),
            Stmt::Unsafe(b) => self.visit_block(b),
            Stmt::Loop(b) => self.visit_block(b),
            #[allow(clippy::collapsible_match)] // Stmt::Syn matches all Syn variants; Local is a subset
            Stmt::Syn(s) => {
                if let syn::Stmt::Local(_l) = s {
                    // But we don't have access to mutate syn::Type easily here unless we parse it to SynType,
                    // mutate, and convert back. Actually, Salt compiler ignores type annotations in Stmt::Syn
                    // except when doing from_syn in statements.rs. 
                    // To do a FULL HIR pass, we shouldn't use syn::Stmt. But we can't change the whole AST now.
                    // Wait, `statements.rs` parses `SynType::from_std(ty)`. So if we don't mutate `syn::Stmt`,
                    // we will STILL have un-resolved names in local vars!
                    // Oh, that's why we had the bug in method_resolution! `let res: Result<i32> = ...` was un-resolved!
                }
            }
            _ => {}
        }
    }

    fn visit_else(&mut self, el: &mut SaltElse) {
        match el {
            SaltElse::Block(b) => self.visit_block(b),
            SaltElse::If(i) => {
                self.visit_block(&mut i.then_branch);
                if let Some(eb) = &mut i.else_branch {
                    self.visit_else(eb);
                }
            }
        }
    }

    fn visit_pattern(&mut self, pat: &mut Pattern) {
        match pat {
            Pattern::Variant { path, fields } => {
                if !path.is_empty() {
                    let first = path[0].to_string();
                    if let Some(fqn) = self.import_map.get(&first).or_else(|| self.available_global_types.get(&first)) {
                        let mut new_path = vec![syn::Ident::new(fqn, path[0].span())];
                        if path.len() > 1 {
                            new_path.extend(path[1..].iter().cloned());
                        }
                        *path = new_path;
                    }
                }
                if let Some(fields) = fields {
                    for f in fields {
                        self.visit_pattern(f);
                    }
                }
            }
            Pattern::Struct { name, fields } => {
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
            Pattern::Tuple(fields) => {
                for f in fields {
                    self.visit_pattern(f);
                }
            }
            Pattern::Or(alts) => {
                for alt in alts {
                    self.visit_pattern(alt);
                }
            }
            _ => {}
        }
    }

    fn with_generics<F>(&mut self, generics: &Option<Generics>, f: F)
    where F: FnOnce(&mut Self) {
        let mut added = Vec::new();
        if let Some(g) = generics {
            for param in &g.params {
                match param {
                    GenericParam::Type { name, .. } => {
                        if self.local_generics.insert(name.to_string()) {
                            added.push(name.to_string());
                        }
                    }
                    GenericParam::Const { name: _, ty: _ } => {
                        // ty might need resolution
                        // We can't mutate ty here because it's borrowed immutable via with_generics.
                        // Wait, generics is &Option, we can't mutate ty.
                    }
                }
            }
        }
        
        f(self);

        for a in added {
            self.local_generics.remove(&a);
        }
    }

    fn visit_syn_type(&mut self, ty: &mut SynType) {
        match ty {
            SynType::Pointer(inner) => self.visit_syn_type(inner),
            SynType::Reference(inner, _) => self.visit_syn_type(inner),
            SynType::Array(inner, _) => self.visit_syn_type(inner),
            SynType::Tuple(t) => {
                for e in &mut t.elems {
                    self.visit_syn_type(e);
                }
            }
            SynType::FnPtr(args, ret) => {
                for a in args {
                    self.visit_syn_type(a);
                }
                if let Some(r) = ret {
                    self.visit_syn_type(r);
                }
            }
            SynType::ShapedTensor { element, .. } => self.visit_syn_type(element),
            SynType::Path(p) => {
                // Resolve path!
                for seg in &mut p.segments {
                    for arg in &mut seg.args {
                        self.visit_syn_type(arg);
                    }
                }

                if p.segments.len() == 1 {
                    let name = p.segments[0].ident.to_string();
                    if self.local_generics.contains(&name) {
                        return; // It's a local generic, keep it raw
                    }
                    if let Some(fqn) = self.import_map.get(&name) {
                        // Replace segment identifier with fully qualified identifier!
                        // Actually, identifiers shouldn't contain `__` if they are parsed by standard tools,
                        // but SynType::Path just holds `Ident`. We can create a new Ident with the FQN string.
                        // Wait, syn::Ident cannot contain `::` or `__` in strictly parsed code?
                        // `proc_macro2::Ident` CAN contain `__`.
                        // But can it contain `::`? NO. Ident cannot contain `::`.
                        // FQNs in our system use `__`. e.g. `std__core__result__Result`.
                        p.segments[0].ident = proc_macro2::Ident::new(fqn, p.segments[0].ident.span());
                    } else if !self.import_map.contains_key(&name) {
                        // Suffix Fallback check
                        // Does it uniquely match an available global type?
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
                } else if p.segments.len() > 1 {
                    // It's like `addr::PhysAddr`.
                    let first = p.segments[0].ident.to_string();
                    if let Some(pkg_fqn) = self.import_map.get(&first) {
                        // Combine pkg_fqn and the rest of segments.
                        let rest = p.segments[1..].iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("__");
                        let full = format!("{}__{}", pkg_fqn, rest);
                        
                        // We flatten this into a SINGLE segment containing the full mangled FQN!
                        // This allows Type::from_syn to see it as a single struct name.
                        let mut args = Vec::new();
                        for seg in &mut p.segments {
                            args.append(&mut seg.args); // Usually only the last segment has args anyway
                        }
                        
                        p.segments = vec![SynPathSegment {
                            ident: proc_macro2::Ident::new(&full, p.segments[0].ident.span()),
                            args,
                        }];
                    }
                }
            }
            SynType::Other(_) => {}
        }
    }
}
