/// # GenericResolver — Canonical Generic Type Resolution
///
/// This module consolidates all generic type inference into one place.
/// Previously, generic resolution was scattered across 5 independent code paths
/// in emit_method_call, unify_generics, resolve_codegen_type, and emit_struct.
///
/// ## Inference Pipeline (in order):
/// 1. **Turbofish** — explicit type arguments (`::<T>`)
/// 2. **Struct-level** — generics from receiver type (`impl<T> Vec<T>`)
/// 3. **Argument** — trace call argument types and unify with parameter patterns
/// 4. **Return-type** — unify return type template against expected type
/// 5. **Phantom** — infer unresolved generics from `Fn` return types
/// 6. **Completeness** — verify all generics are resolved
use std::collections::{BTreeMap, HashMap};
use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::grammar::{GenericParam, SaltFn};

/// Central generic resolution engine.
pub struct GenericResolver<'a, 'ctx, 'b> {
    ctx: &'b mut LoweringContext<'a, 'ctx>,
}

impl<'a, 'ctx, 'b> GenericResolver<'a, 'ctx, 'b> {
    pub fn new(ctx: &'b mut LoweringContext<'a, 'ctx>) -> Self {
        Self { ctx }
    }

    /// Build a complete generic type map for a function or method call.
    ///
    /// This is the single entry point that replaces the duplicated logic in
    /// `emit_method_call` (5614-5762) and `unify_generics` (954-1093).
    ///
    /// # Arguments
    /// * `template` — The function template being called
    /// * `turbofish_args` — Explicit type arguments from turbofish syntax
    /// * `call_arg_exprs` — The actual call argument expressions (for tracing)
    /// * `local_vars` — Current local variable scope
    /// * `expected_ret_ty` — Expected return type (from `let x: T = ...`)
    /// * `self_ty` — Receiver type for method calls (None for free functions)
    /// * `struct_generics` — Generic params from the struct template (for methods)
    /// * `struct_concrete_args` — Concrete type args already extracted from receiver
    #[allow(clippy::too_many_arguments)] // REASON: all 9 params independently meaningful; bundling would obscure intent
    pub fn resolve_generics(&mut self,
        template: &SaltFn,
        turbofish_args: &[Type],
        call_arg_exprs: &[syn::Expr],
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
        expected_ret_ty: Option<&Type>,
        self_ty: Option<&Type>,
        struct_generics: Option<&[GenericParam]>,
        struct_concrete_args: &[Type],
    ) -> Result<BTreeMap<String, Type>, String> {
        let mut map = BTreeMap::new();

        // Compute struct-level generic names for filtering downstream
        // func.generics.params = [impl_params... , method_params...] by convention
        let struct_generic_names: Vec<String> = struct_generics
            .map(|sg| sg.iter().map(generic_param_name).collect())
            .unwrap_or_default();

        // ── Phase 1: Turbofish (explicit generics) ──────────────────────
        self.apply_turbofish(template, turbofish_args, &mut map);
        
        // ── Phase 1b: Struct turbofish fallback ─────────────────────────
        // If function has no generics but turbofish was provided, they belong to the struct
        if map.is_empty() && !turbofish_args.is_empty() {
            self.apply_struct_turbofish(struct_generics, turbofish_args, &mut map);
        }

        // ── Phase 2: Struct-level generics from receiver ────────────────
        self.apply_struct_generics(struct_generics, struct_concrete_args, &mut map);

        // ── Phase 3: Argument inference ─────────────────────────────────
        self.infer_from_arguments(template, call_arg_exprs, local_vars, &mut map)?;

        // ── Phase 4: Self-type inference (methods only) ─────────────────
        if let Some(sty) = self_ty {
            self.infer_from_self_type(template, sty, call_arg_exprs, local_vars, &mut map)?;
        }

        // ── Phase 5: Return-type inference ──────────────────────────────
        self.infer_from_return_type(template, expected_ret_ty, &mut map)?;

        // ── Phase 6: Phantom generic inference ──────────────────────────
        // Use only METHOD-level declared generics to avoid struct-level Fn types 
        // polluting phantom inference (e.g., Filter<I,F>::map<F2,T> where F=Fn(Bool))
        self.infer_phantom_generics_method_only(template, &struct_generic_names, &mut map);

        // ── Phase 7: Completeness ───────────────────────────────────────
        // Only verify method-level params; struct-level are handled by concrete_tys pipeline
        self.verify_completeness_method_only(template, &struct_generic_names, &mut map, expected_ret_ty, self_ty)?;

        Ok(map)
    }

