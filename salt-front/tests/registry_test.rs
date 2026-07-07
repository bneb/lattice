use saltc::registry::{Registry, ModuleInfo};

#[test]
fn test_registry_basic() {
    let mut reg = Registry::new();
    let mod_info = ModuleInfo::new("test_pkg");
    reg.register(mod_info);
    
    assert!(reg.modules.contains_key("test_pkg"));
}

#[test]
fn test_module_info_defaults() {
    let info = ModuleInfo::new("my.pkg");
    assert_eq!(info.package, "my.pkg");
    assert!(info.functions.is_empty());
    assert!(info.structs.is_empty());
    assert!(info.struct_templates.is_empty());
    assert!(info.enum_templates.is_empty());
    assert!(info.function_templates.is_empty());
    assert!(info.enums.is_empty());
    assert!(info.constants.is_empty());
    assert!(info.globals.is_empty());
    assert!(info.impls.is_empty());
    assert!(info.imports.is_empty());
}
