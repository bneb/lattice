use crate::types::{Type, TypeKey};

pub fn type_to_type_key(ty: &Type) -> TypeKey {
    match ty {
        Type::Struct(name) => {
            let parts: Vec<&str> = name.split("__").collect();
            if parts.len() > 1 {
                TypeKey {
                    path: parts[..parts.len()-1].iter().map(|s| s.to_string()).collect(),
                    name: name.clone(),
                    specialization: Some(vec![]),
                }
            } else {
                TypeKey { path: vec![], name: name.clone(), specialization: Some(vec![]) }
            }
        }
        Type::Concrete(name, args) => {
            let parts: Vec<&str> = name.split("__").collect();
            if parts.len() > 1 {
                TypeKey {
                    path: parts[..parts.len()-1].iter().map(|s| s.to_string()).collect(),
                    name: name.clone(),
                    specialization: Some(args.clone()),
                }
            } else {
                TypeKey { path: vec![], name: name.clone(), specialization: Some(args.clone()) }
            }
        }
        Type::Reference(inner, _) => type_to_type_key(inner),
        Type::Owned(inner) => type_to_type_key(inner),
        _ => TypeKey { path: vec![], name: format!("{:?}", ty), specialization: None }
    }
}
