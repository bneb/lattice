use std::collections::{BTreeMap, HashMap, HashSet};
use crate::hir::ids::VarId;
use crate::hir::expr::{Expr, ExprKind, Literal, Block};
use crate::hir::stmt::{Stmt, StmtKind, Pattern};
use crate::hir::items::{Item, ItemKind};
use crate::hir::types::Type;

/// Extracted function signature for the global symbol table.
#[derive(Clone, Debug)]
pub struct FnSig {
    pub params: Vec<Type>,
    pub return_type: Type,
    /// Marks which parameter positions are `consume` (linear) parameters.
    /// A consumed argument becomes inaccessible after the call.
    pub linear_params: Vec<bool>,
}

/// Struct definition for the type checker's struct registry.
#[derive(Clone, Debug)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    /// Inherent methods: method_name -> FnSig (first param is self/&self).
    pub methods: HashMap<String, FnSig>,
    /// Generic type parameter names (e.g., ["T"] for `struct Wrapper<T>`).
    /// Empty for non-generic structs.
    pub type_params: Vec<String>,
}

/// Trait definition for the type checker's trait registry.
#[derive(Clone, Debug)]
pub struct TraitDef {
    pub name: String,
    /// Required method signatures that any implementor must provide.
    pub required_methods: HashMap<String, FnSig>,
}

/// Type-checking context.
/// Walks HIR bottom-up, infers types from leaves (literals/variables),
/// and proves that operations at branches (binary ops, calls) are legal.
pub struct TypeckContext {
    /// Maps a local VarId to its resolved Type.
    local_env: HashMap<VarId, Type>,

    /// Function signatures keyed by name.
    functions: HashMap<String, FnSig>,

    /// Struct definitions keyed by name.
    structs: HashMap<String, StructDef>,

    /// Trait definitions keyed by name.
    _traits: HashMap<String, TraitDef>,

    /// Accumulated type errors (non-fatal collection mode).
    pub errors: Vec<String>,

    /// Linear type tracking: variables that have been consumed by a
    /// `consume`-annotated parameter. Any subsequent use is an error.
    consumed_vars: HashSet<VarId>,
}

impl Default for TypeckContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeckContext {
    /// Create a TypeckContext with no global definitions.
    pub fn new() -> Self {
        Self {
            local_env: HashMap::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            _traits: HashMap::new(),
            errors: Vec::new(),
            consumed_vars: HashSet::new(),
        }
    }

    /// Create a TypeckContext pre-loaded with global definitions
    /// extracted from the lowered HIR items.
    pub fn with_items(items: &[Item]) -> Self {
        let mut functions = HashMap::new();
        let mut structs = HashMap::new();
        let mut traits = HashMap::new();
        let mut trait_errors: Vec<String> = Vec::new();

        // Pass 1: Register functions, structs, and traits
        for item in items {
            match &item.kind {
                ItemKind::Fn(f) => {
                    let param_count = f.inputs.len();
                    let sig = FnSig {
                        params: f.inputs.iter().map(|p| p.ty.clone()).collect(),
                        return_type: f.output.clone(),
                        linear_params: vec![false; param_count],
                    };
                    functions.insert(item.name.clone(), sig);
                }
                ItemKind::Struct(s) => {
                    let type_params: Vec<String> = s.generics.params.iter().filter_map(|p| {
                        match p {
                            crate::hir::items::GenericParam::Type(name) => Some(name.clone()),
                            _ => None,
                        }
                    }).collect();
                    let def = StructDef {
                        name: item.name.clone(),
                        fields: s.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect(),
                        methods: HashMap::new(),
                        type_params,
                    };
                    structs.insert(item.name.clone(), def);
                }
                ItemKind::Trait(t) => {
                    let mut required_methods = HashMap::new();
                    for trait_item in &t.items {
                        let crate::hir::items::TraitItem::Fn { name, func } = trait_item; {
                            let param_count = func.inputs.len();
                            let sig = FnSig {
                                params: func.inputs.iter().map(|p| p.ty.clone()).collect(),
                                return_type: func.output.clone(),
                                linear_params: vec![false; param_count],
                            };
                            required_methods.insert(name.clone(), sig);
                        }
                    }
                    traits.insert(item.name.clone(), TraitDef {
                        name: item.name.clone(),
                        required_methods,
                    });
                }
                _ => {}
            }
        }

        // Pass 2: Inject impl methods into struct definitions.
        // Handles both inherent impls and trait impls with compliance checking.
        for item in items {
            if let ItemKind::Impl(imp) = &item.kind {
                if let Some(trait_ref) = &imp.trait_ref {
                    // ── Trait impl: compliance check + method injection ──
                    let trait_name = match trait_ref {
                        Type::Struct(name) => name.clone(),
                        _ => {
                            trait_errors.push(format!("Invalid trait reference: {:?}", trait_ref));
                            continue;
                        }
                    };
                    let struct_name = match Self::extract_struct_name(&imp.self_ty) {
                        Ok(n) => n,
                        Err(e) => { trait_errors.push(e); continue; }
                    };
                    let trait_def = match traits.get(&trait_name) {
                        Some(td) => td.clone(),
                        None => {
                            trait_errors.push(format!("Unknown trait: '{}'", trait_name));
                            continue;
                        }
                    };

                    // Build map of provided methods
                    let mut provided: HashMap<String, FnSig> = HashMap::new();
                    for impl_item in &imp.items {
                        let crate::hir::items::ImplItem::Fn { name, func } = impl_item; {
                            let param_count = func.inputs.len();
                            let sig = FnSig {
                                params: func.inputs.iter().map(|p| p.ty.clone()).collect(),
                                return_type: func.output.clone(),
                                linear_params: vec![false; param_count],
                            };
                            provided.insert(name.clone(), sig);
                        }
                    }

                    // Compliance: every required method must be present with matching signature
                    let mut compliant = true;
                    for (req_name, req_sig) in &trait_def.required_methods {
                        match provided.get(req_name) {
                            None => {
                                trait_errors.push(format!(
                                    "Missing required method '{}' for trait '{}' on '{}'",
                                    req_name, trait_name, struct_name
                                ));
                                compliant = false;
                            }
                            Some(provided_sig) => {
                                // Compare signatures with SelfType tolerance:
                                // In the trait, &self is Type::SelfType.
                                // In the impl, &self is also Type::SelfType.
                                // So direct comparison works for params.
                                let params_match = req_sig.params.len() == provided_sig.params.len()
                                    && req_sig.params.iter().zip(provided_sig.params.iter()).all(|(r, p)| {
                                        r == p || *r == Type::SelfType && *p == Type::SelfType
                                    });
                                let ret_match = req_sig.return_type == provided_sig.return_type;
                                if !params_match || !ret_match {
                                    trait_errors.push(format!(
                                        "Signature mismatch for method '{}' in trait '{}' on '{}': \
                                         expected ({:?}) -> {:?}, found ({:?}) -> {:?}",
                                        req_name, trait_name, struct_name,
                                        req_sig.params, req_sig.return_type,
                                        provided_sig.params, provided_sig.return_type
                                    ));
                                    compliant = false;
                                }
                            }
                        }
                    }

                    // If compliant, inject methods into the struct
                    if compliant {
                        if let Some(struct_def) = structs.get_mut(&struct_name) {
                            for (method_name, method_sig) in provided {
                                struct_def.methods.insert(method_name, method_sig);
                            }
                        }
                    }
                } else {
                    // ── Inherent impl: inject methods directly ──
                    if let Ok(struct_name) = Self::extract_struct_name(&imp.self_ty) {
                        if let Some(struct_def) = structs.get_mut(&struct_name) {
                            for impl_item in &imp.items {
                                let crate::hir::items::ImplItem::Fn { name, func } = impl_item; {
                                    let param_count = func.inputs.len();
                                    let sig = FnSig {
                                        params: func.inputs.iter().map(|p| p.ty.clone()).collect(),
                                        return_type: func.output.clone(),
                                        linear_params: vec![false; param_count],
                                    };
                                    struct_def.methods.insert(name.clone(), sig);
                                }
                            }
                        }
                    }
                }
            }
        }

        Self {
            local_env: HashMap::new(),
            functions,
            structs,
            _traits: traits,
            errors: trait_errors,
            consumed_vars: HashSet::new(),
        }
    }

    /// Safely extracts the struct name from an impl block's target type.
    fn extract_struct_name(ty: &Type) -> Result<String, String> {
        match ty {
            Type::Struct(name) => Ok(name.clone()),
            _ => Err(format!("Cannot implement methods on non-struct type: {:?}", ty)),
        }
    }

    /// Manually register a function signature (useful for tests).
    pub fn register_fn(&mut self, name: impl Into<String>, params: Vec<Type>, return_type: Type) {
        let linear_params = vec![false; params.len()];
        self.functions.insert(name.into(), FnSig { params, return_type, linear_params });
    }

    /// Manually register a non-generic struct definition (useful for tests).
    pub fn register_struct(&mut self, name: impl Into<String>, fields: Vec<(&str, Type)>) {
        let name = name.into();
        let def = StructDef {
            name: name.clone(),
            fields: fields.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
            methods: HashMap::new(),
            type_params: vec![],
        };
        self.structs.insert(name, def);
    }

    /// Register an inherent method on a struct.
    /// The `params` should include the self/&self parameter as the first element.
    pub fn register_method(
        &mut self,
        struct_name: &str,
        method_name: impl Into<String>,
        params: Vec<Type>,
        return_type: Type,
    ) {
        if let Some(struct_def) = self.structs.get_mut(struct_name) {
            let linear_params = vec![false; params.len()];
            struct_def.methods.insert(method_name.into(), FnSig { params, return_type, linear_params });
        }
    }

