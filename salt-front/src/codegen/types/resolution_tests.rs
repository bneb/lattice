#[cfg(test)]
mod tests {
    use crate::types::{Type, TypeKey};
    use crate::codegen::types::resolution::{type_to_type_key, pick_canonical_key};
    use crate::registry::{Registry, ModuleInfo};

    #[test]
    fn test_pick_canonical_prefers_loaded_module() {
        // A stdlib type leaked under the entry package (`main__Slice`) must not
        // shadow its real FQN when the defining module is loaded.
        let mut reg = Registry::new();
        reg.register(ModuleInfo::new("std.core.slice"));
        let keys = ["main__Slice".to_string(), "std__core__slice__Slice".to_string()];
        let picked = pick_canonical_key(keys.iter(), "Slice", Some(&reg));
        assert_eq!(picked.as_deref(), Some("std__core__slice__Slice"));
    }

    #[test]
    fn test_pick_canonical_single_and_sorted_fallback() {
        // Single match returns as-is; with no loaded module, pick is deterministic.
        let one = ["main__Foo".to_string()];
        assert_eq!(pick_canonical_key(one.iter(), "Foo", None).as_deref(), Some("main__Foo"));
        let two = ["b__Bar".to_string(), "a__Bar".to_string()];
        assert_eq!(pick_canonical_key(two.iter(), "Bar", None).as_deref(), Some("a__Bar"));
    }

    #[test]
    fn test_type_to_type_key_concrete_no_path() {
        let k = type_to_type_key(&Type::Concrete("Vec".into(), vec![Type::I32]));
        assert_eq!(k.name, "Vec");
        assert_eq!(k.path, vec![] as Vec<String>);
        assert_eq!(k.specialization, Some(vec![Type::I32]));
    }

    #[test]
    fn test_type_to_type_key_concrete_with_path() {
        let k = type_to_type_key(&Type::Concrete(
            "std__collections__HashMap".into(), vec![Type::I64, Type::I64],
        ));
        assert_eq!(k.name, "std__collections__HashMap");
        assert_eq!(k.path, vec!["std", "collections"]);
        assert_eq!(k.specialization, Some(vec![Type::I64, Type::I64]));
    }

    #[test]
    fn test_type_to_type_key_owned_struct() {
        let k = type_to_type_key(&Type::Owned(Box::new(Type::Struct("Foo".into()))));
        assert_eq!(k.name, "Foo");
        assert_eq!(k.path, vec![] as Vec<String>);
        assert_eq!(k.specialization, Some(vec![]));
    }

    #[test]
    fn test_type_to_type_key_owned_concrete() {
        let k = type_to_type_key(&Type::Owned(Box::new(
            Type::Concrete("pkg__Bar".into(), vec![Type::I64]),
        )));
        assert_eq!(k.name, "pkg__Bar");
        assert_eq!(k.path, vec!["pkg"]);
        assert_eq!(k.specialization, Some(vec![Type::I64]));
    }

    #[test]
    fn test_type_to_type_key_fallback() {
        let k = type_to_type_key(&Type::I32);
        assert_eq!(k.name, "I32");
        assert_eq!(k.specialization, None);
    }
}