    // ════════════════════════════════════════════════════════════════════
    // Phase implementations
    // ════════════════════════════════════════════════════════════════════

    /// Phase 1: Map turbofish args to function-level generic params.
    fn apply_turbofish(&mut self,
        template: &SaltFn,
        turbofish_args: &[Type],
        map: &mut BTreeMap<String, Type>,
    ) {
        if let Some(generics) = &template.generics {
            for (i, param) in generics.params.iter().enumerate() {
                if let Some(arg) = turbofish_args.get(i) {
                    let name = generic_param_name(param);
                    map.insert(name, arg.clone());
                }
            }
        }
    }

    /// Phase 1b: When function has no generics, turbofish may target struct generics.
    fn apply_struct_turbofish(&mut self,
        struct_generics: Option<&[GenericParam]>,
        turbofish_args: &[Type],
        map: &mut BTreeMap<String, Type>,
    ) {
        if let Some(params) = struct_generics {
            for (i, param) in params.iter().enumerate() {
                if let Some(arg) = turbofish_args.get(i) {
                    let name = generic_param_name(param);
                    map.insert(name, arg.clone());
                }
            }
        }
    }

    /// Phase 2: Apply struct-level generic bindings from receiver concrete args.
    fn apply_struct_generics(&mut self,
        struct_generics: Option<&[GenericParam]>,
        concrete_args: &[Type],
        map: &mut BTreeMap<String, Type>,
    ) {
        if let Some(params) = struct_generics {
            for (i, param) in params.iter().enumerate() {
                if let Some(arg) = concrete_args.get(i) {
                    // Skip unresolved generic placeholders — these would poison the map
                    // and block Phase 3 (argument inference) from binding the real type.
                    // e.g., Box<T>::new(Simple{val:10}) has struct_concrete_args=[Struct("T")]
                    if arg.has_generics() {
                        continue;
                    }
                    let name = generic_param_name(param);
                    map.entry(name).or_insert_with(|| arg.clone());
                }
            }
        }
    }

