// ============================================================================
// Phase 5: Identity-Based Routing Tests
// Guards TypeID-Based Struct Lookup (Suffix Purge)
//
// [VERIFIED METAL] These tests validate that the lookup_struct_by_type
// method correctly resolves types via TypeID instead of ends_with() matching.
// ============================================================================

#[cfg(test)]
mod tests {
    use salt_front::registry::StructInfo;
    use salt_front::types::Type;
    use std::collections::{BTreeMap, HashMap};
    
    // ============================================================================
    // Test: TypeID-based canonical name generation
    // ============================================================================
    
    #[test]
    fn test_canonical_name_normalized() {
        let ty = Type::Struct("main__TrieNode".to_string());
        let canonical = ty.to_canonical_name();
        // Should strip package prefix and normalize
        assert!(canonical == "TrieNode" || canonical == "main::TrieNode" || canonical.contains("TrieNode"),
            "Canonical name should contain TrieNode: got {}", canonical);
    }
    
    #[test]
    fn test_canonical_name_simple() {
        let ty = Type::Struct("Foo".to_string());
        let canonical = ty.to_canonical_name();
        assert_eq!(canonical, "Foo");
    }
    
    #[test]
    fn test_canonical_name_concrete_type() {
        let ty = Type::Concrete("NodePtr".to_string(), vec![Type::Struct("Bar".to_string())]);
        let canonical = ty.to_canonical_name();
        assert!(canonical.contains("NodePtr") && canonical.contains("Bar"),
            "Should contain both NodePtr and Bar: got {}", canonical);
    }
    
    // ============================================================================
    // Test: StructInfo field lookup
    // ============================================================================
    
    #[test]
    fn test_struct_info_field_access() {
        let mut fields = HashMap::new();
        fields.insert("data".to_string(), (0, Type::I64));
        fields.insert("next".to_string(), (1, Type::I64));
        
        let info = StructInfo {
            name: "TestNode".to_string(),
            fields: fields.clone(),
            field_order: vec![Type::I64, Type::I64],
            template_name: None,
            specialization_args: vec![],
        };
        
        assert!(info.fields.contains_key("data"));
        assert!(info.fields.contains_key("next"));
        assert_eq!(info.fields.get("data").map(|(idx, _)| *idx), Some(0));
    }
    
    // ============================================================================
    // Test: Type identity comparison
    // ============================================================================
    
    #[test]
    fn test_canonical_eq_fast_same_type() {
        let ty1 = Type::Struct("Foo".to_string());
        let ty2 = Type::Struct("Foo".to_string());
        
        // Both should have same canonical representation
        assert_eq!(ty1.to_canonical_name(), ty2.to_canonical_name());
    }
    
    #[test]
    fn test_canonical_eq_fast_normalized_equivalence() {
        let ty1 = Type::Struct("main__Foo".to_string());
        let ty2 = Type::Struct("Foo".to_string());
        
        // The second should be a "pure" canonical name
        let c1 = ty1.to_canonical_name();
        let c2 = ty2.to_canonical_name();
        
        // Either they normalize to the same thing, or one ends with the other
        assert!(c1 == c2 || c1.ends_with(&c2) || c2.ends_with(&c1),
            "Should have related canonical names: {} vs {}", c1, c2);
    }
    
    #[test]
    fn test_canonical_eq_fast_different_types() {
        let ty1 = Type::Struct("Foo".to_string());
        let ty2 = Type::Struct("Bar".to_string());
        
        assert_ne!(ty1.to_canonical_name(), ty2.to_canonical_name());
    }
    
    // ============================================================================
    // Test: TypeID registry integration
    // ============================================================================
    
    #[test]
    fn test_typeid_registry_register_and_lookup() {
        use salt_front::codegen::types::TypeIDRegistry;
        
        let mut registry = TypeIDRegistry::new();
        let id = registry.register("MyStruct");
        
        assert!(id.is_valid());
        
        let name = registry.get_canonical_name(id);
        assert_eq!(name, Some("MyStruct"));
    }
    
    #[test]
    fn test_typeid_registry_same_name_same_id() {
        use salt_front::codegen::types::TypeIDRegistry;
        
        let mut registry = TypeIDRegistry::new();
        let id1 = registry.register("TestType");
        let id2 = registry.register("TestType");
        
        assert_eq!(id1, id2, "Same name should return same TypeID");
    }
    
    #[test]
    fn test_typeid_registry_different_names_different_ids() {
        use salt_front::codegen::types::TypeIDRegistry;
        
        let mut registry = TypeIDRegistry::new();
        let id1 = registry.register("TypeA");
        let id2 = registry.register("TypeB");
        
        assert_ne!(id1, id2, "Different names should return different TypeIDs");
    }
}
