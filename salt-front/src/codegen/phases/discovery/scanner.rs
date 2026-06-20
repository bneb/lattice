use crate::grammar::{SaltFile, Item, SaltImpl, ImportDecl};
use crate::types::{Type, TypeKey};
use crate::common::mangling::Mangler;
use crate::codegen::context::LoweringContext;

pub fn scan_defs_from_file_impl(ctx: &mut LoweringContext, file: &SaltFile, is_main_file: bool) -> Result<(), String> {
    // 1. Setup Package Prefix
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

    // 2. Setup File-Local Imports
    ctx.discovery.imports.clear();
    ctx.discovery.imports.extend(file.imports.clone());
    
    // Inject self-imports for local types to support qualified resolution
    if !pkg_prefix.is_empty() {
        let mut self_imports = Vec::new();
        for item in &file.items {
            let (ident_name, mangled_str) = match item {
                Item::Struct(s) => (&s.name, format!("{}{}", pkg_prefix, s.name)),
                Item::Enum(e) => (&e.name, format!("{}{}", pkg_prefix, e.name)),
                Item::Fn(f) => (&f.name, if f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" ) { f.name.to_string() } else { format!("{}{}", pkg_prefix, f.name) }),
                Item::ExternFn(e) => (&e.name, e.name.to_string()),
                Item::Global(g) => (&g.name, format!("{}{}", pkg_prefix, g.name)),
                Item::Const(c) => (&c.name, format!("{}{}", pkg_prefix, c.name)),
                _ => continue
            };
            
            let mangled_ident = syn::Ident::new(&mangled_str, proc_macro2::Span::call_site());
            let mut p = syn::punctuated::Punctuated::new();
            p.push(mangled_ident);
            
            self_imports.push(ImportDecl { 
                name: p,
                alias: Some(ident_name.clone()),
                group: None
            });
        }
        ctx.discovery.imports.extend(self_imports);
    }

    // 3. Scan Items
    for item in &file.items {
        match item {
            Item::Global(g) => {
                 let name = format!("{}{}", pkg_prefix, g.name);
                 let ty = crate::codegen::type_bridge::resolve_type(ctx, &g.ty);
                 ctx.discovery.globals.insert(name.clone(), ty);
                 
                 if is_main_file {
                     let mut out = String::new();
                     crate::codegen::type_bridge::emit_global_def(ctx, &mut out, g)?;
                     ctx.emission.decl_out.push_str(&out);
                 }
            }
            Item::Fn(f) => {
                let is_extern = f.attributes.iter().any(|a| a.name == "extern");
                let is_no_mangle = f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export" );
                if is_extern {
                    ctx.emission.external_decls.insert(f.name.to_string());
                }

                let name = if is_no_mangle || is_extern {
                    f.name.to_string()
                } else {
                     format!("{}{}", pkg_prefix, f.name)
                };
                
                let ret_ty = if let Some(rt) = &f.ret_type {
                    crate::codegen::type_bridge::resolve_type(ctx, rt)
                } else {
                     Type::Unit
                };
                let args: Vec<Type> = f.args.iter()
                                     .filter_map(|arg| arg.ty.as_ref().map(|t| crate::codegen::type_bridge::resolve_type(ctx, t)))
                                     .collect();
                ctx.discovery.globals.insert(name.clone(), Type::Fn(args, Box::new(ret_ty)));
                
                let current_imports = ctx.discovery.imports.clone();
                ctx.discovery.generic_impls.insert(name, (f.clone(), current_imports));
            }
            Item::Impl(i) => {
                match i {
                    SaltImpl::Methods { target_ty, methods, generics } => {
                        let _saved_map = ctx.expansion.current_type_map.clone();
                        if let Some(g) = generics {
                            for param in &g.params {
                                let name = match param {
                                    crate::grammar::GenericParam::Type { name, .. } => name,
                                    crate::grammar::GenericParam::Const { name, .. } => name,
                                };
                                ctx.expansion.current_type_map.insert(name.to_string(), Type::Struct(name.to_string()));
                            }
                        }

                        if let Some(target) = Type::from_syn(target_ty) {
                            let resolved = crate::codegen::type_bridge::resolve_codegen_type(ctx, &target);
                            let target_mangled = resolved.mangle_suffix();
                            
                            let old_self = ctx.expansion.current_self_ty.clone();
                            ctx.expansion.current_self_ty = Some(resolved.clone());

                            let mut impl_key = resolved.to_key().unwrap_or_else(|| {
                                TypeKey { path: path.clone(), name: resolved.mangle_suffix(), specialization: None }
                            });
                            if impl_key.path.is_empty() && !path.is_empty() {
                                impl_key.path = path.clone();
                            }
                            if generics.is_some() {
                                 impl_key.specialization = None;
                            }

                            for m in methods {
                                 let ret_ty = if let Some(rt) = &m.ret_type {
                                     Type::from_syn(rt).unwrap_or(Type::Unit)
                                 } else {
                                      Type::Unit
                                 };
                                 let args: Vec<Type> = m.args.iter()
                                         .filter_map(|arg| arg.ty.as_ref().and_then(Type::from_syn))
                                         .collect();
                                 
                                 ctx.discovery.trait_registry.register_simple(impl_key.clone(), m.clone(), Some(resolved.clone()), ctx.discovery.imports.clone());
                                 ctx.discovery.globals.insert(format!("{}__{}", target_mangled, m.name), Type::Fn(args, Box::new(ret_ty)));
                            }
                            ctx.expansion.current_self_ty = old_self;
                        }
                        ctx.expansion.current_type_map = _saved_map;
                    }
                    SaltImpl::Trait { trait_name: _, target_ty, methods, generics } => {
                        // Trait impl scanning logic...
                        if let Some(target) = Type::from_syn(target_ty) {
                             let resolved = crate::codegen::type_bridge::resolve_codegen_type(ctx, &target);
                             let mut impl_key = resolved.to_key().unwrap_or_else(|| {
                                TypeKey { path: path.clone(), name: resolved.mangle_suffix(), specialization: None }
                             });
                             if generics.is_some() { impl_key.specialization = None; }
                             
                             for m in methods {
                                 ctx.discovery.trait_registry.register_simple(impl_key.clone(), m.clone(), Some(resolved.clone()), ctx.discovery.imports.clone());
                             }
                        }
                    }
                    SaltImpl::Concept { .. } => {
                        // Concepts are used for verification, not direct emission.
                        // For now, just skip during scanning.
                    }
                }
            }
            Item::Const(c) => {
                 let name = format!("{}{}", pkg_prefix, c.name);
                 let ty = crate::codegen::type_bridge::resolve_type(ctx, &c.ty);
                 ctx.discovery.globals.insert(name.clone(), ty);
                 
                 if is_main_file {
                     let mut out = String::new();
                     crate::codegen::type_bridge::emit_const(ctx, &mut out, c)?;
                     ctx.emission.decl_out.push_str(&out);
                 }
            }
            _ => {}
        }
    }

    Ok(())
}