    /// Phase 3: Infer generics from call argument types via structural unification.
    fn infer_from_arguments(&mut self,
        template: &SaltFn,
        call_arg_exprs: &[syn::Expr],
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
        map: &mut BTreeMap<String, Type>,
    ) -> Result<(), String> {
        let trace_locals: BTreeMap<String, Type> = local_vars.iter()
            .map(|(k, (t, _))| (k.clone(), t.clone()))
            .collect();

        // Collect declared generic names for normalization
        let declared_generics = self.get_declared_generic_names(template);

        // For method calls, skip `self` parameter (index 0 in template.args)
        // call_arg_exprs doesn't include `self` — it's the explicit args only
        let is_method = template.args.first()
            .map(|a| a.name == "self")
            .unwrap_or(false);
        let param_offset = if is_method { 1 } else { 0 };

        for (arg_idx, func_arg) in template.args.iter().skip(param_offset).enumerate() {
            if let Some(call_expr) = call_arg_exprs.get(arg_idx) {
                if let Some(pat_syn) = &func_arg.ty {
                    if let Some(raw_pat_ty) = Type::from_syn(pat_syn) {
                        // Normalize Struct("F2") → Generic("F2") for declared generics
                        let pat_ty = normalize_generics(&raw_pat_ty, &declared_generics);

                        // Trace the concrete type of the argument expression
                        let concrete_result = crate::codegen::tracer::TypeTracer::trace_expr_type(
                            self.ctx, call_expr, &trace_locals
                        );

                        // Fallback: resolve function names to their Fn type
                        let concrete_result = if concrete_result.is_err() {
                            self.try_resolve_fn_arg(call_expr, local_vars).ok_or_else(|| "".to_string())
                        } else {
                            concrete_result
                        };

                        if let Ok(concrete_ty) = concrete_result {
                            unify_types(&pat_ty, &concrete_ty, map)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Fallback: Try to resolve a path expression as a function name to get Fn type.
    fn try_resolve_fn_arg(&mut self,
        expr: &syn::Expr,
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
    ) -> Option<Type> {
        if let syn::Expr::Path(p) = expr {
            if p.path.segments.len() == 1 {
                let dummy_call: syn::ExprCall = syn::parse_quote! { #p() };
                let mut resolver = crate::codegen::expr::resolver::CallSiteResolver::new(self.ctx);
                if let Ok(crate::codegen::expr::resolver::CallKind::Function(_, ret_ty, param_tys, _)) = resolver.resolve_call(&dummy_call, local_vars, None) {
                    return Some(Type::Fn(param_tys, Box::new(ret_ty)));
                }
            }
        }
        None
    }

    /// Phase 4: Infer generics from Self type (method calls).
    fn infer_from_self_type(&mut self,
        template: &SaltFn,
        defined_self: &Type,
        call_arg_exprs: &[syn::Expr],
        local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>,
        map: &mut BTreeMap<String, Type>,
    ) -> Result<(), String> {
        let is_instance_method = template.args.first()
            .map(|arg| arg.name == "self")
            .unwrap_or(false);

        if is_instance_method {
            if let Some(first_expr) = call_arg_exprs.first() {
                let trace_locals: BTreeMap<String, Type> = local_vars.iter()
                    .map(|(k, (t, _))| (k.clone(), t.clone()))
                    .collect();
                if let Ok(ty) = crate::codegen::tracer::TypeTracer::trace_expr_type(
                    self.ctx, first_expr, &trace_locals
                ) {
                    unify_types(defined_self, &ty, map)?;
                }
            }
        }
        Ok(())
    }

    /// Phase 5: Infer generics from expected return type.
    fn infer_from_return_type(&mut self,
        template: &SaltFn,
        expected_ret_ty: Option<&Type>,
        map: &mut BTreeMap<String, Type>,
    ) -> Result<(), String> {
        let Some(expected) = expected_ret_ty else { return Ok(()) };
        
        // Only proceed if there are still unmapped generics
        let unmapped = self.get_unmapped_generics(template, map);
        if unmapped.is_empty() { return Ok(()); }

        // Resolve template return type WITHOUT current specialization context
        let template_ret_ty = {
            let rt = match &template.ret_type {
                Some(rt) => rt.clone(),
                None => return Ok(()),
            };
            self.ctx.with_generic_context(
                BTreeMap::new(),
                Type::Unit,
                Vec::new(),
                |ctx| crate::codegen::type_bridge::resolve_type(ctx, &rt),
            )
        };

        let mut inferred = BTreeMap::new();
        unify_types(&template_ret_ty, expected, &mut inferred)?;

        for name in &unmapped {
            if let Some(ty) = inferred.get(name) {
                if !map.contains_key(name) {
                    map.insert(name.clone(), ty.clone());
                }
            }
        }
        Ok(())
    }

    /// Phase 6: Infer phantom generics from Fn return types.
    /// 
    /// Example: `Map<I, F, T>` where `F = Fn(i64)->i64` => `T = i64`.
    /// Phantom generics don't appear in struct fields but represent the
    /// output type of a function-typed generic parameter.
    fn _infer_phantom_generics_from_template(&mut self,
        template: &SaltFn,
        map: &mut BTreeMap<String, Type>,
    ) {
        if let Some(generics) = &template.generics {
            let declared: Vec<String> = generics.params.iter()
                .map(generic_param_name)
                .collect();
            infer_phantom_generics(&declared, map);
        }
    }

    /// Phase 6 (method-only): Infer phantom generics using only method-level params.
    /// 
    /// func.generics.params includes impl-level params (I, F) prepended to method-level (F2, T).
    /// Struct-level Fn types (e.g., F=Fn(i64,Bool)) would pollute phantom inference,
    /// so we filter them out and only consider method-level declared generics.
    fn infer_phantom_generics_method_only(&mut self,
        template: &SaltFn,
        struct_generic_names: &[String],
        map: &mut BTreeMap<String, Type>,
    ) {
        if let Some(generics) = &template.generics {
            let method_only_declared: Vec<String> = generics.params.iter()
                .map(generic_param_name)
                .filter(|name| !struct_generic_names.contains(name))
                .collect();
            infer_phantom_generics(&method_only_declared, map);
        }
    }

    /// Phase 7 (method-only): Verify only method-level generics are resolved.
    /// 
    /// Struct-level params are handled by the concrete_tys pipeline and don't need
    /// to be verified here. This prevents false "Unresolved generic 'I'" errors
    /// when I, F are correctly in concrete_tys but not in the resolver's map.
    fn verify_completeness_method_only(&mut self,
        template: &SaltFn,
        struct_generic_names: &[String],
        map: &mut BTreeMap<String, Type>,
        expected_ret_ty: Option<&Type>,
        _self_ty: Option<&Type>,
    ) -> Result<(), String> {
        // Only check method-level generics (skip struct-level)
        let required: Vec<String> = template.generics.as_ref()
            .map(|g| g.params.iter()
                .map(generic_param_name)
                .filter(|name| !struct_generic_names.contains(name))
                .collect::<Vec<_>>())
            .unwrap_or_default();

        for req in &required {
            if !map.contains_key(req) {
                if let Some(inferred) = self.infer_single_from_return(req, template, expected_ret_ty)? {
                    map.insert(req.clone(), inferred);
                } else {
                    return Err(format!("Unresolved generic '{}' in function '{}'", req, template.name));
                }
            }
        }

        // Struct-level generics from self_ty are handled externally
        Ok(())
    }

    /// Phase 7: Verify all required generics are resolved.
    fn _verify_completeness(&mut self,
        template: &SaltFn,
        map: &mut BTreeMap<String, Type>,
        expected_ret_ty: Option<&Type>,
        self_ty: Option<&Type>,
    ) -> Result<(), String> {
        // Check function-level generics
        let required = template.generics.as_ref()
            .map(|g| g.params.iter().map(generic_param_name).collect::<Vec<_>>())
            .unwrap_or_default();

        for req in &required {
            if !map.contains_key(req) {
                // Last-resort: try return type inference
                if let Some(inferred) = self.infer_single_from_return(req, template, expected_ret_ty)? {
                    map.insert(req.clone(), inferred);
                } else {
                    return Err(format!("Unresolved generic '{}' in function '{}'", req, template.name));
                }
            }
        }

        // Check struct-level generics from self_ty
        if let Some(sty) = self_ty {
            let struct_generics = extract_generic_names_from_type(sty);
            for sg in &struct_generics {
                if !map.contains_key(sg) {
                    if let Some(inferred) = self.infer_single_from_return(sg, template, expected_ret_ty)? {
                        map.insert(sg.clone(), inferred);
                    } else {
                        return Err(format!(
                            "Unresolved struct generic '{}' in method '{}'. Consider using turbofish syntax.",
                            sg, template.name
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Last-resort inference for a single generic from the return type.
    fn infer_single_from_return(&mut self,
        generic_name: &str,
        template: &SaltFn,
        expected_ret_ty: Option<&Type>,
    ) -> Result<Option<Type>, String> {
        let expected = match expected_ret_ty {
            Some(e) => e,
            None => return Ok(None),
        };
        let template_ret_ty = {
            let rt = match template.ret_type.as_ref() {
                Some(r) => r,
                None => return Ok(None),
            };
            self.ctx.with_generic_context(
                BTreeMap::new(),
                Type::Unit,
                Vec::new(),
                |ctx| crate::codegen::type_bridge::resolve_type(ctx, rt),
            )
        };

        // Normalize Struct("T") → Generic("T") for all declared generics.
        // Without this, resolve_type produces Struct("T") (not Generic("T")),
        // and unify_types can't bind it against the concrete expected type.
        let declared: Vec<String> = template.generics.as_ref()
            .map(|g| g.params.iter().map(generic_param_name).collect())
            .unwrap_or_default();
        let normalized = normalize_generics(&template_ret_ty, &declared);

        let mut temp_map = BTreeMap::new();
        unify_types(&normalized, expected, &mut temp_map)?;
        Ok(temp_map.remove(generic_name))
    }

    /// Collect all declared generic param names from a function template.
    fn get_declared_generic_names(&mut self, template: &SaltFn) -> Vec<String> {
        template.generics.as_ref()
            .map(|g| g.params.iter().map(generic_param_name).collect())
            .unwrap_or_default()
    }

    /// Collect generic param names that are NOT yet in the map.
    fn get_unmapped_generics(&mut self, template: &SaltFn, map: &BTreeMap<String, Type>) -> Vec<String> {
        template.generics.as_ref()
            .map(|g| g.params.iter()
                .map(generic_param_name)
                .filter(|name| !map.contains_key(name))
                .collect())
            .unwrap_or_default()
    }
}

// ════════════════════════════════════════════════════════════════════════
// Standalone utility functions (used by both GenericResolver and callers)
// ════════════════════════════════════════════════════════════════════════

/// Extract the name from a GenericParam.
pub fn generic_param_name(param: &GenericParam) -> String {
    match param {
        GenericParam::Type { name, .. } => name.to_string(),
        GenericParam::Const { name, .. } => name.to_string(),
    }
}

/// Infer phantom generics from resolved Fn return types.
///
/// Example: `Map<I, F, T>` where `F = Fn(i64)->i64` => `T = i64`.
/// When exactly one generic is unresolved and there are resolved Fn types
/// among the declared generics, bind the unresolved generic to the Fn's return type.
pub fn infer_phantom_generics(
    declared_generics: &[String],
    map: &mut BTreeMap<String, Type>,
) {
    let unresolved: Vec<String> = declared_generics.iter()
        .filter(|g| !map.contains_key(*g))
        .cloned()
        .collect();

    if unresolved.is_empty() { return; }

    // Collect return types from Fn types that are bound to DECLARED generics only
    // This avoids false matches from struct-level generics like F = Fn(i64, Bool)
    let fn_return_types: Vec<Type> = declared_generics.iter()
        .filter_map(|g| map.get(g))
        .filter_map(|ty| {
            if let Type::Fn(_, ret) = ty {
                Some((**ret).clone())
            } else {
                None
            }
        })
        .collect();

    // If there's exactly one unresolved generic and exactly one declared Fn return type,
    // use the Fn's return type as the phantom generic
    if unresolved.len() == 1 && fn_return_types.len() == 1 {
        map.insert(unresolved[0].clone(), fn_return_types[0].clone());
    }
}

/// Normalize a type by converting `Struct("X")` → `Generic("X")` for declared generics.
///
/// The parser produces `SynType::Path("F2")` → `Type::Struct("F2")` for generic parameters,
/// because it can't distinguish struct from generic names. This function normalizes
/// multi-character generics (like `F2`, `Output`, `Allocator`) so `unify_types` can bind them.
pub fn normalize_generics(ty: &Type, declared: &[String]) -> Type {
    match ty {
        Type::Struct(name) if declared.contains(name) => Type::Generic(name.clone()),
        Type::Concrete(name, args) => {
            let normalized_args: Vec<Type> = args.iter()
                .map(|a| normalize_generics(a, declared))
                .collect();
            // The name itself might be a generic (rare)
            if declared.contains(name) {
                Type::Generic(name.clone())
            } else {
                Type::Concrete(name.clone(), normalized_args)
            }
        }
        Type::Reference(inner, m) => Type::Reference(
            Box::new(normalize_generics(inner, declared)), *m
        ),
        Type::Pointer { element, provenance, is_mutable } => Type::Pointer {
            element: Box::new(normalize_generics(element, declared)),
            provenance: provenance.clone(),
            is_mutable: *is_mutable,
        },
        Type::Fn(args, ret) => Type::Fn(
            args.iter().map(|a| normalize_generics(a, declared)).collect(),
            Box::new(normalize_generics(ret, declared)),
        ),
        Type::Array(inner, len, rank) => Type::Array(
            Box::new(normalize_generics(inner, declared)), *len, *rank
        ),
        Type::Owned(inner) => Type::Owned(Box::new(normalize_generics(inner, declared))),
        Type::Atomic(inner) => Type::Atomic(Box::new(normalize_generics(inner, declared))),
        _ => ty.clone(),
    }
}

pub use crate::codegen::generic_unify::{unify_types, extract_generic_names_from_type};

