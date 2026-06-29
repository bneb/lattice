#![allow(dead_code)]

use crate::codegen::context::CodegenContext;
use crate::common::mangling::Mangler;
use crate::evaluator::ConstValue;
use crate::grammar::{GenericParam, ImportDecl, SaltElse, SaltFn, Stmt, SynType};
use crate::types::{Type, TypeKey};
use std::collections::BTreeMap;

/// Scan a function body for type references, triggering lazy specialization.
pub fn scan_types_in_fn(ctx: &CodegenContext, func: &SaltFn) -> Result<(), String> {
    for arg in &func.args {
        if let Some(ty) = &arg.ty { ctx.bridge_resolve_type(ty); }
    }
    if let Some(ret) = &func.ret_type { ctx.bridge_resolve_type(ret); }
    for stmt in &func.body.stmts { scan_stmt(ctx, stmt)?; }
    Ok(())
}

// ---- Statement scanning ---------------------------------------------------

fn scan_stmt(ctx: &CodegenContext, stmt: &Stmt) -> Result<(), String> {
    match stmt {
        Stmt::Syn(s) => scan_syn_stmt(ctx, s),
        Stmt::If(f) => scan_if_stmt(ctx, f),
        Stmt::While(w) => { scan_expr(ctx, &w.cond)?; for s in &w.body.stmts { scan_stmt(ctx, s)?; } Ok(()) }
        Stmt::For(f) => {
            if let syn::Expr::Range(r) = &f.iter {
                if let Some(s) = &r.start { scan_expr(ctx, s)?; }
                if let Some(e) = &r.end { scan_expr(ctx, e)?; }
            }
            for s in &f.body.stmts { scan_stmt(ctx, s)?; }
            Ok(())
        }
        Stmt::Expr(e, _) => scan_expr(ctx, e),
        Stmt::Return(e) => { if let Some(expr) = e { scan_expr(ctx, expr)?; } Ok(()) }
        Stmt::MapWindow { addr, body, .. } => { scan_expr(ctx, addr)?; for s in &body.stmts { scan_stmt(ctx, s)?; } Ok(()) }
        Stmt::Unsafe(b) => { for s in &b.stmts { scan_stmt(ctx, s)?; } Ok(()) }
        _ => Ok(()),
    }
}

fn scan_syn_stmt(ctx: &CodegenContext, s: &syn::Stmt) -> Result<(), String> {
    match s {
        syn::Stmt::Local(l) => {
            if let syn::Pat::Type(pt) = &l.pat {
                ctx.bridge_resolve_type(&SynType::from_std(*pt.ty.clone()).map_err(|e| e.to_string())?);
            }
            if let Some(init) = &l.init { scan_expr(ctx, &init.expr)?; }
            Ok(())
        }
        syn::Stmt::Expr(e, _) => scan_expr(ctx, e),
        _ => Ok(()),
    }
}

fn scan_if_stmt(ctx: &CodegenContext, f: &crate::grammar::SaltIf) -> Result<(), String> {
    scan_expr(ctx, &f.cond)?;
    for s in &f.then_branch.stmts { scan_stmt(ctx, s)?; }
    if let Some(eb) = &f.else_branch {
        match eb.as_ref() {
            SaltElse::Block(b) => { for s in &b.stmts { scan_stmt(ctx, s)?; } }
            SaltElse::If(nested) => { scan_stmt(ctx, &Stmt::If(nested.as_ref().clone()))?; }
        }
    }
    Ok(())
}

// ---- Expression scanning --------------------------------------------------

fn scan_expr(ctx: &CodegenContext, expr: &syn::Expr) -> Result<(), String> {
    match expr {
        syn::Expr::Call(c) => scan_expr_call(ctx, c),
        syn::Expr::Struct(s) => {
            let ty_syn = syn::Type::Path(syn::TypePath { qself: None, path: s.path.clone() });
            ctx.bridge_resolve_type(&SynType::from_std(ty_syn).map_err(|e| e.to_string())?);
            for f in &s.fields { scan_expr(ctx, &f.expr)?; }
            Ok(())
        }
        syn::Expr::Cast(c) => {
            scan_expr(ctx, &c.expr)?;
            ctx.bridge_resolve_type(&SynType::from_std(*c.ty.clone()).map_err(|e| e.to_string())?);
            Ok(())
        }
        syn::Expr::Binary(b) => { scan_expr(ctx, &b.left)?; scan_expr(ctx, &b.right)?; Ok(()) }
        syn::Expr::Unary(u) => scan_expr(ctx, &u.expr),
        syn::Expr::Paren(p) => scan_expr(ctx, &p.expr),
        syn::Expr::MethodCall(m) => scan_expr_method_call(ctx, m),
        syn::Expr::Block(b) => {
            for s in &b.block.stmts { scan_stmt(ctx, &Stmt::Syn(s.clone()))?; }
            Ok(())
        }
        _ => Ok(()),
    }
}

