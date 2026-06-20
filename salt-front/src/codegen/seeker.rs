use crate::codegen::context::LoweringContext;
use crate::codegen::collector::MonomorphizationTask;
use crate::types::{Type, TypeKey};
use crate::grammar::{Stmt};
use std::collections::BTreeMap;
use syn::{Expr, Pat};
use crate::common::mangling::Mangler;

use crate::codegen::tracer::TypeTracer;

/// The "Visitor" Pattern (The LLVM/Clang Standard)
/// Instead of a manual match block that is prone to human error, we implement a Trait-Based AST Walker (Seeker).
pub struct Seeker<'a, 'ctx, 'b> {
    ctx: &'b mut LoweringContext<'a, 'ctx>,
}

impl<'a, 'ctx, 'b> Seeker<'a, 'ctx, 'b> {
    pub fn new(ctx: &'b mut LoweringContext<'a, 'ctx>) -> Self {
        Self { ctx }
    }

    /// Deterministic symbol mangling for monomorphized types.
    /// This ensures that call-site generator and definition generator produce identical symbols.
    /// Format: TypeName_Param1_Param2__MethodName (double underscore before method)
    /// Examples: Vec_u8__push, RawVec_i32__with_capacity, Result_i32_bool__map
    pub fn mangle_method_name(type_name: &str, method: &str, type_params: &[Type]) -> String {
        if type_params.is_empty() {
            return format!("{}__{}", type_name, method);
        }
        let params: Vec<String> = type_params.iter().map(|t| t.mangle_suffix()).collect();
        format!("{}_{}__{}", type_name, params.join("_"), method)
    }

    /// Ensure call-site discovery uses the same mangling logic as definition generation.
    pub fn mangle_monomorphized_call(&mut self, receiver_ty: &Type, method_name: &str, type_params: &[Type]) -> String {
        let type_name = receiver_ty.mangle_suffix();
        Self::mangle_method_name(&type_name, method_name, type_params)
    }


    pub fn resolve_receiver_type(&mut self, expr: &Expr, locals: &BTreeMap<String, Type>) -> Option<Type> {
         match expr {
             Expr::Path(path) => {
                 let name = path.path.segments.last()?.ident.to_string();

                 // 1. Check Local Scope
                 if let Some(ty) = locals.get(&name) {
                     return Some(ty.clone());
                 }

                 // 2. Check Global Scope (Codegen Context Cache)
                 if let Some(global_ty) = self.ctx.globals().get(&name) {
                     return Some(global_ty.clone());
                 }
                 
                 // 3. Check Registry (Cross-Module Globals) - uses fallback
                 if let Ok(key) = self.ctx.resolve_path_to_fqn(&path.path) {
                      if let Some(ty) = self.ctx.lookup_global_type(&key) {
                           return Some(ty);
                      }
                 }

                 // 4. Check for Static Module Paths (e.g., MyStruct::method)
                 if let Some(c) = name.chars().next() {
                     if c.is_uppercase() {
                        return Some(Type::Struct(name));
                     }
                 }

                 None
             },
             
             Expr::Unary(un) if matches!(un.op, syn::UnOp::Deref(_)) => {
                 let inner_ty = self.resolve_receiver_type(&un.expr, locals)?;
                 if let Type::Reference(inner, _) = inner_ty {
                     Some(*inner)
                 } else {
                     None
                 }
             },

             _ => None,
         }
    }

