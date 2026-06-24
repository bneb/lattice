use crate::types::{Type, TypeKey};
use crate::codegen::context::LoweringContext;
use crate::registry::{StructInfo, EnumInfo};
use crate::evaluator::ConstValue;
use std::collections::HashMap;
use crate::common::mangling::Mangler;

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    pub fn expand_template_structure(&mut self,
        template_name: &str,
        args: &[Type],
    ) -> Result<StructInfo, String> {
        let (generics, fields) = {
            let template = self.discovery.struct_templates.get(template_name)
                .cloned()
                .ok_or_else(|| format!("Template '{}' not found in registry", template_name))?;
            (template.generics.clone(), template.fields.clone())
        };

        let mut _import_guard = None;
        if let Some(registry) = self.config.registry {
             let parts: Vec<&str> = template_name.split("__").collect();
             if parts.len() > 1 {
                 for (pkg_name, mod_info) in &registry.modules {
                      let pkg_mangled = pkg_name.replace(".", "__");
                      let prefix = format!("{}__", pkg_mangled);
                      if template_name.starts_with(&prefix) {
                           let mut combined_imports = mod_info.imports.clone();
                           {
                                let pkg_prefix_ident = format!("{}__", pkg_mangled);
                                for (s_name, s_def) in &mod_info.struct_templates {
                                     let has_generics = s_def.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false);
                                     if has_generics { continue; }
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                                for (s_name, _) in &mod_info.structs {
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                           }
                           let old_imports = std::mem::replace(&mut self.discovery.imports, combined_imports);
                           _import_guard = Some(old_imports);
                           break; 
                      }
                 }
             }
        }

        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
            if let Some(old_imports) = _import_guard {
                self.discovery.imports = old_imports;
            }
            return Ok(StructInfo {
                name: template_name.to_string(),
                fields: std::collections::HashMap::new(),
                field_order: vec![],
                field_alignments: vec![],
                template_name: Some(template_name.to_string()),
                specialization_args: vec![],
            });
        }

        let old_map = self.expansion.current_type_map.clone();
        let old_generic_args = self.expansion.current_generic_args.clone();
        let mut type_map = old_map.clone();
        
        if let Some(gen) = &generics {
            for (param, arg) in gen.params.iter().zip(args.iter()) {
                 let name = match param {
                     crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                     crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                 };
                 type_map.insert(name, arg.clone());
            }
        }

        {
            self.expansion.current_type_map = type_map;
            self.expansion.current_generic_args = args.to_vec();
        }

        let mut resolved_fields = HashMap::new();
        let mut field_order = Vec::new();
        let mut field_alignments = Vec::new();

        for (i, field) in fields.iter().enumerate() {
            let mut field_ty = super::resolution::resolve_type(self, &field.ty);
            if field.attributes.iter().any(|a| a.name == "packed") {
                 if let Type::Array(inner, len, _) = field_ty {
                      field_ty = Type::Array(inner, len, true);
                 }
            }
            let align = crate::grammar::attr::extract_align(&field.attributes);
            resolved_fields.insert(field.name.to_string(), (i, field_ty.clone()));
            field_order.push(field_ty);
            field_alignments.push(align);
        }
        
        {
            self.expansion.current_type_map = old_map;
            self.expansion.current_generic_args = old_generic_args;
        }
        if let Some(old_imports) = _import_guard {
            self.discovery.imports = old_imports;
        }

        let methods = self.discovery.find_methods_for_type(template_name);
        for method_name in methods {
             let key = TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
             if let Some((func, _, _)) = self.discovery.trait_registry.get_legacy(&key, &method_name) {
                 if let Some(g) = &func.generics {
                     if !g.params.is_empty() { continue; }
                 }
             }
             let full_name = format!("{}__{}", template_name, method_name);
             let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
             let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }

        Ok(StructInfo {
            name: self.specialize_template(template_name, args, false)?.mangle(),
            fields: resolved_fields,
            field_order,
            field_alignments,
            template_name: Some(template_name.to_string()),
            specialization_args: args.to_vec(),
        })
    }

    pub fn expand_enum_structure(&mut self,
        template_name: &str,
        args: &[Type],
    ) -> Result<EnumInfo, String> {
        let (generics, variants) = {
            let template = self.discovery.enum_templates.get(template_name)
                .cloned()
                .ok_or_else(|| format!("Enum template '{}' not found", template_name))?;
            (template.generics.clone(), template.variants.clone())
        };

        let mut _import_guard = None;
        if let Some(registry) = self.config.registry {
            let parts: Vec<&str> = template_name.split("__").collect();
            if parts.len() > 1 {
                for (pkg_name, mod_info) in &registry.modules {
                    let pkg_mangled = pkg_name.replace(".", "__");
                    let prefix = format!("{}__", pkg_mangled);
                    if template_name.starts_with(&prefix) {
                        let combined_imports = mod_info.imports.clone();
                        let old_imports = std::mem::replace(&mut self.discovery.imports, combined_imports);
                        _import_guard = Some(old_imports);
                        break;
                    }
                }
            }
        }

        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
            if let Some(old_imports) = _import_guard {
                self.discovery.imports = old_imports;
            }
            return Ok(EnumInfo {
                name: template_name.to_string(),
                variants: HashMap::new(),
                template_name: Some(template_name.to_string()),
                specialization_args: vec![],
            });
        }

        let old_map = self.expansion.current_type_map.clone();
        let mut type_map = old_map.clone();
        if let Some(gen) = &generics {
            for (param, arg) in gen.params.iter().zip(args.iter()) {
                let name = match param {
                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                };
                type_map.insert(name, arg.clone());
            }
        }

        { self.expansion.current_type_map = type_map; }

        let mut resolved_variants = HashMap::new();
        for (i, variant) in variants.iter().enumerate() {
            let mut variant_fields = Vec::new();
            for field_ty_syn in &variant.fields {
                variant_fields.push(super::resolution::resolve_type(self, field_ty_syn));
            }
            resolved_variants.insert(variant.name.to_string(), (i as u32, variant_fields));
        }

        { self.expansion.current_type_map = old_map; }
        if let Some(old_imports) = _import_guard {
            self.discovery.imports = old_imports;
        }

        let methods = self.discovery.find_methods_for_type(template_name);
        for method_name in methods {
            let key = TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
            if let Some((func, _, _)) = self.discovery.trait_registry.get_legacy(&key, &method_name) {
                if let Some(g) = &func.generics {
                    if !g.params.is_empty() { continue; }
                }
            }
            let full_name = format!("{}__{}", template_name, method_name);
            let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
            let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }

        Ok(EnumInfo {
            name: self.specialize_template(template_name, args, true)?.mangle(),
            variants: resolved_variants,
            template_name: Some(template_name.to_string()),
            specialization_args: args.to_vec(),
        })
    }
}