// ---- Call expression scanning ---------------------------------------------

fn scan_expr_call(ctx: &CodegenContext, c: &syn::ExprCall) -> Result<(), String> {
    scan_expr(ctx, &c.func)?;
    for a in &c.args { scan_expr(ctx, a)?; }
    if let syn::Expr::Path(p) = &*c.func {
        let mut generic_args = Vec::new();
        for seg in &p.path.segments {
            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                for arg in &args.args { generic_args.push(resolve_generic_arg(ctx, arg)?); }
            }
        }
        if let Ok(target_key) = ctx.resolve_path_to_fqn(&p.path) {
            let _ = resolve_scan_expr_call_fqn(ctx, p, &target_key, &generic_args);
        }
    }
    Ok(())
}

fn resolve_scan_expr_call_fqn(
    ctx: &CodegenContext, p: &syn::ExprPath, target_key: &TypeKey, generic_args: &[Type],
) -> Result<(), String> {
    let full_mangled = target_key.mangle();
    if let Some((template_key, method_name, is_generic)) = scan_resolve_template_method(ctx, target_key) {
        let base_parts: Vec<&str> = template_key.split("__").collect();
        let (b_path, b_name) = if base_parts.len() > 1 {
            (base_parts[..base_parts.len() - 1].iter().map(|s| s.to_string()).collect(),
             base_parts.last().unwrap().to_string())
        } else { (vec![], template_key.clone()) };
        let key_obj = TypeKey { path: b_path, name: b_name, specialization: None };
        if let Some((f, _, _)) = ctx.trait_registry().get_legacy(&key_obj, &method_name) {
            let old_map = scan_setup_type_map(ctx, &f, is_generic, &template_key, generic_args);
            scan_request_specialization(ctx, p, &f, &full_mangled)?;
            *ctx.current_type_map_mut() = old_map;
        }
    }
    if let Some((_, Type::Fn(_, box_ret))) = ctx.resolve_global_signature(&full_mangled) {
        let _ = ctx.bridge_resolve_codegen_type(&box_ret);
    }
    Ok(())
}

fn resolve_generic_arg(ctx: &CodegenContext, arg: &syn::GenericArgument) -> Result<Type, String> {
    match arg {
        syn::GenericArgument::Type(ty) => {
            Ok(ctx.bridge_resolve_type(&SynType::from_std(ty.clone()).map_err(|e| e.to_string())?))
        }
        syn::GenericArgument::Const(expr) => {
            if let Ok(ConstValue::Integer(val)) = ctx.evaluator.borrow_mut().eval_expr(expr) {
                Ok(Type::Struct(val.to_string()))
            } else { Ok(Type::Struct("0".to_string())) }
        }
        _ => Ok(Type::Struct("0".to_string())),
    }
}

fn scan_resolve_template_method(ctx: &CodegenContext, target_key: &TypeKey) -> Option<(String, String, bool)> {
    let full_mangled = target_key.mangle();
    let parts: Vec<&str> = full_mangled.split("__").collect();
    if parts.len() < 2 { return None; }
    let base_name = Mangler::mangle(&parts[..parts.len() - 1]);
    let method_name = parts.last()?.to_string();
    if !ctx.struct_templates().contains_key(&base_name) && !ctx.enum_templates().contains_key(&base_name) {
        return None;
    }
    let is_generic = {
        if let Some(t) = ctx.struct_templates().get(&base_name) {
            t.generics.as_ref().is_some_and(|g| !g.params.is_empty())
        } else if let Some(e) = ctx.enum_templates().get(&base_name) {
            e.generics.as_ref().is_some_and(|g| !g.params.is_empty())
        } else { false }
    };
    Some((base_name, method_name, is_generic))
}