    fn has_packed_attr(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            attr.path().is_ident("packed")
        })
    }

    /// Exhaustively discovers all physical requirements of an expression.
    pub fn discover_requirements(&mut self, expr: &Expr, tasks: &mut Vec<MonomorphizationTask>, locals: &mut BTreeMap<String, Type>) -> Result<(), String> {
        match expr {
            // Struct literals trigger layout resolution
            Expr::Struct(s) => {
                let path_ty = syn::Type::Path(syn::TypePath { qself: None, path: s.path.clone() });
                let resolved_ty = crate::codegen::type_bridge::resolve_type(self.ctx, &crate::grammar::SynType::from_std(path_ty).map_err(|e| e.to_string())?);
                
                if let Type::Concrete(base, args) = &resolved_ty {
                     self.ctx.ensure_struct_exists(base, args)?;
                } else if let Type::Struct(name) = &resolved_ty {
                     self.ctx.ensure_struct_exists(name, &[])?;
                }

                // Recurse into fields
                for field in &s.fields {
                    self.discover_requirements(&field.expr, tasks, locals)?;
                }
            }

            // Function calls trigger specialization resolution
            Expr::Call(c) => self.discover_call_requirements(c, tasks, locals)?,

             Expr::MethodCall(m) => self.discover_method_call_requirements(m, tasks, locals)?,

            // Indexing implies array/pointer layout knowledge
            Expr::Index(i) => {
                self.discover_requirements(&i.expr, tasks, locals)?;
                self.discover_requirements(&i.index, tasks, locals)?;
            }

            // Recursive completeness
            Expr::If(i) => {
                self.discover_requirements(&i.cond, tasks, locals)?;
                // Recurse into block statements
                for s in &i.then_branch.stmts { 
                    self.walk_stmt(&Stmt::Syn(s.clone()), tasks, locals)?;
                }
                
                if let Some((_, else_br)) = &i.else_branch {
                    self.discover_requirements(else_br, tasks, locals)?;
                }
            }

            Expr::Array(a) => {
                 for elem in &a.elems { self.discover_requirements(elem, tasks, locals)?; }
            }
            Expr::Assign(a) => {
                self.discover_requirements(&a.left, tasks, locals)?;
                self.discover_requirements(&a.right, tasks, locals)?;
            }
            Expr::Binary(b) => {
                self.discover_requirements(&b.left, tasks, locals)?;
                self.discover_requirements(&b.right, tasks, locals)?;
            }
            Expr::Unary(u) => {
                self.discover_requirements(&u.expr, tasks, locals)?;
            }
            Expr::Cast(c) => {
                self.discover_requirements(&c.expr, tasks, locals)?;
                // Cast type might be struct? Usually primitive.
                // If it's a struct (reinterpret_cast via turbofish often), the type resolution happens there.
                // But Expr::Cast in Rust is `expr as Type`. Salt allows `as Type`.
                // resolve_type handles struct existence if we parse it.
                // We should check the type `c.ty`.
                let ty = crate::codegen::type_bridge::resolve_type(self.ctx, &crate::grammar::SynType::from_std(*c.ty.clone()).map_err(|e| e.to_string())?);
                if let Type::Struct(name) = &ty {
                     self.ctx.ensure_struct_exists(name, &[])?;
                } else if let Type::Concrete(base, args) = &ty {
                     self.ctx.ensure_struct_exists(base, args)?;
                }
            }
            Expr::Field(f) => {
                self.discover_requirements(&f.base, tasks, locals)?;
            }
            Expr::Paren(p) => {
                self.discover_requirements(&p.expr, tasks, locals)?;
            }
            Expr::Reference(r) => {
                self.discover_requirements(&r.expr, tasks, locals)?;
            }
            Expr::Tuple(t) => {
                for elem in &t.elems { self.discover_requirements(elem, tasks, locals)?; }
            }
            Expr::Match(m) => {
                self.discover_requirements(&m.expr, tasks, locals)?;
                for arm in &m.arms {
                    self.discover_requirements(&arm.body, tasks, locals)?;
                }
            }
            Expr::Return(r) => {
                if let Some(e) = &r.expr { self.discover_requirements(e, tasks, locals)?; }
            }
             Expr::Block(b) => {
                let mut sub_locals = locals.clone();
                for stmt in &b.block.stmts {
                    self.walk_stmt(&Stmt::Syn(stmt.clone()), tasks, &mut sub_locals)?;
                }
            },
            
            Expr::Break(_) | Expr::Continue(_) | Expr::Lit(_) | Expr::Path(_) => {}
            
            _ => {
                // Catch-all for others
            }
        }
        Ok(())
    }

