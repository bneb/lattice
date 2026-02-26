use crate::codegen::context::LoweringContext;
use crate::types::{Type, TypeKey};
use crate::codegen::expr::utils::{resolve_path_to_enum, EnumVariantResolution, resolve_package_prefix_ctx};
use std::collections::{BTreeMap, HashMap};
use crate::common::mangling::Mangler;
use crate::grammar::SaltFn;
use crate::codegen::collector::MonomorphizationTask;

#[derive(Debug)]
pub enum CallKind {
    /// Name, RetTy, ArgTys, LazyTask
    Function(String, Type, Vec<Type>, Option<Box<MonomorphizationTask>>), 
    /// Intrinsic Name, Explicit Generics (e.g. "size_of", [U8])
    Intrinsic(String, Vec<Type>),
    /// Enum Variant Construction
    EnumConstructor(EnumVariantResolution),
    /// Struct Literal: Name, Field Types (for in-place initialization)
    StructLiteral(String, Vec<(String, Type)>),
    /// Transparent Vec Accessor: method_name ("get_unchecked"|"set_unchecked"), 
    /// element_type, receiver_expr, args (index, [value for set])
    TransparentVecAccess {
        method: String,
        element_ty: Type,
        receiver: Box<syn::Expr>,
        args: Vec<syn::Expr>,
    },
}

struct ResolutionTarget {
    template: SaltFn,
    base_name: String,
    kind: TargetKind,
    self_ty: Option<Type>, // Only for methods
    imports: Vec<crate::grammar::ImportDecl>,
}

#[derive(Debug, PartialEq)]
enum TargetKind {
    Local,
    Global,
    Method,
}

pub struct CallSiteResolver<'a, 'ctx, 'b> {
    ctx: &'b mut LoweringContext<'a, 'ctx>,
}

impl<'a, 'ctx, 'b> CallSiteResolver<'a, 'ctx, 'b> {
    pub fn new(ctx: &'b mut LoweringContext<'a, 'ctx>) -> Self {
        Self { ctx }
    }

