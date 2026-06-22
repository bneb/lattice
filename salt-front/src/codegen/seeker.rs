use crate::codegen::context::LoweringContext;
use crate::codegen::collector::MonomorphizationTask;
use crate::types::{Type, TypeKey};
use crate::grammar::{Stmt};
use std::collections::BTreeMap;
use syn::{Expr, Pat};
use crate::common::mangling::Mangler;

use crate::codegen::tracer::TypeTracer;
use crate::codegen::seeker_resolve::is_task_concrete;

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

    /// Infer implicit generics for a static method call by matching the struct
    /// template's generic params against the current type map context.
    fn infer_implicit_generics(&mut self, target_key: &TypeKey, concrete_args: &mut Vec<Type>) {
        let mangled_key = target_key.mangle();
        let parts: Vec<&str> = mangled_key.split("__").collect();
        if parts.len() <= 1 {
            return;
        }
        let base_name = Mangler::mangle(&parts[..parts.len() - 1]);
        let struct_def = match self.ctx.struct_templates().get(&base_name) {
            Some(s) => s,
            None => return,
        };
        let generics = match &struct_def.generics {
            Some(g) => g,
            None => return,
        };
        if !concrete_args.is_empty() || generics.params.is_empty() {
            return;
        }
        for p in generics.params.iter() {
            let p_name = match p {
                crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
            };
            if let Some(ctx_ty) = self.ctx.current_type_map().get(&p_name) {
                concrete_args.push(ctx_ty.clone());
            }
        }
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

                    if let Ok(target_key) = self.ctx.resolve_path_to_fqn(&path.path) {
                        let mut concrete_args = self.ctx.extract_call_site_generics(&path.path)?;
                        self.infer_implicit_generics(&target_key, &mut concrete_args);

                        // A) Try to match as a GLOBAL Function Task (e.g. std::core::slab_alloc::dealloc)
                        if let Some(global_task) = self.ctx.resolve_global_to_task(&target_key, concrete_args.clone()) {
                            if is_task_concrete(&global_task) {
                                tasks.push(global_task);
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
                                 
                                 base_name.contains("Vec");

                                 match self.ctx.resolve_method_to_task(&recv_ty, &method_name, method_args) {
                                     Ok(task) => {
                                         if is_task_concrete(&task) {
                                             base_name.contains("Vec");
                                             tasks.push(task);
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
            Stmt::Return(Some(e)) => { self.discover_requirements(e, tasks, locals)?; }
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
         for seg in &path.segments {
             if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                 for arg in &args.args {
                     match arg {
                         syn::GenericArgument::Type(ty) => {
                             params.push(crate::codegen::type_bridge::resolve_type(self, &crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?));
                         }
                         syn::GenericArgument::Const(syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(i), .. })) => {
                             if let Ok(val) = i.base10_parse::<i64>() {
                                 params.push(Type::Struct(val.to_string()));
                             }
                         }
                         _ => {}
                     }
                 }
             }
         }
         Ok(params)
    }
}
