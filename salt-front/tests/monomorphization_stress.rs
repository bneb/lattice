use std::collections::{BTreeMap, HashMap};
use salt_front::types::Type;
use salt_front::types::TypeKey;
use salt_front::registry::StructInfo;

#[test]
fn test_specialized_concrete_layout() {
    let mut reg: HashMap<TypeKey, StructInfo> = HashMap::new();
    
    // Define a specialized struct: Box_i64 { val: i64 }
    let fields = vec![Type::I64];
    let mut fields_map = HashMap::new();
    fields_map.insert("val".to_string(), (0, Type::I64));
    
    let info = StructInfo {
        name: "Box_i64".to_string(),
        fields: fields_map,
        field_order: fields,
        template_name: None,
        specialization_args: vec![],
    };
    
    let key = TypeKey { path: vec![], name: "Box_i64".to_string(), specialization: None };
    reg.insert(key, info);
    
    let concrete_ty = Type::Concrete("Box".to_string(), vec![Type::I64]);
    
    // align_of should be 8
    assert_eq!(concrete_ty.align_of(&reg), 8);
    // size_of should be 8
    assert_eq!(concrete_ty.size_of(&reg), 8);
    
    // Define a specialized struct: Pair_i32_i32 { a: i32, b: i32 }
    let pair_fields = vec![Type::I32, Type::I32];
    let mut pair_map = HashMap::new();
    pair_map.insert("a".to_string(), (0, Type::I32));
    pair_map.insert("b".to_string(), (1, Type::I32));
    
    let pair_info = StructInfo {
        name: "Pair_i32_i32".to_string(),
        fields: pair_map,
        field_order: pair_fields,
        template_name: None,
        specialization_args: vec![],
    };
    let pair_key = TypeKey { path: vec![], name: "Pair_i32_i32".to_string(), specialization: None };
    reg.insert(pair_key, pair_info);
    
    let pair_ty = Type::Concrete("Pair".to_string(), vec![Type::I32, Type::I32]);
    
    assert_eq!(pair_ty.align_of(&reg), 4);
    assert_eq!(pair_ty.size_of(&reg), 8);
}

#[test]
fn test_complex_nested_layout() {
    let mut reg: HashMap<TypeKey, StructInfo> = HashMap::new();
    
    // Struct A { x: i32 }
    let fields_a = vec![Type::I32];
    let mut map_a = HashMap::new();
    map_a.insert("x".to_string(), (0, Type::I32));
    let info_a = StructInfo { name: "A".to_string(), fields: map_a, field_order: fields_a, template_name: None, specialization_args: vec![] };
    let key_a = TypeKey { path: vec![], name: "A".to_string(), specialization: None };
    reg.insert(key_a, info_a);
    
    // Struct B { arr: [A; 3], y: i64 }
    let fields_b = vec![
        Type::Array(Box::new(Type::Struct("A".to_string())), 3, false),
        Type::I64
    ];
    let mut map_b = HashMap::new();
    map_b.insert("arr".to_string(), (0, fields_b[0].clone()));
    map_b.insert("y".to_string(), (1, fields_b[1].clone()));
    
    let info_b = StructInfo { name: "B".to_string(), fields: map_b, field_order: fields_b, template_name: None, specialization_args: vec![] };
    let key_b = TypeKey { path: vec![], name: "B".to_string(), specialization: None };
    reg.insert(key_b, info_b);
    
    let ty_b = Type::Struct("B".to_string());
    assert_eq!(ty_b.size_of(&reg), 24);
    assert_eq!(ty_b.align_of(&reg), 8);

    // Deep recursion check (depth 33 fallback)
    let mut deep_ty = Type::I32;
    for _ in 0..35 {
        deep_ty = Type::Array(Box::new(deep_ty), 1, false);
    }
    assert_eq!(deep_ty.size_of(&reg), 8); // Fallback to 8
    assert_eq!(deep_ty.align_of(&reg), 8); // Fallback to 8
}