fn scan_setup_type_map(
    ctx: &CodegenContext, f: &SaltFn, is_generic_struct: bool, template_key: &str, generic_args: &[Type],
) -> BTreeMap<String, Type> {
    let old_map = ctx.current_type_map().clone();
    if is_generic_struct {
        if let Some(t) = ctx.struct_templates().get(template_key) {
            if let Some(generics) = &t.generics {
                scan_apply_generic_args(ctx, &generics.params, generic_args);
            }
        } else if let Some(e) = ctx.enum_templates().get(template_key) {
            if let Some(generics) = &e.generics {
                scan_apply_generic_args(ctx, &generics.params, generic_args);
            }
        }
    } else if let Some(generics) = &f.generics {
        scan_apply_generic_args(ctx, &generics.params, generic_args);
    }
    if let Some(rt) = &f.ret_type { let _ = ctx.bridge_resolve_type(rt); }
    let unknown_ty = SynType::Other("UnknownSelf".to_string());
    for a in &f.args { let _ = ctx.bridge_resolve_type(a.ty.as_ref().unwrap_or(&unknown_ty)); }
    old_map
}

fn scan_apply_generic_args(
    ctx: &CodegenContext, params: &syn::punctuated::Punctuated<GenericParam, syn::Token![,]>,
    generic_args: &[Type],
) {
    for (i, param) in params.iter().enumerate() {
        let name = match param {
            GenericParam::Type { name, .. } => name,
            GenericParam::Const { name, .. } => name,
        };
        if let Some(arg) = generic_args.get(i) {
            ctx.current_type_map_mut().insert(name.to_string(), arg.clone());
        }
    }
}

fn scan_request_specialization(
    ctx: &CodegenContext, p: &syn::ExprPath, f: &SaltFn, full_mangled: &str,
) -> Result<(), String> {
    let segments_len = p.path.segments.len();
    if segments_len < 2 { return Ok(()); }
    let mut base_path = p.path.clone();
    let method_seg = base_path.segments.pop()
        .ok_or_else(|| "Failed to pop method segment".to_string())?.into_value();
    let base_ty_syn = syn::Type::Path(syn::TypePath { qself: None, path: base_path });
    let self_ty = ctx.bridge_resolve_type(&SynType::from_std(base_ty_syn).map_err(|e| e.to_string())?);
    let mut concrete_tys: Vec<Type> = Vec::new();
    if let Type::Concrete(_, args) = &self_ty { concrete_tys.extend(args.clone()); }
    if let syn::PathArguments::AngleBracketed(args) = &method_seg.arguments {
        for arg in &args.args { concrete_tys.push(resolve_generic_arg(ctx, arg)?); }
    }
    let method_generic_count = f.generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
    let turbofish_count = match &method_seg.arguments {
        syn::PathArguments::AngleBracketed(args) => args.args.len(),
        _ => 0,
    };
    if turbofish_count >= method_generic_count {
        let _ = ctx.request_specialization(full_mangled, concrete_tys, Some(self_ty));
    }
    Ok(())
}

// ---- Method call scanning -------------------------------------------------

fn scan_expr_method_call(ctx: &CodegenContext, m: &syn::ExprMethodCall) -> Result<(), String> {
    scan_expr(ctx, &m.receiver)?;
    for a in &m.args { scan_expr(ctx, a)?; }
    let Some(recv_ty) = resolve_receiver_scan_helper(ctx, &m.receiver) else { return Ok(()); };
    let method_name = m.method.to_string();
    let method_generics = scan_collect_method_generics(ctx, m)?;
    let Some((func_def, impl_ty, _)) = scan_lookup_method(ctx, &recv_ty, &method_name) else { return Ok(()); };
    let mut concrete_tys = Vec::new();
    if let Type::Concrete(_, args) = &recv_ty { concrete_tys.extend(args.clone()); }
    concrete_tys.extend(method_generics);
    scan_request_method_specialization(ctx, &func_def, impl_ty, &recv_ty, &method_name, concrete_tys, m)
}

fn scan_collect_method_generics(ctx: &CodegenContext, m: &syn::ExprMethodCall) -> Result<Vec<Type>, String> {
    let mut method_generics = Vec::new();
    if let Some(turbofish) = &m.turbofish {
        for arg in &turbofish.args {
            if let syn::GenericArgument::Type(ty) = arg {
                method_generics.push(ctx.bridge_resolve_type(
                    &SynType::from_std(ty.clone()).map_err(|e| e.to_string())?
                ));
            }
        }
    }
    Ok(method_generics)
}