    /// Register a generic struct definition (useful for tests).
    pub fn register_generic_struct(
        &mut self,
        name: impl Into<String>,
        type_params: Vec<&str>,
        fields: Vec<(&str, Type)>,
    ) {
        let name = name.into();
        let def = StructDef {
            name: name.clone(),
            fields: fields.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
            methods: HashMap::new(),
            type_params: type_params.into_iter().map(|s| s.to_string()).collect(),
        };
        self.structs.insert(name, def);
    }

    // ─── Monomorphization Engine ──────────────────────────────────────────

    /// Monomorphize a generic struct with concrete type arguments.
    /// Returns the mangled name of the monomorphized struct.
    pub fn monomorphize_struct(
        &mut self,
        base_name: &str,
        type_args: &[Type],
    ) -> Result<String, String> {
        // Build the mangled name for cache lookup
        let mangled = Self::mangle_name(base_name, type_args);

        // Cache hit: already monomorphized
        if self.structs.contains_key(&mangled) {
            return Ok(mangled);
        }

        // Look up the generic template
        let template = self.structs.get(base_name)
            .cloned()
            .ok_or_else(|| format!("Unknown struct: '{}'", base_name))?;

        // Verify arity
        if type_args.len() != template.type_params.len() {
            return Err(format!(
                "Generic arity mismatch for '{}': expected {} type arguments, found {}",
                base_name, template.type_params.len(), type_args.len()
            ));
        }

        // Build substitution map
        let mapping: BTreeMap<String, Type> = template.type_params.iter()
            .zip(type_args.iter())
            .map(|(param, arg)| (param.clone(), arg.clone()))
            .collect();

        // Substitute field types
        let mono_fields: Vec<(String, Type)> = template.fields.iter()
            .map(|(name, ty)| (name.clone(), ty.substitute(&mapping)))
            .collect();

        // Substitute method signatures
        let mono_methods: HashMap<String, FnSig> = template.methods.iter()
            .map(|(mname, sig)| {
                let mono_sig = FnSig {
                    params: sig.params.iter().map(|p| p.substitute(&mapping)).collect(),
                    return_type: sig.return_type.substitute(&mapping),
                    linear_params: sig.linear_params.clone(),
                };
                (mname.clone(), mono_sig)
            })
            .collect();

        // Register the monomorphized struct
        let mono_def = StructDef {
            name: mangled.clone(),
            fields: mono_fields,
            methods: mono_methods,
            type_params: vec![], // Concrete: no more type params
        };
        self.structs.insert(mangled.clone(), mono_def);

        Ok(mangled)
    }

    /// Generate a mangled name for a monomorphized struct.
    /// e.g., "Wrapper" + [I64] -> "Wrapper_I64"
    fn mangle_name(base: &str, type_args: &[Type]) -> String {
        let mut name = base.to_string();
        for arg in type_args {
            name.push('_');
            name.push_str(&format!("{:?}", arg));
        }
        name
    }

    // ─── Expression Type-Checking ─────────────────────────────────────────

    /// Evaluate an expression bottom-up, enforce type rules,
    /// and ATTACH the resolved type to the HIR node.
    fn typeck_method_call(&mut self, receiver: &mut Expr, method: &str, args: &mut [Expr]) -> Result<Type, String> {
                // 1. Evaluate the receiver
                let receiver_ty = self.typeck_expr(receiver)?;

                // 2. The receiver must be a struct type
                let struct_name = match &receiver_ty {
                    Type::Struct(name) => name.clone(),
                    _ => return Err(format!(
                        "Cannot call method '{}' on non-struct type {:?}",
                        method, receiver_ty
                    )),
                };

                // 3. Look up the struct and its method table
                let struct_def = self.structs.get(&struct_name)
                    .ok_or_else(|| format!(
                        "Compiler bug: struct '{}' not in registry", struct_name
                    ))?
                    .clone();

                let sig = struct_def.methods.get(method)
                    .ok_or_else(|| format!(
                        "Struct '{}' has no method named '{}'",
                        struct_name, method
                    ))?
                    .clone();

                // 4. The receiver is implicit argument 0.
                //    sig.params[0] is the self parameter type.
                //    No auto-ref: types must match exactly.
                if sig.params.is_empty() {
                    return Err(format!(
                        "Method '{}' on '{}' has no self parameter",
                        method, struct_name
                    ));
                }

                let self_param_ty = &sig.params[0];
                // SelfType is compatible with the receiver's concrete struct type
                let self_matches = &receiver_ty == self_param_ty
                    || *self_param_ty == Type::SelfType;
                if !self_matches {
                    return Err(format!(
                        "Method '{}' expects self parameter {:?}, but receiver is {:?}",
                        method, self_param_ty, receiver_ty
                    ));
                }

                // 5. Verify arity: explicit args + implicit self = sig.params
                let expected_explicit = sig.params.len() - 1;
                if args.len() != expected_explicit {
                    return Err(format!(
                        "Arity mismatch for method '{}': expected {} arguments, found {}",
                        method, expected_explicit, args.len()
                    ));
                }

                // 6. Type-check each explicit argument
                for (i, arg) in args.iter_mut().enumerate() {
                    let arg_ty = self.typeck_expr(arg)?;
                    let expected_ty = &sig.params[i + 1]; // skip self
                    if &arg_ty != expected_ty {
                        return Err(format!(
                            "Type mismatch in argument {} of method '{}': expected {:?}, found {:?}",
                            i, method, expected_ty, arg_ty
                        ));
                    }
                }

                // 7. Return the method's return type
                Ok(sig.return_type)
    }

    fn typeck_call(&mut self, callee: &mut Expr, args: &mut [Expr]) -> Result<Type, String> {
                // 1. Extract the function name from the callee
                let fn_name = match &callee.kind {
                    ExprKind::UnresolvedIdent(name) => name.clone(),
                    ExprKind::Path(_def_id) => {
                        return Err("Typeck for Path-based calls not yet implemented".into());
                    }
                    other => {
                        return Err(format!("Cannot call non-function: {:?}", other));
                    }
                };

                // 2. Look up in the function registry
                let sig = self.functions.get(&fn_name)
                    .cloned()
                    .ok_or_else(|| format!("Unknown function: '{}'", fn_name))?;

                // 3. Verify arity
                if args.len() != sig.params.len() {
                    return Err(format!(
                        "Arity mismatch for '{}': expected {} arguments, found {}",
                        fn_name, sig.params.len(), args.len()
                    ));
                }

                // 4. Type-check each argument against the signature
                for (i, arg) in args.iter_mut().enumerate() {
                    let arg_ty = self.typeck_expr(arg)?;
                    let expected_ty = &sig.params[i];
                    if &arg_ty != expected_ty {
                        return Err(format!(
                            "Type mismatch in argument {} of '{}': expected {:?}, found {:?}",
                            i, fn_name, expected_ty, arg_ty
                        ));
                    }
                }

                // 5. Linear type consumption: mark consumed arguments
                for (i, arg) in args.iter().enumerate() {
                    if i < sig.linear_params.len() && sig.linear_params[i] {
                        if let ExprKind::Var(var_id) = &arg.kind {
                            if self.consumed_vars.contains(var_id) {
                                return Err(format!(
                                    "Double consume: VarId({}) already consumed",
                                    var_id.0
                                ));
                            }
                            self.consumed_vars.insert(*var_id);
                        }
                    }
                }

                // 6. The call expression's type IS the function's return type
                Ok(sig.return_type)
    }

