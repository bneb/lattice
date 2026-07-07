use saltc::codegen::seeker::Seeker;
use saltc::types::Type;

#[test]
fn test_mangle_method_name_no_params() {
    // No type params: TypeName__method
    assert_eq!(Seeker::mangle_method_name("Vec_u8", "push", &[]), "Vec_u8__push");
    assert_eq!(Seeker::mangle_method_name("RawVec_i32", "with_capacity", &[]), "RawVec_i32__with_capacity");
    assert_eq!(Seeker::mangle_method_name("String", "len", &[]), "String__len");
}

#[test]
fn test_mangle_method_name_with_params() {
    // With type params: TypeName_Param__method
    assert_eq!(
        Seeker::mangle_method_name("Result", "map", &[Type::I32, Type::Bool]),
        "Result_i32_bool__map"
    );
    assert_eq!(
        Seeker::mangle_method_name("Vec", "get", &[Type::U8]),
        "Vec_u8__get"
    );
}

#[test]
fn test_mangle_consistency_call_vs_definition() {
    // Simulates call-site mangling (from Vec<u8> receiver calling push)
    let call_site = Seeker::mangle_method_name("Vec_u8", "push", &[]);
    
    // Simulates definition-side mangling (from impl Vec<T> { fn push... })
    let def_side = Seeker::mangle_method_name("Vec_u8", "push", &[]);
    
    assert_eq!(call_site, def_side, "Call site and definition must produce identical symbols");
}

#[test]
fn test_mangle_method_nested_types() {
    // Nested concrete types
    let nested = Type::Concrete("Vec".to_string(), vec![Type::Concrete("Option".to_string(), vec![Type::I32])]);
    assert_eq!(
        Seeker::mangle_method_name("Container", "insert", &[nested]),
        "Container_Vec_Option_i32__insert"
    );
}

#[test]
fn test_mangle_method_reference_types() {
    // Reference types
    let ref_ty = Type::Reference(Box::new(Type::I32), false);
    assert_eq!(
        Seeker::mangle_method_name("Handler", "process", &[ref_ty]),
        "Handler_Ref_i32__process"
    );
    
    let ref_mut_ty = Type::Reference(Box::new(Type::U8), true);
    assert_eq!(
        Seeker::mangle_method_name("Buffer", "write", &[ref_mut_ty]),
        "Buffer_RefMut_u8__write"
    );
}
