// src/codegen/struct_deriver.rs
// Synthetic Method Generator for Stringable::write_to
// Auto-derives structural formatting for user-defined structs

use crate::codegen::context::LoweringContext;
use crate::types::Type;

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    /// Derive a `write_to` implementation for a struct type.
    /// Generates MLIR that prints: "StructName { field1: value1, field2: value2 }"
    /// 
    /// This is the "Structural Introspection" pattern used by Swift's CustomStringConvertible.
    pub fn derive_struct_write_to(&mut self,
        out: &mut String,
        struct_name: &str,
        self_val: &str,
        self_ty: &Type,
        writer_val: &str,
    ) -> Result<(), String> {
        // 1. Get struct field information
        let fields = self.get_struct_fields(struct_name)?;
        
        // 2. Print struct header: "StructName { "
        let header = format!("{} {{ ", struct_name.split("__").last().unwrap_or(struct_name));
        self.emit_derived_print_literal(out, writer_val, &header)?;
        
        // Always spill to a temporary alloca so field access via GEP works.
        // LLVM's mem2reg pass will optimize this away, so it's zero-cost.
        // This avoids fragile string-prefix matching on SSA variable names.
        let id = self.next_id();
        let c1 = format!("%c1_derive_{}", id);
        let alloca = format!("%derive_alloca_{}", id);
        let mlir_struct_ty = self_ty.to_mlir_storage_type(self)?;
        out.push_str(&format!("    {} = arith.constant 1 : i64\n", c1));
        out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", alloca, c1, mlir_struct_ty));
        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", self_val, alloca, mlir_struct_ty));
        let struct_ptr = alloca;

        // 3. Iterate over fields and print each
        for (i, (field_name, field_ty)) in fields.iter().enumerate() {
            // A. Print field label: "field_name: "
            let label = format!("{}: ", field_name);
            self.emit_derived_print_literal(out, writer_val, &label)?;
            
            // B. Extract field value via GEP (use struct_ptr which is always a pointer)
            let uid = self.next_id();
            let field_ptr = format!("%field_ptr_{}_{}", struct_name.replace("::", "_"), uid);
            let mlir_struct_ty = self_ty.to_mlir_storage_type(self)?;
            out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n",
                field_ptr, struct_ptr, i, mlir_struct_ty));
            
            // C. Load field value
            let field_val = format!("%field_val_{}_{}", struct_name.replace("::", "_"), uid);
            let field_mlir_ty = field_ty.to_mlir_type(self)?;
            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n",
                field_val, field_ptr, field_mlir_ty));
            
            // D. Dispatch to appropriate print function based on field type
            self.emit_print_typed(out, &field_val, field_ty)?;
            
            // E. Print separator: ", "
            if i < fields.len() - 1 {
                self.emit_derived_print_literal(out, writer_val, ", ")?;
            }
        }
        
        // 4. Print closing brace: " }"
        self.emit_derived_print_literal(out, writer_val, " }")?;
        
        Ok(())
    }
    
    /// Get the fields of a struct (name, type) pairs.
    /// Uses the struct_templates registry.
    pub(crate) fn get_struct_fields(&mut self, struct_name: &str) -> Result<Vec<(String, Type)>, String> {
        // Try to find in struct_templates
        {
            let templates = self.struct_templates();
            if let Some(def) = templates.get(struct_name) {
                let field_defs: Vec<_> = def.fields.iter()
                    .map(|f| (f.name.to_string(), f.ty.clone()))
                    .collect();
                let _ = templates;
                let mut fields = Vec::new();
                for (field_name, field_ty_ast) in &field_defs {
                    let field_ty = crate::codegen::type_bridge::resolve_type(self, field_ty_ast);
                    fields.push((field_name.clone(), field_ty));
                }
                return Ok(fields);
            }
        }
        
        // Phase 5: Use centralized template lookup
        if let Some(template_name) = self.find_struct_template_by_name(struct_name) {
            let field_defs = {
                let templates = self.struct_templates();
                templates.get(&template_name).map(|def| {
                    def.fields.iter()
                        .map(|f| (f.name.to_string(), f.ty.clone()))
                        .collect::<Vec<_>>()
                })
            };
            if let Some(field_defs) = field_defs {
                let mut fields = Vec::new();
                for (field_name, field_ty_ast) in &field_defs {
                    let field_ty = crate::codegen::type_bridge::resolve_type(self, field_ty_ast);
                    fields.push((field_name.clone(), field_ty));
                }
                return Ok(fields);
            }
        }
        
        Err(format!("Struct '{}' not found in templates for write_to derivation", struct_name))
    }
    
    /// Emit a print literal for derived write_to (uses the same hook as println)
    fn emit_derived_print_literal(&mut self, out: &mut String, _writer: &str, literal: &str) -> Result<(), String> {
        // Register the string in the string_literals buffer
        let global_name = format!("__str_derive_{}", self.next_id());
        let len = literal.len();
        
        // Escape special characters for MLIR string literal
        let escaped = literal
            .replace("\\", "\\\\")
            .replace("\n", "\\n")
            .replace("\"", "\\\"");
        
        // Add to string literals for finalization
        self.string_literals_mut().push((global_name.clone(), escaped.clone(), len));
        
        // Emit addressof and call
        let str_ptr = format!("%derive_str_ptr_{}", self.next_id());
        let str_len = format!("%derive_str_len_{}", self.next_id());
        
        out.push_str(&format!("    {} = llvm.mlir.addressof @{} : !llvm.ptr\n", str_ptr, global_name));
        out.push_str(&format!("    {} = arith.constant {} : i64\n", str_len, len));
        
        self.entity_registry_mut().register_hook("__salt_print_literal");
        out.push_str(&format!("    func.call @__salt_print_literal({}, {}) : (!llvm.ptr, i64) -> ()\n", str_ptr, str_len));
        
        Ok(())
    }
    
    /// Check if a type has a custom write_to implementation.
    /// If not, auto-derivation should be triggered.
    pub fn has_custom_write_to(&mut self, ty: &Type) -> bool {
        // For now, primitives have intrinsic implementations
        match ty {
            Type::I8 | Type::I16 | Type::I32 | Type::I64 |
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::Usize |
            Type::F32 | Type::F64 | Type::Bool => true,
            Type::Reference(_, _) => true,
            // Structs need derivation unless manually implemented
            Type::Struct(_name) | Type::Concrete(_name, _) => {
                // Check TraitRegistry for custom write_to
                let type_key = crate::codegen::type_bridge::type_to_type_key(ty);
                self.trait_registry().contains_method(&type_key, "write_to")
            }
            _ => false,
        }
    }
}