    /// The "Brain" of the operation.
    /// Resolves a generic call into a concrete specialization (LazyTask).
    pub fn resolve_call(
        &mut self, 
        call: &syn::ExprCall, 
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
        expected_ty: Option<&Type>
    ) -> Result<CallKind, String> {
        
        // 0a. Field-Based Method Call Detection (FIXES RECURSION BUG)
        // If call.func is Expr::Field (e.g., GLOBAL_ALLOC.alloc), this is a method call on a receiver.
        // We must resolve via method_registry, NOT as a free function.
        if let syn::Expr::Field(field_expr) = &*call.func {
            let method_name = match &field_expr.member {
                syn::Member::Named(ident) => ident.to_string(),
                syn::Member::Unnamed(idx) => format!("{}", idx.index),
            };

            
            // Infer the receiver type
            let receiver_ty = crate::codegen::type_bridge::infer_expr_type(self.ctx, &field_expr.base, local_vars)?;
            // Infer the receiver type
            
            // TRANSPARENT VEC ACCESSOR INTERCEPT (Zero-Overhead Path)
            // For Vec<T>::get_unchecked and Vec<T>::set_unchecked, bypass normal method resolution
            // and emit direct MLIR llvm.load/llvm.store at the call site.
            if method_name == "get_unchecked" || method_name == "set_unchecked" {
                // Check if receiver is Vec<T> or &Vec<T> or &mut Vec<T>
                let inner_ty = match &receiver_ty {
                    Type::Reference(inner, _) => inner.as_ref().clone(),
                    Type::Concrete(_, _) => receiver_ty.clone(),
                    Type::Struct(name) if name.contains("Vec") => receiver_ty.clone(),
                    _ => receiver_ty.clone(),
                };
                
                // Extract element type from Vec<T>
                let element_ty = match &inner_ty {
                    Type::Concrete(name, args) if name.contains("Vec") && !args.is_empty() => {

                        args[0].clone()
                    },
                    Type::Struct(name) if name.contains("Vec_") => {
                        // Extract type from mangled name like "std__collections__vec__Vec_i32"
                        let suffix = name.rsplit('_').next().unwrap_or("i64");

                        match suffix {
                            "i32" => Type::I32,
                            "i64" => Type::I64,
                            "u8" => Type::U8,
                            "f32" => Type::F32,
                            "f64" => Type::F64,
                            _ => Type::I64, // Default fallback
                        }
                    },
                    _ => Type::I64, // Fallback - will be refined in handler
                };
                
                return Ok(CallKind::TransparentVecAccess {
                    method: method_name.clone(),
                    element_ty,
                    receiver: Box::new((*field_expr.base).clone()),
                    args: call.args.iter().cloned().collect(),
            });
            }
            
            // Construct the method key for method lookup
            let type_key = crate::codegen::type_bridge::type_to_type_key(&receiver_ty);
            
            // [V4.0 TRAIT SOLVER] Two-Tier Dispatch System
            // Tier 1: Try signature-aware overload resolution (TraitRegistry)
            // Tier 2: Fall back to legacy name-based lookup (MethodRegistry)
            
            // STEP 1: Infer Argument Types for Overload Resolution
            let args_vec: Vec<syn::Expr> = call.args.iter().cloned().collect();
            let arg_types: Vec<Type> = args_vec.iter()
                .filter_map(|expr| {
                    crate::codegen::type_bridge::infer_expr_type(self.ctx, expr, local_vars).ok()
                })
                .collect();
            
            // STEP 2: Try Trait-Based Overload Resolution (Tier 1 - V4.0 Path)
            // Clone the result inside the borrow scope to satisfy Rust's borrow checker
            let trait_result: Option<(SaltFn, Option<Type>, Vec<crate::grammar::ImportDecl>)> = {
                let registry = self.ctx.trait_registry();
                registry.resolve_overload(&type_key, &method_name, &arg_types)
                    .map(|resolved| (resolved.func.clone(), resolved.self_ty.clone(), resolved.imports.clone()))
            };
            
            // [V4.0 SOVEREIGN] Signature-First: TraitRegistry is the ONLY path
            // No Tier 2 fallback - if method not in TraitRegistry, it's not found
            let method_info: Option<(SaltFn, Option<Type>, Vec<crate::grammar::ImportDecl>)> = 
                if let Some(resolved) = trait_result {

                    Some(resolved)
                } else {
                    // [V4.0] Try legacy-style lookup via TraitRegistry (for unregistered methods)
                    self.ctx.trait_registry().get_legacy(&type_key, &method_name)
                };
            
            // STEP 4: Dispatch
            if let Some((func, self_ty, imports)) = method_info {

                
                // Prepare target for unification and mangling
                let target = ResolutionTarget {
                    template: func.clone(),
                    base_name: format!("{}__{}",  crate::common::mangling::Mangler::mangle_type_key(&type_key), method_name),
                    kind: TargetKind::Method,
                    self_ty: self_ty.clone(),
                    imports: imports.clone(),
                };
                
                // Unify generics based on receiver and arguments
                // For instance method calls, extract concrete type args from receiver
                // e.g., for map.get() where map: HashMap<i64, i64>, extract [i64, i64]
                let receiver_generics: Vec<Type> = match &self_ty {
                    Some(Type::Concrete(_, args)) => args.clone(),
                    _ => vec![],
                };

                let spec_map = self.unify_generics(&target, &receiver_generics, &args_vec, local_vars, expected_ty)?;
                
                // Calculate mangled name
                let mangled_name = self.mangle_specialization(&target.base_name, &spec_map, &target.template);
                
                // Resolve signature
                let (ret_ty, arg_tys) = self.resolve_signature(&target.template, &spec_map)?;
                
                // Package LazyTask
                // [DETERMINISTIC ORDERING] Use struct template's parameter order for concrete_tys.
                // The function template's generics may be in non-deterministic order.
                let concrete_tys: Vec<Type> = {
                    let mut tys = Vec::new();
                    let mut used_struct_order = false;
                    // Try struct template order first
                    if let Some(st) = &self_ty {
                        let struct_name = match st {
                            Type::Struct(n) | Type::Concrete(n, _) => Some(n.clone()),
                            _ => None,
                        };
                        if let Some(name) = struct_name {
                            if let Some(tmpl) = self.ctx.struct_templates().get(&name) {
                                if let Some(sg) = &tmpl.generics {
                                    tys = sg.params.iter().map(|p| {
                                        let pn = match p {
                                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                        };
                                        spec_map.get(&pn).cloned().unwrap_or(Type::Unit)
                                    }).collect();
                                    used_struct_order = true;
                                }
                            }
                        }
                    }
                    // Fall back to function template generics (for free functions)
                    if !used_struct_order {
                        if let Some(g) = &target.template.generics {
                            tys = g.params.iter().map(|p| {
                                let name = match p {
                                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                };
                                spec_map.get(&name).cloned().unwrap_or(Type::Unit)
                            }).collect();
                        }
                    }
                    tys
                };

                let resolved_self = self_ty.as_ref().map(|st| st.substitute(&spec_map));
                
                let lazy_task = Box::new(crate::codegen::collector::MonomorphizationTask {
                    identity: TypeKey { path: vec![], name: mangled_name.clone(), specialization: None },
                    mangled_name: mangled_name.clone(),
                    func: target.template.clone(),
                    concrete_tys,
                    self_ty: resolved_self,
                    imports: target.imports,
                    type_map: spec_map,
                });

                return Ok(CallKind::Function(mangled_name, ret_ty, arg_tys, Some(lazy_task)));
            } else {

            }
        }
        
        // 0c. EARLY INTRINSIC INTERCEPT (Fixes "Lookup Trap")
        // Check for protected intrinsic names BEFORE resolve_path mangles them.
        // This prevents println → test__println transformation.
        if let syn::Expr::Path(path_expr) = &*call.func {
            if path_expr.path.segments.len() == 1 {
                let raw_name = path_expr.path.segments[0].ident.to_string();
                if self.is_intrinsic(&raw_name) {
                    // Extract explicit generics from the path segment
                    let explicit_generics = self.extract_generics_from_segment(&path_expr.path.segments[0])?;
                    return Ok(CallKind::Intrinsic(raw_name, explicit_generics));
                }
                
                // 0d. EARLY STRUCT LITERAL INTERCEPT
                // Check if raw_name (or mangled version) is a struct type
                // This prevents Board(...) from being resolved as a function call
                let mangled_name = self.ctx.mangle_fn_name(&raw_name);
                
                // TOP MINDS: Ensure struct exists before checking registry
                // This handles cross-function hydration where struct isn't in registry yet
                let _ = self.ctx.ensure_struct_exists(&mangled_name, &[]);
                let _ = self.ctx.ensure_struct_exists(&raw_name, &[]);
                
                {
                    let struct_reg = self.ctx.struct_registry();
                    // Try mangled name first (e.g., main__Board)
                    if let Some(info) = struct_reg.values().find(|i| i.name == mangled_name.to_string()) {
                        let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                            .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                            .collect::<Vec<_>>();
                        fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                        let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                        return Ok(CallKind::StructLiteral(mangled_name.to_string(), fields));
                    }
                    // Also try raw name (for unmangled structs)
                    if let Some(info) = struct_reg.values().find(|i| i.name == raw_name) {
                        let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                            .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                            .collect::<Vec<_>>();
                        fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                        let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                        return Ok(CallKind::StructLiteral(raw_name, fields));
                    }
                    // [VERIFIED METAL] Phase 5: Use centralized struct lookup
                    if let Some(info) = self.ctx.find_struct_by_name(&raw_name) {
                        let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                            .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                            .collect::<Vec<_>>();
                        fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                        let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                        return Ok(CallKind::StructLiteral(info.name.clone(), fields));
                    }
                }
                
                // TOP MINDS: Template-based fallback - the struct might be in templates but not registry yet
                // Try to find in struct_templates and trigger instantiation
                {
                    // [VERIFIED METAL] Phase 5: Use centralized template lookup
                    let (has_exact, suffix_match) = {
                        let templates = self.ctx.struct_templates();
                        let exact = templates.contains_key(&mangled_name.to_string());
                        let suffix_m = self.ctx.find_struct_template_by_name(&raw_name);
                        (exact, suffix_m)
                    };
                    
                    // Try exact match first
                    if has_exact {
                        let _ = self.ctx.ensure_struct_exists(&mangled_name, &[]);
                        // Re-check registry after instantiation
                        let struct_reg = self.ctx.struct_registry();
                        if let Some(info) = struct_reg.values().find(|i| i.name == mangled_name.to_string()) {
                            let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                                .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                                .collect::<Vec<_>>();
                            fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                            let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                            return Ok(CallKind::StructLiteral(mangled_name.to_string(), fields));
                        }
                    }
                    
                    // Try suffix match
                    if let Some(template_name) = suffix_match {
                        let _ = self.ctx.ensure_struct_exists(&template_name, &[]);
                        // Re-check registry after instantiation
                        let struct_reg = self.ctx.struct_registry();
                        if let Some(info) = struct_reg.values().find(|i| i.name == template_name) {
                            let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                                .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                                .collect::<Vec<_>>();
                            fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                            let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                            return Ok(CallKind::StructLiteral(info.name.clone(), fields));
                        }
                    }
                }
            }
        }
        
        // 0e. [SOVEREIGN FIX V3] EARLY MODULE FUNCTION INTERCEPT
        // Check for module-level functions BEFORE resolve_path mangles them.
        // This prevents EMPTY() -> HashMap::EMPTY() transformation inside impl blocks.
        // V3: Check imports list since current_package is None during hydration.
        if let syn::Expr::Path(path_expr) = &*call.func {
            if path_expr.path.segments.len() == 1 {
                let raw_name = path_expr.path.segments[0].ident.to_string();
                
                // Check if raw_name matches the suffix of any import
                // Imports contain entries like "std__collections__hash_map__EMPTY" with alias="EMPTY"
                let imports = self.ctx.imports();
                for imp in imports.iter() {
                    // [SOVEREIGN V3.1] Handle self-imports with aliases
                    // e.g., name=["std__collections__hash_map__EMPTY"], alias=Some("EMPTY")
                    if imp.name.len() == 1 && imp.group.is_none() {
                        // Check if alias matches our raw_name
                        let alias_matches = imp.alias.as_ref().map_or(false, |a| a.to_string() == raw_name);
                        if alias_matches {
                            let single_str = imp.name[0].to_string();

                            
                            // Extract package path from the import (strip the __raw_name suffix)
                            if single_str.contains("__") {
                                let pkg_mangled = &single_str[..single_str.len() - raw_name.len() - 2]; // -2 for "__"
                                let pkg_path = pkg_mangled.replace("__", ".");
                                
                                // Look up in registry
                                if let Some(registry) = self.ctx.config.registry {
                                    if let Some(mod_info) = registry.modules.get(&pkg_path) {
                                        if let Some(func) = mod_info.function_templates.get(&raw_name) {

                                            
                                            let empty_map = std::collections::BTreeMap::new();
                                            let (ret_ty, arg_tys) = self.resolve_signature(func, &empty_map)?;
                                            
                                            let lazy_task = Box::new(crate::codegen::collector::MonomorphizationTask {
                                                identity: TypeKey { path: vec![], name: single_str.clone(), specialization: None },
                                                mangled_name: single_str.clone(),
                                                func: func.clone(),
                                                concrete_tys: vec![],
                                                self_ty: None,
                                                imports: mod_info.imports.clone(),
                                                type_map: std::collections::BTreeMap::new(),
                                            });
                                            
                                            return Ok(CallKind::Function(single_str, ret_ty, arg_tys, Some(lazy_task)));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // 0b. Pre-Phase: Resolve Path and Explicit Generics (original flow)
        let (func_name, explicit_generics) = self.resolve_path(&call.func)?;
        
        // 1. Intrinsic Check (Fast Path) - for qualified paths like intrin::macos_syscall
        if self.is_intrinsic(&func_name) {
             return Ok(CallKind::Intrinsic(func_name, explicit_generics));
        }
        
        // 2. Enum Variant Check (Constructor)
        // 2. Enum Variant Check (Constructor)
        if let Some(res) = resolve_path_to_enum(self.ctx, &func_name, &explicit_generics, expected_ty) {

            return Ok(CallKind::EnumConstructor(res));
        }


        // 2.5 Struct Literal Check (In-Place Constructor)
        // Check if func_name matches a known struct type - emit direct initialization instead of function call
        {
            let struct_reg = self.ctx.struct_registry();
            if let Some(info) = struct_reg.values().find(|i| i.name == func_name) {
                let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                    .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                    .collect::<Vec<_>>();
                fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                return Ok(CallKind::StructLiteral(func_name, fields));
            }
        }

        // 3. Identify Target (Local / Global / Method)
        let target = self.identify_target(&func_name, &explicit_generics, &call.args, local_vars)
            .ok_or_else(|| {
                format!("Undefined function or symbol: '{}'", func_name)
            })?;


        // 4. Unify Generics (Interface for Inference)
        // Map T -> Concrete based on (Explicit Args + Inference from Call Args)
        // 4. Unify Generics (Interface for Inference)
        // Map T -> Concrete based on (Explicit Args + Inference from Call Args + Context)
        let args_vec: Vec<syn::Expr> = call.args.iter().cloned().collect();
        let spec_map = self.unify_generics(&target, &explicit_generics, &args_vec, local_vars, expected_ty)?;

        // 5. Calculate Mangled Name (Identity)
        let mangled_name = self.mangle_specialization(&target.base_name, &spec_map, &target.template);

        // 6. Resolve Signature (Return Type & Arg Types)
        // We must substitute the generics in the signature with our concrete map.
        let (ret_ty, arg_tys) = self.resolve_signature(&target.template, &spec_map)?;

        // 7. Package LazyTask
        // If the function is generic OR we are in a generic context that specializes it,
        // we create a task. even non-generic functions are packaged as tasks for "Lazy Discovery".
        // [DETERMINISTIC ORDERING] Always prefer struct template's parameter order for concrete_tys.
        let concrete_tys: Vec<Type> = {
            let mut tys = Vec::new();
            let mut used_struct_order = false;
            // Priority 1: Use struct template parameter order (deterministic)
            if let Some(self_ty) = &target.self_ty {
                let struct_name = match self_ty {
                    Type::Struct(n) | Type::Concrete(n, _) => Some(n.clone()),
                    _ => None,
                };
                if let Some(name) = struct_name {
                    if let Some(tmpl) = self.ctx.struct_templates().get(&name) {
                        if let Some(sg) = &tmpl.generics {
                            tys = sg.params.iter().map(|p| {
                                let pn = match p {
                                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                };
                                spec_map.get(&pn).cloned().unwrap_or(Type::Unit)
                            }).collect();
                            // Also add any method-level generics not in struct template
                            if let Some(g) = &target.template.generics {
                                for param in &g.params {
                                    let fn_name = match param {
                                        crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                        crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                    };
                                    let already_in_struct = sg.params.iter().any(|sp| {
                                        let sn = match sp {
                                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                        };
                                        sn == fn_name
                                    });
                                    if !already_in_struct {
                                        if let Some(ty) = spec_map.get(&fn_name) {
                                            tys.push(ty.clone());
                                        }
                                    }
                                }
                            }
                            used_struct_order = true;
                        }
                    }
                }
            }
            // Priority 2: Function template generics (for free functions)
            if !used_struct_order {
                if let Some(g) = &target.template.generics {
                    tys = g.params.iter().map(|p| {
                        let name = match p {
                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                        };
                        spec_map.get(&name).cloned().unwrap_or(Type::Unit)
                    }).collect();
                } else if !spec_map.is_empty() {
                    // [PHASE 3.5] Function has no function-level generics, but inference
                    // bound struct-level generics. Use struct's generic params.
                    if let Some(self_ty) = &target.self_ty {
                        let struct_name = match self_ty {
                            Type::Struct(n) | Type::Concrete(n, _) => Some(n.clone()),
                            _ => None,
                        };
                        if let Some(name) = struct_name {
                            if let Some(tmpl) = self.ctx.struct_templates().get(&name) {
                                if let Some(sg) = &tmpl.generics {
                                    tys = sg.params.iter().map(|p| {
                                        let pn = match p {
                                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                        };
                                        spec_map.get(&pn).cloned().unwrap_or(Type::Unit)
                                    }).collect();
                                }
                            }
                        }
                    }
                }
            }
            tys
        };

        let type_key = TypeKey {
            path: vec![],
            name: mangled_name.clone(),
            specialization: None,
        };
        
        // Resolve self type if method
        let resolved_self = if let Some(st) = &target.self_ty {
             Some(st.substitute(&spec_map))
        } else {
             None
        };

        let lazy_task = Box::new(crate::codegen::collector::MonomorphizationTask {
            identity: type_key,
            mangled_name: mangled_name.clone(),
            func: target.template.clone(),
            concrete_tys,
            self_ty: resolved_self,
            imports: target.imports, // Use captured imports from definition site
            type_map: spec_map,
        });

        Ok(CallKind::Function(
            mangled_name, 
            ret_ty, 
            arg_tys, 
            Some(lazy_task)
        ))
    }
    
    // --- Helper Logic ---

    fn resolve_path(&mut self, expr: &syn::Expr) -> Result<(String, Vec<Type>), String> {
        if let syn::Expr::Path(p) = expr {
             let segments: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();

             let mut generics = Vec::new();
             
             // Extract generics from ALL segments
             // e.g. Vec::<u8>::with_capacity -> u8 is on 'Vec' segment
             for segment in &p.path.segments {
                 if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                     for arg in &args.args {
                         if let syn::GenericArgument::Type(ty) = arg {
                             generics.push(crate::codegen::type_bridge::resolve_type(self.ctx, &crate::grammar::SynType::from_std(ty.clone()).unwrap()));
                         }
                     }
                 }
             }

             // Special Case: intrin module
             if segments.first().map(|s| s == "intrin").unwrap_or(false) {
                 if segments.len() == 2 {
                     return Ok((segments[1].clone(), generics));
                 }
             }

             // Package Resolution (Imports)
             if let Some((pkg, item)) = resolve_package_prefix_ctx(self.ctx, &segments) {
                 let full_name = if item.is_empty() { pkg } else { format!("{}__{}", pkg, item) };
                 return Ok((full_name, generics));
             }
             
             // Default: Mangled Local Path
             let mangled = Mangler::mangle(&segments);
             
             // Check if it matches a global alias/import exactly
             let resolved_name = self.ctx.imports().iter().find_map(|imp| {
                if imp.alias.as_ref().map_or(false, |a| a == &mangled) {
                     Some(Mangler::mangle(&imp.name.iter().map(|i| i.to_string()).collect::<Vec<_>>()))
                } else if let Some(group) = &imp.group {
                     if group.iter().any(|id| id.to_string() == mangled) {
                         let pkg_mangled = Mangler::mangle(&imp.name.iter().map(|i| i.to_string()).collect::<Vec<_>>());
                         if pkg_mangled.is_empty() {
                             Some(mangled.clone())
                         } else {
                             Some(format!("{}__{}",pkg_mangled, mangled))
                         }
                     } else { None }
                } else { None }
             });
             
             // [KERNEL FIX] Extern fn declarations take priority over wildcard imports.
             // If the symbol is declared as `extern fn` in this file, don't expand it.
             if segments.len() == 1 && self.ctx.external_decls().contains(&mangled) {
                 return Ok((mangled, generics));
             }

             // [FIX] Wildcard Import Resolution: Check `use X::*` imports via Registry
             // When import has no alias AND no group, search that module's exports for the symbol.
             let resolved_name = resolved_name.or_else(|| {
                 if let Some(reg) = self.ctx.config.registry {
                     for imp in self.ctx.imports().iter() {
                         // Wildcard import: has path, no alias, no group
                         let is_wildcard = imp.alias.is_none() && imp.group.is_none() && !imp.name.is_empty();
                         let import_path = imp.name.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(".");
                        
                         if is_wildcard {
                             if let Some(mod_info) = reg.modules.get(&import_path) {
                                 let pkg_prefix = mod_info.package.replace(".", "__");

                                 
                                 // Check if first segment (type name) exists in this module's exports
                                 if segments.len() >= 1 {
                                     let type_name = &segments[0];
                                     
                                     // Check struct templates (e.g., InterpolatedStringHandler)
                                     if mod_info.struct_templates.contains_key(type_name) {
                                         let fqn = format!("{}__{}",pkg_prefix, mangled);
                                         return Some(fqn);
                                     }
                                     
                                     // Check concrete structs
                                     if mod_info.structs.contains_key(type_name) {
                                         let fqn = format!("{}__{}",pkg_prefix, mangled);
                                         return Some(fqn);
                                     }
                                     
                                     // Check standalone functions
                                     if segments.len() == 1 && mod_info.functions.contains_key(type_name) {
                                         let fqn = format!("{}__{}",pkg_prefix, type_name);
                                         return Some(fqn);
                                     }
                                     
                                     // Check enums
                                     if mod_info.enum_templates.contains_key(type_name) {
                                         let fqn = format!("{}__{}",pkg_prefix, mangled);
                                         return Some(fqn);
                                     }
                                 }
                             } else {

                             }
                         }
                     }
                 }
                 None
             }).unwrap_or(mangled);


             Ok((resolved_name, generics))

        } else {
             Err("Call target must be a path".to_string())
        }
    }
    
    fn is_intrinsic(&mut self, name: &str) -> bool {
        name == "size_of" || name == "align_of" || name == "zeroed" || name == "unreachable" ||
        name == "popcount" || name == "ctpop" || name == "println" || name == "print" ||
        name == "trailing_zeros" || name == "cttz" || name == "leading_zeros" || name == "ctlz" ||
        // [FIX] Bit manipulation intrinsics used by kernel scheduler (std.math.* aliases)
        name == "ctz_u64" || name == "clz_u64" || name == "popcount_u64" ||
        name == "reinterpret_cast" || name == "ref_to_addr" || name == "is_null" ||
        // [SOVEREIGN OPTIMIZATION] Bulk memory intrinsics
        name == "memset" || name == "memcpy" ||
        // V1.6 Refined Intrinsics (Phase 4A/4B)
        name == "fused_cross_entropy" || name == "ml__fused_cross_entropy" ||
        name == "read_vector" ||
        // V2.2 Shadow Reduction: Register-resident tensor updates
        name == "update_tensor" || name == "fma_update" ||
        // [SOVEREIGN V3] ML Intrinsics
        name == "matmul" || name.starts_with("matmul_into") || name == "update_weights" || name == "v_fma" || name == "v_add" || name == "v_mul" || name == "v_max" || name == "v_sum" || name == "v_hsum" || name == "v_relu" || name == "v_broadcast" ||
        name == "__internal_dispatch_matmul" || name == "__internal_fma_update" ||
        name == "mmap_view" || name == "cast_view" ||
        name.contains("macos_syscall") ||
        name.starts_with("intrin_") || name.starts_with("tensor_alloc") || name.contains("ptr_offset") || name.contains("ptr_read") || name.contains("ptr_write") ||
        // [SOVEREIGN PHASE 3] Shaped tensor allocation
        name == "alloc_tensor" ||
        // [SOVEREIGN V6] Vector Intrinsics
        name == "vector_load" || name == "vector_store" || name == "vector_fma" || name == "vector_reduce_add" || name == "vector_splat" ||
        // [SOVEREIGN V6] Target Feature Detection
        name.starts_with("target__") ||
        // [std.nn] Neural network building blocks
        name == "add_bias" || name == "relu" || name == "relu_grad" ||
        name == "zeros" || name == "scale" || name == "argmax" ||
        name == "sigmoid" || name == "tanh_activation" ||
        name == "softmax_cross_entropy_grad" ||
        // [OPERATION MATH KERNEL] std.math → LLVM intrinsics
        name.starts_with("std__math__") ||
        name == "expf" || name == "logf" || name == "sqrtf" || name == "powf" ||
        name == "sinf" || name == "cosf" || name == "fabsf" || name == "floorf" || name == "ceilf" ||
        // [SOVEREIGN FIX] Atomic intrinsics for kernel lock-free data structures
        name == "cmpxchg" || name.contains("atomic_cas") || name.contains("ptr_is_null") ||
        // [salt.atomic] Concurrency primitives — must bypass package mangling
        // so they route to the intrinsic handler in intrinsics.rs
        name == "spin_loop_hint" || name == "cycle_counter" || name == "read_tls_deadline" ||
        name == "atomic_add_i64" || name == "atomic_load_i64" || name == "atomic_store_i64" ||
        // [salt.fn_ptr] Function pointer address extraction
        name == "fn_addr"
    }
    
    /// Extract generic type arguments from a path segment (e.g., println::<T> -> [T])
    fn extract_generics_from_segment(&mut self, segment: &syn::PathSegment) -> Result<Vec<Type>, String> {
        let mut generics = Vec::new();
        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
            for arg in &args.args {
                if let syn::GenericArgument::Type(ty) = arg {
                    generics.push(crate::codegen::type_bridge::resolve_type(self.ctx, &crate::grammar::SynType::from_std(ty.clone()).unwrap()));
                }
            }
        }
        Ok(generics)
    }

    fn identify_target(&mut self, 
        name: &str, 
        _generics: &[Type],
        _args_exprs: &syn::punctuated::Punctuated<syn::Expr, syn::token::Comma>,
        _local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>
    ) -> Option<ResolutionTarget> {
        
        let (canonical_name, _is_external) = self.resolve_canonical_name(name);
        
        // 1. Check Local Definitions (File Scope) using Canonical Name
        {
             let file = self.ctx.config.file;
             for item in &file.items {
                 if let crate::grammar::Item::Fn(f) = item {
                      let m = self.ctx.mangle_fn_name(&f.name.to_string());
                      if m.to_string() == canonical_name || f.name.to_string() == name {
                          return Some(ResolutionTarget {
                              template: f.clone(),
                              base_name: if f.attributes.iter().any(|a| a.name == "no_mangle") { f.name.to_string() } else { m.to_string() }, // Use the canonical mangled name
                              kind: TargetKind::Local,
                              self_ty: None,
                              imports: self.ctx.imports().clone(),
                          });
                      }
                 }
                 // Handle Externs - always use C symbol name (never mangle)
                 if let crate::grammar::Item::ExternFn(f) = item {
                     let m = f.name.to_string();
                     if m == canonical_name || f.name.to_string() == name {
                         let wrapper = SaltFn {
                             attributes: f.attributes.clone(),
                             is_pub: f.is_pub,
                             name: syn::Ident::new(&f.name.to_string(), proc_macro2::Span::call_site()),
                             generics: None,
                             args: f.args.clone(),
                             ret_type: f.ret_type.clone(),
                             body: crate::grammar::SaltBlock { stmts: vec![] },
                             requires: vec![],
                             ensures: vec![],
                         };
                         return Some(ResolutionTarget {
                               template: wrapper,
                               base_name: m,
                               kind: TargetKind::Global,
                               self_ty: None,
                               imports: vec![],
                         });
                     }
                 }
             }
        }

        // 2. Check Generic Impls (Global) using Canonical Name
        if let Some((f, imports)) = self.ctx.generic_impls().get(&canonical_name) {
             return Some(ResolutionTarget {
                 template: f.clone(),
                 base_name: canonical_name.clone(),
                 kind: TargetKind::Global,
                 self_ty: None,
                 imports: imports.clone(),
             });
        }
        
        // 2.5 [SOVEREIGN FIX] Hierarchical Scope Resolution
        // Check Registry for module-level functions in current package.
        // This enables EMPTY(), DELETED() etc. to be visible from impl blocks.

        if let Some(registry) = self.ctx.config.registry {
            let current_pkg = &*self.ctx.current_package;
            if let Some(pkg) = current_pkg.as_ref() {
                let pkg_path = pkg.name.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(".");
                if let Some(mod_info) = registry.modules.get(&pkg_path) {
                    // Try to find the raw function name in this module's function_templates
                    // First try the raw name parameter
                    if let Some(func) = mod_info.function_templates.get(name) {
                        let pkg_mangled = pkg_path.replace(".", "__");

                        return Some(ResolutionTarget {
                            template: func.clone(),
                            base_name: format!("{}__{}", pkg_mangled, name),
                            kind: TargetKind::Global,
                            self_ty: None,
                            imports: mod_info.imports.clone(),
                        });
                    }
                    
                    // [SOVEREIGN V2] Also try the simple name from canonical_name's last segment
                    // This handles cases where name was already mangled (e.g., HashMap__DELETED -> DELETED)
                    let simple_name = canonical_name.rsplit("__").next().unwrap_or(&canonical_name);
                    if simple_name != name {
                        if let Some(func) = mod_info.function_templates.get(simple_name) {
                            let pkg_mangled = pkg_path.replace(".", "__");

                            return Some(ResolutionTarget {
                                template: func.clone(),
                                base_name: format!("{}__{}", pkg_mangled, simple_name),
                                kind: TargetKind::Global,
                                self_ty: None,
                                imports: mod_info.imports.clone(),
                            });
                        }
                    }
                }
            }
        }
        
        // 2.6 [SOVEREIGN V4] Direct Registry Probing for Module-Level Functions  
        // When the canonical_name looks like HashMap__DELETED but the actual function is
        // at module level (hash_map__DELETED), we extract the simple name and search
        // ALL registry modules for it. This handles hydration context where imports are lost.
        if let Some(registry) = self.ctx.config.registry {
            // Extract the simple function name from the end of canonical_name
            // e.g., "std__collections__hash_map__HashMap__DELETED" -> "DELETED"
            let simple_name = canonical_name.rsplit("__").next().unwrap_or(&canonical_name);
            
            // Also extract what we think is the package prefix from canonical (for matching)
            // e.g., "std__collections__hash_map__HashMap__DELETED" 
            // We want to check if DELETED exists in std.collections.hash_map module
            
            // Direct Registry Probe: Check ALL modules for this simple function name
            for (pkg_path, mod_info) in &registry.modules {
                if let Some(func) = mod_info.function_templates.get(simple_name) {
                    // Found! Construct the correctly mangled name
                    let pkg_mangled = pkg_path.replace(".", "__");
                    let correct_name = format!("{}__{}", pkg_mangled, simple_name);
                    

                    return Some(ResolutionTarget {
                        template: func.clone(),
                        base_name: correct_name,
                        kind: TargetKind::Global,
                        self_ty: None,
                        imports: mod_info.imports.clone(),
                    });
                }
            }
        }
        
        // 3. Static Method Resolution
        // Uses canonical name to split
        let parts: Vec<&str> = canonical_name.split("__").collect();
        if parts.len() >= 2 {
             let method = parts.last().unwrap();
             
             // Iterative Split Attempt: Try to find where Path ends and Name begins
             // e.g. std__collections__vec__Vec -> Path=[std, collections, vec], Name=Vec
             for i in (0..parts.len()-1).rev() {
                 let name_part = parts[i];
                 let path_parts = &parts[..i];
                 
                 let possible_path: Vec<String> = path_parts.iter().map(|s| s.to_string()).collect();
                 
                 let template_key = TypeKey { 
                     path: possible_path.clone(), 
                     name: name_part.to_string(), 
                     specialization: None 
                 };
                 
                 // [V4.0 SOVEREIGN] Use TraitRegistry for method lookup
                 if let Some((func, self_ty, imports)) = self.ctx.trait_registry().get_legacy(&template_key, method) {
                     return Some(ResolutionTarget {
                         template: func.clone(),
                         base_name: canonical_name.clone(),
                         kind: TargetKind::Method,
                         self_ty: self_ty.clone(),
                         imports: imports.clone(),
                     }); 
                 }
                 
                 // Also try ignoring path entirely if Registry used flattened keys (unlikely but safe fallback)
                 // (Omitted to avoid pollution)
             }
             
             // Fallback: Try "path=[] name=Base" (Old Logic)
             let base = Mangler::mangle(&parts[..parts.len()-1]);
             let template_key = TypeKey { path: vec![], name: base.clone(), specialization: None };
             // [V4.0 SOVEREIGN] Use TraitRegistry for method lookup
             if let Some((func, self_ty, imports)) = self.ctx.trait_registry().get_legacy(&template_key, method) {
                 return Some(ResolutionTarget {
                     template: func.clone(),
                     base_name: canonical_name.clone(),
                     kind: TargetKind::Method,
                     self_ty: self_ty.clone(),
                     imports: imports.clone(),
                 }); 
             }
        }
        
        None
    }

    fn resolve_canonical_name(&mut self, name: &str) -> (String, bool) {
        // RECURSION ANCHOR: Check if we're calling ourselves
        // If 'name' matches the unmangled suffix of current_fn_name, it's a recursive call
        let current_fn = self.ctx.current_fn_name();
        if !current_fn.is_empty() {
            // Extract the function's simple name from current_fn (e.g., "main__fib" -> "fib")
            let current_simple = current_fn.rsplit("__").next().unwrap_or(&current_fn);
            // Extract simple name from input (e.g., "__fib" -> "fib", "fib" -> "fib")
            let input_simple = name.trim_start_matches('_').trim_start_matches('_');
            
            if current_simple == input_simple {
                return (current_fn.clone(), true);
            }
        }

        drop(current_fn);
        
        // If it already looks fully qualified (contains __), assume it is valid
        if name.contains("__") {
            return (name.to_string(), true);
        }
        
        // Priority 1: Intrinsics (before package prefix to avoid main__popcount)
        if self.is_intrinsic(name) {
            return (name.to_string(), false);
        }
        
        // Priority 2: Extern functions (use raw C symbol name, never mangle)
        if self.ctx.external_decls().contains(name) {
            return (name.to_string(), true);
        }
        
        // Priority 3: Current Package Prefix
        let current_pkg = &*self.ctx.current_package;
        let current_pkg_prefix = if let Some(pkg) = current_pkg.as_ref() {
             Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>())
        } else {
             "".to_string()
        };
        
        let candidate = if current_pkg_prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}__{}", current_pkg_prefix, name)
        };
        
        (candidate, false)
    }

    pub fn unify_generics(&mut self, 
        target: &ResolutionTarget, 
        explicit_generics: &[Type],
        call_args: &[syn::Expr],
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
        expected_ret_ty: Option<&Type>
    ) -> Result<BTreeMap<String, Type>, String> {

        
        // Extract struct generic params from self_ty
        let struct_gen_params: Option<Vec<crate::grammar::GenericParam>> = target.self_ty.as_ref().and_then(|self_ty| {
            let struct_name = match self_ty {
                Type::Struct(name) | Type::Concrete(name, _) => Some(name.clone()),
                _ => None,
            }?;
            
            self.ctx.struct_templates().get(&struct_name)
                .and_then(|s| s.generics.as_ref().map(|g| g.params.iter().cloned().collect()))
                .or_else(|| self.ctx.enum_templates().get(&struct_name)
                    .and_then(|e| e.generics.as_ref().map(|g| g.params.iter().cloned().collect())))
                .or_else(|| {
                    self.ctx.find_struct_template_by_name(&struct_name).and_then(|tn| {
                        self.ctx.struct_templates().get(&tn)
                            .and_then(|t| t.generics.as_ref().map(|g| g.params.iter().cloned().collect()))
                    })
                })
        });
        
        // Extract concrete args from self_ty
        let mut struct_concrete_args = Vec::new();
        if let Some(self_ty) = &target.self_ty {
            match self_ty {
                Type::Concrete(_, args) => struct_concrete_args.extend(args.iter().cloned()),
                _ => {}
            }
        }
        
        let mut resolver = crate::codegen::generic_resolver::GenericResolver::new(self.ctx);
        resolver.resolve_generics(
            &target.template,
            explicit_generics,
            call_args,
            local_vars,
            expected_ret_ty,
            target.self_ty.as_ref(),
            struct_gen_params.as_deref(),
            &struct_concrete_args,
        )
    }

    fn verify_completeness(&mut self, 
        template: &SaltFn, 
        map: &mut BTreeMap<String, Type>,
        expected_ret_ty: Option<&Type>
    ) -> Result<(), String> {
        self.verify_completeness_with_struct_generics(template, map, expected_ret_ty, None)
    }

    /// Extended completeness check that also handles struct-level generics.
    /// When `self_ty` is provided (e.g., `Ptr<T>` for static methods), any
    /// unbound struct-level generics (like T) are inferred from `expected_ret_ty`.
    pub fn verify_completeness_with_struct_generics(&mut self, 
        template: &SaltFn, 
        map: &mut BTreeMap<String, Type>,
        expected_ret_ty: Option<&Type>,
        self_ty: Option<&Type>
    ) -> Result<(), String> {
        // 1. Collect all required generics from the function-level template definition
        let required_generics = template.generics.as_ref()
            .map(|g| g.params.iter().map(|p| match p {
                 crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                 crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
            }).collect::<Vec<_>>())
            .unwrap_or_default();

        for req in &required_generics {
            if !map.contains_key(req) {
                // 2. CONTEXTUAL INFERENCE: Can we solve for 'req' using the return type?
                // Example: let x: u32 = zeroed(); -> Resolve T to u32
                if let Some(inferred_ty) = self.infer_from_return_context(req, template, expected_ret_ty) {
                    map.insert(req.clone(), inferred_ty);
                } else {
                    // 3. FATAL: Generic is completely unconstrained
                    return Err(format!("Unresolved Generic '{}' in function '{}'", req, template.name));
                }
            }
        }

        // 4. STRUCT-LEVEL GENERIC INFERENCE
        // For static methods on generic structs (e.g., Ptr::empty(), Ptr::from_addr()),
        // T is a struct-level generic, NOT a function-level generic.
        // We infer it by unifying the template return type against expected_ret_ty.
        if let Some(sty) = self_ty {
            // Extract unbound struct-level generic names from self_ty
            // e.g., Concrete("Ptr", [Generic("T")]) -> ["T"]
            let struct_generics = match sty {
                Type::Concrete(_, args) => {
                    args.iter().filter_map(|a| {
                        match a {
                            Type::Generic(name) => Some(name.clone()),
                            Type::Struct(name) if name.len() == 1 && name.chars().all(|c| c.is_uppercase()) => Some(name.clone()),
                            _ => None,
                        }
                    }).collect::<Vec<_>>()
                },
                _ => vec![],
            };

            for sg in &struct_generics {
                if !map.contains_key(sg) {
                    // Try to infer from return type context
                    if let Some(inferred_ty) = self.infer_from_return_context(sg, template, expected_ret_ty) {

                        map.insert(sg.clone(), inferred_ty);
                    } else {
                        return Err(format!("Unresolved struct generic '{}' in method '{}'. Consider using turbofish syntax.", sg, template.name));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn infer_from_return_context(&mut self,
        generic_name: &str,
        template: &SaltFn,
        expected_ret_ty: Option<&Type>
    ) -> Option<Type> {
        let expected = expected_ret_ty?;
        // Need to convert template.ret_type (AST) to Type first
        // CRITICAL: We must resolve this WITHOUT the current specialization context
        // to preserve 'T' as a generic parameter rather than substituting it with 'u8'.
        let template_ret_ty = {
            self.ctx.with_generic_context(
                BTreeMap::new(), 
                Type::Unit, 
                Vec::new(),
                |ctx| {
                    if let Some(rt) = &template.ret_type {
                        crate::codegen::type_bridge::resolve_type(ctx, rt)
                    } else {
                        Type::Unit
                    }
                }
            )
        };

        // Handle nested returns (e.g., -> Ptr<T> or -> Result<T, E>)
        // We perform a "structural match" to find the generic usage position
        // This is effectively `unify_types` but extracting the other side.
        


        let mut temp_map: BTreeMap<String, Type> = BTreeMap::new();
        // Since we want to find T, we treat 'template_ret_ty' as pattern and 'expected' as concrete.
        if self.unify_types(&template_ret_ty, expected, &mut temp_map).is_ok() {
             if let Some(res) = temp_map.get(generic_name) {
                 return Some(res.clone());
             }
        }
        
        None
    }

    pub fn unify_types(&mut self, pattern: &Type, concrete: &Type, map: &mut BTreeMap<String, Type>) -> Result<(), String> {
        match (pattern, concrete) {
            (Type::Generic(name), _) => {
                 if let Some(existing) = map.get(name) {
                     if existing != concrete {
                         // Check for integer coercion: if existing is an explicit integer type (from turbofish)
                         // and concrete is also an integer (from literal inference), accept the explicit type.
                         // This allows identity::<i32>(42) to work even when 42 traces as i64.
                         if existing.is_integer() && concrete.is_integer() {
                             // Accept - explicit turbofish type takes precedence over inferred literal type
                             return Ok(());
                         }
                         return Err(format!("Generic {} mismatch: {:?} vs {:?}", name, existing, concrete));
                     }
                 } else {
                     map.insert(name.clone(), concrete.clone());
                 }
                 Ok(())
            },
            (p, c) if p == c => Ok(()),
            (Type::Reference(p_inner, _), Type::Reference(c_inner, _)) |
            (Type::Owned(p_inner), Type::Owned(c_inner)) |
            (Type::Atomic(p_inner), Type::Atomic(c_inner)) => self.unify_types(p_inner, c_inner, map),

            // Pointer ↔ Pointer: unify inner element types
            (Type::Pointer { element: p_elem, .. }, Type::Pointer { element: c_elem, .. }) => {
                self.unify_types(p_elem, c_elem, map)
            },

            // Pointer ↔ Concrete(Ptr): structural equivalence bridge
            // Template return type resolves as Type::Pointer { element: T }
            // but expected type from context is Type::Concrete("Ptr", [I32])
            (Type::Pointer { element: p_elem, .. }, Type::Concrete(c_name, c_args))
                if c_name.contains("Ptr") && c_args.len() == 1 =>
            {
                self.unify_types(p_elem, &c_args[0], map)
            },
            (Type::Concrete(p_name, p_args), Type::Pointer { element: c_elem, .. })
                if p_name.contains("Ptr") && p_args.len() == 1 =>
            {
                self.unify_types(&p_args[0], c_elem, map)
            },
            
            (Type::Array(p_inner, pl, _), Type::Array(c_inner, cl, _)) => {
                 if pl != cl { return Err("Array length mismatch in inference".to_string()); }
                 self.unify_types(p_inner, c_inner, map)
            },
            
            (Type::Fn(p_args, p_ret), Type::Fn(c_args, c_ret)) => {
                 self.unify_types(p_ret, c_ret, map)?;
                 for (pa, ca) in p_args.iter().zip(c_args) {
                     self.unify_types(pa, ca, map)?;
                 }
                 Ok(())
            },
            
            (Type::Concrete(p_name, p_args), Type::Concrete(c_name, c_args)) if p_name == c_name => {
                 for (p_arg, c_arg) in p_args.iter().zip(c_args.iter()) {
                     self.unify_types(p_arg, c_arg, map)?;
                 }
                 Ok(())
            },
            
            // Legacy Struct("T") fallback
            (Type::Struct(name), _) if name.len() == 1 && name.chars().all(|c| c.is_uppercase()) => {
                 if let Some(existing) = map.get(name) {
                     if existing != concrete {
                         // Check for integer coercion compatibility
                         if existing.is_integer() && concrete.is_integer() {
                             return Ok(());
                         }
                         // Check for auto-deref: T bound to &X should unify with X
                         if let Type::Reference(inner, _) = existing {
                             if inner.as_ref() == concrete {
                                 return Ok(());
                             }
                         }
                         if let Type::Reference(inner, _) = concrete {
                             if inner.as_ref() == existing {
                                 return Ok(());
                             }
                         }
                         return Err(format!("Generic {} mismatch: {:?} vs {:?}", name, existing, concrete));
                     }
                     Ok(())
                 } else {
                     map.insert(name.clone(), concrete.clone());
                     Ok(())
                 }
            },

            // Handle Concrete types with unresolved generic placeholders
            // e.g., Concrete("RawVec", [Struct("T")]) matching Concrete("RawVec", [I64])
            (Type::Concrete(p_name, p_args), Type::Concrete(c_name, c_args)) => {
                // Structural check: must be same base container
                if p_name != c_name {
                    return Err(format!("Container mismatch: {} vs {}", p_name, c_name));
                }
                if p_args.len() != c_args.len() {
                    return Err(format!("Generic arity mismatch in {}: {} vs {}", p_name, p_args.len(), c_args.len()));
                }
                // Recursive unification of type arguments
                for (p_arg, c_arg) in p_args.iter().zip(c_args.iter()) {
                    self.unify_types(p_arg, c_arg, map)?;
                }
                Ok(())
            },

            // [CANONICAL RESOLUTION] Concrete vs Struct: strict equality after canonicalization.
            // With proper FQN resolution in the tracer, these types should not need fuzzy matching.
            (Type::Concrete(p_name, _p_args), Type::Struct(s_name)) => {
                Err(format!("Cannot unify Concrete({}) with Struct({}). Types must match exactly after canonicalization.", p_name, s_name))
            },

            // SOUNDNESS: Container pattern cannot unify with integer scalar value
            (Type::Concrete(p_name, _), scalar) if scalar.is_integer() => {
                Err(format!("Cannot unify container {} with integer {:?}", p_name, scalar))
            },
            
            // Allow other Concrete vs non-primitive cases during generic resolution
            (Type::Concrete(_p_name, _), _other) => {
                // During monomorphization, we may see partially resolved types
                // Let specialization complete the binding
                Ok(())
            },

            // Explicit Integer Coercion: Allow turbofish types to coerce inferred literals
            // e.g., identity::<i32>(42) works when 42 is inferred as i64
            (p, c) if p.is_integer() && c.is_integer() => Ok(()),

            // Auto-deref coercion: &T can unify with T in some contexts
            (Type::Reference(p_inner, _), c) => self.unify_types(p_inner, c, map),
            (p, Type::Reference(c_inner, _)) => self.unify_types(p, c_inner, map),

            // STRICT TYPE ENFORCEMENT: Reject all other structural mismatches
            (p, c) => {
                Err(format!("STRICT TYPE MISMATCH: Expected {:?}, got {:?}", p, c))
            }
        }
    }

    fn mangle_specialization(&mut self, base_name: &str, map: &BTreeMap<String, Type>, template: &SaltFn) -> String {
        // If no generics, identity
        if map.is_empty() { return base_name.to_string(); }
        
        let mut suffix_parts = Vec::new();
        
        // [DETERMINISTIC ORDERING FIX]
        // Priority 1: Use the STRUCT TEMPLATE's declared parameter order when this is a method.
        // The function template's generics may be in non-deterministic order (from HashMap
        // iteration during impl block registration), e.g. [A, T] instead of [T, A].
        // The struct template always preserves the declaration order from source code.
        let mut used_struct_order = false;
        if let Some((struct_prefix, _method)) = base_name.rsplit_once("__") {
            let gen_params = if let Some(s) = self.ctx.struct_templates().get(struct_prefix) {
                s.generics.as_ref().map(|g| g.params.clone())
            } else if let Some(e) = self.ctx.enum_templates().get(struct_prefix) {
                e.generics.as_ref().map(|g| g.params.clone())
            } else { None };
            
            if let Some(params) = gen_params {
                // Use struct template's parameter order
                for param in &params {
                    let name = match param {
                        crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                        crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                    };
                    if let Some(ty) = map.get(&name) {
                        suffix_parts.push(ty.mangle_suffix());
                    }
                }
                // Also add any remaining map entries not in struct params (method-level generics)
                if let Some(g) = &template.generics {
                    for param in &g.params {
                        let name = match param {
                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                        };
                        let already_added = params.iter().any(|p: &crate::grammar::GenericParam| {
                            let n = match p {
                                crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                            };
                            n == name
                        });
                        if !already_added {
                            if let Some(ty) = map.get(&name) {
                                suffix_parts.push(ty.mangle_suffix());
                            }
                        }
                    }
                }
                used_struct_order = true;
            }
        }
        
        // Priority 2: Fall back to function template's own generics (for free functions)
        if !used_struct_order {
            if let Some(g) = &template.generics {
                for param in &g.params {
                    let name = match param {
                        crate::grammar::GenericParam::Type { name, .. } => name,
                        crate::grammar::GenericParam::Const { name, .. } => name,
                    };
                    if let Some(ty) = map.get(&name.to_string()) {
                        suffix_parts.push(ty.mangle_suffix());
                    } else {
                        suffix_parts.push("Unit".to_string());
                    }
                }
            }
        }
        
        // Priority 3: If still empty but map has entries, use sorted keys as final fallback
        if suffix_parts.is_empty() && !map.is_empty() {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(ty) = map.get(key) {
                    suffix_parts.push(ty.mangle_suffix());
                }
            }
        }
        
        if suffix_parts.is_empty() {
             base_name.to_string()
        } else {
             format!("{}_{}", base_name, suffix_parts.join("_"))
        }
    }
    
    fn resolve_signature(&mut self, template: &SaltFn, map: &BTreeMap<String, Type>) -> Result<(Type, Vec<Type>), String> {
         let ret = if let Some(rt) = &template.ret_type {
             let resolved = crate::codegen::type_bridge::resolve_type(self.ctx, rt);
             let substituted = resolved.substitute(map);
             let substituted = resolved.substitute(map);
             substituted
         } else { Type::Unit };
         
         let args = template.args.iter().map(|a| {
             crate::codegen::type_bridge::resolve_type(self.ctx, a.ty.as_ref().unwrap()).substitute(map)
         }).collect();
         
         Ok((ret, args))
    }
}

