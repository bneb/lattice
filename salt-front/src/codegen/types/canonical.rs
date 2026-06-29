//! Canonical Type Identity System
//!
//! The TypeID system provides O(1) type identity comparison based on structural hashing,
//! eliminating fragile string suffix matching throughout the Salt compiler.
//!
//! # Design Principles
//!
//! 1. **Structural Identity**: TypeIDs are computed from normalized type representations
//! 2. **Prefix Normalization**: Package prefixes (main__, std__, etc.) are stripped
//! 3. **Underscore Normalization**: Variations (_ vs __) are normalized
//! 4. **Inception Flattening**: NodePtr<NodePtr<T>> → NodePtr<T>

use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;

/// A structural hash of a type, providing O(1) identity comparison.
///
/// TypeIDs are computed from normalized type representations that:
/// - Strip package prefixes (main__, std__core__, etc.)
/// - Normalize underscore variations
/// - Flatten inception (NodePtr<NodePtr<T>> → NodePtr<T>)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TypeID(u64);

impl TypeID {
    /// Creates an invalid/null TypeID.
    pub fn null() -> Self {
        TypeID(0)
    }
    
    /// Returns true if this TypeID is valid (non-null).
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
    
    /// Returns the raw hash value for debugging.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for TypeID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypeID(0x{:016x})", self.0)
    }
}

/// Global registry mapping TypeID → canonical type information.
///
/// This registry provides:
/// - O(1) lookup from canonical name to TypeID
/// - O(1) lookup from TypeID to canonical name
/// - Deduplication of equivalent type representations
#[derive(Debug, Clone)]
pub struct TypeIDRegistry {
    /// TypeID → canonical name mapping
    ids: HashMap<TypeID, String>,
    /// Canonical name → TypeID reverse mapping
    reverse: HashMap<String, TypeID>,
}

impl Default for TypeIDRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeIDRegistry {
    /// Creates a new empty TypeID registry.
    pub fn new() -> Self {
        Self {
            ids: HashMap::new(),
            reverse: HashMap::new(),
        }
    }
    
    /// Registers a canonical name and returns its TypeID.
    ///
    /// If the name is already registered, returns the existing TypeID.
    /// Otherwise, computes a new TypeID from the structural hash.
    pub fn register(&mut self, canonical_name: &str) -> TypeID {
        // Check for existing registration
        if let Some(&existing) = self.reverse.get(canonical_name) {
            return existing;
        }
        
        // Compute structural hash
        let mut hasher = DefaultHasher::new();
        canonical_name.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Avoid null TypeID (0)
        let id = TypeID(if hash == 0 { 1 } else { hash });
        
        // Handle hash collision (extremely rare but possible)
        let final_id = if self.ids.contains_key(&id) {
            // Collision resolution: increment until unique
            let mut collision_id = id.0;
            loop {
                collision_id = collision_id.wrapping_add(1);
                let candidate = TypeID(collision_id);
                if !self.ids.contains_key(&candidate) {
                    break candidate;
                }
            }
        } else {
            id
        };
        
        self.ids.insert(final_id, canonical_name.to_string());
        self.reverse.insert(canonical_name.to_string(), final_id);
        final_id
    }
    
    /// Gets the canonical name for a TypeID.
    pub fn get_canonical_name(&self, id: TypeID) -> Option<&str> {
        self.ids.get(&id).map(|s| s.as_str())
    }
    
    /// Looks up a TypeID by canonical name without registering.
    pub fn lookup(&self, canonical_name: &str) -> Option<TypeID> {
        self.reverse.get(canonical_name).copied()
    }
    
    /// Returns the number of registered types.
    pub fn len(&self) -> usize {
        self.ids.len()
    }
    
    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    
    /// Iterates over all registered (TypeID, canonical_name) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (TypeID, &str)> {
        self.ids.iter().map(|(&id, name)| (id, name.as_str()))
    }
    
    /// Clears all registrations (for testing).
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.ids.clear();
        self.reverse.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_type_id_registration() {
        let mut registry = TypeIDRegistry::new();
        
        let id1 = registry.register("NodePtr_TrieNode");
        let id2 = registry.register("NodePtr_TrieNode");
        
        assert_eq!(id1, id2, "Same canonical name should produce same TypeID");
        assert!(id1.is_valid());
    }
    
    #[test]
    fn test_different_types_different_ids() {
        let mut registry = TypeIDRegistry::new();
        
        let id1 = registry.register("NodePtr_TrieNode");
        let id2 = registry.register("NodePtr_ListNode");
        
        assert_ne!(id1, id2, "Different types should have different TypeIDs");
    }
    
    #[test]
    fn test_lookup_and_reverse() {
        let mut registry = TypeIDRegistry::new();
        
        let id = registry.register("Vec_i32");
        
        assert_eq!(registry.get_canonical_name(id), Some("Vec_i32"));
        assert_eq!(registry.lookup("Vec_i32"), Some(id));
        assert_eq!(registry.lookup("Vec_i64"), None);
    }
    
    #[test]
    fn test_null_type_id() {
        let null_id = TypeID::null();
        assert!(!null_id.is_valid());
        assert_eq!(null_id.raw(), 0);
    }

    #[test]
    fn test_registry_is_empty() {
        let reg = TypeIDRegistry::new();
        assert!(reg.is_empty());
    }

    #[test]
    fn test_registry_len() {
        let mut reg = TypeIDRegistry::new();
        assert_eq!(reg.len(), 0);
        reg.register("Foo");
        assert_eq!(reg.len(), 1);
        reg.register("Bar");
        assert_eq!(reg.len(), 2);
        // Duplicate registration doesn't increase len
        reg.register("Foo");
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn test_registry_clear() {
        let mut reg = TypeIDRegistry::new();
        reg.register("Foo");
        reg.register("Bar");
        assert!(!reg.is_empty());
        reg.clear();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn test_registry_iter() {
        let mut reg = TypeIDRegistry::new();
        reg.register("A");
        reg.register("B");
        let pairs: Vec<_> = reg.iter().collect();
        assert_eq!(pairs.len(), 2);
        for (id, name) in &pairs {
            assert!(id.is_valid());
            assert!(*name == "A" || *name == "B");
        }
    }

    #[test]
    fn test_type_id_display() {
        let id = TypeID(0xdeadbeef);
        let s = format!("{}", id);
        assert_eq!(s, "TypeID(0x00000000deadbeef)");
    }

    #[test]
    fn test_get_canonical_name_unknown_id() {
        let reg = TypeIDRegistry::new();
        assert_eq!(reg.get_canonical_name(TypeID::null()), None);
        assert_eq!(reg.get_canonical_name(TypeID(999)), None);
    }

    #[test]
    fn test_register_duplicate_returns_same_id() {
        let mut reg = TypeIDRegistry::new();
        let id1 = reg.register("Foo");
        assert_eq!(reg.len(), 1);
        let id2 = reg.register("Foo");
        assert_eq!(id1, id2, "Duplicate registration should return same ID");
        assert_eq!(reg.len(), 1, "Duplicate should not increase registry size");
        assert_eq!(reg.get_canonical_name(id1), Some("Foo"));
    }
}
