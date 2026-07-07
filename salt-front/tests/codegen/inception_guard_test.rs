// ============================================================================
// Inception Guard Tests
// Guards against recursive pointer wrapping (NodePtr<NodePtr<T>> → NodePtr<T>)
//
// [VERIFIED METAL] These tests validate the Single Indirection Property
// which prevents N² pointer chasing and layout computation cycles.
// ============================================================================

#[cfg(test)]
mod tests {
    use saltc::types::Type;
    use saltc::codegen::type_bridge::{flatten_nested_ptr, extract_ptr_inner};

    // ============================================================================
    // Test: extract_ptr_inner helper
    // ============================================================================
    
    #[test]
    fn test_extract_ptr_inner_nodeptr_simple() {
        let name = "NodePtr_TrieNode";
        let result = extract_ptr_inner(name);
        assert_eq!(result, Some("TrieNode".to_string()));
    }
    
    #[test]
    fn test_extract_ptr_inner_nodeptr_double_underscore() {
        let name = "std__core__node_ptr__NodePtr__TrieNode";
        let result = extract_ptr_inner(name);
        assert_eq!(result, Some("TrieNode".to_string()));
    }
    
    #[test]
    fn test_extract_ptr_inner_nodeptr_with_package() {
        let name = "NodePtr_main__FooBar";
        let result = extract_ptr_inner(name);
        assert_eq!(result, Some("main__FooBar".to_string()));
    }
    
    #[test]
    fn test_extract_ptr_inner_no_ptr() {
        let name = "SimpleStruct";
        let result = extract_ptr_inner(name);
        assert_eq!(result, None);
    }
    
    #[test]
    fn test_extract_ptr_inner_ptr_only() {
        // Just "NodePtr" with no inner type
        let name = "NodePtr";
        let result = extract_ptr_inner(name);
        assert_eq!(result, None);
    }
    
    #[test]
    fn test_extract_ptr_inner_generic_ptr() {
        let name = "Ptr_u8";
        let result = extract_ptr_inner(name);
        assert_eq!(result, Some("u8".to_string()));
    }
    
    // ============================================================================
    // Test: flatten_nested_ptr for Concrete types
    // ============================================================================
    
    #[test]
    fn test_flatten_inception_concrete_nodeptr_nodeptr() {
        // NodePtr<NodePtr<T>> should become T (the innermost non-pointer type)
        let inner_inner = Type::Struct("TrieNode".to_string());
        let inner = Type::Concrete("NodePtr".to_string(), vec![inner_inner.clone()]);
        let outer = Type::Concrete("NodePtr".to_string(), vec![inner]);
        
        let result = flatten_nested_ptr(&outer, 0, "test");
        
        // Should extract to the innermost non-pointer type
        assert_eq!(result, inner_inner, "NodePtr<NodePtr<T>> should flatten to T");
    }
    
    #[test]
    fn test_flatten_inception_concrete_no_change_for_non_pointer_inner() {
        // NodePtr<TrieNode> should stay as TrieNode (inner is not a pointer)
        let inner = Type::Struct("TrieNode".to_string());
        let outer = Type::Concrete("NodePtr".to_string(), vec![inner.clone()]);
        
        let result = flatten_nested_ptr(&outer, 0, "test");
        
        // Concrete<NodePtr, [TrieNode]> - inner is not a pointer, so no change
        // BUT the function returns the original outer since TrieNode is not k_is_ptr_type()
        assert_eq!(result, outer, "NodePtr<TrieNode> should not change");
    }
    
    #[test]
    fn test_flatten_inception_concrete_preserves_primitives() {
        // NodePtr<i32> should stay the same
        let outer = Type::Concrete("NodePtr".to_string(), vec![Type::I32]);
        
        let result = flatten_nested_ptr(&outer, 0, "test");
        
        assert_eq!(result, outer, "NodePtr<i32> should not change");
    }
    
    // ============================================================================
    // Test: flatten_nested_ptr for Struct types
    // ============================================================================
    
    #[test]
    fn test_flatten_inception_struct_extracts_inner() {
        // Struct("NodePtr_TrieNode") should extract to Struct("TrieNode")
        let ty = Type::Struct("NodePtr_TrieNode".to_string());
        
        let result = flatten_nested_ptr(&ty, 0, "test");
        
        assert_eq!(result, Type::Struct("TrieNode".to_string()));
    }
    
    #[test]
    fn test_flatten_inception_struct_nested_nodeptr() {
        // Struct("NodePtr_NodePtr_TrieNode") should flatten recursively
        let ty = Type::Struct("NodePtr_NodePtr_TrieNode".to_string());
        
        let result = flatten_nested_ptr(&ty, 0, "test");
        
        // First extracts "NodePtr_TrieNode", then extracts "TrieNode"
        assert_eq!(result, Type::Struct("TrieNode".to_string()));
    }
    
    #[test]
    fn test_flatten_inception_non_ptr_struct_unchanged() {
        // Regular structs should be unchanged
        let ty = Type::Struct("MyStruct".to_string());
        
        let result = flatten_nested_ptr(&ty, 0, "test");
        
        assert_eq!(result, ty);
    }
    
    // ============================================================================
    // Test: Depth limit safety
    // ============================================================================
    
    #[test]
    fn test_flatten_inception_respects_depth_limit() {
        // Create a deeply nested pointer type
        // This should trigger the depth limit and return without panic
        let mut ty = Type::Struct("Base".to_string());
        for _ in 0..15 {
            ty = Type::Concrete("NodePtr".to_string(), vec![ty]);
        }
        
        // Should not panic, and should return something
        let _ = flatten_nested_ptr(&ty, 0, "depth_test");
    }
    
    // ============================================================================
    // Test: Edge cases
    // ============================================================================
    
    #[test]
    fn test_flatten_inception_empty_args() {
        // Concrete with empty args should be unchanged
        let ty = Type::Concrete("NodePtr".to_string(), vec![]);
        
        let result = flatten_nested_ptr(&ty, 0, "test");
        
        assert_eq!(result, ty);
    }
    
    #[test]
    fn test_flatten_inception_non_ptr_concrete_unchanged() {
        // Non-pointer Concrete types should be unchanged
        let ty = Type::Concrete("Vec".to_string(), vec![Type::I32]);
        
        let result = flatten_nested_ptr(&ty, 0, "test");
        
        assert_eq!(result, ty);
    }
    
    #[test]
    fn test_flatten_inception_primitive_unchanged() {
        // Primitives should be unchanged
        let ty = Type::I64;
        
        let result = flatten_nested_ptr(&ty, 0, "test");
        
        assert_eq!(result, ty);
    }
    
    #[test]
    fn test_flatten_inception_reference_unchanged() {
        // References should be unchanged (handled elsewhere)
        let ty = Type::Reference(Box::new(Type::I32), false);
        
        let result = flatten_nested_ptr(&ty, 0, "test");
        
        assert_eq!(result, ty);
    }
}
