#[cfg(test)]
mod tests {
    use crate::types::{Type, TypeKey};
    use crate::codegen::types::resolution::type_to_type_key;

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