    fn typeck_struct_lit(&mut self, name: &str, type_args: &[Type], fields: &mut [(String, Expr)]) -> Result<Type, String> {
                // 1. If generic type_args are provided, monomorphize first
                let effective_name = if !type_args.is_empty() {
                    self.monomorphize_struct(name, type_args)?
                } else {
                    name.to_string()
                };

                // 2. Verify the struct exists (original or monomorphized)
                let struct_def = self.structs.get(&effective_name)
                    .cloned()
                    .ok_or_else(|| format!("Unknown struct: '{}'", effective_name))?;

                // 3. Verify field count
                if fields.len() != struct_def.fields.len() {
                    return Err(format!(
                        "Struct '{}' expects {} fields, but {} were provided",
                        effective_name, struct_def.fields.len(), fields.len()
                    ));
                }

                // 4. Type-check each provided field against the (monomorphized) definition
                let field_type_map: BTreeMap<String, Type> = struct_def.fields.iter()
                    .cloned()
                    .collect();
                for (field_name, field_expr) in fields.iter_mut() {
                    let expr_ty = self.typeck_expr(field_expr)?;
                    let expected_ty = field_type_map.get(field_name)
                        .ok_or_else(|| format!(
                            "Struct '{}' has no field named '{}'",
                            effective_name, field_name
                        ))?;
                    if &expr_ty != expected_ty {
                        return Err(format!(
                            "Type mismatch in field '{}' of '{}': expected {:?}, found {:?}",
                            field_name, effective_name, expected_ty, expr_ty
                        ));
                    }
                }

                // 5. The struct literal's type is the (possibly monomorphized) struct
                Ok(Type::Struct(effective_name))
    }
pub fn typeck_expr(&mut self, expr: &mut Expr) -> Result<Type, String> {
        let resolved_type = match &mut expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(_) => Type::I64,
                Literal::Float(_) => Type::F64,
                Literal::Bool(_) => Type::Bool,
                Literal::String(_) => Type::Struct("String".into()),
            },
            ExprKind::Var(var_id) => self.typeck_var(*var_id)?,
            ExprKind::Binary { op, lhs, rhs } => self.typeck_binary_op(op.clone(), lhs, rhs)?,
            ExprKind::Unary { op, expr: inner } => self.typeck_unary_op(op.clone(), inner)?,
            ExprKind::Ref(inner) => Type::Reference(Box::new(self.typeck_expr(inner)?), false),
            ExprKind::MethodCall { receiver, method, args } => self.typeck_method_call(receiver, method, args)?,
            ExprKind::Call { callee, args } => self.typeck_call(callee, args)?,
            ExprKind::Assign { lhs, rhs } => {
                let lhs_ty = self.typeck_expr(lhs)?;
                let rhs_ty = self.typeck_expr(rhs)?;
                if lhs_ty != rhs_ty {
                    return Err(format!("Assignment type mismatch: expected {:?}, found {:?}", lhs_ty, rhs_ty));
                }
                Type::Unit
            }
            ExprKind::Return(Some(inner)) => {
                self.typeck_expr(inner)?;
                Type::Never
            }
            ExprKind::Return(None) => Type::Never,
            ExprKind::Block(block) => self.typeck_block(block)?,
            ExprKind::If { cond, then_branch, else_branch } => self.typeck_if(cond, then_branch, else_branch.as_deref_mut())?,
            ExprKind::StructLit { name, type_args, fields } => self.typeck_struct_lit(name, type_args, fields)?,
            ExprKind::Field { base, field } => self.typeck_field_access(base, field)?,
            ExprKind::Requires(cond) | ExprKind::Ensures(cond) => {
                let cond_ty = self.typeck_expr(cond)?;
                if cond_ty != Type::Bool {
                    return Err(format!("Contract condition must be Bool, found {:?}", cond_ty));
                }
                Type::Unit
            }
            ExprKind::Yield(maybe_val) => {
                if let Some(val) = maybe_val {
                    self.typeck_expr(val)?;
                }
                Type::Unit
            }
            ExprKind::UnresolvedIdent(name) => return Err(format!("Unresolved identifier: '{}'", name)),
            _ => return Err(format!("Typeck not yet implemented for: {:?}", expr.kind)),
        };

        expr.ty = resolved_type.clone();
        Ok(resolved_type)
    }

    fn typeck_var(&self, var_id: crate::hir::ids::VarId) -> Result<Type, String> {
        if self.consumed_vars.contains(&var_id) {
            return Err(format!("Use after consume: VarId({}) has been consumed by a linear parameter", var_id.0));
        }
        self.local_env.get(&var_id)
            .cloned()
            .ok_or_else(|| format!("Typeck: VarId({}) has no known type", var_id.0))
    }

    fn typeck_binary_op(&mut self, op: crate::hir::expr::BinOp, lhs: &mut Expr, rhs: &mut Expr) -> Result<Type, String> {
        let lhs_ty = self.typeck_expr(lhs)?;
        let rhs_ty = self.typeck_expr(rhs)?;
        if lhs_ty != rhs_ty {
            return Err(format!("Type mismatch: cannot apply '{:?}' to {:?} and {:?}", op, lhs_ty, rhs_ty));
        }
        Ok(if op.is_relational() { Type::Bool } else { lhs_ty })
    }

    fn typeck_unary_op(&mut self, op: crate::hir::expr::UnOp, inner: &mut Expr) -> Result<Type, String> {
        let inner_ty = self.typeck_expr(inner)?;
        match op {
            crate::hir::expr::UnOp::Not => {
                if inner_ty != Type::Bool {
                    return Err(format!("Cannot apply '!' to {:?}", inner_ty));
                }
                Ok(Type::Bool)
            }
            crate::hir::expr::UnOp::Neg => Ok(inner_ty),
            crate::hir::expr::UnOp::Deref => {
                match inner_ty {
                    Type::Reference(inner_ty, _) => Ok(*inner_ty),
                    Type::Pointer { element, .. } => Ok(*element),
                    other => Err(format!("Cannot dereference non-pointer type: {:?}", other)),
                }
            }
        }
    }

    fn typeck_if(&mut self, cond: &mut Expr, then_branch: &mut crate::hir::expr::Block, else_branch: Option<&mut Expr>) -> Result<Type, String> {
        let cond_ty = self.typeck_expr(cond)?;
        if cond_ty != Type::Bool {
            return Err(format!("If condition must be Bool, found {:?}", cond_ty));
        }
        let then_ty = self.typeck_block(then_branch)?;
        if let Some(else_expr) = else_branch {
            let else_ty = self.typeck_expr(else_expr)?;
            if then_ty != else_ty {
                return Err(format!("If/else branch type mismatch: then={:?}, else={:?}", then_ty, else_ty));
            }
            Ok(then_ty)
        } else {
            Ok(Type::Unit)
        }
    }

    fn typeck_field_access(&mut self, base: &mut Expr, field: &str) -> Result<Type, String> {
        let base_ty = self.typeck_expr(base)?;
        let struct_name = match &base_ty {
            Type::Struct(name) => name.clone(),
            _ => return Err(format!("Cannot access field '{}' on non-struct type {:?}", field, base_ty)),
        };
        let struct_def = self.structs.get(&struct_name)
            .ok_or_else(|| format!("Compiler bug: struct '{}' not found in registry", struct_name))?;
        struct_def.fields.iter()
            .find(|(n, _)| n == field)
            .map(|(_, ty)| ty.clone())
            .ok_or_else(|| format!("Struct '{}' has no field named '{}'", struct_name, field))
    }


    // ─── Statement Type-Checking ──────────────────────────────────────────

    /// Evaluate a statement and update the local_env with new bindings.
    pub fn typeck_stmt(&mut self, stmt: &mut Stmt) -> Result<(), String> {
        match &mut stmt.kind {
            StmtKind::Local(local) => {
                let init_ty = if let Some(init) = &mut local.init {
                    Some(self.typeck_expr(init)?)
                } else {
                    None
                };

                if let (Some(ann_ty), Some(init_ty)) = (&local.ty, &init_ty) {
                    if ann_ty != init_ty {
                        return Err(format!(
                            "Type mismatch in let binding: annotated {:?}, found {:?}",
                            ann_ty, init_ty
                        ));
                    }
                }

                let final_ty = local.ty.clone()
                    .or(init_ty)
                    .unwrap_or(Type::Unit);

                if let Pattern::Bind { var_id, .. } = &local.pat {
                    self.local_env.insert(*var_id, final_ty);
                }
                Ok(())
            }

            StmtKind::Expr(expr) | StmtKind::Semi(expr) => {
                self.typeck_expr(expr)?;
                Ok(())
            }

            StmtKind::Return(Some(expr)) => {
                self.typeck_expr(expr)?;
                Ok(())
            }
            StmtKind::Return(None) => Ok(()),

            StmtKind::While { cond, body } => {
                let cond_ty = self.typeck_expr(cond)?;
                if cond_ty != Type::Bool {
                    return Err(format!("While condition must be Bool, found {:?}", cond_ty));
                }
                self.typeck_block(body)?;
                Ok(())
            }

            StmtKind::Loop(body) => {
                self.typeck_block(body)?;
                Ok(())
            }

            StmtKind::Break | StmtKind::Continue => Ok(()),

            StmtKind::For { var, var_name: _, iter, body } => {
                let _iter_ty = self.typeck_expr(iter)?;
                self.local_env.insert(*var, Type::Unit);
                self.typeck_block(body)?;
                Ok(())
            }

            StmtKind::Assume(cond_expr) => {
                let cond_ty = self.typeck_expr(cond_expr)?;
                if cond_ty != Type::Bool {
                    return Err(format!(
                        "Compiler Bug: Injected Assume condition must be Bool, found {:?}",
                        cond_ty
                    ));
                }
                Ok(())
            }
        }
    }

    // ─── Block Type-Checking ──────────────────────────────────────────────

    /// Type-check all statements in a block. Returns the block's type.
    pub fn typeck_block(&mut self, block: &mut Block) -> Result<Type, String> {
        for stmt in &mut block.stmts {
            self.typeck_stmt(stmt)?;
        }
        if let Some(val) = &mut block.value {
            let ty = self.typeck_expr(val)?;
            block.ty = ty.clone();
            Ok(ty)
        } else {
            block.ty = Type::Unit;
            Ok(Type::Unit)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::ids::VarId;
    use crate::hir::expr::{Expr, ExprKind, Literal, BinOp};
    use crate::hir::stmt::{Stmt, StmtKind, Pattern, Local};
    use crate::hir::types::Type;

    fn mk_expr(kind: ExprKind) -> Expr {
        Expr { kind, ty: Type::Unit, span: proc_macro2::Span::call_site() }
    }

    fn mk_int(n: i64) -> Expr {
        mk_expr(ExprKind::Literal(Literal::Int(n)))
    }

    fn mk_bool(b: bool) -> Expr {
        mk_expr(ExprKind::Literal(Literal::Bool(b)))
    }

    fn mk_var(id: u32) -> Expr {
        mk_expr(ExprKind::Var(VarId(id)))
    }

    fn mk_binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        mk_expr(ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) })
    }

    fn mk_call(name: &str, args: Vec<Expr>) -> Expr {
        mk_expr(ExprKind::Call {
            callee: Box::new(mk_expr(ExprKind::UnresolvedIdent(name.into()))),
            args,
        })
    }

    fn mk_stmt(kind: StmtKind) -> Stmt {
        Stmt { kind, span: proc_macro2::Span::call_site() }
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 1: Literal Inference
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_typeck_int_literal() {
        let mut ctx = TypeckContext::new();
        let mut expr = mk_int(5);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
        assert_eq!(expr.ty, Type::I64);
    }

    #[test]
    fn test_typeck_bool_literal() {
        let mut ctx = TypeckContext::new();
        let mut expr = mk_bool(true);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Bool);
        assert_eq!(expr.ty, Type::Bool);
    }

    #[test]
    fn test_typeck_float_literal() {
        let mut ctx = TypeckContext::new();
        let mut expr = mk_expr(ExprKind::Literal(Literal::Float(3.14)));
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::F64);
        assert_eq!(expr.ty, Type::F64);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 1: Binary Ops
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_typeck_binary_add() {
        let mut ctx = TypeckContext::new();
        let mut expr = mk_binary(BinOp::Add, mk_int(5), mk_int(10));
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
        assert_eq!(expr.ty, Type::I64);
    }

    #[test]
    fn test_typeck_binary_relational() {
        let mut ctx = TypeckContext::new();
        let mut expr = mk_binary(BinOp::Lt, mk_int(5), mk_int(10));
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Bool);
        assert_eq!(expr.ty, Type::Bool);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 1: Mismatch Rejection
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_typeck_binary_mismatch_rejected() {
        let mut ctx = TypeckContext::new();
        let mut expr = mk_binary(BinOp::Add, mk_int(5), mk_bool(true));
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Type mismatch"), "got: {}", err);
        assert!(err.contains("I64"), "got: {}", err);
        assert!(err.contains("Bool"), "got: {}", err);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 1: Variable Binding & Lookup
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_typeck_let_binding_and_var_lookup() {
        let mut ctx = TypeckContext::new();
        let mut let_stmt = mk_stmt(StmtKind::Local(Local {
            pat: Pattern::Bind { name: "x".into(), var_id: VarId(0), mutable: false },
            ty: None,
            init: Some(mk_int(5)),
        }));
        ctx.typeck_stmt(&mut let_stmt).unwrap();

        let mut var_expr = mk_var(0);
        let ty = ctx.typeck_expr(&mut var_expr).unwrap();
        assert_eq!(ty, Type::I64);
        assert_eq!(var_expr.ty, Type::I64);
    }

    #[test]
    fn test_typeck_annotated_let_mismatch() {
        let mut ctx = TypeckContext::new();
        let mut stmt = mk_stmt(StmtKind::Local(Local {
            pat: Pattern::Bind { name: "x".into(), var_id: VarId(0), mutable: false },
            ty: Some(Type::Bool),
            init: Some(mk_int(5)),
        }));
        let result = ctx.typeck_stmt(&mut stmt);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Type mismatch"));
    }

    #[test]
    fn test_typeck_unresolved_var_fails() {
        let mut ctx = TypeckContext::new();
        let mut expr = mk_var(99);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no known type"));
    }

    #[test]
    fn test_typeck_nested_binary() {
        let mut ctx = TypeckContext::new();
        let inner = mk_binary(BinOp::Add, mk_int(5), mk_int(10));
        let mut expr = mk_binary(BinOp::Gt, inner, mk_int(3));
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Bool);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 2: Global Call Resolution
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_typeck_call_happy_path() {
        // Register add(I64, I64) -> I64
        let mut ctx = TypeckContext::new();
        ctx.register_fn("add", vec![Type::I64, Type::I64], Type::I64);

        // Call add(5, 10) — should resolve to I64
        let mut expr = mk_call("add", vec![mk_int(5), mk_int(10)]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
        assert_eq!(expr.ty, Type::I64);
    }

    #[test]
    fn test_typeck_call_arity_mismatch() {
        let mut ctx = TypeckContext::new();
        ctx.register_fn("add", vec![Type::I64, Type::I64], Type::I64);

        // Call add(5) — too few arguments
        let mut expr = mk_call("add", vec![mk_int(5)]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Arity mismatch"), "got: {}", err);
        assert!(err.contains("expected 2"), "got: {}", err);
        assert!(err.contains("found 1"), "got: {}", err);
    }

    #[test]
    fn test_typeck_call_arg_type_mismatch() {
        let mut ctx = TypeckContext::new();
        ctx.register_fn("add", vec![Type::I64, Type::I64], Type::I64);

        // Call add(5, true) — second arg is Bool, expected I64
        let mut expr = mk_call("add", vec![mk_int(5), mk_bool(true)]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Type mismatch in argument 1"), "got: {}", err);
        assert!(err.contains("I64"), "got: {}", err);
        assert!(err.contains("Bool"), "got: {}", err);
    }

    #[test]
    fn test_typeck_call_unknown_function() {
        let mut ctx = TypeckContext::new();
        // No functions registered — call to nonexistent function
        let mut expr = mk_call("ghost", vec![mk_int(1)]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown function: 'ghost'"));
    }

    #[test]
    fn test_typeck_call_return_type_propagation() {
        // Register is_even(I64) -> Bool
        let mut ctx = TypeckContext::new();
        ctx.register_fn("is_even", vec![Type::I64], Type::Bool);

        // Call is_even(42) — the result should be Bool
        let mut expr = mk_call("is_even", vec![mk_int(42)]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Bool);
        assert_eq!(expr.ty, Type::Bool);
    }

    #[test]
    fn test_typeck_call_nested_in_binary() {
        // Register square(I64) -> I64
        let mut ctx = TypeckContext::new();
        ctx.register_fn("square", vec![Type::I64], Type::I64);

        // square(5) + 10 — should be I64
        let call = mk_call("square", vec![mk_int(5)]);
        let mut expr = mk_binary(BinOp::Add, call, mk_int(10));
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
    }

    #[test]
    fn test_typeck_with_items_constructor() {
        use crate::hir::ids::DefId;
        use crate::hir::items::*;
        use crate::hir::expr::Block;

        // Build a dummy HIR Item representing `fn double(x: I64) -> I64`
        let item = Item {
            id: DefId(0),
            name: "double".into(),
            vis: Visibility::Public,
            kind: ItemKind::Fn(Fn {
                inputs: vec![Param { name: "x".into(), ty: Type::I64 }],
                output: Type::I64,
                body: Some(Block { stmts: vec![], value: None, ty: Type::Unit }),
                generics: Generics::default(),
                is_async: false,
            }),
            span: proc_macro2::Span::call_site(),
        };

        // Construct context from items
        let mut ctx = TypeckContext::with_items(&[item]);

        // Call double(7) — should resolve to I64
        let mut expr = mk_call("double", vec![mk_int(7)]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 3: Struct Literal Typechecking
    // ═════════════════════════════════════════════════════════════════════

    fn mk_struct_lit(name: &str, fields: Vec<(&str, Expr)>) -> Expr {
        mk_expr(ExprKind::StructLit {
            name: name.into(),
            type_args: vec![],
            fields: fields.into_iter().map(|(n, e)| (n.to_string(), e)).collect(),
        })
    }

    fn mk_generic_struct_lit(name: &str, type_args: Vec<Type>, fields: Vec<(&str, Expr)>) -> Expr {
        mk_expr(ExprKind::StructLit {
            name: name.into(),
            type_args,
            fields: fields.into_iter().map(|(n, e)| (n.to_string(), e)).collect(),
        })
    }

    fn mk_field(base: Expr, field: &str) -> Expr {
        mk_expr(ExprKind::Field {
            base: Box::new(base),
            field: field.into(),
        })
    }

    #[test]
    fn test_typeck_struct_lit_happy_path() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Point", vec![("x", Type::I64), ("y", Type::I64)]);

        // Point { x: 5, y: 10 } — should resolve to Struct("Point")
        let mut expr = mk_struct_lit("Point", vec![("x", mk_int(5)), ("y", mk_int(10))]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Struct("Point".into()));
        assert_eq!(expr.ty, Type::Struct("Point".into()));
    }

    #[test]
    fn test_typeck_struct_lit_unknown_struct() {
        let mut ctx = TypeckContext::new();
        // No structs registered
        let mut expr = mk_struct_lit("Ghost", vec![("x", mk_int(1))]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown struct: 'Ghost'"));
    }

    #[test]
    fn test_typeck_struct_lit_field_count_mismatch() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Point", vec![("x", Type::I64), ("y", Type::I64)]);

        // Only provide one field — should fail
        let mut expr = mk_struct_lit("Point", vec![("x", mk_int(5))]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("expects 2 fields"), "got: {}", err);
        assert!(err.contains("1 were provided"), "got: {}", err);
    }

    #[test]
    fn test_typeck_struct_lit_field_type_mismatch() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Point", vec![("x", Type::I64), ("y", Type::I64)]);

        // Point { x: 5, y: true } — y should be I64, not Bool
        let mut expr = mk_struct_lit("Point", vec![("x", mk_int(5)), ("y", mk_bool(true))]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Type mismatch in field 'y'"), "got: {}", err);
        assert!(err.contains("I64"), "got: {}", err);
        assert!(err.contains("Bool"), "got: {}", err);
    }

    #[test]
    fn test_typeck_struct_lit_unknown_field() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Point", vec![("x", Type::I64), ("y", Type::I64)]);

        // Point { x: 5, z: 10 } — "z" doesn't exist on Point
        let mut expr = mk_struct_lit("Point", vec![("x", mk_int(5)), ("z", mk_int(10))]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no field named 'z'"));
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 3: Field Access Typechecking
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_typeck_field_access_happy_path() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Point", vec![("x", Type::I64), ("y", Type::I64)]);

        // Construct: Point { x: 5, y: 10 }.x — should resolve to I64
        let struct_lit = mk_struct_lit("Point", vec![("x", mk_int(5)), ("y", mk_int(10))]);
        let mut expr = mk_field(struct_lit, "x");
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
        assert_eq!(expr.ty, Type::I64);
    }

    #[test]
    fn test_typeck_field_access_on_non_struct() {
        let mut ctx = TypeckContext::new();

        // 42.x — cannot access field on I64
        let mut expr = mk_field(mk_int(42), "x");
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("non-struct type"), "got: {}", err);
        assert!(err.contains("I64"), "got: {}", err);
    }

    #[test]
    fn test_typeck_field_access_unknown_field() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Point", vec![("x", Type::I64), ("y", Type::I64)]);

        // Point { x: 5, y: 10 }.z — "z" doesn't exist
        let struct_lit = mk_struct_lit("Point", vec![("x", mk_int(5)), ("y", mk_int(10))]);
        let mut expr = mk_field(struct_lit, "z");
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no field named 'z'"));
    }

    #[test]
    fn test_typeck_field_access_via_variable() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Player", vec![("health", Type::I64), ("alive", Type::Bool)]);

        // Simulate: let player: Player; player.health
        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let mut expr = mk_field(mk_var(0), "health");
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
    }

    #[test]
    fn test_typeck_field_in_binary_op() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Point", vec![("x", Type::I64), ("y", Type::I64)]);

        // Point { x: 3, y: 4 }.x + Point { x: 1, y: 2 }.y — should be I64
        let p1 = mk_struct_lit("Point", vec![("x", mk_int(3)), ("y", mk_int(4))]);
        let p2 = mk_struct_lit("Point", vec![("x", mk_int(1)), ("y", mk_int(2))]);
        let lhs = mk_field(p1, "x");
        let rhs = mk_field(p2, "y");
        let mut expr = mk_binary(BinOp::Add, lhs, rhs);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
    }

    #[test]
    fn test_typeck_with_items_struct() {
        use crate::hir::ids::DefId;
        use crate::hir::items::*;

        // Build a dummy HIR Item for `struct Vec2 { x: F64, y: F64 }`
        let item = Item {
            id: DefId(1),
            name: "Vec2".into(),
            vis: Visibility::Public,
            kind: ItemKind::Struct(Struct {
                fields: vec![
                    Field { name: "x".into(), ty: Type::F64, vis: Visibility::Public },
                    Field { name: "y".into(), ty: Type::F64, vis: Visibility::Public },
                ],
                generics: Generics::default(),
                invariants: vec![],
            }),
            span: proc_macro2::Span::call_site(),
        };

        let mut ctx = TypeckContext::with_items(&[item]);

        // Vec2 { x: 1.0, y: 2.0 } — should resolve to Struct("Vec2")
        let mut expr = mk_struct_lit("Vec2", vec![
            ("x", mk_expr(ExprKind::Literal(Literal::Float(1.0)))),
            ("y", mk_expr(ExprKind::Literal(Literal::Float(2.0)))),
        ]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Struct("Vec2".into()));

        // Vec2 { x: 1.0, y: 2.0 }.x — should resolve to F64
        let struct_lit = mk_struct_lit("Vec2", vec![
            ("x", mk_expr(ExprKind::Literal(Literal::Float(1.0)))),
            ("y", mk_expr(ExprKind::Literal(Literal::Float(2.0)))),
        ]);
        let mut field_expr = mk_field(struct_lit, "y");
        let field_ty = ctx.typeck_expr(&mut field_expr).unwrap();
        assert_eq!(field_ty, Type::F64);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 4: Pointer Physics — Address-Of & Deref
    // ═════════════════════════════════════════════════════════════════════

    fn mk_ref(inner: Expr) -> Expr {
        mk_expr(ExprKind::Ref(Box::new(inner)))
    }

    fn mk_deref(inner: Expr) -> Expr {
        mk_expr(ExprKind::Unary {
            op: crate::hir::expr::UnOp::Deref,
            expr: Box::new(inner),
        })
    }

    fn mk_method_call(receiver: Expr, method: &str, args: Vec<Expr>) -> Expr {
        mk_expr(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            method: method.into(),
            args,
        })
    }

    #[test]
    fn test_typeck_ref_produces_reference_type() {
        let mut ctx = TypeckContext::new();
        // &5 — should produce Reference(I64, false)
        let mut expr = mk_ref(mk_int(5));
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Reference(Box::new(Type::I64), false));
        assert_eq!(expr.ty, Type::Reference(Box::new(Type::I64), false));
    }

    #[test]
    fn test_typeck_ref_nested() {
        let mut ctx = TypeckContext::new();
        // &&5 — Reference(Reference(I64))
        let mut expr = mk_ref(mk_ref(mk_int(5)));
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Reference(
            Box::new(Type::Reference(Box::new(Type::I64), false)),
            false
        ));
    }

    #[test]
    fn test_typeck_deref_reference() {
        let mut ctx = TypeckContext::new();
        // *(&5) — should unwrap Reference(I64) back to I64
        let mut expr = mk_deref(mk_ref(mk_int(5)));
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
        assert_eq!(expr.ty, Type::I64);
    }

    #[test]
    fn test_typeck_deref_raw_integer_fatal_error() {
        let mut ctx = TypeckContext::new();
        // *5 — cannot dereference a non-pointer type
        let mut expr = mk_deref(mk_int(5));
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Cannot dereference non-pointer type"), "got: {}", err);
        assert!(err.contains("I64"), "got: {}", err);
    }

    #[test]
    fn test_typeck_deref_bool_fatal_error() {
        let mut ctx = TypeckContext::new();
        // *true — cannot dereference a non-pointer type
        let mut expr = mk_deref(mk_bool(true));
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot dereference non-pointer type"));
    }

    #[test]
    fn test_typeck_ref_deref_roundtrip() {
        let mut ctx = TypeckContext::new();
        // *(&x) where x: I64 — full roundtrip back to I64
        ctx.local_env.insert(VarId(0), Type::I64);
        let mut expr = mk_deref(mk_ref(mk_var(0)));
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 4: Method Engine — Inherent Impls
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_typeck_method_call_happy_path() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Player", vec![("health", Type::I64)]);
        // heal(self: Player, amount: I64) -> Player
        ctx.register_method(
            "Player", "heal",
            vec![Type::Struct("Player".into()), Type::I64],
            Type::Struct("Player".into()),
        );

        // Simulate: player.heal(10) where player: Player
        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let mut expr = mk_method_call(mk_var(0), "heal", vec![mk_int(10)]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Struct("Player".into()));
        assert_eq!(expr.ty, Type::Struct("Player".into()));
    }

    #[test]
    fn test_typeck_method_call_arity_mismatch() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Player", vec![("health", Type::I64)]);
        // heal(self: Player, amount: I64) -> Player
        ctx.register_method(
            "Player", "heal",
            vec![Type::Struct("Player".into()), Type::I64],
            Type::Struct("Player".into()),
        );

        // player.heal() — missing the amount argument
        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let mut expr = mk_method_call(mk_var(0), "heal", vec![]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Arity mismatch"), "got: {}", err);
        assert!(err.contains("expected 1"), "got: {}", err);
        assert!(err.contains("found 0"), "got: {}", err);
    }

    #[test]
    fn test_typeck_method_call_arg_type_mismatch() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Player", vec![("health", Type::I64)]);
        // heal(self: Player, amount: I64) -> Player
        ctx.register_method(
            "Player", "heal",
            vec![Type::Struct("Player".into()), Type::I64],
            Type::Struct("Player".into()),
        );

        // player.heal(true) — Bool where I64 expected
        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let mut expr = mk_method_call(mk_var(0), "heal", vec![mk_bool(true)]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Type mismatch in argument 0"), "got: {}", err);
        assert!(err.contains("I64"), "got: {}", err);
        assert!(err.contains("Bool"), "got: {}", err);
    }

    #[test]
    fn test_typeck_method_call_unknown_method() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Player", vec![("health", Type::I64)]);
        // No methods registered

        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let mut expr = mk_method_call(mk_var(0), "fly", vec![]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no method named 'fly'"));
    }

    #[test]
    fn test_typeck_method_call_on_non_struct() {
        let mut ctx = TypeckContext::new();

        // 42.heal(10) — cannot call method on I64
        let mut expr = mk_method_call(mk_int(42), "heal", vec![mk_int(10)]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("non-struct type"), "got: {}", err);
        assert!(err.contains("I64"), "got: {}", err);
    }

    #[test]
    fn test_typeck_method_call_ref_receiver_exact_match() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Player", vec![("health", Type::I64)]);
        // get_health(self: &Player) -> I64
        // The method expects a Reference receiver — no auto-ref.
        ctx.register_method(
            "Player", "get_health",
            vec![Type::Reference(Box::new(Type::Struct("Player".into())), false), ],
            Type::I64,
        );

        // player.get_health() where player: Player (not &Player)
        // This must FAIL because we enforce exact type matching.
        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let mut expr = mk_method_call(mk_var(0), "get_health", vec![]);
        let result = ctx.typeck_expr(&mut expr);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("expects self parameter"), "got: {}", err);
    }

    #[test]
    fn test_typeck_method_return_type_propagation() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Player", vec![("health", Type::I64)]);
        // is_alive(self: Player) -> Bool
        ctx.register_method(
            "Player", "is_alive",
            vec![Type::Struct("Player".into())],
            Type::Bool,
        );

        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let mut expr = mk_method_call(mk_var(0), "is_alive", vec![]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Bool);
        assert_eq!(expr.ty, Type::Bool);
    }

    #[test]
    fn test_typeck_method_chained_with_field() {
        let mut ctx = TypeckContext::new();
        ctx.register_struct("Player", vec![("health", Type::I64)]);
        // clone_player(self: Player) -> Player
        ctx.register_method(
            "Player", "clone_player",
            vec![Type::Struct("Player".into())],
            Type::Struct("Player".into()),
        );

        // player.clone_player().health — method returns Player, then .health -> I64
        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let method = mk_method_call(mk_var(0), "clone_player", vec![]);
        let mut expr = mk_field(method, "health");
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 4.5: Impl Extraction Pass — with_items() Injection
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_typeck_with_items_impl_method_injection() {
        use crate::hir::ids::DefId;
        use crate::hir::items::*;
        use crate::hir::expr::Block;

        // struct Player { health: I64 }
        let struct_item = Item {
            id: DefId(0),
            name: "Player".into(),
            vis: Visibility::Public,
            kind: ItemKind::Struct(Struct {
                fields: vec![
                    Field { name: "health".into(), ty: Type::I64, vis: Visibility::Public },
                ],
                generics: Generics::default(),
                invariants: vec![],
            }),
            span: proc_macro2::Span::call_site(),
        };

        // impl Player { fn heal(self: Player, amount: I64) -> Player { ... } }
        let impl_item = Item {
            id: DefId(1),
            name: String::new(),
            vis: Visibility::Public,
            kind: ItemKind::Impl(Impl {
                generics: Generics::default(),
                trait_ref: None,
                self_ty: Type::Struct("Player".into()),
                items: vec![
                    ImplItem::Fn {
                        name: "heal".into(),
                        func: crate::hir::items::Fn {
                            inputs: vec![
                                Param { name: "self".into(), ty: Type::Struct("Player".into()) },
                                Param { name: "amount".into(), ty: Type::I64 },
                            ],
                            output: Type::Struct("Player".into()),
                            body: Some(Block { stmts: vec![], value: None, ty: Type::Unit }),
                            generics: Generics::default(),
                            is_async: false,
                        },
                    },
                ],
            }),
            span: proc_macro2::Span::call_site(),
        };

        // Build context from items — impl block AFTER struct (order independence test)
        let mut ctx = TypeckContext::with_items(&[struct_item, impl_item]);

        // player.heal(10) should resolve via the injected method
        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let mut expr = mk_method_call(mk_var(0), "heal", vec![mk_int(10)]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Struct("Player".into()));
    }

    #[test]
    fn test_typeck_with_items_impl_before_struct() {
        use crate::hir::ids::DefId;
        use crate::hir::items::*;
        use crate::hir::expr::Block;

        // impl Player { fn is_alive(self: Player) -> Bool { ... } }
        let impl_item = Item {
            id: DefId(0),
            name: String::new(),
            vis: Visibility::Public,
            kind: ItemKind::Impl(Impl {
                generics: Generics::default(),
                trait_ref: None,
                self_ty: Type::Struct("Player".into()),
                items: vec![
                    ImplItem::Fn {
                        name: "is_alive".into(),
                        func: crate::hir::items::Fn {
                            inputs: vec![
                                Param { name: "self".into(), ty: Type::Struct("Player".into()) },
                            ],
                            output: Type::Bool,
                            body: Some(Block { stmts: vec![], value: None, ty: Type::Unit }),
                            generics: Generics::default(),
                            is_async: false,
                        },
                    },
                ],
            }),
            span: proc_macro2::Span::call_site(),
        };

        // struct Player { health: I64 }  — AFTER the impl block
        let struct_item = Item {
            id: DefId(1),
            name: "Player".into(),
            vis: Visibility::Public,
            kind: ItemKind::Struct(Struct {
                fields: vec![
                    Field { name: "health".into(), ty: Type::I64, vis: Visibility::Public },
                ],
                generics: Generics::default(),
                invariants: vec![],
            }),
            span: proc_macro2::Span::call_site(),
        };

        // Build context: impl FIRST, struct SECOND — tests order-independence
        let mut ctx = TypeckContext::with_items(&[impl_item, struct_item]);

        // player.is_alive() should still resolve
        ctx.local_env.insert(VarId(0), Type::Struct("Player".into()));
        let mut expr = mk_method_call(mk_var(0), "is_alive", vec![]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Bool);
    }

    #[test]
    fn test_typeck_with_items_multiple_methods() {
        use crate::hir::ids::DefId;
        use crate::hir::items::*;
        use crate::hir::expr::Block;

        let struct_item = Item {
            id: DefId(0),
            name: "Counter".into(),
            vis: Visibility::Public,
            kind: ItemKind::Struct(Struct {
                fields: vec![
                    Field { name: "value".into(), ty: Type::I64, vis: Visibility::Public },
                ],
                generics: Generics::default(),
                invariants: vec![],
            }),
            span: proc_macro2::Span::call_site(),
        };

        // impl Counter { fn inc(self, n) -> Counter; fn get(self) -> I64 }
        let impl_item = Item {
            id: DefId(1),
            name: String::new(),
            vis: Visibility::Public,
            kind: ItemKind::Impl(Impl {
                generics: Generics::default(),
                trait_ref: None,
                self_ty: Type::Struct("Counter".into()),
                items: vec![
                    ImplItem::Fn {
                        name: "inc".into(),
                        func: crate::hir::items::Fn {
                            inputs: vec![
                                Param { name: "self".into(), ty: Type::Struct("Counter".into()) },
                                Param { name: "n".into(), ty: Type::I64 },
                            ],
                            output: Type::Struct("Counter".into()),
                            body: Some(Block { stmts: vec![], value: None, ty: Type::Unit }),
                            generics: Generics::default(),
                            is_async: false,
                        },
                    },
                    ImplItem::Fn {
                        name: "get".into(),
                        func: crate::hir::items::Fn {
                            inputs: vec![
                                Param { name: "self".into(), ty: Type::Struct("Counter".into()) },
                            ],
                            output: Type::I64,
                            body: Some(Block { stmts: vec![], value: None, ty: Type::Unit }),
                            generics: Generics::default(),
                            is_async: false,
                        },
                    },
                ],
            }),
            span: proc_macro2::Span::call_site(),
        };

        let mut ctx = TypeckContext::with_items(&[struct_item, impl_item]);
        ctx.local_env.insert(VarId(0), Type::Struct("Counter".into()));

        // counter.inc(5) — should be Counter
        let mut inc_expr = mk_method_call(mk_var(0), "inc", vec![mk_int(5)]);
        let inc_ty = ctx.typeck_expr(&mut inc_expr).unwrap();
        assert_eq!(inc_ty, Type::Struct("Counter".into()));

        // counter.get() — should be I64
        let mut get_expr = mk_method_call(mk_var(0), "get", vec![]);
        let get_ty = ctx.typeck_expr(&mut get_expr).unwrap();
        assert_eq!(get_ty, Type::I64);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 5: Monomorphization Engine
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn test_monomorphize_simple_wrapper() {
        // struct Wrapper<T> { value: T }
        let mut ctx = TypeckContext::new();
        ctx.register_generic_struct(
            "Wrapper", vec!["T"],
            vec![("value", Type::Generic("T".into()))],
        );

        // Wrapper<I64> { value: 5 }
        let mut expr = mk_generic_struct_lit(
            "Wrapper", vec![Type::I64],
            vec![("value", mk_int(5))],
        );
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Struct("Wrapper_I64".into()));
    }

    #[test]
    fn test_monomorphize_pair() {
        // struct Pair<A, B> { first: A, second: B }
        let mut ctx = TypeckContext::new();
        ctx.register_generic_struct(
            "Pair", vec!["A", "B"],
            vec![
                ("first", Type::Generic("A".into())),
                ("second", Type::Generic("B".into())),
            ],
        );

        // Pair<I64, Bool> { first: 1, second: true }
        let mut expr = mk_generic_struct_lit(
            "Pair", vec![Type::I64, Type::Bool],
            vec![("first", mk_int(1)), ("second", mk_bool(true))],
        );
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Struct("Pair_I64_Bool".into()));
    }

    #[test]
    fn test_monomorphize_nested_generic() {
        // struct Wrapper<T> { value: T }
        let mut ctx = TypeckContext::new();
        ctx.register_generic_struct(
            "Wrapper", vec!["T"],
            vec![("value", Type::Generic("T".into()))],
        );
        // Also register a concrete Inner struct
        ctx.register_struct("Inner", vec![("x", Type::I64)]);

        // Wrapper<Inner> { value: Inner { x: 1 } }
        let inner_lit = mk_struct_lit("Inner", vec![("x", mk_int(1))]);
        let mut expr = mk_generic_struct_lit(
            "Wrapper", vec![Type::Struct("Inner".into())],
            vec![("value", inner_lit)],
        );
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Struct("Wrapper_Struct(\"Inner\")".into()));
    }

    #[test]
    fn test_monomorphize_field_access() {
        // struct Wrapper<T> { value: T }
        let mut ctx = TypeckContext::new();
        ctx.register_generic_struct(
            "Wrapper", vec!["T"],
            vec![("value", Type::Generic("T".into()))],
        );

        // let w = Wrapper<I64> { value: 42 }
        let lit = mk_generic_struct_lit(
            "Wrapper", vec![Type::I64],
            vec![("value", mk_int(42))],
        );
        // w.value should resolve to I64
        let mut expr = mk_field(lit, "value");
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::I64);
    }

    #[test]
    fn test_monomorphize_type_mismatch() {
        // struct Wrapper<T> { value: T }
        let mut ctx = TypeckContext::new();
        ctx.register_generic_struct(
            "Wrapper", vec!["T"],
            vec![("value", Type::Generic("T".into()))],
        );

        // Wrapper<I64> { value: true } — should fail: Bool != I64
        let mut expr = mk_generic_struct_lit(
            "Wrapper", vec![Type::I64],
            vec![("value", mk_bool(true))],
        );
        let err = ctx.typeck_expr(&mut expr).unwrap_err();
        assert!(err.contains("Type mismatch"), "Expected type mismatch error, got: {}", err);
    }

    #[test]
    fn test_monomorphize_arity_mismatch() {
        // struct Wrapper<T> { value: T }  — 1 type param
        let mut ctx = TypeckContext::new();
        ctx.register_generic_struct(
            "Wrapper", vec!["T"],
            vec![("value", Type::Generic("T".into()))],
        );

        // Wrapper<I64, Bool> { value: 5 } — too many type args
        let mut expr = mk_generic_struct_lit(
            "Wrapper", vec![Type::I64, Type::Bool],
            vec![("value", mk_int(5))],
        );
        let err = ctx.typeck_expr(&mut expr).unwrap_err();
        assert!(err.contains("Generic arity mismatch"), "Expected arity error, got: {}", err);
    }

    #[test]
    fn test_monomorphize_with_methods() {
        // struct Container<T> { value: T }
        let mut ctx = TypeckContext::new();
        ctx.register_generic_struct(
            "Container", vec!["T"],
            vec![("value", Type::Generic("T".into()))],
        );
        // Add generic method: get(self: Container<T>) -> T
        ctx.structs.get_mut("Container").unwrap().methods.insert(
            "get".into(),
            FnSig {
                params: vec![Type::Struct("Container".into())],
                return_type: Type::Generic("T".into()),
                linear_params: vec![false],
            },
        );

        // Monomorphize: Container<I64>
        let mangled = ctx.monomorphize_struct("Container", &[Type::I64]).unwrap();
        assert_eq!(mangled, "Container_I64");

        // The monomorphized struct should have get() -> I64
        let mono_def = ctx.structs.get(&mangled).unwrap();
        let get_sig = mono_def.methods.get("get").unwrap();
        assert_eq!(get_sig.return_type, Type::I64);
    }

    #[test]
    fn test_monomorphize_caches_second_use() {
        // struct Wrapper<T> { value: T }
        let mut ctx = TypeckContext::new();
        ctx.register_generic_struct(
            "Wrapper", vec!["T"],
            vec![("value", Type::Generic("T".into()))],
        );

        // First call creates the monomorphized struct
        let name1 = ctx.monomorphize_struct("Wrapper", &[Type::I64]).unwrap();
        let struct_count_after_first = ctx.structs.len();

        // Second call should hit the cache — no new struct created
        let name2 = ctx.monomorphize_struct("Wrapper", &[Type::I64]).unwrap();
        assert_eq!(name1, name2);
        assert_eq!(ctx.structs.len(), struct_count_after_first);
    }

    #[test]
    fn test_monomorphize_with_items() {
        use crate::hir::ids::DefId;
        use crate::hir::items::*;

        // struct Wrapper<T> { value: T }
        let struct_item = Item {
            id: DefId(0),
            name: "Wrapper".into(),
            vis: Visibility::Public,
            kind: ItemKind::Struct(Struct {
                fields: vec![
                    Field { name: "value".into(), ty: Type::Generic("T".into()), vis: Visibility::Public },
                ],
                generics: Generics {
                    params: vec![GenericParam::Type("T".into())],
                },
                invariants: vec![],
            }),
            span: proc_macro2::Span::call_site(),
        };

        // Build context from items
        let mut ctx = TypeckContext::with_items(&[struct_item]);

        // Wrapper<Bool> { value: true }
        let mut expr = mk_generic_struct_lit(
            "Wrapper", vec![Type::Bool],
            vec![("value", mk_bool(true))],
        );
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Struct("Wrapper_Bool".into()));
    }

    // ═════════════════════════════════════════════════════════════════════
    // Phase 6: Z3 Verification Engine — Track A (Contract Nodes)
    // ═════════════════════════════════════════════════════════════════════

    fn mk_requires(cond: Expr) -> Expr {
        mk_expr(ExprKind::Requires(Box::new(cond)))
    }

    fn mk_ensures(cond: Expr) -> Expr {
        mk_expr(ExprKind::Ensures(Box::new(cond)))
    }

    #[test]
    fn test_requires_bool_passes() {
        // requires(x < 10) where x: I64
        let mut ctx = TypeckContext::new();
        ctx.local_env.insert(VarId(0), Type::I64);

        let cond = mk_expr(ExprKind::Binary {
            op: BinOp::Lt,
            lhs: Box::new(mk_var(0)),
            rhs: Box::new(mk_int(10)),
        });
        let mut expr = mk_requires(cond);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Unit);
    }

    #[test]
    fn test_requires_non_bool_fails() {
        // requires(5 + 10) — I64, not Bool
        let mut ctx = TypeckContext::new();

        let arith = mk_expr(ExprKind::Binary {
            op: BinOp::Add,
            lhs: Box::new(mk_int(5)),
            rhs: Box::new(mk_int(10)),
        });
        let mut expr = mk_requires(arith);
        let err = ctx.typeck_expr(&mut expr).unwrap_err();
        assert!(err.contains("Contract condition must be Bool"), "Got: {}", err);
    }

    #[test]
    fn test_ensures_bool_passes() {
        // ensures(result > 0) where result: I64
        let mut ctx = TypeckContext::new();
        ctx.local_env.insert(VarId(0), Type::I64);

        let cond = mk_expr(ExprKind::Binary {
            op: BinOp::Gt,
            lhs: Box::new(mk_var(0)),
            rhs: Box::new(mk_int(0)),
        });
        let mut expr = mk_ensures(cond);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Unit);
    }

    #[test]
    fn test_ensures_non_bool_fails() {
        // ensures(42) — literal I64, not Bool
        let mut ctx = TypeckContext::new();

        let mut expr = mk_ensures(mk_int(42));
        let err = ctx.typeck_expr(&mut expr).unwrap_err();
        assert!(err.contains("Contract condition must be Bool"), "Got: {}", err);
    }

    #[test]
    fn test_requires_in_block() {
        use crate::hir::stmt::{Stmt, StmtKind, Pattern, Local};

        let mut ctx = TypeckContext::new();

        // let x: I64 = 5;
        let let_stmt = Stmt {
            kind: StmtKind::Local(Local {
                pat: Pattern::Bind { name: "x".into(), var_id: VarId(0), mutable: false },
                ty: Some(Type::I64),
                init: Some(mk_int(5)),
            }),
            span: proc_macro2::Span::call_site(),
        };

        // requires(x < 10)
        let cond = mk_expr(ExprKind::Binary {
            op: BinOp::Lt,
            lhs: Box::new(mk_var(0)),
            rhs: Box::new(mk_int(10)),
        });
        let req_stmt = Stmt {
            kind: StmtKind::Semi(mk_requires(cond)),
            span: proc_macro2::Span::call_site(),
        };

        let mut block = Block {
            stmts: vec![let_stmt, req_stmt],
            value: None,
            ty: Type::Unit,
        };

        let block_ty = ctx.typeck_block(&mut block).unwrap();
        assert_eq!(block_ty, Type::Unit);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Trait Engine: Compliance Verification
    // ═════════════════════════════════════════════════════════════════════

    /// Helper: build Item nodes for a trait + struct + trait impl for testing
    fn mk_trait_item(name: &str, methods: Vec<(&str, Vec<Type>, Type)>) -> Item {
        use crate::hir::ids::DefId;
        use crate::hir::items::*;

        let items: Vec<TraitItem> = methods.into_iter().map(|(mname, params, ret)| {
            TraitItem::Fn {
                name: mname.to_string(),
                func: crate::hir::items::Fn {
                    inputs: params.into_iter().map(|ty| Param { name: "arg".into(), ty }).collect(),
                    output: ret,
                    body: None,
                    generics: Generics::default(),
                    is_async: false,
                },
            }
        }).collect();

        Item {
            id: DefId(100),
            name: name.to_string(),
            vis: Visibility::Public,
            kind: ItemKind::Trait(Trait { generics: Generics::default(), items }),
            span: proc_macro2::Span::call_site(),
        }
    }

    fn mk_struct_item(name: &str, fields: Vec<(&str, Type)>) -> Item {
        use crate::hir::ids::DefId;
        use crate::hir::items::*;

        Item {
            id: DefId(200),
            name: name.to_string(),
            vis: Visibility::Public,
            kind: ItemKind::Struct(Struct {
                fields: fields.into_iter().map(|(n, ty)| Field { name: n.into(), ty, vis: Visibility::Public }).collect(),
                generics: Generics::default(),
                invariants: vec![],
            }),
            span: proc_macro2::Span::call_site(),
        }
    }

    fn mk_impl_trait_item(trait_name: &str, struct_name: &str, methods: Vec<(&str, Vec<Type>, Type)>) -> Item {
        use crate::hir::ids::DefId;
        use crate::hir::items::*;

        let items: Vec<ImplItem> = methods.into_iter().map(|(mname, params, ret)| {
            ImplItem::Fn {
                name: mname.to_string(),
                func: crate::hir::items::Fn {
                    inputs: params.into_iter().map(|ty| Param { name: "arg".into(), ty }).collect(),
                    output: ret,
                    body: Some(Block { stmts: vec![], value: None, ty: Type::Unit }),
                    generics: Generics::default(),
                    is_async: false,
                },
            }
        }).collect();

        Item {
            id: DefId(300),
            name: trait_name.to_string(),
            vis: Visibility::Public,
            kind: ItemKind::Impl(Impl {
                generics: Generics::default(),
                trait_ref: Some(Type::Struct(trait_name.to_string())),
                self_ty: Type::Struct(struct_name.to_string()),
                items,
            }),
            span: proc_macro2::Span::call_site(),
        }
    }

    #[test]
    fn test_trait_registration() {
        // trait Write { fn write(&self, data: I64) -> Bool; }
        let trait_item = mk_trait_item("Write", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);
        let ctx = TypeckContext::with_items(&[trait_item]);

        assert!(ctx._traits.contains_key("Write"));
        let t = &ctx._traits["Write"];
        assert_eq!(t.required_methods.len(), 1);
        assert!(t.required_methods.contains_key("write"));
        let sig = &t.required_methods["write"];
        assert_eq!(sig.params, vec![Type::SelfType, Type::I64]);
        assert_eq!(sig.return_type, Type::Bool);
    }

    #[test]
    fn test_trait_impl_compliant() {
        // trait Write { fn write(&self, data: I64) -> Bool; }
        // struct Console { fd: I64 }
        // impl Write for Console { fn write(&self, data: I64) -> Bool { ... } }
        let trait_item = mk_trait_item("Write", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);
        let struct_item = mk_struct_item("Console", vec![("fd", Type::I64)]);
        let impl_item = mk_impl_trait_item("Write", "Console", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);

        let ctx = TypeckContext::with_items(&[trait_item, struct_item, impl_item]);
        assert!(ctx.errors.is_empty(), "Expected no errors, got: {:?}", ctx.errors);
    }

    #[test]
    fn test_trait_impl_missing_method() {
        // trait Write { fn write(&self, data: I64) -> Bool; }
        // struct Console {}
        // impl Write for Console {} — missing write!
        let trait_item = mk_trait_item("Write", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);
        let struct_item = mk_struct_item("Console", vec![]);
        let impl_item = mk_impl_trait_item("Write", "Console", vec![]); // empty!

        let ctx = TypeckContext::with_items(&[trait_item, struct_item, impl_item]);
        assert!(!ctx.errors.is_empty());
        assert!(ctx.errors[0].contains("Missing required method 'write'"), "Got: {}", ctx.errors[0]);
    }

    #[test]
    fn test_trait_impl_wrong_return_type() {
        // trait Write { fn write(&self, data: I64) -> Bool; }
        // impl Write for Console { fn write(&self, data: I64) -> I64 { ... } } — wrong return
        let trait_item = mk_trait_item("Write", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);
        let struct_item = mk_struct_item("Console", vec![]);
        let impl_item = mk_impl_trait_item("Write", "Console", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::I64), // I64 != Bool
        ]);

        let ctx = TypeckContext::with_items(&[trait_item, struct_item, impl_item]);
        assert!(!ctx.errors.is_empty());
        assert!(ctx.errors[0].contains("Signature mismatch"), "Got: {}", ctx.errors[0]);
    }

    #[test]
    fn test_trait_impl_wrong_param_count() {
        // trait Write { fn write(&self, data: I64) -> Bool; }
        // impl Write for Console { fn write(&self) -> Bool { ... } } — missing param
        let trait_item = mk_trait_item("Write", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);
        let struct_item = mk_struct_item("Console", vec![]);
        let impl_item = mk_impl_trait_item("Write", "Console", vec![
            ("write", vec![Type::SelfType], Type::Bool), // missing I64 param
        ]);

        let ctx = TypeckContext::with_items(&[trait_item, struct_item, impl_item]);
        assert!(!ctx.errors.is_empty());
        assert!(ctx.errors[0].contains("Signature mismatch"), "Got: {}", ctx.errors[0]);
    }

    #[test]
    fn test_trait_impl_injects_methods() {
        // After compliance, console.write(42) should be callable
        let trait_item = mk_trait_item("Write", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);
        let struct_item = mk_struct_item("Console", vec![("fd", Type::I64)]);
        let impl_item = mk_impl_trait_item("Write", "Console", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);

        let mut ctx = TypeckContext::with_items(&[trait_item, struct_item, impl_item]);
        assert!(ctx.errors.is_empty(), "Errors: {:?}", ctx.errors);

        // console.write(42) should resolve
        ctx.local_env.insert(VarId(0), Type::Struct("Console".into()));
        let mut expr = mk_method_call(mk_var(0), "write", vec![mk_int(42)]);
        let ty = ctx.typeck_expr(&mut expr).unwrap();
        assert_eq!(ty, Type::Bool);
    }

    #[test]
    fn test_trait_self_type_substitution() {
        // trait Clone { fn clone(&self) -> Self; }
        // We verify that SelfType in both trait and impl aligns
        let trait_item = mk_trait_item("Cloneable", vec![
            ("clone_it", vec![Type::SelfType], Type::SelfType),
        ]);
        let struct_item = mk_struct_item("Widget", vec![]);
        let impl_item = mk_impl_trait_item("Cloneable", "Widget", vec![
            ("clone_it", vec![Type::SelfType], Type::SelfType),
        ]);

        let ctx = TypeckContext::with_items(&[trait_item, struct_item, impl_item]);
        assert!(ctx.errors.is_empty(), "Errors: {:?}", ctx.errors);

        // Widget should now have clone_it method
        let widget_def = &ctx.structs["Widget"];
        assert!(widget_def.methods.contains_key("clone_it"));
    }

    #[test]
    fn test_multiple_traits_same_struct() {
        // trait Read { fn read(&self) -> I64; }
        // trait Write { fn write(&self, data: I64) -> Bool; }
        // impl Read for Socket { ... }
        // impl Write for Socket { ... }
        let read_trait = mk_trait_item("Read", vec![
            ("read", vec![Type::SelfType], Type::I64),
        ]);
        let write_trait = mk_trait_item("Write", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);
        let struct_item = mk_struct_item("Socket", vec![("fd", Type::I64)]);
        let read_impl = mk_impl_trait_item("Read", "Socket", vec![
            ("read", vec![Type::SelfType], Type::I64),
        ]);
        let write_impl = mk_impl_trait_item("Write", "Socket", vec![
            ("write", vec![Type::SelfType, Type::I64], Type::Bool),
        ]);

        let ctx = TypeckContext::with_items(&[
            read_trait, write_trait, struct_item, read_impl, write_impl,
        ]);
        assert!(ctx.errors.is_empty(), "Errors: {:?}", ctx.errors);

        let socket_def = &ctx.structs["Socket"];
        assert!(socket_def.methods.contains_key("read"));
        assert!(socket_def.methods.contains_key("write"));
    }

    #[test]
    fn test_trait_unknown_error() {
        // impl UnknownTrait for Console { ... }
        let struct_item = mk_struct_item("Console", vec![]);
        let impl_item = mk_impl_trait_item("UnknownTrait", "Console", vec![
            ("foo", vec![], Type::Unit),
        ]);

        let ctx = TypeckContext::with_items(&[struct_item, impl_item]);
        assert!(!ctx.errors.is_empty());
        assert!(ctx.errors[0].contains("Unknown trait: 'UnknownTrait'"), "Got: {}", ctx.errors[0]);
    }

    #[test]
    fn test_lowering_trait_method_names() {
        // Verify that lowered TraitItem carries correct method names
        let trait_item = mk_trait_item("Hashable", vec![
            ("hash", vec![Type::SelfType], Type::U64),
            ("eq", vec![Type::SelfType, Type::SelfType], Type::Bool),
        ]);

        let ctx = TypeckContext::with_items(&[trait_item]);
        let t = &ctx._traits["Hashable"];
        assert!(t.required_methods.contains_key("hash"));
        assert!(t.required_methods.contains_key("eq"));
        assert_eq!(t.required_methods["hash"].return_type, Type::U64);
        assert_eq!(t.required_methods["eq"].return_type, Type::Bool);
    }

    // ═════════════════════════════════════════════════════════════════════
    // Linear Type Consumption Tests (KeuOS ABI Phase 4)
    // ═════════════════════════════════════════════════════════════════════

    /// Helper: register a function where specific params are marked `consume`.
    fn register_linear_fn(
        ctx: &mut TypeckContext,
        name: &str,
        params: Vec<Type>,
        linear_mask: Vec<bool>,
        return_type: Type,
    ) {
        ctx.functions.insert(name.to_string(), FnSig {
            params,
            return_type,
            linear_params: linear_mask,
        });
    }

    #[test]
    fn test_linear_consume_basic() {
        // fn push(ring: I64, payload: consume I64) -> Unit
        // Calling push(ring, payload) should succeed and consume payload.
        let mut ctx = TypeckContext::new();
        let ring_id = VarId(0);
        let payload_id = VarId(1);
        ctx.local_env.insert(ring_id, Type::I64);
        ctx.local_env.insert(payload_id, Type::I64);

        register_linear_fn(
            &mut ctx,
            "push",
            vec![Type::I64, Type::I64],
            vec![false, true],  // second param is consume
            Type::Unit,
        );

        // Build: push(ring, payload)
        let mut call_expr = mk_expr(ExprKind::Call {
            callee: Box::new(mk_expr(ExprKind::UnresolvedIdent("push".into()))),
            args: vec![mk_var(0), mk_var(1)],
        });

        let result = ctx.typeck_expr(&mut call_expr);
        assert!(result.is_ok(), "Basic consume should succeed: {:?}", result);

        // After the call, payload_id should be consumed
        assert!(ctx.consumed_vars.contains(&payload_id),
            "VarId(1) should be consumed after call");
    }

    #[test]
    fn test_use_after_consume() {
        // fn push(ring: I64, payload: consume I64) -> Unit
        // Calling push(ring, payload) then using payload again should fail.
        let mut ctx = TypeckContext::new();
        let ring_id = VarId(0);
        let payload_id = VarId(1);
        ctx.local_env.insert(ring_id, Type::I64);
        ctx.local_env.insert(payload_id, Type::I64);

        register_linear_fn(
            &mut ctx,
            "push",
            vec![Type::I64, Type::I64],
            vec![false, true],
            Type::Unit,
        );

        // Call 1: push(ring, payload) — consumes payload
        let mut call_expr = mk_expr(ExprKind::Call {
            callee: Box::new(mk_expr(ExprKind::UnresolvedIdent("push".into()))),
            args: vec![mk_var(0), mk_var(1)],
        });
        let result = ctx.typeck_expr(&mut call_expr);
        assert!(result.is_ok(), "First call should succeed");

        // Now try to USE payload again (standalone var reference)
        let mut use_expr = mk_var(1);
        let result = ctx.typeck_expr(&mut use_expr);
        assert!(result.is_err(), "Use after consume should fail");
        let err = result.unwrap_err();
        assert!(err.contains("Use after consume"),
            "Error should mention 'Use after consume': {}", err);
    }

    #[test]
    fn test_double_consume() {
        // fn push(ring: I64, payload: consume I64) -> Unit
        // Calling push(ring, payload) twice should fail on the second call.
        let mut ctx = TypeckContext::new();
        let ring_id = VarId(0);
        let payload_id = VarId(1);
        ctx.local_env.insert(ring_id, Type::I64);
        ctx.local_env.insert(payload_id, Type::I64);

        register_linear_fn(
            &mut ctx,
            "push",
            vec![Type::I64, Type::I64],
            vec![false, true],
            Type::Unit,
        );

        // Call 1: push(ring, payload) — consumes payload
        let mut call1 = mk_expr(ExprKind::Call {
            callee: Box::new(mk_expr(ExprKind::UnresolvedIdent("push".into()))),
            args: vec![mk_var(0), mk_var(1)],
        });
        let result = ctx.typeck_expr(&mut call1);
        assert!(result.is_ok(), "First call should succeed");

        // Call 2: push(ring, payload) — payload already consumed
        let mut call2 = mk_expr(ExprKind::Call {
            callee: Box::new(mk_expr(ExprKind::UnresolvedIdent("push".into()))),
            args: vec![mk_var(0), mk_var(1)],
        });
        let result = ctx.typeck_expr(&mut call2);
        assert!(result.is_err(), "Double consume should fail");
        let err = result.unwrap_err();
        assert!(
            err.contains("Use after consume") || err.contains("Double consume"),
            "Error should mention consumption: {}", err
        );
    }
}
