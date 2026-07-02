use crate::types::Type;
use crate::codegen::context::LoweringContext;
use crate::registry::{StructInfo, EnumInfo};
use crate::evaluator::ConstValue;
use std::collections::HashMap;
use crate::codegen::type_bridge::resolve_type;

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    pub fn map_generics(&mut self, generics: &Option<crate::grammar::Generics>, args: &[Type], template_name: &str, old_const_vals: &mut Vec<(String, Option<ConstValue>)>) {

         if let Some(gen) = generics {
             for (i, param) in gen.params.iter().enumerate() {
                 if let Some(concrete) = args.get(i) {
                     let c_t: Type = concrete.clone();
                     let name = match param {
                         crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                         crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                     };
                     if Type::is_protected_name(&name) {
                           panic!("Shadowing Guard: Generic parameter '{}' covers a protected type name in template '{}'", name, template_name);
                      }
                     self.current_type_map_mut().insert(name.clone(), c_t.clone());

                     
                     // Const Generic Injection
                     if let Type::Struct(val_str) = &c_t {
                         if let Ok(int_val) = val_str.parse::<i64>() {
                             let old = self.evaluator.constant_table.insert(name.clone(), ConstValue::Integer(int_val));
                             old_const_vals.push((name, old));
                         }
                     }
                 }
             }
         }
    }

    /// Performs the structural expansion of a template by mapping generic
    /// parameters to concrete arguments and resolving field types.
    /// This is side-effect free w.r.t the struct registry.
    pub fn expand_template_structure(&mut self,
        template_name: &str,
        args: &[Type],
    ) -> Result<StructInfo, String> {
        // 1. Transactional Read: Extract Template Data
        // generics and fields are cloned to free struct_templates for the next level of recursion.
        let templates = self.struct_templates();
        let template = match templates.get(template_name) {
            Some(t) => t.clone(),
            None => return Err(format!("Template '{}' not found in registry.", template_name)),
        };
        let generics = template.generics.clone();
        let fields = template.fields.clone();

        // Fix: Context Swap to Template Definition Scope to prevent Key Drift
        // This makes sure that field resolution (e.g. "GlobalSlabAlloc") happens in the std lib context, NOT the user context.
        let mut _import_guard = None;
        if let Some(registry) = self.config.registry {
             let parts: Vec<&str> = template_name.split("__").collect();
             if parts.len() > 1 {
                 for (pkg_name, mod_info) in &registry.modules {
                      let pkg_mangled = pkg_name.replace(".", "__");
                      let prefix = format!("{}__", pkg_mangled);
                      if template_name.starts_with(&prefix) {
                           let mut combined_imports = mod_info.imports.clone();
                           // Synthesize self-imports ONLY for non-generic types
                           // Generic types (like Vec<T>, SlabCache<SIZE>) should be resolved
                           // via their categorical export metadata which preserves generic_params.
                           {
                                let pkg_prefix_ident = format!("{}__", pkg_mangled);
                                
                                // Only add non-generic struct templates as simple aliases
                                for (s_name, s_def) in &mod_info.struct_templates {
                                     // Skip generic templates - they need explicit instantiation
                                     let has_generics = s_def.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false);
                                     if has_generics {
                                         continue;
                                     }
                                     
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                                
                                // Concrete (non-template) structs can be aliased directly
                                for s_name in mod_info.structs.keys() {
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                           }
                           // Direct import swap (ImportContextGuard expects CodegenContext)
                           let old_imports = std::mem::replace(&mut *self.imports_mut(), combined_imports);
                           _import_guard = Some(old_imports);
                           break; 
                      }
                 }
             }

        }

        // 2. Validate Argument Count
        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
            // Instead of hard error, return placeholder for deferred expansion
            // This handles cases like Vec<T> inside String definition - the T will be
            // substituted later when the actual specialization is requested with concrete args.
            // Only log for debugging, don't fail compilation.

            // Restore imports if they were swapped for template definition scope
            if let Some(old_imports) = _import_guard {
                *self.imports_mut() = old_imports;
            }
            
            // Return a stub StructInfo with the template name - indicates "unspecialized"
            return Ok(StructInfo {
                name: template_name.to_string(),
                fields: std::collections::HashMap::new(),
                field_order: vec![],
                field_alignments: vec![],
                template_name: Some(template_name.to_string()),
                specialization_args: vec![],
            });
        }



        // 3. State Snapshot: Prepare new type mapping
        let old_map = self.current_type_map().clone();
        let old_generic_args = self.current_generic_args().clone();

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

        // 4. Transactional Update: Set the recursion context
        {
            *self.current_type_map_mut() = type_map;
            *self.current_generic_args_mut() = args.to_vec();
        }

        // 5. Recursive Discovery: Map fields in the new context
        let mut resolved_fields = HashMap::new();
        let mut field_order = Vec::new();
        let mut field_alignments = Vec::new();

        for (i, field) in fields.iter().enumerate() {
            // resolve_type is recursive and might access struct_templates/current_type_map
            let mut field_ty = resolve_type(self, &field.ty);

            // Handle @packed attribute
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
        
        // 6. Transactional Restore: Roll back the context
        {
            *self.current_type_map_mut() = old_map;
            *self.current_generic_args_mut() = old_generic_args;
        }
        // Restore imports that were swapped for template definition scope.
        // Without this, the caller's import context is permanently clobbered
        // with the template's module imports (e.g., Slice's 1-import context
        // overwrites main's 21-import context).
        if let Some(old_imports) = _import_guard {
            *self.imports_mut() = old_imports;
        }

        // Phase B: API Surface Discovery (Eager Method Registration)
        let methods = self.find_methods_for_template(template_name);
        for method_name in methods {
             // Skip generic methods. They require inference/turbofish at call site.
             // Registry stores full mangled name in 'name' field with empty path for Struct types.
             let key = crate::types::TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
             
             if let Some((func, _, _)) = self.trait_registry().get_legacy(&key, &method_name) {
                 if let Some(g) = &func.generics {
                     if !g.params.is_empty() {
                         continue; 
                     }
                 }
             } 

             let full_name = format!("{}__{}", template_name, method_name);
             let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
             let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }


        // 7. Return Metadata
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
         // 1. Transactional Read: Extract Enum Template Data
        let (generics, variants) = {
            let templates = self.enum_templates();
            let template = templates.get(template_name)
                .cloned()
                .ok_or_else(|| format!("Enum Template '{}' not found", template_name))?;
            (template.generics.clone(), template.variants.clone())
        };

        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
             return Err(format!("Generic mismatch for enum {}", template_name));
        }

        // 3. State Snapshot
        let old_map = self.current_type_map().clone();
        let old_generic_args = self.current_generic_args().clone();

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

        // 4. Transactional Update: Set recursion context
        {
            *self.current_type_map_mut() = type_map;
            *self.current_generic_args_mut() = args.to_vec();
        }
        
        let mut resolved_variants = Vec::new();
        let mut max_payload_size = 0;
        
        // 5. Recursive Discovery
        for (idx, v) in variants.iter().enumerate() {
             let p_ty: Option<Type> = if v.tys.is_empty() {
                 None
             } else if v.tys.len() == 1 {
                 Some(crate::codegen::type_bridge::resolve_type(self, &v.tys[0]))
             } else {
                 let types: Vec<Type> = v.tys.iter()
                     .map(|sy| crate::codegen::type_bridge::resolve_type(self, sy))
                     .collect();
                 Some(Type::Tuple(types))
             };
             if let Some(ref ty) = p_ty {
                 let size = ty.size_of(self.struct_registry());
                 if size > max_payload_size { max_payload_size = size; }
             }
             resolved_variants.push((v.name.to_string(), p_ty, idx as i32));
        }

        // 6. Transactional Restore
        {
            *self.current_type_map_mut() = old_map;
            *self.current_generic_args_mut() = old_generic_args;
        }

        // Phase B: API Surface Discovery
        let methods = self.find_methods_for_template(template_name);
        for method_name in methods {
             // Skip generic methods. They require inference/turbofish at call site.
             // Registry stores full mangled name in 'name' field with empty path for Struct types.
             let key = crate::types::TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
             

             if let Some((func, _, _)) = self.trait_registry().get_legacy(&key, &method_name) {
                 if let Some(g) = &func.generics {
                     if !g.params.is_empty() {

                         continue; 
                     }
                 }
             }

             let full_name = format!("{}__{}", template_name, method_name);
             let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
             let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }


        Ok(EnumInfo {
            name: self.specialize_template(template_name, args, true)?.mangle(),
            variants: resolved_variants,
            max_payload_size,
            template_name: Some(template_name.to_string()),
            specialization_args: args.to_vec(),
        })
    }

}

