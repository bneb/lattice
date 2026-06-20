use crate::codegen::context::CodegenContext;
use crate::types::{Type};
use std::collections::BTreeMap;
use syn::{Expr};

pub trait TypeTracer {
    fn trace_expr_type(&self, expr: &Expr, locals: &BTreeMap<String, Type>) -> Result<Type, String>;
    fn resolve_field_type(&self, receiver_ty: &Type, field_name: &str) -> Result<Type, String>;
    fn resolve_method_info(&self, receiver_ty: &Type, method_name: &str) -> Result<(crate::grammar::SaltFn, Option<Type>), String>;
    fn substitute_generics(&self, ty: &Type, type_map: &BTreeMap<String, Type>) -> Type;
    /// Canonicalize raw struct names within a Type to their FQN equivalents.
    fn canonicalize_type(&self, ty: &Type) -> Type;
}

impl<'a> TypeTracer for CodegenContext<'a> {
    fn trace_expr_type(&self, expr: &Expr, locals: &BTreeMap<String, Type>) -> Result<Type, String> {
        match expr {
            Expr::Path(path) if path.path.segments.len() == 1 => {
                let name = &path.path.segments[0].ident.to_string();
                
                // 1. Local Variable Lookup
                if let Some(ty) = locals.get(name) {
                    return Ok(ty.clone());
                }
                
                // 2. FQN/Global Lookup
                if let Ok(key) = self.resolve_path_to_fqn(&path.path) {
                    if let Some(ty) = self.lookup_global_type(&key) {
                        return Ok(ty);
                    }
                }

                // 3. Native Global/Constant Fallback
                if let Some(ty) = self.globals().get(name) {
                     return Ok(ty.clone());
                }

                Err(format!("KeuOS Tracer: Unknown local or path: {}", name))
            },
            
            Expr::Field(f) => {
                let receiver_ty = self.trace_expr_type(&f.base, locals)?;
                if let syn::Member::Named(ident) = &f.member {
                    self.resolve_field_type(&receiver_ty, &ident.to_string())
                } else {
                    Err("KeuOS Tracer: Tuple indexing not implemented in tracer.".to_string())
                }
            },

            Expr::MethodCall(m) => {
                let receiver_ty = self.trace_expr_type(&m.receiver, locals)?;
                let (func, trait_ty) = self.resolve_method_info(&receiver_ty, &m.method.to_string())?;
                
                let mut type_map = BTreeMap::new();
                type_map.insert("Self".to_string(), trait_ty.unwrap_or_else(|| receiver_ty.clone()));
                
                // Handle Turbofish and Generic Substitution
                if let Some(turbofish) = &m.turbofish {
                    if let Some(g) = &func.generics {
                        for (i, param) in g.params.iter().enumerate() {
                            let p_name = match param {
                                crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                            };
                            if let Some(syn::GenericArgument::Type(ty)) = turbofish.args.iter().nth(i) {
                                // Use the Ptr-aware from_std bridge
                                let syn_ty = crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?;
                                let concrete = Type::from_syn(&syn_ty).unwrap_or(Type::Unit);
                                type_map.insert(p_name, concrete);
                            }
                        }
                    }
                }

                let ret_ty_node = if let Some(rt) = &func.ret_type {
                     Type::from_syn(rt).unwrap_or(Type::Unit)
                } else {
                     Type::Unit
                };
                
                Ok(self.substitute_generics(&ret_ty_node, &type_map))
            },

            Expr::Call(c) => {
                if let Expr::Path(p) = &*c.func {
                    let path_string = p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");
                    
                    // Built-in intrinsics
                    if path_string.ends_with("size_of") { return Ok(Type::I64); }
                    
                    // Ptr Offset maintains pointer identity
                    if path_string.contains("ptr_offset") {
                        if let Some(first) = c.args.first() {
                            return self.trace_expr_type(first, locals);
                        }
                    }

                    let key = self.resolve_path_to_fqn(&p.path)?;
                    if let Some((_, ret)) = self.resolve_global_signature(&key.mangle()) {
                        return Ok(ret);
                    }
                }
                Ok(Type::Unit)
            },
            
            Expr::Lit(lit) => {
                match &lit.lit {
                    // String literals are StringView by default
                    syn::Lit::Str(_) => Ok(Type::Struct("std__core__str__StringView".to_string())),
                    syn::Lit::Int(_) => Ok(Type::I64),
                    syn::Lit::Bool(_) => Ok(Type::Bool),
                    syn::Lit::Float(_) => Ok(Type::F32), // Salt defaults to f32 for benchmarks
                    _ => Ok(Type::Unit)
                }
            },
            
            Expr::Reference(r) => {
                 let inner = self.trace_expr_type(&r.expr, locals)?;
                 Ok(Type::Reference(Box::new(inner), r.mutability.is_some()))
            }

            // Struct literal: Node { val: 42 } => Type::Struct("main__Node")
            // Canonical Resolution: Construct FQN deterministically, verify in Symbol Table.
            // Source of Truth: struct_templates (the Symbol Table), not string patterns.
            Expr::Struct(s) => {
                let raw_name = s.path.segments.iter()
                    .map(|seg| seg.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("__");
                
                // Step 1: Construct candidate FQN from current package + raw name
                let fqn = {
                    let pkg = self.current_package.borrow();
                    let candidate = if let Some(pkg) = pkg.as_ref() {
                        let prefix = pkg.name.iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join("__");
                        if prefix.is_empty() { raw_name.clone() } else { format!("{}__{}", prefix, raw_name) }
                    } else {
                        raw_name.clone()
                    };
                    drop(pkg);
                    
                    // Step 2: Verify against Symbol Table (struct_templates)
                    if self.struct_templates().contains_key(&candidate) {
                        candidate
                    } else if self.struct_templates().contains_key(&raw_name) {
                        // Already-qualified or no-package struct
                        raw_name
                    } else {
                        // Fallback: use candidate (external dependency or forward ref)
                        candidate
                    }
                };
                Ok(Type::Struct(fqn))
            }

            // Cast expression: addr as Ptr<T> => target type
            Expr::Cast(c) => {
                let syn_ty = crate::grammar::SynType::from_std((*c.ty).clone())
                    .map_err(|e| e.to_string())?;
                Type::from_syn(&syn_ty)
                    .ok_or_else(|| "Cannot trace cast target type".to_string())
            }
            
            _ => Err(format!("Type tracing not implemented for expression type: {:?}", expr)),
        }
    }

    fn resolve_field_type(&self, receiver_ty: &Type, field_name: &str) -> Result<Type, String> {
        // Dereference through References and Pointers to find the base struct
        let mut current = receiver_ty;
        while let Some(inner) = current.get_ptr_element() {
            current = inner;
        }
        
        let (base_name, concrete_args) = match current {
            Type::Struct(n) => (n.clone(), vec![]),
            Type::Concrete(n, args) => (n.clone(), args.clone()),
            _ => return Err(format!("Cannot access field '{}' on non-struct type {:?}", field_name, current)),
        };
        
        if let Some(struct_def) = self.struct_templates().get(base_name.as_str()) {
             if let Some(f) = struct_def.fields.iter().find(|f| f.name == field_name) {
                  let raw_field_ty = Type::from_syn(&f.ty).unwrap_or(Type::Unit);
                  // Canonicalize raw struct names from AST to FQNs
                  let field_ty = self.canonicalize_type(&raw_field_ty);
                  let mut map = BTreeMap::new();
                  if let Some(g) = &struct_def.generics {
                       for (i, param) in g.params.iter().enumerate() {
                           let name = match param {
                               crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                               crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                           };
                           if let Some(arg) = concrete_args.get(i) {
                               map.insert(name, arg.clone());
                           }
                       }
                  }
                  return Ok(self.substitute_generics(&field_ty, &map));
             }
        }
        
        // Fallback to structural lookup in the registry
        if let Some(info) = self.lookup_struct_by_type(current) {
             if let Some((_, f_ty)) = info.fields.get(field_name) {
                 return Ok(f_ty.clone());
             }
        }

        Err(format!("Field '{}' not found in struct {}", field_name, base_name))
    }

    fn resolve_method_info(&self, receiver_ty: &Type, method_name: &str) -> Result<(crate::grammar::SaltFn, Option<Type>), String> {
        let (func, trait_ty, _) = self.resolve_method(receiver_ty, method_name)?;
        Ok((func, trait_ty))
    }

    fn substitute_generics(&self, ty: &Type, type_map: &BTreeMap<String, Type>) -> Type {
        ty.substitute(type_map) // Delegate to the robust implementation in types.rs
    }

    /// Recursively canonicalize raw struct names within a Type tree.
    /// E.g., Concrete("Ptr", [Struct("Node")]) => Concrete("Ptr", [Struct("main__Node")])
    fn canonicalize_type(&self, ty: &Type) -> Type {
        match ty {
            Type::Struct(raw_name) => {
                // Skip already-qualified names (contain "__")
                if raw_name.contains("__") {
                    return ty.clone();
                }
                // Skip generic type parameters (single uppercase letter)
                if raw_name.len() == 1 && raw_name.chars().all(|c| c.is_uppercase()) {
                    return ty.clone();
                }
                // Apply same canonicalization logic as Expr::Struct
                let pkg = self.current_package.borrow();
                let candidate = if let Some(pkg) = pkg.as_ref() {
                    let prefix = pkg.name.iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join("__");
                    if prefix.is_empty() { raw_name.clone() } else { format!("{}__{}", prefix, raw_name) }
                } else {
                    return ty.clone();
                };
                drop(pkg);
                
                if self.struct_templates().contains_key(&candidate) {
                    Type::Struct(candidate)
                } else {
                    ty.clone() // External or generic param — keep as-is
                }
            }
            Type::Concrete(name, args) => {
                let canon_args: Vec<Type> = args.iter().map(|a| self.canonicalize_type(a)).collect();
                Type::Concrete(name.clone(), canon_args)
            }
            Type::Reference(inner, mutable) => {
                Type::Reference(Box::new(self.canonicalize_type(inner)), *mutable)
            }
            Type::Pointer { element, provenance, is_mutable } => {
                Type::Pointer {
                    element: Box::new(self.canonicalize_type(element)),
                    provenance: provenance.clone(),
                    is_mutable: *is_mutable,
                }
            }
            Type::Tuple(elems) => {
                let canon_elems: Vec<Type> = elems.iter().map(|e| self.canonicalize_type(e)).collect();
                Type::Tuple(canon_elems)
            }
            Type::Array(inner, size, packed) => {
                Type::Array(Box::new(self.canonicalize_type(inner)), *size, *packed)
            }
            // Primitives, Unit, etc. — no struct names to canonicalize
            _ => ty.clone(),
        }
    }
}

impl<'a, 'ctx> TypeTracer for crate::codegen::context::LoweringContext<'a, 'ctx> {
    fn trace_expr_type(&self, expr: &Expr, locals: &BTreeMap<String, Type>) -> Result<Type, String> {
        match expr {
            Expr::Path(path) if path.path.segments.len() == 1 => {
                let name = &path.path.segments[0].ident.to_string();
                if let Some(ty) = locals.get(name) {
                    return Ok(ty.clone());
                }
                if let Ok(key) = self.resolve_path_to_fqn(&path.path) {
                    if let Some(ty) = self.lookup_global_type(&key) {
                        return Ok(ty);
                    }
                }
                if let Some(ty) = self.discovery.globals.get(name) {
                    return Ok(ty.clone());
                }
                Err(format!("KeuOS Tracer: Unknown local or path: {}", name))
            },
            Expr::Field(f) => {
                let receiver_ty = self.trace_expr_type(&f.base, locals)?;
                if let syn::Member::Named(ident) = &f.member {
                    self.resolve_field_type(&receiver_ty, &ident.to_string())
                } else {
                    Err("KeuOS Tracer: Tuple indexing not implemented in tracer.".to_string())
                }
            },
            Expr::MethodCall(m) => {
                let receiver_ty = self.trace_expr_type(&m.receiver, locals)?;
                let (func, trait_ty) = self.resolve_method_info(&receiver_ty, &m.method.to_string())?;
                let mut type_map = BTreeMap::new();
                type_map.insert("Self".to_string(), trait_ty.unwrap_or_else(|| receiver_ty.clone()));
                if let Some(turbofish) = &m.turbofish {
                    if let Some(g) = &func.generics {
                        for (i, param) in g.params.iter().enumerate() {
                            let p_name = match param {
                                crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                            };
                            if let Some(syn::GenericArgument::Type(ty)) = turbofish.args.iter().nth(i) {
                                let syn_ty = crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?;
                                let concrete = Type::from_syn(&syn_ty).unwrap_or(Type::Unit);
                                type_map.insert(p_name, concrete);
                            }
                        }
                    }
                }
                let ret_ty_node = if let Some(rt) = &func.ret_type {
                    Type::from_syn(rt).unwrap_or(Type::Unit)
                } else {
                    Type::Unit
                };
                Ok(self.substitute_generics(&ret_ty_node, &type_map))
            },
            Expr::Call(c) => {
                if let Expr::Path(p) = &*c.func {
                    let path_string = p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");
                    if path_string.ends_with("size_of") { return Ok(Type::I64); }
                    if path_string.contains("ptr_offset") {
                        if let Some(first) = c.args.first() {
                            return self.trace_expr_type(first, locals);
                        }
                    }
                    let key = self.resolve_path_to_fqn(&p.path)?;
                    if let Some((_, ret)) = self.resolve_global_signature(&key.mangle()) {
                        return Ok(ret);
                    }
                }
                Ok(Type::Unit)
            },
            Expr::Lit(lit) => {
                match &lit.lit {
                    syn::Lit::Str(_) => Ok(Type::Struct("std__core__str__StringView".to_string())),
                    syn::Lit::Int(_) => Ok(Type::I64),
                    syn::Lit::Bool(_) => Ok(Type::Bool),
                    syn::Lit::Float(_) => Ok(Type::F32),
                    _ => Ok(Type::Unit)
                }
            },
            Expr::Reference(r) => {
                let inner = self.trace_expr_type(&r.expr, locals)?;
                Ok(Type::Reference(Box::new(inner), r.mutability.is_some()))
            }
            Expr::Struct(s) => {
                let raw_name = s.path.segments.iter()
                    .map(|seg| seg.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("__");
                let fqn = {
                    let candidate = if let Some(pkg) = self.current_package.as_ref() {
                        let prefix = pkg.name.iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join("__");
                        if prefix.is_empty() { raw_name.clone() } else { format!("{}__{}", prefix, raw_name) }
                    } else {
                        raw_name.clone()
                    };
                    if self.struct_templates().contains_key(&candidate) {
                        candidate
                    } else if self.struct_templates().contains_key(&raw_name) {
                        raw_name
                    } else {
                        candidate
                    }
                };
                Ok(Type::Struct(fqn))
            }
            Expr::Cast(c) => {
                let syn_ty = crate::grammar::SynType::from_std((*c.ty).clone())
                    .map_err(|e| e.to_string())?;
                Type::from_syn(&syn_ty)
                    .ok_or_else(|| "Cannot trace cast target type".to_string())
            }
            _ => Err(format!("Type tracing not implemented for expression type: {:?}", expr)),
        }
    }

    fn resolve_field_type(&self, receiver_ty: &Type, field_name: &str) -> Result<Type, String> {
        let mut current = receiver_ty;
        while let Some(inner) = current.get_ptr_element() {
            current = inner;
        }
        let (base_name, concrete_args) = match current {
            Type::Struct(n) => (n.clone(), vec![]),
            Type::Concrete(n, args) => (n.clone(), args.clone()),
            _ => return Err(format!("Cannot access field '{}' on non-struct type {:?}", field_name, current)),
        };
        if let Some(struct_def) = self.struct_templates().get(base_name.as_str()) {
            if let Some(f) = struct_def.fields.iter().find(|f| f.name == field_name) {
                let raw_field_ty = Type::from_syn(&f.ty).unwrap_or(Type::Unit);
                let field_ty = self.canonicalize_type(&raw_field_ty);
                let mut map = BTreeMap::new();
                if let Some(g) = &struct_def.generics {
                    for (i, param) in g.params.iter().enumerate() {
                        let name = match param {
                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                        };
                        if let Some(arg) = concrete_args.get(i) {
                            map.insert(name, arg.clone());
                        }
                    }
                }
                return Ok(self.substitute_generics(&field_ty, &map));
            }
        }
        if let Some(info) = self.lookup_struct_by_type(current) {
            if let Some((_, f_ty)) = info.fields.get(field_name) {
                return Ok(f_ty.clone());
            }
        }
        Err(format!("Field '{}' not found in struct {}", field_name, base_name))
    }

    fn resolve_method_info(&self, receiver_ty: &Type, method_name: &str) -> Result<(crate::grammar::SaltFn, Option<Type>), String> {
        let (func, trait_ty, _) = self.resolve_method(receiver_ty, method_name)?;
        Ok((func, trait_ty))
    }

    fn substitute_generics(&self, ty: &Type, type_map: &BTreeMap<String, Type>) -> Type {
        ty.substitute(type_map)
    }

    fn canonicalize_type(&self, ty: &Type) -> Type {
        match ty {
            Type::Struct(raw_name) => {
                if raw_name.contains("__") { return ty.clone(); }
                if raw_name.len() == 1 && raw_name.chars().all(|c| c.is_uppercase()) { return ty.clone(); }
                let candidate = if let Some(pkg) = self.current_package.as_ref() {
                    let prefix = pkg.name.iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join("__");
                    if prefix.is_empty() { raw_name.clone() } else { format!("{}__{}", prefix, raw_name) }
                } else {
                    return ty.clone();
                };
                if self.struct_templates().contains_key(&candidate) {
                    Type::Struct(candidate)
                } else {
                    ty.clone()
                }
            }
            Type::Concrete(name, args) => {
                let canon_args: Vec<Type> = args.iter().map(|a| self.canonicalize_type(a)).collect();
                Type::Concrete(name.clone(), canon_args)
            }
            Type::Reference(inner, mutable) => {
                Type::Reference(Box::new(self.canonicalize_type(inner)), *mutable)
            }
            Type::Pointer { element, provenance, is_mutable } => {
                Type::Pointer {
                    element: Box::new(self.canonicalize_type(element)),
                    provenance: provenance.clone(),
                    is_mutable: *is_mutable,
                }
            }
            Type::Tuple(elems) => {
                let canon_elems: Vec<Type> = elems.iter().map(|e| self.canonicalize_type(e)).collect();
                Type::Tuple(canon_elems)
            }
            Type::Array(inner, size, packed) => {
                Type::Array(Box::new(self.canonicalize_type(inner)), *size, *packed)
            }
            _ => ty.clone(),
        }
    }
}