pub fn discover_call_requirements(&mut self, c: &syn::ExprCall, tasks: &mut Vec<MonomorphizationTask>, locals: &mut BTreeMap<String, Type>) -> Result<(), String>  {
                // Existing call logic from walk_expr_for_calls
                 // 1. Static Calls & Global Functions: e.g., Vec::with_capacity(10) OR dealloc(...)
                if let Expr::Path(path) = &*c.func {
                    // Log path
                    let path_str = path.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");
                    if path_str.contains("array") || path_str.contains("Vec") {
                    }

                    if let Ok(target_key) = self.ctx.resolve_path_to_fqn(&path.path) {
                        if path_str.contains("array") {
                        }
                        let mut concrete_args = self.ctx.extract_call_site_generics(&path.path)?;
                        
                        // If target is a Struct (RawVec), check if we need to infer implicit generics from context
                        let mut _inferred_generics = false;
                        let mangled_key = target_key.mangle();
                        let parts: Vec<&str> = mangled_key.split("__").collect();
                        // Possible Struct Base is everything except the last part (Method)
                        if parts.len() > 1 {
                             let base_name = Mangler::mangle(&parts[..parts.len()-1]);
                             if let Some(struct_def) = self.ctx.struct_templates().get(&base_name) {
                                  if let Some(generics) = &struct_def.generics {
                                       if concrete_args.is_empty() && !generics.params.is_empty() {
                                            for p in generics.params.iter() {
                                                 let p_name = match p {
                                                     crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                                     crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                                 };
                                                 
                                                 if let Some(ctx_ty) = self.ctx.current_type_map().get(&p_name) {
                                                      concrete_args.push(ctx_ty.clone());
                                                      _inferred_generics = true;
                                                 }
                                            }
                                       }
                                  }
                             }
                        }

                        // A) Try to match as a GLOBAL Function Task (e.g. std::core::slab_alloc::dealloc)
                        if let Some(global_task) = self.ctx.resolve_global_to_task(&target_key, concrete_args.clone()) {
                            if is_task_concrete(&global_task) {
                                tasks.push(global_task);
                            } else {
                            }
                        } 
                        // B) Fallback: Try to match as a STATIC Method Task (e.g. RawVec::with_capacity)
                        else {
                             let mangled_name = target_key.mangle();
                             let parts: Vec<&str> = mangled_name.split("__").collect();
                             if parts.len() > 1 {
                                 let base_name = Mangler::mangle(&parts[..parts.len()-1]);
                                 let method_name = parts.last().ok_or_else(|| "Failed to get method name".to_string())?.to_string();
                                 
                                 // Determine Arity of Base Struct
                                 let mut struct_arity = 0;
                                 if let Some(s) = self.ctx.struct_templates().get(&base_name) {
                                     struct_arity = s.generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
                                 } else if let Some(e) = self.ctx.enum_templates().get(&base_name) {
                                     struct_arity = e.generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
                                 }
                                 
                                 if method_name == "array" {
                                 }

                                 // Distribute Args
                                 let (struct_args, method_args) = if concrete_args.len() >= struct_arity {
                                     let (s, m) = concrete_args.split_at(struct_arity);
                                     (s.to_vec(), m.to_vec())
                                 } else {
                                     (concrete_args.clone(), vec![])
                                 };
                                 
                                 let recv_ty = if struct_args.is_empty() {
                                     Type::Struct(base_name.clone())
                                 } else {
                                     Type::Concrete(base_name.clone(), struct_args)
                                 };
                                 
                                 if base_name.contains("Vec") {
                                 }

                                 match self.ctx.resolve_method_to_task(&recv_ty, &method_name, method_args) {
                                     Ok(task) => {
                                         if is_task_concrete(&task) {
                                             if base_name.contains("Vec") {
                                             }
                                             tasks.push(task);
                                         } else {
                                         }
                                     },
                                     Err(_e) => {
                                         if method_name == "array" || base_name.contains("Vec") {
                                         }
                                     }
                                 }
                             }
                        }
                    }
                }
                for arg in &c.args { self.discover_requirements(arg, tasks, locals)?; }
            
Ok(())
}