pub fn zero_attr(ctx: &mut LoweringContext<'_, '_>, ty: &Type) -> Result<String, String> {
    match ty {
        Type::I8 | Type::I16 | Type::I32 | Type::I64 |
        Type::U8 | Type::U16 | Type::U32 | Type::U64 |
        Type::Usize | Type::Bool => {
            let mlir_ty = ty.to_mlir_type(ctx)?;
            Ok(format!("0 : {}", mlir_ty))
        }
        Type::F32 => Ok("0.0 : f32".to_string()),
        Type::F64 => Ok("0.0 : f64".to_string()),
        Type::Reference(_, _) | Type::Pointer { .. } => Ok("null".to_string()),
        Type::Array(inner, len, packed) => {
            if *packed {
                let word_count = (len + 63) / 64;
                let mut words = Vec::new();
                for _ in 0..word_count { words.push("0 : i64"); }
                Ok(format!("[{}]", words.join(", ")))
            } else {
                let inner_zero = zero_attr(ctx, inner)?;
                let mut elements = Vec::new();
                for _ in 0..*len { elements.push(inner_zero.clone()); }
                Ok(format!("[{}]", elements.join(", ")))
            }
        }
        Type::Struct(_) | Type::Concrete(_, _) => {
            let storage = ty.to_mlir_storage_type(ctx)?;
            if storage == "!llvm.ptr" { return Ok("null".to_string()); }
            let info = ctx.struct_registry().values().find(|i| i.name == storage.replace("!struct_", "")).cloned();
            if let Some(inf) = info {
                let mut fields = Vec::new();
                for ft in &inf.field_order { fields.push(zero_attr(ctx, ft)?); }
                Ok(format!("<({})>", fields.join(", ")))
            } else {
                Ok("0 : i64".to_string())
            }
        }
        _ => Ok("0 : i64".to_string()),
    }
}