fn scan_lookup_method(
    ctx: &CodegenContext, recv_ty: &Type, method_name: &str,
) -> Option<(crate::grammar::SaltFn, Option<Type>, Vec<ImportDecl>)> {
    let recv_key = recv_ty.to_key()?;
    ctx.trait_registry().get_legacy(&recv_key, method_name).or_else(|| {
        let template_key = recv_key.to_template();
        ctx.trait_registry().get_legacy(&template_key, method_name)
    })
}

fn scan_request_method_specialization(
    ctx: &CodegenContext, func_def: &SaltFn, impl_ty: Option<Type>, recv_ty: &Type,
    method_name: &str, concrete_tys: Vec<Type>, m: &syn::ExprMethodCall,
) -> Result<(), String> {
    let method_generic_count = func_def.generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
    let turbofish_count = m.turbofish.as_ref().map(|t| t.args.len()).unwrap_or(0);
    if turbofish_count < method_generic_count || concrete_tys.is_empty() { return Ok(()); }
    let template_name = scan_determine_template_name(recv_ty, impl_ty);
    let func_name = format!("{}__{}", template_name, method_name);
    let current_map = ctx.current_type_map().clone();
    let substituted_tys: Vec<Type> = concrete_tys.iter().map(|t| t.substitute(&current_map)).collect();
    let substituted_recv = recv_ty.substitute(&current_map);
    let _ = ctx.request_specialization(&func_name, substituted_tys, Some(substituted_recv));
    Ok(())
}

fn scan_determine_template_name(recv_ty: &Type, impl_ty: Option<Type>) -> String {
    if let Type::Concrete(bx, _) = &recv_ty { bx.clone() }
    else if let Type::Struct(bx) = &recv_ty { bx.clone() }
    else if let Some(it) = impl_ty { it.mangle_suffix() }
    else { recv_ty.mangle_suffix() }
}

// ---- Receiver type resolution ---------------------------------------------

fn resolve_receiver_scan_helper(ctx: &CodegenContext, expr: &syn::Expr) -> Option<Type> {
    match expr {
        syn::Expr::Path(p) => resolve_path_receiver(ctx, p),
        syn::Expr::Field(f) => resolve_field_receiver(ctx, f),
        syn::Expr::Paren(p) => resolve_receiver_scan_helper(ctx, &p.expr),
        _ => None,
    }
}

fn resolve_path_receiver(ctx: &CodegenContext, p: &syn::ExprPath) -> Option<Type> {
    let ident = p.path.get_ident()?;
    let name = ident.to_string();
    if name == "self" { return ctx.current_self_ty().clone(); }
    let segments = vec![name.clone()];
    if let Some((pkg, item)) = ctx.bridge_resolve_package_prefix(&segments) {
        let mangled = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) };
        if let Some(ty) = ctx.resolve_global(&mangled) { return Some(ty); }
    }
    if let Some(ty) = ctx.resolve_global(&name) { return Some(ty); }
    if let Some(pkg) = &*ctx.current_package.borrow() {
        let pkg_name = Mangler::mangle(&pkg.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
        let local_global = format!("{}__{}", pkg_name, name);
        if let Some(ty) = ctx.resolve_global(&local_global) { return Some(ty); }
    }
    None
}

fn resolve_field_receiver(ctx: &CodegenContext, f: &syn::ExprField) -> Option<Type> {
    let base_ty = resolve_receiver_scan_helper(ctx, &f.base)?;
    let inner = if let Type::Reference(inner, _) | Type::Owned(inner) = base_ty { *inner } else { base_ty };
    let fname = if let syn::Member::Named(n) = &f.member { n.to_string() } else { return None; };
    if let Type::Struct(name) = &inner {
        return ctx.struct_registry().values()
            .find(|i| i.name == *name)
            .and_then(|info| info.fields.get(&fname))
            .map(|(_, fty)| fty.clone());
    }
    if let Type::Concrete(base, params) = inner {
        let key = TypeKey { path: vec![], name: base, specialization: Some(params) };
        return ctx.struct_registry().get(&key)
            .and_then(|info| info.fields.get(&fname))
            .map(|(_, fty)| fty.clone());
    }
    None
}