#[test]
fn test_exhaustive_mangle_suffix() {
    let cases = vec![
        (Type::I8, "i8"),
        (Type::I16, "i16"),
        (Type::I32, "i32"),
        (Type::I64, "i64"),
        (Type::U8, "u8"),
        (Type::U16, "u16"),
        (Type::U32, "u32"),
        (Type::U64, "u64"),
        (Type::Usize, "usize"),
        (Type::F32, "f32"),
        (Type::F64, "f64"),
        (Type::Bool, "bool"),
        (Type::Unit, "unit"),
        (Type::Never, "Never"),
        (Type::SelfType, "Self"),
        (Type::Generic("U".to_string()), "U"),
        (Type::Struct("MyStruct".to_string()), "MyStruct"),
        (Type::Enum("MyEnum".to_string()), "MyEnum"),
        (Type::Reference(Box::new(Type::I32), false), "Ref_i32"),
        (Type::Reference(Box::new(Type::I32), true), "RefMut_i32"),
        (Type::Owned(Box::new(Type::I32)), "Owned_i32"),
        (Type::Window(Box::new(Type::I32), "global".to_string()), "Window_i32_global"),
        (Type::Array(Box::new(Type::I32), 10, false), "Array_i32_10"),
        (Type::Atomic(Box::new(Type::I32)), "Atomic_i32"),
        (Type::Tuple(vec![Type::I32, Type::I64]), "Tuple_i32_i64"),
        (Type::Concrete("Vec".to_string(), vec![Type::I32]), "Vec_i32"),
        (Type::Fn(vec![Type::I8], Box::new(Type::I16)), "Fn_i8_i16"),
    ];

    for (ty, expected) in cases {
        assert_eq!(ty.mangle_suffix(), expected);
    }
}

#[test]
fn test_uncommon_types_layout() {
    let reg: HashMap<TypeKey, StructInfo> = HashMap::new();
    
    // Window: size 16 (pointer + len), align 8
    let window_ty = Type::Window(Box::new(Type::I32), "global".to_string());
    assert_eq!(window_ty.size_of(&reg), 16);
    assert_eq!(window_ty.align_of(&reg), 8);
    
    // Fn: size 8 (pointer), align 8
    let fn_ty = Type::Fn(vec![Type::I32], Box::new(Type::I32));
    assert_eq!(fn_ty.size_of(&reg), 8);
    assert_eq!(fn_ty.align_of(&reg), 8);
    
    // Never: size 0, align 1
    assert_eq!(Type::Never.size_of(&reg), 0);
    assert_eq!(Type::Never.align_of(&reg), 1);
    
    // Unit: size 8 (fallback), align 1 (fallback)
    assert_eq!(Type::Unit.size_of(&reg), 8);
    assert_eq!(Type::Unit.align_of(&reg), 1);
}

#[test]
fn test_unregistered_fallbacks() {
    let reg: HashMap<TypeKey, StructInfo> = HashMap::new();
    let struct_ty = Type::Struct("Missing".to_string());
    assert_eq!(struct_ty.align_of(&reg), 8);
    assert_eq!(struct_ty.size_of(&reg), 8);
    
    let concrete_ty = Type::Concrete("Missing".to_string(), vec![Type::I32]);
    assert_eq!(concrete_ty.align_of(&reg), 8);
    assert_eq!(concrete_ty.size_of(&reg), 8);
}

#[test]
fn test_empty_tuple() {
    let reg: HashMap<TypeKey, StructInfo> = HashMap::new();
    let tuple_ty = Type::Tuple(vec![]);
    // Empty tuple has size 0 based on current implementation
    assert_eq!(tuple_ty.size_of(&reg), 0);
    
    // Unit (fallback) is size 8
    assert_eq!(Type::Unit.size_of(&reg), 8);
}

#[test]
fn test_from_syn_comprehensive() {
    use salt_front::grammar::SynType;
    
    // Empty tuple -> Unit
    let syn_ty: syn::Type = syn::parse_str("()").unwrap();
    let salt_ty = SynType::from_std(syn_ty).unwrap();
    let ty = Type::from_syn(&salt_ty).unwrap();
    assert_eq!(ty, Type::Unit);
    
    // Array -> Array type
    let syn_arr: syn::Type = syn::parse_str("[i32; 10]").unwrap();
    let salt_arr = SynType::from_std(syn_arr).unwrap();
    let ty_arr = Type::from_syn(&salt_arr).unwrap();
    assert!(matches!(ty_arr, Type::Array(_, 10, _)));
}

#[test]
fn test_enum_align() {
    let reg: HashMap<TypeKey, StructInfo> = HashMap::new();
    let enum_ty = Type::Enum("MyEnum".to_string());
    assert_eq!(enum_ty.align_of(&reg), 8);
}

#[test]
fn test_recursive_tuple_layout() {
    let reg: HashMap<TypeKey, StructInfo> = HashMap::new();
    let inner = Type::Tuple(vec![Type::I8, Type::I64]); // size 16, align 8
    let outer = Type::Tuple(vec![Type::I8, inner]);     // size 24, align 8
    assert_eq!(outer.align_of(&reg), 8);
    assert_eq!(outer.size_of(&reg), 24);
}
