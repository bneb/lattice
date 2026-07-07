// ============================================================================
// Phase 4: MLIR Header Finalization Tests
// Guards Fixed-Point Buffering and Canonical Alias Generation
//
// [VERIFIED METAL] These tests validate that the EmissionState::finalize()
// method correctly generates canonical MLIR aliases from the TypeIDRegistry.
// ============================================================================

#[cfg(test)]
mod tests {
    use saltc::codegen::phases::emission::{EmissionState, normalize_type_name_for_mlir};
    
    // ============================================================================
    // Test: normalize_type_name_for_mlir
    // ============================================================================
    
    #[test]
    fn test_normalize_basic() {
        assert_eq!(normalize_type_name_for_mlir("TrieNode"), "TrieNode");
    }
    
    #[test]
    fn test_normalize_double_underscore() {
        assert_eq!(normalize_type_name_for_mlir("main__TrieNode"), "main_TrieNode");
    }
    
    #[test]
    fn test_normalize_angle_brackets() {
        assert_eq!(normalize_type_name_for_mlir("NodePtr<TrieNode>"), "NodePtr_TrieNode_");
    }
    
    #[test]
    fn test_normalize_complex() {
        assert_eq!(
            normalize_type_name_for_mlir("std__core__node_ptr__NodePtr<main__Foo>"),
            "std_core_node_ptr_NodePtr_main_Foo_"
        );
    }
    
    #[test]
    fn test_normalize_comma_space() {
        assert_eq!(normalize_type_name_for_mlir("Map<K, V>"), "Map_K_V_");
    }
    
    // ============================================================================
    // Test: TypeIDRegistry integration with EmissionState
    // ============================================================================
    
    #[test]
    fn test_emission_state_default_body_buffer_empty() {
        let state = EmissionState::new();
        assert!(state.get_buffered_body().is_empty());
        assert!(!state.fixed_point_reached);
    }
    
    #[test]
    fn test_emission_state_buffer_body() {
        let mut state = EmissionState::new();
        state.buffer_body("  func.func @test() {\n");
        state.buffer_body("    func.return\n");
        state.buffer_body("  }\n");
        
        let body = state.get_buffered_body();
        assert!(body.contains("func.func @test()"));
        assert!(body.contains("func.return"));
    }
    
    #[test]
    fn test_generate_canonical_aliases_empty_registry() {
        let state = EmissionState::new();
        
        let aliases = state.generate_canonical_aliases(|_| None);
        
        assert!(aliases.contains("Canonical type aliases"));
    }
    
    #[test]
    fn test_generate_canonical_aliases_with_registered_type() {
        let mut state = EmissionState::new();
        
        // Register a type
        let _id = state.type_id_registry.register("TrieNode");
        
        // Generate aliases with a mock lookup
        let aliases = state.generate_canonical_aliases(|name| {
            if name == "TrieNode" {
                Some("!llvm.struct<\"TrieNode\", (!llvm.ptr, i1)>".to_string())
            } else {
                None
            }
        });
        
        assert!(aliases.contains("!struct_TrieNode"));
        assert!(aliases.contains("!llvm.struct<\"TrieNode\""));
    }
    
    #[test]
    fn test_generate_canonical_aliases_normalizes_names() {
        let mut state = EmissionState::new();
        
        // Register a type with double underscores
        let _id = state.type_id_registry.register("main__TrieNode");
        
        // Generate aliases
        let aliases = state.generate_canonical_aliases(|name| {
            if name == "main__TrieNode" {
                Some("!llvm.struct<\"main__TrieNode\", (!llvm.ptr)>".to_string())
            } else {
                None
            }
        });
        
        // Should be normalized to single underscore
        assert!(aliases.contains("!struct_main_TrieNode"), "Should normalize __ to _");
    }
    
    #[test]
    fn test_finalize_integrates_all_components() {
        let mut state = EmissionState::new();
        
        // Register a type
        let _id = state.type_id_registry.register("TestStruct");
        
        // Buffer some body code
        state.buffer_body("  func.func @main() {\n    func.return\n  }\n");
        
        // Finalize with mock lookup
        let output = state.finalize(
            "module {\n",
            |name| {
                if name == "TestStruct" {
                    Some("!llvm.struct<\"TestStruct\", (i32)>".to_string())
                } else {
                    None
                }
            }
        );
        
        // Should contain all parts
        assert!(output.contains("Canonical type aliases"));
        assert!(output.contains("!struct_TestStruct"));
        assert!(output.contains("module {"));
        assert!(output.contains("FUNCTION BODIES"));
        assert!(output.contains("func.func @main()"));
        assert!(state.fixed_point_reached);
    }
    
    #[test]
    fn test_finalize_deduplicates_aliases() {
        let mut state = EmissionState::new();
        
        // Register same type multiple times (shouldn't happen, but test dedup)
        let id1 = state.type_id_registry.register("Foo");
        let id2 = state.type_id_registry.register("Foo");
        assert_eq!(id1, id2, "Same name should return same TypeID");
        
        let output = state.finalize("", |name| {
            if name == "Foo" {
                Some("!llvm.struct<\"Foo\", (i64)>".to_string())
            } else {
                None
            }
        });
        
        // Count occurrences of !struct_Foo =
        let count = output.matches("!struct_Foo =").count();
        assert_eq!(count, 1, "Should only emit alias once");
    }
}