pub fn discover_method_call_requirements(&mut self, m: &syn::ExprMethodCall, tasks: &mut Vec<MonomorphizationTask>, locals: &mut BTreeMap<String, Type>) -> Result<(), String>  {
                let receiver_ty = self.resolve_receiver_type(&m.receiver, locals).unwrap_or(Type::Unit);
                if let Type::Struct(_) | Type::Concrete(..) | Type::Reference(..) = receiver_ty {
                      let generics = {
                          let mut res = Vec::new();
                          if let Some(t) = &m.turbofish {
                              for a in &t.args {
                                  match a {
                                       syn::GenericArgument::Type(ty) => {
                                           res.push(crate::codegen::type_bridge::resolve_type(self.ctx, &crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?));
                                       }
                                       syn::GenericArgument::Const(syn::Expr::Lit(syn::ExprLit{lit: syn::Lit::Int(li),..})) => 
                                          res.push(Type::Struct(li.base10_digits().to_string())),
                                      _ => res.push(Type::Unit)
                                  }
                              }
                          }
                          res
                      };
                      
                      match self.ctx.resolve_method_to_task(&receiver_ty, &m.method.to_string(), generics) {
                          Ok(task) => {
                              if is_task_concrete(&task) {
                                  tasks.push(task);
                              } else {
                              }
                          },
                          Err(_e) => {
                          }
                      }
                }
                self.discover_requirements(&m.receiver, tasks, locals)?;
                for arg in &m.args { self.discover_requirements(arg, tasks, locals)?; }
            
Ok(())
}


    pub fn walk_stmt(&mut self, stmt: &Stmt, tasks: &mut Vec<MonomorphizationTask>, locals: &mut BTreeMap<String, Type>) -> Result<(), String> {
         match stmt {
            Stmt::Syn(s) => self.walk_syn_stmt(s, tasks, locals)?,
            Stmt::Expr(e, _) => self.discover_requirements(e, tasks, locals)?,
            Stmt::While(w) => {
                 self.discover_requirements(&w.cond, tasks, locals)?;
                 for s in &w.body.stmts { self.walk_stmt(s, tasks, locals)?; }
            },
            Stmt::For(f) => {
                 self.discover_requirements(&f.iter, tasks, locals)?;
                 for s in &f.body.stmts { self.walk_stmt(s, tasks, locals)?; }
            }
            Stmt::If(i) => {
                 self.discover_requirements(&i.cond, tasks, locals)?;
                 for s in &i.then_branch.stmts { self.walk_stmt(s, tasks, locals)?; }
                 if let Some(else_br) = &i.else_branch {
                      match &**else_br {
                          crate::grammar::SaltElse::Block(b) => {
                              for s in &b.stmts { self.walk_stmt(s, tasks, locals)?; }
                          }
                          crate::grammar::SaltElse::If(elif) => {
                               // Recursive Logic
                               self.walk_stmt(&Stmt::If(elif.as_ref().clone()), tasks, locals)?;
                          }
                      }
                 }
            }
            Stmt::Return(opt_e) => {
                 if let Some(e) = opt_e { self.discover_requirements(e, tasks, locals)?; }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn walk_syn_stmt(&mut self, stmt: &syn::Stmt, tasks: &mut Vec<MonomorphizationTask>, locals: &mut BTreeMap<String, Type>) -> Result<(), String> {
        match stmt {
            syn::Stmt::Local(l) => {
                 if let Some(init) = &l.init {
                     let mut ty = self.ctx.trace_expr_type(&init.expr, locals).unwrap_or(Type::Unit);
                     
                     // Check for @packed attribute
                     if Self::has_packed_attr(&l.attrs) {
                         if let Type::Array(inner, len, _) = &ty {
                             if **inner == Type::Bool {
                                  ty = Type::Array(inner.clone(), *len, true);
                             }
                         }
                     }
                     
                     if let Pat::Ident(id) = &l.pat {
                         locals.insert(id.ident.to_string(), ty);
                     }
                     self.discover_requirements(&init.expr, tasks, locals)?;
                 }
            }
            syn::Stmt::Expr(e, _) => self.discover_requirements(e, tasks, locals)?,
            _ => {}
        }
        Ok(())
    }
}

// Helper trait to convert Stmt to Expr for Expr::If recursive hack? 
// No, I just handled Stmt::Syn in walk_stmt. 




impl<'a, 'ctx> LoweringContext<'a, 'ctx> {

    // Helper to replace the above due to signature mismatch with reality
    pub fn extract_call_site_generics(&mut self, path: &syn::Path) -> Result<Vec<Type>, String> {
         let mut params = Vec::new();
         // Check ALL segments for generics (e.g. Vec::<u8>::new)
         for seg in &path.segments {
             if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                 for arg in &args.args {
                     match arg {
                         syn::GenericArgument::Type(ty) => {
                             params.push(crate::codegen::type_bridge::resolve_type(self, &crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?));
                         }
                         syn::GenericArgument::Const(c) => {
                              // evaluate const?
                              if let syn::Expr::Lit(l) = c {
                                   if let syn::Lit::Int(i) = &l.lit {
                                       if let Ok(val) = i.base10_parse::<i64>() {
                                            // Mapping const generic to I32/I64 type hack for now as per type system
                                            params.push(Type::Struct(val.to_string()));
                                       }
                                   }
                              }
                         }
                         _ => {}
                     }
                 }
             }
         }
         Ok(params)
    }

    pub fn resolve_method_to_task(&mut self, receiver_ty: &Type, method_name: &str, generics: Vec<Type>) -> Result<MonomorphizationTask, String> {
        let (func, trait_ty, imports) = self.resolve_method(receiver_ty, method_name)?;
        
        let mut type_map = BTreeMap::new();
        // Add Self?
        // Strip Reference wrapper from self_ty - during hydration, current_self_ty should be
        // the concrete type (e.g., Result<...>), not Reference(Result<...>), otherwise Self mangling is broken
        let mut self_ty = if let Some(t) = trait_ty.as_ref() { t.clone() } else { receiver_ty.clone() };
        while let Type::Reference(inner, _) = self_ty {
            self_ty = *inner;
        }
        
        // 1. Hydrate Impl Scope (e.g., T -> bool)
        let mut base_ty = receiver_ty;
        while let Type::Reference(inner, _) = base_ty {
            base_ty = inner;
        }

        // Helper to retrieve params for a struct/enum name
        let get_params = |name: &str| -> Option<Vec<crate::grammar::GenericParam>> {
             if let Some(s) = self.struct_templates().get(name) {
                 s.generics.as_ref().map(|g| g.params.iter().cloned().collect())
             } else if let Some(e) = self.enum_templates().get(name) {
                 e.generics.as_ref().map(|g| g.params.iter().cloned().collect())
             } else {
                 None
             }
        };
        
        if let Type::Concrete(name, args) = base_ty {
             if let Some(ps) = get_params(name) {
                 for (i, p) in ps.iter().enumerate() {
                     let p_name = match p {
                         crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                         crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                     };
                     if let Some(arg) = args.get(i) {
                         type_map.insert(p_name, arg.clone());
                     }
                 }
             }
        } else if let Type::Struct(name) = base_ty {
            // FALLBACK: If we are calling a static method on a bare struct inside a generic context,
            // we must pull the mapping from the CURRENT context to prevent _SIZE leaks.
            if let Some(current_self) = self.current_self_ty().as_ref() {
                // We strip ref from current_self too just in case
                let mut curr_base = current_self;
                while let Type::Reference(inner, _) = curr_base { curr_base = inner; }

                if let Type::Concrete(curr_name, curr_args) = curr_base {
                    if curr_name == name {
                        if let Some(ps) = get_params(name) {
                            for (i, p) in ps.iter().enumerate() {
                                let p_name = match p {
                                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                };
                                if let Some(arg) = curr_args.get(i) {
                                    type_map.insert(p_name, arg.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        
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
        
        // 3. Identity Construction
        // We must substitute generics in self_ty with type_map (e.g. Vec<T> -> Vec<u8>)
        fn substitute(ty: &Type, map: &BTreeMap<String, Type>) -> Type {
            match ty {
                Type::Struct(name) => {
                    if let Some(replacement) = map.get(name) {
                        replacement.clone()
                    } else {
                        Type::Struct(name.clone())
                    }
                }
                Type::Concrete(name, args) => {
                    let new_args = args.iter().map(|a| substitute(a, map)).collect();
                    Type::Concrete(name.clone(), new_args)
                }
                Type::Reference(inner, m) => Type::Reference(Box::new(substitute(inner, map)), *m),
                Type::Window(inner, r) => Type::Window(Box::new(substitute(inner, map)), r.clone()),
                Type::Array(inner, len, packed) => Type::Array(Box::new(substitute(inner, map)), *len, *packed),
                Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| substitute(e, map)).collect()),
                // Add more variants if necessary (Fn, etc)
                _ => ty.clone()
            }
        }
        
        let concrete_self = substitute(&self_ty, &type_map);
        
        // Use unified mangling invariant (Seeker::mangle_method_name)
        let mangled_name = crate::codegen::seeker::Seeker::mangle_method_name(
            &concrete_self.mangle_suffix(),
            method_name,
            &generics
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

    pub fn resolve_global_to_task(&mut self, key: &TypeKey, concrete_args: Vec<Type>) -> Option<MonomorphizationTask> {
         let module_path = key.path.join(".");
         
         if let Some(reg) = self.config.registry {
             if let Some(module) = reg.modules.get(&module_path) {
                 if let Some(func) = module.function_templates.get(&key.name) {
                     // Found function def!
                     
                     let mut type_map = BTreeMap::new();
                     if let Some(g) = &func.generics {
                         for (i, p) in g.params.iter().enumerate() {
                             let p_name = match p {
                                 crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                 crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                             };
                             if let Some(arg) = concrete_args.get(i) {
                                  type_map.insert(p_name, arg.clone());
                             }
                         }
                     }
                     
                     let mangled_name = if concrete_args.is_empty() {
                          format!("{}__{}", module_path.replace(".", "__"), func.name)
                     } else {
                          let mut s = format!("{}__{}", module_path.replace(".", "__"), func.name);
                          for arg in &concrete_args {
                              s.push('_');
                              s.push_str(&arg.mangle_suffix());
                          }
                          s
                     };

                     return Some(MonomorphizationTask {
                         identity: key.clone(),
                         mangled_name,
                         func: func.clone(),
                         concrete_tys: concrete_args,
                         self_ty: None,
                         imports: module.imports.clone(),
                         type_map,
                     });
                 }
             } else if module_path.is_empty() {
                 // Check LOCAL file
                 for item in &self.config.file.items {
                     if let crate::grammar::Item::Fn(f) = item {
                         if f.name.to_string() == key.name {
                            // Found local match
                             let pkg_prefix = if let Some(pkg) = &self.config.file.package {
                                  Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>()) + "__"
                             } else {
                                  "".to_string()
                             };

                             let mut task_imports = self.config.file.imports.clone();
                             // Inject self-imports for local types to ensure resolution works within the task
                             if !pkg_prefix.is_empty() {
                                  // We must add "Self Imports" for every struct/enum in the file
                                  for item in &self.config.file.items {
                                      let (ident_name, mangled_str) = match item {
                                          crate::grammar::Item::Struct(s) => (&s.name, format!("{}{}", pkg_prefix, s.name)),
                                          crate::grammar::Item::Enum(e) => (&e.name, format!("{}{}", pkg_prefix, e.name)),
                                          _ => continue
                                      };
                                      // Skip Ptr if handled globally, but here we just blindly add alias which is fine
                                      let mangled_ident = syn::Ident::new(&mangled_str, proc_macro2::Span::call_site());
                                      let mut p = syn::punctuated::Punctuated::new();
                                      p.push(mangled_ident);
                                      task_imports.push(crate::grammar::ImportDecl { 
                                          name: p, 
                                          alias: Some(ident_name.clone()), 
                                          group: None 
                                      });
                                  }
                             }
                             
                             let mut type_map = BTreeMap::new();
                             if let Some(g) = &f.generics {
                                 for (i, p) in g.params.iter().enumerate() {
                                     let p_name = match p {
                                         crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                         crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                     };
                                     if let Some(arg) = concrete_args.get(i) {
                                          type_map.insert(p_name, arg.clone());
                                     }
                                 }
                             }
                             
                             let mangled_name = if concrete_args.is_empty() {
                                  format!("{}{}", pkg_prefix, f.name)
                             } else {
                                  let mut s = format!("{}{}", pkg_prefix, f.name);
                                  for arg in &concrete_args {
                                      s.push('_');
                                      s.push_str(&arg.mangle_suffix());
                                  }
                                  s
                             };
                             
                             if f.name.to_string().contains("test_basic_arrays") {
                             }
        
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
                     }
                 }
             } else {
             }
         }
         None
    }

    pub fn scan_function_for_calls(&mut self, func: &crate::grammar::SaltFn) -> Result<Vec<MonomorphizationTask>, String> {
        let mut tasks = Vec::new();
        let mut locals = BTreeMap::new();
        // Register arguments
        for arg in &func.args {
            if let Some(ty) = &arg.ty {
                locals.insert(arg.name.to_string(), crate::codegen::type_bridge::resolve_type(self, ty));
            }
        }
        
        let mut seeker = Seeker::new(self);
        
        for stmt in &func.body.stmts {
            seeker.walk_stmt(stmt, &mut tasks, &mut locals)?;
        }
        Ok(tasks)
    }
}

fn is_task_concrete(task: &MonomorphizationTask) -> bool {
    let name = &task.mangled_name;
    // P0: Forbidden residues that indicate a template body leak
    !(name.contains("_T") || name.contains("_E") || name.contains("_SIZE") || name.contains("_U"))
}
