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

pub(crate) struct ResolutionTarget {
    template: SaltFn,
    base_name: String,
    self_ty: Option<Type>, // Only for methods
    imports: Vec<crate::grammar::ImportDecl>,
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
        if let syn::Expr::Field(field_expr) = &*call.func {
            if let Some(res) = self.resolve_field_method_call(field_expr, call, local_vars, expected_ty)? {
                return Ok(res);
            }
        }
        
        if let syn::Expr::Path(path_expr) = &*call.func {
            if let Some(res) = self.resolve_early_intercepts(path_expr)? {
                return Ok(res);
            }
        }
        
        self.resolve_standard_call(call, local_vars, expected_ty)
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
                             let syn_ty = crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?;
                             generics.push(crate::codegen::type_bridge::resolve_type(self.ctx, &syn_ty));
                         }
                     }
                 }
             }

             // Special Case: intrin module
             if segments.first().map(|s| s == "intrin").unwrap_or(false)
                 && segments.len() == 2 {
                     return Ok((segments[1].clone(), generics));
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
                if imp.alias.as_ref().is_some_and(|a| a == &mangled) {
                     Some(Mangler::mangle(&imp.name.iter().map(|i| i.to_string()).collect::<Vec<_>>()))
                } else if let Some(group) = &imp.group {
                     if group.iter().any(|id| *id == mangled) {
                         let pkg_mangled = Mangler::mangle(&imp.name.iter().map(|i| i.to_string()).collect::<Vec<_>>());
                         if pkg_mangled.is_empty() {
                             Some(mangled.clone())
                         } else {
                             Some(format!("{}__{}",pkg_mangled, mangled))
                         }
                     } else { None }
                } else { None }
             });
             
             // Extern fn declarations take priority over wildcard imports.
             // If the symbol is declared as `extern fn` in this file, don't expand it.
             if segments.len() == 1 && self.ctx.external_decls().contains(&mangled) {
                 return Ok((mangled, generics));
             }

             // Wildcard Import Resolution: Check `use X::*` imports via Registry
             let resolved_name = resolved_name.or_else(|| self.resolve_wildcard_import(&segments, &mangled))
                 .unwrap_or(mangled);


             Ok((resolved_name, generics))

        } else {
             Err("Call target must be a path".to_string())
        }
    }
    
    fn is_intrinsic(&mut self, name: &str) -> bool {
        name == "size_of" || name == "align_of" || name == "zeroed" || name == "unreachable" ||
        name == "popcount" || name == "ctpop" || name == "println" || name == "print" ||
        name == "trailing_zeros" || name == "cttz" || name == "leading_zeros" || name == "ctlz" ||
        // Bit manipulation intrinsics used by kernel scheduler (std.math.* aliases)
        name == "ctz_u64" || name == "clz_u64" || name == "popcount_u64" ||
        name == "reinterpret_cast" || name == "ref_to_addr" || name == "is_null" ||
        // Bulk memory intrinsics
        name == "memset" || name == "memcpy" ||
        // Refined Intrinsics (Phase 4A/4B)
        name == "fused_cross_entropy" || name == "ml__fused_cross_entropy" ||
        name == "read_vector" ||
        // Shadow Reduction: Register-resident tensor updates
        name == "update_tensor" || name == "fma_update" ||
        // ML Intrinsics
        name == "matmul" || name.starts_with("matmul_into") || name == "update_weights" || name == "v_fma" || name == "v_add" || name == "v_mul" || name == "v_max" || name == "v_sum" || name == "v_hsum" || name == "v_relu" || name == "v_broadcast" || name == "v_load" || name == "v_store" ||
        name == "__internal_dispatch_matmul" || name == "__internal_fma_update" ||
        name == "mmap_view" || name == "cast_view" ||
        name.contains("macos_syscall") ||
        name.starts_with("intrin_") || name.starts_with("tensor_alloc") || name.contains("ptr_offset") || name.contains("ptr_read") || name.contains("ptr_write") ||
        // Shaped tensor allocation
        name == "alloc_tensor" ||
        // Vector Intrinsics
        name == "vector_load" || name == "vector_store" || name == "vector_fma" || name == "vector_reduce_add" || name == "vector_splat" ||
        // Target Feature Detection
        name.starts_with("target__") ||
        // Neural network building blocks
        name == "add_bias" ||
        // std.math → LLVM intrinsics
        name.starts_with("std__math__") ||
        name == "expf" || name == "logf" || name == "sqrtf" || name == "powf" ||
        name == "sinf" || name == "cosf" || name == "fabsf" || name == "floorf" || name == "ceilf" ||
        // Atomic intrinsics for kernel lock-free data structures
        name == "cmpxchg" || name.contains("atomic_cas") || name.contains("ptr_is_null") ||
        // Concurrency primitives — must bypass package mangling
        // so they route to the intrinsic handler in intrinsics.rs
        name == "spin_loop_hint" || name == "cycle_counter" || name == "read_tls_deadline" ||
        name == "atomic_add_i64" || name == "atomic_load_i64" || name == "atomic_store_i64" ||
        name == "atomic_load_ptr" || name == "atomic_swap_ptr" ||
        name == "m4_wfe" || name == "m4_dmb_ish" || name == "m4_sev" || name == "trap" ||
        // Function pointer address extraction
        name == "fn_addr"
    }
    
    /// Extract generic type arguments from a path segment (e.g., println::<T> -> [T])
    fn extract_generics_from_segment(&mut self, segment: &syn::PathSegment) -> Result<Vec<Type>, String> {
        let mut generics = Vec::new();
        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
            for arg in &args.args {
                if let syn::GenericArgument::Type(ty) = arg {
                    let syn_ty = crate::grammar::SynType::from_std(ty.clone()).map_err(|e| e.to_string())?;
                    generics.push(crate::codegen::type_bridge::resolve_type(self.ctx, &syn_ty));
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

        if let Some(target) = self.try_identify_local(name, &canonical_name) {
            return Some(target);
        }
        if let Some(target) = self.try_identify_generic_impl(&canonical_name) {
            return Some(target);
        }
        if let Some(target) = self.try_identify_hierarchical(name, &canonical_name) {
            return Some(target);
        }
        if let Some(target) = self.try_identify_registry_probe(&canonical_name) {
            return Some(target);
        }
        if let Some(target) = self.try_identify_static_method(&canonical_name) {
            return Some(target);
        }

        None
    }

    fn try_identify_local(&mut self, name: &str, canonical_name: &str) -> Option<ResolutionTarget> {
        for item in &self.ctx.config.file.items {
            if let crate::grammar::Item::Fn(f) = item {
                let m = self.ctx.mangle_fn_name(&f.name.to_string());
                if m == canonical_name || f.name == name {
                    let base_name = if f.attributes.iter().any(|a| a.name == "no_mangle" || a.name == "export") { f.name.to_string() } else { m.to_string() };
                    return Some(ResolutionTarget { template: f.clone(), base_name, self_ty: None, imports: self.ctx.imports().clone() });
                }
            } else if let crate::grammar::Item::ExternFn(f) = item {
                let m = f.name.to_string();
                if m == canonical_name || f.name == name {
                    let wrapper = SaltFn {
                        attributes: f.attributes.clone(), is_pub: f.is_pub,
                        name: syn::Ident::new(&f.name.to_string(), proc_macro2::Span::call_site()),
                        generics: None, args: f.args.clone(), ret_type: f.ret_type.clone(),
                        body: crate::grammar::SaltBlock { stmts: vec![] },
                        requires: f.requires.clone(), ensures: f.ensures.clone(),
                    };
                    return Some(ResolutionTarget { template: wrapper, base_name: m, self_ty: None, imports: vec![] });
                }
            }
        }
        None
    }

    fn try_identify_generic_impl(&mut self, canonical_name: &str) -> Option<ResolutionTarget> {
        if let Some((f, imports)) = self.ctx.generic_impls().get(canonical_name) {
            return Some(ResolutionTarget { template: f.clone(), base_name: canonical_name.to_string(), self_ty: None, imports: imports.clone() });
        }
        None
    }

    fn try_identify_hierarchical(&mut self, name: &str, canonical_name: &str) -> Option<ResolutionTarget> {
        let registry = self.ctx.config.registry.as_ref()?;
        let current_pkg = self.ctx.current_package.as_ref()?;
        let pkg_path = current_pkg.name.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(".");
        let mod_info = registry.modules.get(&pkg_path)?;
        if let Some(func) = mod_info.function_templates.get(name) {
            let pkg_mangled = pkg_path.replace(".", "__");
            return Some(ResolutionTarget { template: func.clone(), base_name: format!("{}__{}", pkg_mangled, name), self_ty: None, imports: mod_info.imports.clone() });
        }
        let simple_name = canonical_name.rsplit("__").next().unwrap_or(canonical_name);
        if simple_name != name {
            if let Some(func) = mod_info.function_templates.get(simple_name) {
                let pkg_mangled = pkg_path.replace(".", "__");
                return Some(ResolutionTarget { template: func.clone(), base_name: format!("{}__{}", pkg_mangled, simple_name), self_ty: None, imports: mod_info.imports.clone() });
            }
        }
        None
    }

    fn try_identify_registry_probe(&mut self, canonical_name: &str) -> Option<ResolutionTarget> {
        let registry = self.ctx.config.registry.as_ref()?;
        let simple_name = canonical_name.rsplit("__").next().unwrap_or(canonical_name);
        for (pkg_path, mod_info) in &registry.modules {
            if let Some(func) = mod_info.function_templates.get(simple_name) {
                let pkg_mangled = pkg_path.replace(".", "__");
                return Some(ResolutionTarget { template: func.clone(), base_name: format!("{}__{}", pkg_mangled, simple_name), self_ty: None, imports: mod_info.imports.clone() });
            }
        }
        None
    }

    fn try_identify_static_method(&mut self, canonical_name: &str) -> Option<ResolutionTarget> {
        let parts: Vec<&str> = canonical_name.split("__").collect();
        if parts.len() < 2 { return None; }
        let method = parts.last()?;
        for i in (0..parts.len()-1).rev() {
            let possible_path: Vec<String> = parts[..i].iter().map(|s| s.to_string()).collect();
            let key = TypeKey { path: possible_path, name: parts[i].to_string(), specialization: None };
            if let Some((func, self_ty, imports)) = self.ctx.trait_registry().get_legacy(&key, method) {
                return Some(ResolutionTarget { template: func.clone(), base_name: canonical_name.to_string(), self_ty: self_ty.clone(), imports: imports.clone() });
            }
        }
        let base = Mangler::mangle(&parts[..parts.len()-1]);
        if let Some((func, self_ty, imports)) = self.ctx.trait_registry().get_legacy(&TypeKey { path: vec![], name: base.clone(), specialization: None }, method) {
            return Some(ResolutionTarget { template: func.clone(), base_name: canonical_name.to_string(), self_ty: self_ty.clone(), imports: imports.clone() });
        }
        if let Ok((func, self_ty, imports)) = self.ctx.resolve_method(&Type::Struct(base), method) {
            return Some(ResolutionTarget { template: func, base_name: canonical_name.to_string(), self_ty, imports });
        }
        None
    }

    fn resolve_wildcard_import(&mut self, segments: &[String], mangled: &str) -> Option<String> {
        let reg = self.ctx.config.registry.as_ref()?;
        for imp in self.ctx.imports().iter() {
            let is_wildcard = imp.alias.is_none() && imp.group.is_none() && !imp.name.is_empty();
            if !is_wildcard { continue; }
            let import_path = imp.name.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(".");
            let mod_info = match reg.modules.get(&import_path) {
                Some(m) => m,
                None => continue,
            };
            let pkg_prefix = mod_info.package.replace(".", "__");
            if !segments.is_empty() {
                let type_name = &segments[0];
                if mod_info.struct_templates.contains_key(type_name) {
                    return Some(format!("{}__{}", pkg_prefix, mangled));
                }
                if mod_info.structs.contains_key(type_name) {
                    return Some(format!("{}__{}", pkg_prefix, mangled));
                }
                if segments.len() == 1 && mod_info.functions.contains_key(type_name) {
                    return Some(format!("{}__{}", pkg_prefix, type_name));
                }
                if mod_info.enum_templates.contains_key(type_name) {
                    return Some(format!("{}__{}", pkg_prefix, mangled));
                }
            }
        }
        None
    }

    /// Check if `name` is a recursive call to the current function.
    fn detect_recursion(&self, name: &str) -> Option<String> {
        let current_fn = self.ctx.current_fn_name();
        if current_fn.is_empty() { return None; }
        let current_simple = current_fn.rsplit("__").next().unwrap_or(current_fn);
        let input_simple = name.trim_start_matches('_').trim_start_matches('_');
        if current_simple == input_simple { Some(current_fn.clone()) } else { None }
    }

    /// Mangle `name` with the current package prefix if one exists.
    fn apply_package_prefix(&self, name: &str) -> String {
        let current_pkg = &*self.ctx.current_package;
        let prefix = if let Some(pkg) = current_pkg.as_ref() {
            Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>())
        } else { "".to_string() };
        if prefix.is_empty() { name.to_string() } else { format!("{}__{}", prefix, name) }
    }

    fn resolve_canonical_name(&mut self, name: &str) -> (String, bool) {
        if let Some(canonical) = self.detect_recursion(name) { return (canonical, true); }
        if name.contains("__") { return (name.to_string(), true); }
        if self.is_intrinsic(name) { return (name.to_string(), false); }
        if self.ctx.external_decls().contains(name) { return (name.to_string(), true); }
        (self.apply_package_prefix(name), false)
    }

    pub(crate) fn unify_generics(&mut self, 
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
        if let Some(Type::Concrete(_, args)) = &target.self_ty { struct_concrete_args.extend(args.iter().cloned()) }
        
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

    fn _verify_completeness(&mut self, 
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
        // It is inferred by unifying the template return type against expected_ret_ty.
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
        // A "structural match" is performed to find the generic usage position.
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

            // Concrete vs Struct: strict equality after canonicalization.
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
             let _substituted = resolved.substitute(map);
             
             resolved.substitute(map)
         } else { Type::Unit };
         
         let args = template.args.iter().map(|a| {
             let ty = a.ty.as_ref().ok_or_else(|| format!("Missing type for argument {}", a.name))?;
             Ok(crate::codegen::type_bridge::resolve_type(self.ctx, ty).substitute(map))
         }).collect::<Result<Vec<_>, String>>()?;
         
         Ok((ret, args))
    }

    fn resolve_field_method_call(
        &mut self,
        field_expr: &syn::ExprField,
        call: &syn::ExprCall,
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
        expected_ty: Option<&Type>,
    ) -> Result<Option<CallKind>, String> {
        let method_name = match &field_expr.member {
            syn::Member::Named(ident) => ident.to_string(),
            syn::Member::Unnamed(idx) => format!("{}", idx.index),
        };

        let receiver_ty = crate::codegen::type_bridge::infer_expr_type(self.ctx, &field_expr.base, local_vars)?;
        
        if method_name == "get_unchecked" || method_name == "set_unchecked" {
            let inner_ty = match &receiver_ty {
                Type::Reference(inner, _) => inner.as_ref().clone(),
                Type::Concrete(_, _) => receiver_ty.clone(),
                Type::Struct(name) if name.contains("Vec") => receiver_ty.clone(),
                _ => receiver_ty.clone(),
            };
            
            let element_ty = match &inner_ty {
                Type::Concrete(name, args) if name.contains("Vec") && !args.is_empty() => args[0].clone(),
                Type::Struct(name) if name.contains("Vec_") => {
                    let suffix = name.rsplit('_').next().unwrap_or("i64");
                    match suffix {
                        "i32" => Type::I32,
                        "i64" => Type::I64,
                        "u8" => Type::U8,
                        "f32" => Type::F32,
                        "f64" => Type::F64,
                        _ => Type::I64,
                    }
                },
                _ => Type::I64,
            };
            
            return Ok(Some(CallKind::TransparentVecAccess {
                method: method_name,
                element_ty,
                receiver: Box::new((*field_expr.base).clone()),
                args: call.args.iter().cloned().collect(),
            }));
        }
        
        let type_key = crate::codegen::type_bridge::type_to_type_key(&receiver_ty);
        
        let args_vec: Vec<syn::Expr> = call.args.iter().cloned().collect();
        let arg_types: Vec<Type> = args_vec.iter()
            .filter_map(|expr| crate::codegen::type_bridge::infer_expr_type(self.ctx, expr, local_vars).ok())
            .collect();
        
        let trait_result = {
            let registry = self.ctx.trait_registry();
            registry.resolve_overload(&type_key, &method_name, &arg_types)
                .map(|resolved| (resolved.func.clone(), resolved.self_ty.clone(), resolved.imports.clone()))
        };
        
        let method_info = trait_result.or_else(|| {
            self.ctx.trait_registry().get_legacy(&type_key, &method_name)
        }).or_else(|| {
            self.ctx.resolve_method(&receiver_ty, &method_name).ok()
        });

        if let Some((func, self_ty, imports)) = method_info {
            let target = ResolutionTarget {
                template: func.clone(),
                base_name: format!("{}__{}", crate::common::mangling::Mangler::mangle_type_key(&type_key), method_name),
                self_ty: self_ty.clone(),
                imports,
            };
            
            let receiver_generics: Vec<Type> = match &self_ty {
                Some(Type::Concrete(_, args)) => args.clone(),
                _ => vec![],
            };

            let spec_map = self.unify_generics(&target, &receiver_generics, &args_vec, local_vars, expected_ty)?;
            let mangled_name = self.mangle_specialization(&target.base_name, &spec_map, &target.template);
            let (ret_ty, arg_tys) = self.resolve_signature(&target.template, &spec_map)?;
            
            let concrete_tys: Vec<Type> = {
                let mut tys = Vec::new();
                let mut used_struct_order = false;
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

            Ok(Some(CallKind::Function(mangled_name, ret_ty, arg_tys, Some(lazy_task))))
        } else {
            Ok(None)
        }
    }

    fn resolve_early_intercepts(
        &mut self,
        path_expr: &syn::ExprPath,
    ) -> Result<Option<CallKind>, String> {
        if path_expr.path.segments.len() == 1 {
            let raw_name = path_expr.path.segments[0].ident.to_string();
            
            if self.is_intrinsic(&raw_name) {
                let explicit_generics = self.extract_generics_from_segment(&path_expr.path.segments[0])?;
                return Ok(Some(CallKind::Intrinsic(raw_name, explicit_generics)));
            }
            
            let mangled_name = self.ctx.mangle_fn_name(&raw_name);
            
            let _ = self.ctx.ensure_struct_exists(&mangled_name, &[]);
            let _ = self.ctx.ensure_struct_exists(&raw_name, &[]);
            
            {
                let struct_reg = self.ctx.struct_registry();
                if let Some(info) = struct_reg.values().find(|i| i.name == mangled_name) {
                    let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                        .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                        .collect::<Vec<_>>();
                    fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                    let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                    return Ok(Some(CallKind::StructLiteral(mangled_name.to_string(), fields)));
                }
                if let Some(info) = struct_reg.values().filter(|i| i.name == raw_name).min_by(|a, b| a.name.cmp(&b.name)) {
                    let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                        .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                        .collect::<Vec<_>>();
                    fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                    let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                    return Ok(Some(CallKind::StructLiteral(raw_name, fields)));
                }
                if let Some(info) = self.ctx.find_struct_by_name(&raw_name) {
                    let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                        .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                        .collect::<Vec<_>>();
                    fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                    let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                    return Ok(Some(CallKind::StructLiteral(info.name.clone(), fields)));
                }
            }
            
            {
                let (has_exact, suffix_match) = {
                    let templates = self.ctx.struct_templates();
                    let exact = templates.contains_key(&mangled_name.to_string());
                    let suffix_m = self.ctx.find_struct_template_by_name(&raw_name);
                    (exact, suffix_m)
                };
                
                if has_exact {
                    let _ = self.ctx.ensure_struct_exists(&mangled_name, &[]);
                    let struct_reg = self.ctx.struct_registry();
                    if let Some(info) = struct_reg.values().find(|i| i.name == mangled_name) {
                        let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                            .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                            .collect::<Vec<_>>();
                        fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                        let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                        return Ok(Some(CallKind::StructLiteral(mangled_name.to_string(), fields)));
                    }
                }
                
                if let Some(template_name) = suffix_match {
                    let _ = self.ctx.ensure_struct_exists(&template_name, &[]);
                    let struct_reg = self.ctx.struct_registry();
                    if let Some(info) = struct_reg.values().find(|i| i.name == template_name) {
                        let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                            .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                            .collect::<Vec<_>>();
                        fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                        let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                        return Ok(Some(CallKind::StructLiteral(info.name.clone(), fields)));
                    }
                }
            }
            
            let imports = self.ctx.imports();
            for imp in imports.iter() {
                if imp.name.len() == 1 && imp.group.is_none() {
                    let alias_matches = imp.alias.as_ref().is_some_and(|a| *a == raw_name);
                    if alias_matches {
                        let single_str = imp.name[0].to_string();
                        if single_str.contains("__") {
                            let pkg_mangled = &single_str[..single_str.len() - raw_name.len() - 2];
                            let pkg_path = pkg_mangled.replace("__", ".");
                            
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
                                        
                                        return Ok(Some(CallKind::Function(single_str, ret_ty, arg_tys, Some(lazy_task))));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn resolve_standard_call(
        &mut self,
        call: &syn::ExprCall,
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
        expected_ty: Option<&Type>,
    ) -> Result<CallKind, String> {
        let (func_name, explicit_generics) = self.resolve_path(&call.func)?;
        
        if self.is_intrinsic(&func_name) {
             return Ok(CallKind::Intrinsic(func_name, explicit_generics));
        }
        
        if let Some(res) = resolve_path_to_enum(self.ctx, &func_name, &explicit_generics, expected_ty) {
            return Ok(CallKind::EnumConstructor(res));
        }

        {
            let struct_reg = self.ctx.struct_registry();
            if let Some(info) = struct_reg.values().filter(|i| i.name == func_name).min_by(|a, b| a.name.cmp(&b.name)) {
                let mut fields_with_idx: Vec<(String, usize, Type)> = info.fields.iter()
                    .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
                    .collect::<Vec<_>>();
                fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
                let fields: Vec<(String, Type)> = fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect();
                return Ok(CallKind::StructLiteral(func_name, fields));
            }
        }

        let target = self.identify_target(&func_name, &explicit_generics, &call.args, local_vars)
            .ok_or_else(|| {
                format!("Undefined function or symbol: '{}'", func_name)
            })?;

        let args_vec: Vec<syn::Expr> = call.args.iter().cloned().collect();
        let spec_map = self.unify_generics(&target, &explicit_generics, &args_vec, local_vars, expected_ty)?;

        let mangled_name = self.mangle_specialization(&target.base_name, &spec_map, &target.template);

        let (ret_ty, arg_tys) = self.resolve_signature(&target.template, &spec_map)?;

        let concrete_tys: Vec<Type> = {
            let mut tys = Vec::new();
            let mut used_struct_order = false;
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
        
        let resolved_self = target.self_ty.as_ref().map(|st| st.substitute(&spec_map));

        let lazy_task = Box::new(crate::codegen::collector::MonomorphizationTask {
            identity: type_key,
            mangled_name: mangled_name.clone(),
            func: target.template.clone(),
            concrete_tys,
            self_ty: resolved_self,
            imports: target.imports,
            type_map: spec_map,
        });

        Ok(CallKind::Function(
            mangled_name, 
            ret_ty, 
            arg_tys, 
            Some(lazy_task)
        ))
    }
}
