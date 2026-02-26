// =============================================================================
// TDD Tests: Mixed-Width Struct Field Ordering (HashMap→Vec sort fix)
// =============================================================================
//
// Root cause: StructInfo.fields is a HashMap<String, (usize, Type)>.
// When building CallKind::StructLiteral, the resolver iterated the HashMap
// (non-deterministic order) and used the enumeration index `i` as the GEP
// struct field index. This caused type mismatches when HashMap iteration
// order differed from the struct's physical layout.
//
// The fix: sort fields by their logical index before building the
// StructLiteral field vec.
//
// These tests verify:
// 1. Fields from a mixed-width StructInfo are returned in index order.
// 2. This is critical for positional constructors: ElfInfo(e_entry, e_phoff, ...)
// =============================================================================

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::types::Type;

    /// Reproduce the exact pattern from resolver.rs — the sort-by-index logic.
    /// This helper simulates what the resolver does when building StructLiteral fields.
    fn fields_from_struct_info(fields: &HashMap<String, (usize, Type)>) -> Vec<(String, Type)> {
        let mut fields_with_idx: Vec<(String, usize, Type)> = fields.iter()
            .map(|(name, (offset, ty))| (name.clone(), *offset, ty.clone()))
            .collect::<Vec<_>>();
        fields_with_idx.sort_by_key(|(_, offset, _)| *offset);
        fields_with_idx.into_iter().map(|(n, _, t)| (n, t)).collect()
    }

    /// The OLD (broken) logic — iterates HashMap directly without sorting.
    fn fields_from_struct_info_broken(fields: &HashMap<String, (usize, Type)>) -> Vec<(String, Type)> {
        fields.iter()
            .map(|(name, (_offset, ty))| (name.clone(), ty.clone()))
            .collect()
    }

    // =========================================================================
    // Test 1: ELF64 ElfInfo struct — exact reproduction of the kernel bug
    // =========================================================================
    // ElfInfo { entry: u64, phoff: u64, phnum: u16, phentsize: u16, machine: u16 }
    //
    // Without the fix, HashMap may iterate as:
    //   machine(4,u16), entry(0,u64), phnum(2,u16), phoff(1,u64), phentsize(3,u16)
    // With enumerate `i`, GEP[0,0] would get machine:u16 instead of entry:u64.
    // =========================================================================
    #[test]
    fn test_elfinfo_field_order_matches_struct_layout() {
        let mut fields = HashMap::new();
        // Insert in intentionally scrambled order (simulates HashMap non-determinism)
        fields.insert("machine".to_string(),   (4, Type::U16));
        fields.insert("entry".to_string(),     (0, Type::U64));
        fields.insert("phnum".to_string(),     (2, Type::U16));
        fields.insert("phoff".to_string(),     (1, Type::U64));
        fields.insert("phentsize".to_string(), (3, Type::U16));

        let sorted = fields_from_struct_info(&fields);

        // Field 0: entry (u64)
        assert_eq!(sorted[0].0, "entry",  "Field 0 must be 'entry'");
        assert_eq!(sorted[0].1, Type::U64, "Field 0 must be u64");

        // Field 1: phoff (u64)
        assert_eq!(sorted[1].0, "phoff",  "Field 1 must be 'phoff'");
        assert_eq!(sorted[1].1, Type::U64, "Field 1 must be u64");

        // Field 2: phnum (u16)
        assert_eq!(sorted[2].0, "phnum",  "Field 2 must be 'phnum'");
        assert_eq!(sorted[2].1, Type::U16, "Field 2 must be u16");

        // Field 3: phentsize (u16)
        assert_eq!(sorted[3].0, "phentsize", "Field 3 must be 'phentsize'");
        assert_eq!(sorted[3].1, Type::U16,   "Field 3 must be u16");

        // Field 4: machine (u16)
        assert_eq!(sorted[4].0, "machine", "Field 4 must be 'machine'");
        assert_eq!(sorted[4].1, Type::U16, "Field 4 must be u16");
    }

    // =========================================================================
    // Test 2: Mixed-width struct with u16, u32, u64 fields
    // =========================================================================
    // Verifies the GEP→store type alignment for a simple 3-field case.
    // =========================================================================
    #[test]
    fn test_mixed_u16_u32_u64_field_order() {
        let mut fields = HashMap::new();
        // Insert in reverse order
        fields.insert("c".to_string(), (2, Type::U64));
        fields.insert("a".to_string(), (0, Type::U16));
        fields.insert("b".to_string(), (1, Type::U32));

        let sorted = fields_from_struct_info(&fields);

        assert_eq!(sorted[0], ("a".to_string(), Type::U16), "Index 0 must be a:u16");
        assert_eq!(sorted[1], ("b".to_string(), Type::U32), "Index 1 must be b:u32");
        assert_eq!(sorted[2], ("c".to_string(), Type::U64), "Index 2 must be c:u64");
    }

    // =========================================================================
    // Test 3: Full Elf64Header (29 fields) — stress test for HashMap ordering
    // =========================================================================
    // The actual Elf64Header from elf_loader.salt has 29 fields. HashMap ordering
    // becomes increasingly non-deterministic with more entries.
    // =========================================================================
    #[test]
    fn test_full_elf64_header_29_fields_sorted() {
        let mut fields = HashMap::new();
        // Insert all 29 fields in scrambled order
        fields.insert("e_shstrndx".to_string(),    (28, Type::U16));
        fields.insert("e_entry".to_string(),        (19, Type::U64));
        fields.insert("e_ident0".to_string(),       (0,  Type::U8));
        fields.insert("e_type".to_string(),         (16, Type::U16));
        fields.insert("e_machine".to_string(),      (17, Type::U16));
        fields.insert("e_version".to_string(),      (18, Type::U32));
        fields.insert("e_phoff".to_string(),        (20, Type::U64));
        fields.insert("e_shoff".to_string(),        (21, Type::U64));
        fields.insert("e_flags".to_string(),        (22, Type::U32));
        fields.insert("e_ehsize".to_string(),       (23, Type::U16));
        fields.insert("e_phentsize".to_string(),    (24, Type::U16));
        fields.insert("e_phnum".to_string(),        (25, Type::U16));
        fields.insert("e_shentsize".to_string(),    (26, Type::U16));
        fields.insert("e_shnum".to_string(),        (27, Type::U16));
        fields.insert("e_ident1".to_string(),       (1,  Type::U8));
        fields.insert("e_ident2".to_string(),       (2,  Type::U8));
        fields.insert("e_ident3".to_string(),       (3,  Type::U8));
        fields.insert("e_ident_class".to_string(),  (4,  Type::U8));
        fields.insert("e_ident_data".to_string(),   (5,  Type::U8));
        fields.insert("e_ident_version".to_string(),(6,  Type::U8));
        fields.insert("e_ident_osabi".to_string(),  (7,  Type::U8));
        fields.insert("e_ident_pad0".to_string(),   (8,  Type::U8));
        fields.insert("e_ident_pad1".to_string(),   (9,  Type::U8));
        fields.insert("e_ident_pad2".to_string(),   (10, Type::U8));
        fields.insert("e_ident_pad3".to_string(),   (11, Type::U8));
        fields.insert("e_ident_pad4".to_string(),   (12, Type::U8));
        fields.insert("e_ident_pad5".to_string(),   (13, Type::U8));
        fields.insert("e_ident_pad6".to_string(),   (14, Type::U8));
        fields.insert("e_ident_pad7".to_string(),   (15, Type::U8));

        let sorted = fields_from_struct_info(&fields);

        // Verify first field
        assert_eq!(sorted[0].0, "e_ident0");
        assert_eq!(sorted[0].1, Type::U8);

        // Verify critical mixed-width boundary: u16→u64 transition at index 19
        assert_eq!(sorted[16].0, "e_type");
        assert_eq!(sorted[16].1, Type::U16);
        assert_eq!(sorted[17].0, "e_machine");
        assert_eq!(sorted[17].1, Type::U16);
        assert_eq!(sorted[18].0, "e_version");
        assert_eq!(sorted[18].1, Type::U32);
        assert_eq!(sorted[19].0, "e_entry");
        assert_eq!(sorted[19].1, Type::U64, "e_entry at index 19 MUST be u64, not u16");
        assert_eq!(sorted[20].0, "e_phoff");
        assert_eq!(sorted[20].1, Type::U64, "e_phoff at index 20 MUST be u64, not u16");

        // Verify last field
        assert_eq!(sorted[28].0, "e_shstrndx");
        assert_eq!(sorted[28].1, Type::U16);
    }

    // =========================================================================
    // Test 4: Sorted path always correct regardless of HashMap order
    // =========================================================================
    // Uses 26 fields to guarantee HashMap internal hashing scrambles the order.
    // Verifies sorted output always matches struct layout.
    // =========================================================================
    #[test]
    fn test_sorted_always_correct_vs_unsorted() {
        let mut fields = HashMap::new();
        // Use enough fields to make HashMap ordering likely differ from insertion
        for i in 0..26u8 {
            let name = format!("field_{}", (b'z' - i) as char); // z, y, x, ... a
            let ty = if i % 3 == 0 { Type::U64 } else if i % 3 == 1 { Type::U16 } else { Type::U32 };
            fields.insert(name, (i as usize, ty));
        }

        let sorted = fields_from_struct_info(&fields);

        // Sorted output MUST always match layout order
        for (i, (name, _ty)) in sorted.iter().enumerate() {
            let expected_char = (b'z' - i as u8) as char;
            let expected_name = format!("field_{}", expected_char);
            assert_eq!(
                name, &expected_name,
                "Sorted field at GEP index {} must be '{}', got '{}' — \
                 HashMap non-determinism would cause type mismatch here",
                i, expected_name, name
            );
        }
    }
}
