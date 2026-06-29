use crate::registry::StructInfo;
use crate::types::Type;
use crate::codegen::context::CodegenContext;

impl<'a> CodegenContext<'a> {
    // Phase 4: Body buffer accessors
    pub fn buffer_body(&self, code: &str) {
        self.emission.borrow_mut().buffer_body(code);
    }

    pub fn get_buffered_body(&self) -> String {
        self.emission.borrow().get_buffered_body().to_string()
    }

    /// Phase 4: Look up struct MLIR layout by canonical name
    /// Returns the physical MLIR layout string for a given canonical type name.
    pub fn lookup_struct_layout_by_canonical(&self, canonical_name: &str) -> Option<String> {
        let registry = self.struct_registry();

        // Find struct info where:
        // 1. The name matches exactly, OR
        // 2. The canonical name of the struct matches
        for info in registry.values() {
            let info_canonical = Type::Struct(info.name.clone()).to_canonical_name();
            if info.name == canonical_name || info_canonical == canonical_name {
                // Build the MLIR struct layout
                let layout_parts: Vec<String> = info.field_order.iter()
                    .map(|field_ty| self.resolve_mlir_storage_type(field_ty)
                        .unwrap_or_else(|_| "!llvm.ptr".to_string()))
                    .collect();
                return Some(format!("!llvm.struct<\"{}\", ({})>", info.name, layout_parts.join(", ")));
            }
        }

        None
    }

    /// Phase 4: Finalize MLIR output after all specializations
    pub fn finalize_mlir_output(&self, header: &str) -> String {
        let struct_registry = self.struct_registry();

        // Create lookup closure
        let lookup = |canonical_name: &str| -> Option<String> {
            for info in struct_registry.values() {
                let info_canonical = Type::Struct(info.name.clone()).to_canonical_name();
                if info.name == canonical_name || info_canonical == canonical_name {
                    let mut layout_parts = Vec::new();
                    for field_ty in &info.field_order {
                        let s = self.resolve_mlir_storage_type(field_ty)
                            .unwrap_or_else(|_| "!llvm.ptr".to_string());
                        layout_parts.push(s);
                    }
                    return Some(format!("!llvm.struct<\"{}\", ({})>", info.name, layout_parts.join(", ")));
                }
            }
            None
        };

        // Generate canonical aliases
        let emission = self.emission.borrow();
        let aliases = emission.generate_canonical_aliases(lookup);
        drop(emission);

        let mut final_output = String::new();
        final_output.push_str(&aliases);
        final_output.push_str(header);
        final_output.push_str("\n// --- FUNCTION BODIES ---\n");
        final_output.push_str(&self.get_buffered_body());

        final_output
    }

    /// Phase 5: Identity-Based Struct Lookup by TypeID
    /// Resolves a TypeID to its physical StructInfo with zero string matching.
    ///
    /// This is the core of the suffix-based deduplication - the TypeID (structural hash) is used
    /// to directly locate the exact struct, bypassing all `ends_with()` heuristics.
    pub fn lookup_struct_by_id(&self, id: crate::codegen::types::TypeID) -> Option<StructInfo> {
        let registry = self.type_id_registry();
        let canonical_name = registry.get_canonical_name(id)?;

        // Find the physical struct whose canonical name matches the ID's name
        // This uses Phase 1's normalization logic for matching
        let struct_reg = self.struct_registry();
        struct_reg.values().find(|info| {
            let info_canonical = Type::Struct(info.name.clone()).to_canonical_name();
            info.name == canonical_name || info_canonical == canonical_name
        }).cloned()
    }

    /// Phase 5: Identity-Based Struct Lookup by Type
    /// Convenience method that extracts TypeID from a Type and looks up the StructInfo.
    ///
    /// This is the primary entry point for field access hardening.
    /// Instead of suffix matching, the TypeID is computed and a direct lookup is performed.
    pub fn lookup_struct_by_type(&self, ty: &Type) -> Option<StructInfo> {
        // First, try to resolve via TypeID
        let canonical_name = ty.to_canonical_name();
        if let Some(type_id) = self.type_id_registry().lookup(&canonical_name) {
            if let Some(info) = self.lookup_struct_by_id(type_id) {
                return Some(info);
            }
        }

        // Fallback: direct name lookup (for types not yet in registry)
        let canonical_name = ty.to_canonical_name();
        let struct_reg = self.struct_registry();
        struct_reg.values().find(|info| {
            let info_canonical = Type::Struct(info.name.clone()).to_canonical_name();
            info.name == canonical_name || info_canonical == canonical_name
        }).cloned()
    }

    /// Phase 5: Find struct by name with canonical fallback
    /// This replaces all `ends_with()` heuristics with TypeID-based lookup.
    ///
    /// Priority order:
    /// 1. Exact name match
    /// 2. TypeID canonical lookup
    /// 3. Suffix fallback (shortest match wins)
    pub fn find_struct_by_name(&self, name: &str) -> Option<StructInfo> {
        let struct_reg = self.struct_registry();

        // 1. Exact match
        if let Some(info) = struct_reg.values().find(|i| i.name == name) {
            return Some(info.clone());
        }

        // 2. TypeID canonical lookup
        let ty = Type::Struct(name.to_string());
        if let Some(info) = self.lookup_struct_by_type(&ty) {
            return Some(info);
        }

        // 3. Suffix fallback - pick shortest match (most specific)
        let suffix = format!("__{}", name);
        let mut best_match: Option<StructInfo> = None;
        for info in struct_reg.values() {
            if info.name.ends_with(&suffix)
                && best_match.as_ref().is_none_or(|b| info.name.len() < b.name.len()) {
                    best_match = Some(info.clone());
                }
        }
        best_match
    }

    /// Phase 5: Find template by name with suffix fallback
    /// Returns the template key if found.
    pub fn find_struct_template_by_name(&self, name: &str) -> Option<String> {
        let templates = self.struct_templates();

        // 1. Exact match
        if templates.contains_key(name) {
            return Some(name.to_string());
        }

        // 2. Suffix fallback - pick shortest match
        let suffix = format!("__{}", name);
        let mut best_match: Option<String> = None;
        for k in templates.keys() {
            if k.ends_with(&suffix)
                && best_match.as_ref().is_none_or(|b| k.len() < b.len()) {
                    best_match = Some(k.clone());
                }
        }
        best_match
    }

    /// Phase 5: Find enum template by name with suffix fallback
    pub fn find_enum_template_by_name(&self, name: &str) -> Option<String> {
        let templates = self.enum_templates();

        // 1. Exact match
        if templates.contains_key(name) {
            return Some(name.to_string());
        }

        // 2. Suffix fallback - pick shortest match
        let suffix = format!("__{}", name);
        let mut best_match: Option<String> = None;
        for k in templates.keys() {
            if k.ends_with(&suffix)
                && best_match.as_ref().is_none_or(|b| k.len() < b.len()) {
                    best_match = Some(k.clone());
                }
        }
        best_match
    }
}
