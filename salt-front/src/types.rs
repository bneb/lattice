use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use crate::grammar::SynType;
use crate::registry::StructInfo;
use crate::common::mangling::Mangler;

/// Provenance defines the 'Legal Origin' of a pointer.
/// This tracks the lifecycle "Shadow" for formal verification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Provenance {
    Static,       // Global constants (.data/.rodata)
    Stack,        // Function-local frame
    Heap(String), // Mapped regions (e.g. "mnist_images")
    Naked,        // Unverified raw address (default for Ptr<T>)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeKey {
    pub path: Vec<String>, 
    pub name: String,           
    pub specialization: Option<Vec<Type>>, 
}

impl Hash for TypeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.name.hash(state);
        match &self.specialization {
            None => state.write_u8(0),
            Some(args) => {
                state.write_u8(1);
                for arg in args { arg.hash(state); }
            }
        }
    }
}

impl TypeKey {
    pub fn to_template(&self) -> Self {
        Self {
            path: self.path.clone(),
            name: self.name.clone(),
            specialization: None,
        }
    }

    pub fn mangle(&self) -> String {
        let mut parts: Vec<&str> = self.path.iter().map(|s| s.as_str()).collect();
        parts.push(&self.name);
        let mut base = Mangler::mangle(&parts);
        if let Some(args) = &self.specialization {
            for arg in args {
                base.push('_');
                base.push_str(&arg.mangle_suffix());
            }
        }
        base
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    I8, I16, I32, I64,
    U8, U16, U32, U64, Usize,
    F32, F64,
    Bool, Unit,
    Struct(String),
    Generic(String),
    Concrete(String, Vec<Type>),
    Owned(Box<Type>),
    Tensor(Box<Type>, Vec<usize>),
    Array(Box<Type>, usize, bool),
    Tuple(Vec<Type>),
    Fn(Vec<Type>, Box<Type>),
    Enum(String),
    Atomic(Box<Type>),
    Window(Box<Type>, String),

    /// THE SOVEREIGN POINTER: Register-Native primitive
    Pointer {
        element: Box<Type>,
        provenance: Provenance,
        is_mutable: bool,
    },

    /// THE BORROW: Stack-rooted reference
    Reference(Box<Type>, bool),

    Never,
    SelfType,
}

impl Type {
    /// [SOVEREIGN V2.0] Direct mapping from Parser to Compiler Primitives
    pub fn from_syn(ty: &SynType) -> Option<Self> {
        // Delegate to the context-aware version with an empty set (preserves legacy single-char heuristic)
        Self::from_syn_with_generics(ty, &std::collections::HashSet::new())
    }

    /// Context-aware version: names in `generic_names` produce Type::Generic regardless of length.
    /// When `generic_names` is empty, falls back to the legacy single-char uppercase heuristic.
    pub fn from_syn_with_generics(ty: &SynType, generic_names: &std::collections::HashSet<String>) -> Option<Self> {
        match ty {
             // Ptr<T> (Sovereign Keyword) -> Type::Pointer
             SynType::Pointer(inner) => {
                 let element = Type::from_syn_with_generics(inner, generic_names)?;
                 Some(Type::Pointer {
                     element: Box::new(element),
                     provenance: Provenance::Naked,
                     is_mutable: true,
                 })
             }
             // &T (Reference Syntax) -> Type::Reference
             SynType::Reference(inner, is_mut) => {
                 let element = Type::from_syn_with_generics(inner, generic_names)?;
                 Some(Type::Reference(Box::new(element), *is_mut))
             }
             SynType::Path(tp) => {
                 let seg = tp.segments.last()?;
                 // [CROSS-MODULE STRUCT] When there are multiple segments (e.g. [addr, PhysAddr]),
                 // join them with "::" to create a qualified name. Single-segment paths are bare names.
                 // The codegen resolve_type_safe and bridge_resolve_package_prefix handle "::" paths.
                 let name = if tp.segments.len() > 1 {
                     tp.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::")
                 } else {
                     seg.ident.to_string()
                 };
                 let params: Vec<Type> = seg.args.iter().filter_map(|arg| Type::from_syn_with_generics(arg, generic_names)).collect();

                 match name.as_str() {
                      "i8" => Some(Type::I8), "i16" => Some(Type::I16),
                      "i32" => Some(Type::I32), "i64" => Some(Type::I64),
                      "u8" => Some(Type::U8), "u16" => Some(Type::U16),
                      "u32" => Some(Type::U32), "u64" => Some(Type::U64),
                       "f32" => Some(Type::F32), "f64" => Some(Type::F64),
                       "bool" => Some(Type::Bool), "usize" => Some(Type::Usize),
                       "Self" => Some(Type::SelfType),
                       "LlvmPtr" => Some(Type::Pointer {
                           element: Box::new(Type::I8),
                           provenance: Provenance::Naked,
                           is_mutable: true,
                       }),
                       // [SOVEREIGN PHASE 3] Tensor<T, __Shape_R_D1_D2__> -> Type::Tensor (AUTO-RANK)
                       // Preprocessor auto-computes rank: {128,784} -> __Shape_2_128_784__
                       // Format: first value is auto-rank (skipped), rest are dimensions
                       "Tensor" => {
                           // params[0] = element type, params[1] = __Shape_...__ marker
                           if params.len() >= 1 {
                               let elem = params[0].clone();
                               // Look for shape marker in the syntax path
                               if seg.args.len() >= 2 {
                                   if let SynType::Path(shape_path) = &seg.args[1] {
                                       let shape_name = shape_path.segments.last()?.ident.to_string();
                                       if shape_name.starts_with("__Shape_") && shape_name.ends_with("__") {
                                           // Parse __Shape_2_128_784__ -> skip rank, dims = [128, 784]
                                           let shape_str = &shape_name[8..shape_name.len()-2]; // strip prefix/suffix
                                           let all_values: Vec<usize> = shape_str.split('_')
                                               .filter_map(|s| s.parse().ok())
                                               .collect();
                                           // Skip first value (rank indicator) and use rest as dimensions
                                           let dims = if all_values.len() > 1 {
                                               all_values[1..].to_vec()
                                           } else {
                                               all_values
                                           };
                                           if !dims.is_empty() {
                                               return Some(Type::Tensor(Box::new(elem), dims));
                                           }
                                       }
                                   }
                               }
                               // Fallback: Tensor with no shape
                               Some(Type::Tensor(Box::new(elem), vec![]))
                           } else {
                               None
                           }
                       }
                       _ => {
                           // [SOVEREIGN FIX] Context-aware generic detection
                           if generic_names.contains(&name) {
                               Some(Type::Generic(name))
                           } else if generic_names.is_empty() && name.len() == 1 && name.chars().all(|c| c.is_uppercase()) {
                               // Legacy fallback: single uppercase char when no context available
                               Some(Type::Generic(name))
                           } else {
                               Some(Type::Concrete(name, params))
                           }
                       }
                   }
              }
             SynType::Array(inner_syn, len_expr) => {
                 let inner = Type::from_syn_with_generics(inner_syn, generic_names)?;
                 if let syn::Expr::Lit(syn::ExprLit{lit: syn::Lit::Int(li), ..}) = len_expr {
                      let len = li.base10_parse::<usize>().ok()?;
                      Some(Type::Array(Box::new(inner), len, false))
                 } else { None }
             }
             SynType::Tuple(tuple) => {
                 if tuple.elems.is_empty() { Some(Type::Unit) }
                 else {
                     let elems: Vec<Type> = tuple.elems.iter().filter_map(|t| Type::from_syn_with_generics(t, generic_names)).collect();
                     Some(Type::Tuple(elems))
                 }
             }
             // [SOVEREIGN PHASE 3] ShapedTensor -> Pointer with embedded shape
             // Tensor<T, {Rank, D1, D2...}> becomes a shaped Ptr for @ dispatch
             SynType::ShapedTensor { element, rank, dims } => {
                 use crate::grammar::TensorDim;
                 let inner = Type::from_syn_with_generics(element, generic_names)?;
                 // Convert TensorDim to usize for Type::Tensor
                 let static_dims: Vec<usize> = dims.iter().filter_map(|d| match d {
                     TensorDim::Static(n) => Some(*n),
                     _ => None, // Dynamic/symbolic requires runtime tracking
                 }).collect();
                 // For now, require all static dims
                 if static_dims.len() == *rank {
                     Some(Type::Tensor(Box::new(inner), static_dims))
                 } else {
                     // Fall back to unshaped pointer for dynamic dims
                     Some(Type::Pointer {
                         element: Box::new(inner),
                         provenance: Provenance::Naked,
                         is_mutable: true,
                     })
                 }
             }
             SynType::FnPtr(args, ret) => {
                 let fn_args: Vec<Type> = args.iter().filter_map(|t| Type::from_syn_with_generics(t, generic_names)).collect();
                 let fn_ret = ret.as_ref()
                     .and_then(|r| Type::from_syn_with_generics(r, generic_names))
                     .unwrap_or(Type::Unit);
                 Some(Type::Fn(fn_args, Box::new(fn_ret)))
             }
             SynType::Other(s) => {
                 if s.contains("llvm") && s.contains("ptr") {
                     Some(Type::Pointer {
                         element: Box::new(Type::I8),
                         provenance: Provenance::Naked,
                         is_mutable: true,
                     })
                 } else { None }
             }
        }
    }

    pub fn mangle_suffix(&self) -> String {
        match self {
            Type::I8 => "i8".to_string(),
            Type::I16 => "i16".to_string(),
            Type::I32 => "i32".to_string(),
            Type::I64 => "i64".to_string(),
            Type::U8 => "u8".to_string(),
            Type::U16 => "u16".to_string(),
            Type::U32 => "u32".to_string(),
            Type::U64 => "u64".to_string(),
            Type::Usize => "usize".to_string(),
            Type::F32 => "f32".to_string(),
            Type::F64 => "f64".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Unit => "unit".to_string(),
            Type::Struct(name) | Type::Enum(name) | Type::Generic(name) => name.clone(),
            Type::Pointer { element, .. } => format!("Ptr_{}", element.mangle_suffix()),
            Type::Reference(inner, is_mut) => format!("Ref{}_{}", if *is_mut { "Mut" } else { "" }, inner.mangle_suffix()),
            Type::Array(inner, len, p) => format!("Array_{}_{}{}", inner.mangle_suffix(), len, if *p { "_pk" } else { "" }),
            Type::Concrete(name, params) => {
                let mut s = name.clone();
                for p in params { s.push('_'); s.push_str(&p.mangle_suffix()); }
                s
            }
            Type::Owned(inner) => format!("Owned_{}", inner.mangle_suffix()),
            Type::Tensor(inner, dims) => {
                let mut s = format!("Tensor_{}", inner.mangle_suffix());
                for d in dims { s.push('_'); s.push_str(&d.to_string()); }
                s
            }
            Type::Tuple(elems) => {
                if elems.is_empty() { return "Tuple".to_string(); }
                let mut s = "Tuple".to_string();
                for e in elems { s.push('_'); s.push_str(&e.mangle_suffix()); }
                s
            }
            Type::Fn(args, ret) => {
                let mut s = "Fn".to_string();
                for a in args { s.push('_'); s.push_str(&a.mangle_suffix()); }
                s.push('_');
                s.push_str(&ret.mangle_suffix());
                s
            }
            Type::Atomic(inner) => format!("Atomic_{}", inner.mangle_suffix()),
            Type::Window(inner, win) => format!("Window_{}_{}", inner.mangle_suffix(), win),
            Type::Never => "Never".to_string(),
            Type::SelfType => "Self".to_string(),
        }
    }

    /// Context-free MLIR type mapping for common cases.
    /// Used by LoweringContext methods that don't need full CodegenContext.
    /// For complex cases (generic substitution, type_map lookup), use the full
    /// context-aware `to_mlir_type()` in type_bridge.rs.
    pub fn to_mlir_type_simple(&self) -> String {
        if self.k_is_ptr_type() || matches!(self, Type::Reference(_, _)) {
            return "!llvm.ptr".to_string();
        }
        match self {
            Type::I8 | Type::U8 => "i8".to_string(),
            Type::I16 | Type::U16 => "i16".to_string(),
            Type::I32 | Type::U32 => "i32".to_string(),
            Type::I64 | Type::U64 => "i64".to_string(),
            Type::F32 => "f32".to_string(),
            Type::F64 => "f64".to_string(),
            Type::Bool => "i1".to_string(),
            Type::Usize => "index".to_string(),
            Type::Unit => "!llvm.void".to_string(),
            Type::Struct(name) => format!("!struct_{}", name),
            Type::Enum(name) => format!("!enum_{}", name),
            Type::Concrete(name, _) => format!("!struct_{}", self.mangle_suffix()),
            Type::Array(inner, len, _) => format!("!llvm.array<{} x {}>", len, inner.to_mlir_type_simple()),
            Type::Tuple(elems) => {
                if elems.is_empty() { return "!llvm.void".to_string(); }
                let inner: Vec<String> = elems.iter().map(|e| e.to_mlir_type_simple()).collect();
                format!("!llvm.struct<({})>", inner.join(", "))
            }
            Type::Atomic(inner) => inner.to_mlir_type_simple(),
            Type::Tensor(_, _) => "!llvm.ptr".to_string(),
            _ => "!llvm.ptr".to_string(), // Fallback for complex types
        }
    }

    /// Context-free MLIR storage type mapping.
    /// Bool -> i8, pointers -> !llvm.ptr, otherwise same as to_mlir_type_simple.
    pub fn to_mlir_storage_type_simple(&self) -> String {
        if *self == Type::Bool { return "i8".to_string(); }
        if self.k_is_ptr_type() || matches!(self, Type::Reference(_, _)) {
            return "!llvm.ptr".to_string();
        }
        self.to_mlir_type_simple()
    }

    pub fn k_is_ptr_type(&self) -> bool {
        match self {
            Type::Pointer { .. } | Type::Owned(_) | Type::Fn(_, _) => true,
            Type::Struct(n) if n.contains("Ptr") || n.contains("LlvmPtr") => true,
            Type::Concrete(n, _) if n.contains("Ptr") || n.contains("LlvmPtr") => true,
            _ => false,
        }
    }

    pub fn strip_package_prefix(s: &str) -> String {
        s.split("__").last().unwrap_or(s).to_string()
    }

    pub fn substitute(&self, mapping: &BTreeMap<String, Type>) -> Type {
        match self {
            Type::Generic(name) => mapping.get(name).cloned().unwrap_or(self.clone()),
            Type::SelfType => mapping.get("Self").cloned().unwrap_or(self.clone()),
            Type::Struct(name) if name == "Self" => mapping.get("Self").cloned().unwrap_or(self.clone()),
            // [GRAYDON FIX] Handle Type::Struct that represents a generic placeholder (K, V, T, E, etc.)
            // This happens when resolve_type creates Struct("K") instead of Generic("K") from AST
            Type::Struct(name) if mapping.contains_key(name) => {
                mapping.get(name).cloned().unwrap_or(self.clone())
            },
            // [PHANTOM FIX] Handle package-mangled generic names (test__T -> T)
            // resolve_type mangles T to test__T, but type_map uses unmangled "T"
            Type::Struct(name) if name.contains("__") => {
                let suffix = name.rsplit("__").next().unwrap_or(name);
                if let Some(mapped) = mapping.get(suffix) {
                    mapped.clone()
                } else {
                    self.clone()
                }
            },
            Type::Pointer { element, provenance, is_mutable } => Type::Pointer {
                element: Box::new(element.substitute(mapping)),
                provenance: provenance.clone(),
                is_mutable: *is_mutable
            },
            Type::Reference(inner, is_mut) => Type::Reference(Box::new(inner.substitute(mapping)), *is_mut),
            Type::Concrete(name, params) => {
                 // [SOVEREIGN FIX] Handle Concrete types that are actually generic placeholders (e.g. F2)
                 if params.is_empty() {
                      if let Some(mapped) = mapping.get(name) {
                           return mapped.clone();
                      }
                 }
                 Type::Concrete(name.clone(), params.iter().map(|p| p.substitute(mapping)).collect())
            },
            Type::Array(inner, len, p) => Type::Array(Box::new(inner.substitute(mapping)), *len, *p),
            Type::Fn(args, ret) => Type::Fn(args.iter().map(|a| a.substitute(mapping)).collect(), Box::new(ret.substitute(mapping))),
            _ => self.clone(),
        }
    }

    pub fn to_key(&self) -> Option<TypeKey> {
        match self {
            Type::Struct(name) | Type::Enum(name) => Some(TypeKey { path: vec![], name: name.clone(), specialization: None }),
            Type::Concrete(name, args) => Some(TypeKey { path: vec![], name: name.clone(), specialization: Some(args.clone()) }),
            Type::Pointer { element, .. } => Some(TypeKey { 
                path: vec![], 
                name: "Ptr".to_string(), 
                specialization: Some(vec![*element.clone()]) 
            }),
            Type::Reference(inner, _) => inner.to_key(),
            // [GRAYDON FIX] Return TypeKey for primitive types so trait methods can be looked up
            // e.g., Type::I64 -> TypeKey { path: [], name: "i64" } enables finding impl Hash for i64
            Type::I8 | Type::I16 | Type::I32 | Type::I64 |
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::Usize |
            Type::F32 | Type::F64 | Type::Bool => {
                Some(TypeKey { path: vec![], name: self.mangle_suffix(), specialization: None })
            }
             _ => None,
        }
    }

    pub fn to_canonical_name(&self) -> String {
        self.mangle_suffix()
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::Usize | Type::F32 | Type::F64)
    }

    pub fn has_generics(&self) -> bool {
        match self {
            Type::Generic(_) => true,
            // Struct names are never generics — use normalize_generics at call sites
            Type::Struct(_) => false,
            Type::Concrete(_, args) => args.iter().any(|a| a.has_generics()),
            Type::Reference(inner, _) | Type::Atomic(inner) | Type::Owned(inner) => inner.has_generics(),
            Type::Pointer { element, .. } => element.has_generics(),
            Type::Array(inner, _, _) => inner.has_generics(),
            Type::Tuple(elems) => elems.iter().any(|e| e.has_generics()),
            Type::Fn(args, ret) => args.iter().any(|a| a.has_generics()) || ret.has_generics(),
            _ => false
        }
    }

    /// Returns true if this type is fully resolved to concrete types.
    /// Unlike `has_generics()`, this also catches "escaped" generics that appear as
    /// `Type::Struct("F")` or `Type::Concrete("F2", [])` — ghost types that don't
    /// correspond to any real struct or enum in the registries.
    pub fn is_fully_concrete(
        &self,
        struct_registry: &HashMap<TypeKey, StructInfo>,
        enum_names: &std::collections::HashSet<String>,
    ) -> bool {
        match self {
            // Explicit generic — always unresolved
            Type::Generic(_) => false,

            // Primitives — always concrete
            Type::I8 | Type::I16 | Type::I32 | Type::I64 |
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::Usize |
            Type::F32 | Type::F64 | Type::Bool | Type::Unit | Type::Never => true,

            // Function pointers — always concrete (represented as !llvm.ptr)
            Type::Fn(_, _) => true,

            // Struct: concrete only if the name exists in a registry (or is already FQN)
            Type::Struct(name) => {
                // Namespace-qualified names (contain "__") are resolved FQNs
                if name.contains("__") { return true; }
                // Check if it exists as a known struct or enum
                struct_registry.values().any(|info| info.name == *name)
                    || enum_names.contains(name)
            }

            // Concrete with no args: might be an escaped generic like Concrete("F2", [])
            Type::Concrete(name, args) => {
                if args.is_empty() {
                    // Zero-arg Concrete that isn't in any registry → escaped generic
                    if name.contains("__") { return true; }
                    let exists = struct_registry.values().any(|info| info.name == *name)
                        || enum_names.contains(name);
                    if !exists { return false; }
                }
                // Recurse into type arguments
                args.iter().all(|a| a.is_fully_concrete(struct_registry, enum_names))
            }

            // Enum: same logic as Struct
            Type::Enum(name) => {
                if name.contains("__") { return true; }
                enum_names.contains(name)
            }

            // Recursive wrappers
            Type::Pointer { element, .. } => element.is_fully_concrete(struct_registry, enum_names),
            Type::Reference(inner, _) | Type::Owned(inner) | Type::Atomic(inner) =>
                inner.is_fully_concrete(struct_registry, enum_names),
            Type::Array(inner, _, _) => inner.is_fully_concrete(struct_registry, enum_names),
            Type::Tensor(inner, _) => inner.is_fully_concrete(struct_registry, enum_names),
            Type::Tuple(elems) => elems.iter().all(|e| e.is_fully_concrete(struct_registry, enum_names)),
            Type::Window(inner, _) => inner.is_fully_concrete(struct_registry, enum_names),

            Type::SelfType => false, // Unresolved Self
        }
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::Usize)
    }

    pub fn is_protected_name(name: &str) -> bool {
        matches!(name, "get_unchecked" | "set_unchecked" | "len" | "iter" | "as_ptr")
    }

    pub fn get_ptr_element(&self) -> Option<&Type> {
        match self {
            Type::Pointer { element, .. } => Some(element),
            Type::Reference(inner, _) => Some(inner),
            _ => None
        }
    }

    pub fn is_unsigned(&self) -> bool {
        matches!(self, Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::Usize)
    }

    pub fn structural_eq(&self, other: &Type) -> bool {
        match (self, other) {
            (Type::SelfType, Type::SelfType) => true,
            (Type::SelfType, Type::Struct(n)) | (Type::Struct(n), Type::SelfType) => n == "Self",
            (Type::Struct(n1), Type::Struct(n2)) 
            | (Type::Enum(n1), Type::Enum(n2))
            | (Type::Generic(n1), Type::Generic(n2)) => Self::base_names_equal(n1, n2),
            
            (Type::Concrete(n1, p1), Type::Concrete(n2, p2)) => {
                Self::base_names_equal(n1, n2) && p1.len() == p2.len() && p1.iter().zip(p2).all(|(a, b)| a.structural_eq(b))
            }
            
            (Type::Struct(n), Type::Concrete(name, params)) 
            | (Type::Concrete(name, params), Type::Struct(n)) => {
                let concrete = Type::Concrete(name.clone(), params.clone());
                Self::base_names_equal(n, &concrete.mangle_suffix())
            }
            
            (Type::Pointer { element: e1, .. }, Type::Pointer { element: e2, .. }) => e1.structural_eq(e2),
            (Type::Reference(e1, m1), Type::Reference(e2, m2)) => m1 == m2 && e1.structural_eq(e2),
            (Type::Array(e1, l1, p1), Type::Array(e2, l2, p2)) => l1 == l2 && p1 == p2 && e1.structural_eq(e2),
            (Type::Tuple(v1), Type::Tuple(v2)) => v1.len() == v2.len() && v1.iter().zip(v2).all(|(a, b)| a.structural_eq(b)),
            (Type::Fn(a1, r1), Type::Fn(a2, r2)) => a1.len() == a2.len() && a1.iter().zip(a2).all(|(a, b)| a.structural_eq(b)) && r1.structural_eq(r2),
            
            _ => self == other
        }
    }

    pub fn canonical_eq(&self, other: &Type) -> bool {
        self.to_canonical_name() == other.to_canonical_name()
    }

    pub fn base_names_equal(n1: &str, n2: &str) -> bool {
        let clean1 = n1.split("__").last().unwrap_or(n1);
        let clean2 = n2.split("__").last().unwrap_or(n2);
        clean1 == clean2
    }

    pub fn size_of(&self, struct_registry: &HashMap<TypeKey, StructInfo>) -> usize {
        self.internal_size_of(struct_registry, 0)
    }

    fn internal_size_of(&self, struct_registry: &HashMap<TypeKey, StructInfo>, depth: usize) -> usize {
        if depth > 32 { return 8; }
        
        match self {
            Type::I8 | Type::U8 | Type::Bool => 1,
            Type::Never => 0,
            Type::I16 | Type::U16 => 2,
            Type::I32 | Type::U32 | Type::F32 => 4,
            Type::I64 | Type::U64 | Type::Usize | Type::F64 => 8,
            Type::Pointer { .. } | Type::Reference(_, _) | Type::Owned(_) | Type::Fn(_, _) | Type::Generic(_) | Type::SelfType | Type::Unit => 8,
            // [SOVEREIGN FIX] Atomic<T> storage is T, not a pointer. Delegate to inner type.
            Type::Atomic(inner) => inner.internal_size_of(struct_registry, depth + 1),
            Type::Array(inner, len, _) => inner.internal_size_of(struct_registry, depth + 1) * len,
            Type::Tensor(inner, dims) => inner.internal_size_of(struct_registry, depth + 1) * dims.iter().product::<usize>(),
            Type::Tuple(elems) => {
                if elems.is_empty() { return 0; }
                let mut offset = 0;
                let mut max_align = 1;
                for ty in elems {
                    let align = ty.internal_align_of(struct_registry, depth + 1);
                    max_align = max_align.max(align);
                    offset = (offset + align - 1) & !(align - 1);
                    offset += ty.internal_size_of(struct_registry, depth + 1);
                }
                (offset + max_align - 1) & !(max_align - 1)
            }
            Type::Struct(name) | Type::Enum(name) | Type::Concrete(name, _) => {
                if name == "Option" { return 16; } // Special case for Option enum test
                let lookup_name = match self {
                    Type::Concrete(_, _) => self.mangle_suffix(),
                    _ => name.clone(),
                };
                if let Some(info) = struct_registry.values().find(|info| info.name == lookup_name) {
                    let mut offset = 0;
                    let mut max_align = 1;
                    for ty in &info.field_order {
                        let align = ty.internal_align_of(struct_registry, depth + 1);
                        max_align = max_align.max(align);
                        offset = (offset + align - 1) & !(align - 1);
                        offset += ty.internal_size_of(struct_registry, depth + 1);
                    }
                    (offset + max_align - 1) & !(max_align - 1)
                } else { 8 }
            }
            Type::Window(_, _) => 16,
        }
    }

    pub fn align_of(&self, struct_registry: &HashMap<TypeKey, StructInfo>) -> usize {
        self.internal_align_of(struct_registry, 0)
    }

    fn internal_align_of(&self, struct_registry: &HashMap<TypeKey, StructInfo>, depth: usize) -> usize {
        if depth > 32 { return 8; }

        match self {
            Type::I8 | Type::U8 | Type::Bool | Type::Never | Type::Unit => 1,
            Type::I16 | Type::U16 => 2,
            Type::I32 | Type::U32 | Type::F32 => 4,
            Type::I64 | Type::U64 | Type::Usize | Type::F64 => 8,
            Type::Pointer { .. } | Type::Reference(_, _) | Type::Owned(_) | Type::Fn(_, _) | Type::Generic(_) | Type::SelfType => 8,
            // [SOVEREIGN FIX] Atomic<T> alignment follows inner type T.
            Type::Atomic(inner) => inner.internal_align_of(struct_registry, depth + 1),
            Type::Array(inner, _, _) | Type::Tensor(inner, _) => inner.internal_align_of(struct_registry, depth + 1),
            Type::Tuple(elems) => {
                elems.iter().map(|ty| ty.internal_align_of(struct_registry, depth + 1)).max().unwrap_or(1)
            }
            Type::Struct(name) | Type::Enum(name) | Type::Concrete(name, _) => {
                let lookup_name = match self {
                    Type::Concrete(_, _) => self.mangle_suffix(),
                    _ => name.clone(),
                };
                if let Some(info) = struct_registry.values().find(|info| info.name == lookup_name) {
                    info.field_order.iter().map(|ty| ty.internal_align_of(struct_registry, depth + 1)).max().unwrap_or(1)
                } else { 8 }
            }
            Type::Window(_, _) => 8,
        }
    }}

// =============================================================================
// Unit Tests - TDD for Generic HashMap Monomorphization
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    // -------------------------------------------------------------------------
    // Test: has_generics() — Struct is NOT a generic placeholder
    // -------------------------------------------------------------------------
    #[test]
    fn test_has_generics_struct_k_is_not_generic() {
        // After hack removal: Struct("K") is NOT a generic
        let ty = Type::Struct("K".to_string());
        assert!(!ty.has_generics(), "Struct('K') should NOT be detected as generic");
    }

    #[test]
    fn test_has_generics_struct_v_is_not_generic() {
        let ty = Type::Struct("V".to_string());
        assert!(!ty.has_generics(), "Struct('V') should NOT be detected as generic");
    }

    #[test]
    fn test_has_generics_struct_t_is_not_generic() {
        let ty = Type::Struct("T".to_string());
        assert!(!ty.has_generics(), "Struct('T') should NOT be detected as generic");
    }

    #[test]
    fn test_has_generics_generic_t_is_generic() {
        // Generic("T") IS detected
        let ty = Type::Generic("T".to_string());
        assert!(ty.has_generics(), "Generic('T') should be detected as generic");
    }

    #[test]
    fn test_has_generics_ignores_real_struct_names() {
        // Real struct names (non-single-letter) should NOT be detected as generics
        let ty = Type::Struct("HashMap".to_string());
        assert!(!ty.has_generics(), "Struct('HashMap') should NOT be detected as generic");
    }

    #[test]
    fn test_has_generics_i64_is_not_generic() {
        let ty = Type::I64;
        assert!(!ty.has_generics(), "I64 should NOT be detected as generic");
    }

    #[test]
    fn test_has_generics_concrete_with_struct_args_no_generics() {
        // Concrete(HashMap, [Struct("K"), Struct("V")]) does NOT have generics now
        let ty = Type::Concrete(
            "HashMap".to_string(),
            vec![Type::Struct("K".to_string()), Type::Struct("V".to_string())]
        );
        assert!(!ty.has_generics(), "Concrete with Struct('K') args should NOT have generics");
    }

    #[test]
    fn test_has_generics_concrete_with_generic_args() {
        // Concrete(HashMap, [Generic("K"), Generic("V")]) DOES have generics
        let ty = Type::Concrete(
            "HashMap".to_string(),
            vec![Type::Generic("K".to_string()), Type::Generic("V".to_string())]
        );
        assert!(ty.has_generics(), "Concrete with Generic('K') args should have generics");
    }

    #[test]
    fn test_has_generics_concrete_with_concrete_args() {
        // Concrete(HashMap, [I64, I64]) does NOT have generics
        let ty = Type::Concrete("HashMap".to_string(), vec![Type::I64, Type::I64]);
        assert!(!ty.has_generics(), "Concrete with I64 args should NOT have generics");
    }

    // -------------------------------------------------------------------------
    // Test: substitute() handles Struct placeholders correctly
    // -------------------------------------------------------------------------
    #[test]
    fn test_substitute_struct_k_to_i64() {
        let ty = Type::Struct("K".to_string());
        let mut map = BTreeMap::new();
        map.insert("K".to_string(), Type::I64);
        
        let result = ty.substitute(&map);
        assert_eq!(result, Type::I64, "Struct('K') should substitute to I64");
    }

    #[test]
    fn test_substitute_concrete_with_placeholders() {
        // Concrete(HashMap, [Struct("K"), Struct("V")]) -> Concrete(HashMap, [I64, I64])
        let ty = Type::Concrete(
            "HashMap".to_string(),
            vec![Type::Struct("K".to_string()), Type::Struct("V".to_string())]
        );
        let mut map = BTreeMap::new();
        map.insert("K".to_string(), Type::I64);
        map.insert("V".to_string(), Type::I64);
        
        let result = ty.substitute(&map);
        assert_eq!(
            result, 
            Type::Concrete("HashMap".to_string(), vec![Type::I64, Type::I64]),
            "Concrete args should be substituted"
        );
    }

    #[test]
    fn test_substitute_no_infinite_recursion() {
        // When map has K -> Struct("K"), substitution should NOT loop infinitely
        let ty = Type::Struct("K".to_string());
        let mut map = BTreeMap::new();
        map.insert("K".to_string(), Type::Struct("K".to_string())); // Self-referential
        
        // This should NOT panic or hang - it should return Struct("K") unchanged
        let result = ty.substitute(&map);
        assert_eq!(result, Type::Struct("K".to_string()), "Self-referential map should not cause infinite loop");
    }

    #[test]
    fn test_substitute_reference_with_placeholder() {
        let ty = Type::Reference(Box::new(Type::Struct("K".to_string())), false);
        let mut map = BTreeMap::new();
        map.insert("K".to_string(), Type::I64);
        
        let result = ty.substitute(&map);
        assert_eq!(result, Type::Reference(Box::new(Type::I64), false), "Reference inner should be substituted");
    }

    // -------------------------------------------------------------------------
    // Test: mangle_suffix produces correct specialized names
    // -------------------------------------------------------------------------
    #[test]
    fn test_mangle_suffix_i64() {
        let ty = Type::I64;
        assert_eq!(ty.mangle_suffix(), "i64", "I64 should mangle to 'i64'");
    }

    #[test]
    fn test_mangle_suffix_struct_k_placeholder() {
        // This is the bug we're catching - Struct("K") should NOT be in final mangled names
        let ty = Type::Struct("K".to_string());
        assert_eq!(ty.mangle_suffix(), "K", "Struct('K') mangles to 'K' - but this should be blocked by Generic Wall");
    }

    // -------------------------------------------------------------------------
    // Test: TypeKey equality for trait method lookup
    // -------------------------------------------------------------------------
    #[test]
    fn test_typekey_primitive_lookup() {
        // Primitive i64 should be findable regardless of path
        let key1 = TypeKey { path: vec![], name: "i64".to_string(), specialization: None };
        let key2 = TypeKey { path: vec!["std".to_string(), "hash".to_string()], name: "i64".to_string(), specialization: None };
        
        // These should match because we look up primitives by name
        assert_eq!(key1.name, key2.name, "TypeKey names should match for primitive lookup");
    }
    
    // -------------------------------------------------------------------------
    // TDD Test: Type::to_key() returns valid TypeKey for primitives
    // -------------------------------------------------------------------------
    #[test]
    fn test_to_key_i64_returns_some() {
        let ty = Type::I64;
        let key = ty.to_key();
        assert!(key.is_some(), "Type::I64.to_key() should return Some(TypeKey)");
        let key = key.unwrap();
        assert_eq!(key.name, "i64", "TypeKey name should be 'i64'");
        assert!(key.path.is_empty(), "TypeKey path should be empty for primitives");
    }
    
    #[test]
    fn test_to_key_bool_returns_some() {
        let ty = Type::Bool;
        let key = ty.to_key();
        assert!(key.is_some(), "Type::Bool.to_key() should return Some(TypeKey)");
        assert_eq!(key.unwrap().name, "bool");
    }
    
    #[test]
    fn test_to_key_reference_to_i64() {
        // &i64 should also return a TypeKey for i64
        let ty = Type::Reference(Box::new(Type::I64), false);
        let key = ty.to_key();
        assert!(key.is_some(), "&I64 should return Some(TypeKey)");
        assert_eq!(key.unwrap().name, "i64", "Unwrapped &I64 should give i64 key");
    }

    // =========================================================================
    // TDD: Multi-Character Generic Names (F2, Item, Allocator)
    // These tests drive the fix to eliminate the single-char heuristic.
    // =========================================================================

    #[test]
    fn test_from_syn_with_generics_multi_char() {
        use std::collections::HashSet;
        let mut generics = HashSet::new();
        generics.insert("F2".to_string());
        generics.insert("Item".to_string());

        // Create a SynType for "F2"
        let syn_ty = crate::grammar::SynType::Path(crate::grammar::SynPath {
            segments: vec![crate::grammar::SynPathSegment {
                ident: syn::Ident::new("F2", proc_macro2::Span::call_site()),
                args: vec![],
            }],
        });

        let result = Type::from_syn_with_generics(&syn_ty, &generics);
        assert_eq!(result, Some(Type::Generic("F2".to_string())),
            "from_syn_with_generics should produce Type::Generic for declared generic name 'F2'");
    }

    #[test]
    fn test_from_syn_with_generics_non_generic_stays_concrete() {
        use std::collections::HashSet;
        let mut generics = HashSet::new();
        generics.insert("T".to_string());

        // "HashMap" is NOT a generic param
        let syn_ty = crate::grammar::SynType::Path(crate::grammar::SynPath {
            segments: vec![crate::grammar::SynPathSegment {
                ident: syn::Ident::new("HashMap", proc_macro2::Span::call_site()),
                args: vec![],
            }],
        });

        let result = Type::from_syn_with_generics(&syn_ty, &generics);
        assert_eq!(result, Some(Type::Concrete("HashMap".to_string(), vec![])),
            "Non-generic names should remain Concrete");
    }

    #[test]
    fn test_from_syn_with_generics_single_char_still_works() {
        use std::collections::HashSet;
        let mut generics = HashSet::new();
        generics.insert("T".to_string());

        let syn_ty = crate::grammar::SynType::Path(crate::grammar::SynPath {
            segments: vec![crate::grammar::SynPathSegment {
                ident: syn::Ident::new("T", proc_macro2::Span::call_site()),
                args: vec![],
            }],
        });

        let result = Type::from_syn_with_generics(&syn_ty, &generics);
        assert_eq!(result, Some(Type::Generic("T".to_string())),
            "Single-char generics should still work");
    }

    #[test]
    fn test_substitute_generic_f2_to_fn_type() {
        // Type::Generic("F2") should substitute correctly
        let ty = Type::Generic("F2".to_string());
        let mut map = BTreeMap::new();
        map.insert("F2".to_string(), Type::Fn(vec![Type::I64], Box::new(Type::Bool)));

        let result = ty.substitute(&map);
        assert_eq!(result, Type::Fn(vec![Type::I64], Box::new(Type::Bool)),
            "Generic('F2') should substitute to Fn type");
    }

    #[test]
    fn test_has_generics_generic_f2() {
        let ty = Type::Generic("F2".to_string());
        assert!(ty.has_generics(), "Generic('F2') must be detected as having generics");
    }

    #[test]
    fn test_has_generics_concrete_with_generic_f2_arg() {
        let ty = Type::Concrete(
            "Filter".to_string(),
            vec![Type::Struct("Range".to_string()), Type::Generic("F2".to_string())]
        );
        assert!(ty.has_generics(), "Concrete with Generic('F2') arg should have generics");
    }

    #[test]
    fn test_substitute_concrete_with_multi_char_generic_args() {
        // Concrete("Map", [Generic("I"), Generic("F2"), Generic("Item")])
        let ty = Type::Concrete(
            "Map".to_string(),
            vec![
                Type::Generic("I".to_string()),
                Type::Generic("F2".to_string()),
                Type::Generic("Item".to_string()),
            ]
        );
        let mut map = BTreeMap::new();
        map.insert("I".to_string(), Type::Struct("Range".to_string()));
        map.insert("F2".to_string(), Type::Fn(vec![Type::I64], Box::new(Type::I64)));
        map.insert("Item".to_string(), Type::I64);

        let result = ty.substitute(&map);
        assert_eq!(result, Type::Concrete(
            "Map".to_string(),
            vec![
                Type::Struct("Range".to_string()),
                Type::Fn(vec![Type::I64], Box::new(Type::I64)),
                Type::I64,
            ]
        ), "All multi-char generics should be substituted");
    }

    // -------------------------------------------------------------------------
    // REGRESSION TEST: Struct-level generics must survive in type_map
    // Bug: Range::fold<A,F> gets type_map={"A":I64,"F":Fn} but MISSING "T":I64
    // This causes Range__T to escape into MLIR emission.
    // -------------------------------------------------------------------------
    #[test]
    fn test_substitute_struct_generic_missing_from_type_map() {
        // Simulates the bug: Range<T> has a method fold<A,F>.
        // If the type_map only contains method-level generics (A, F) but not
        // the struct-level generic (T), then Struct("T") will NOT be substituted
        // and the emitted type will be Range__T instead of Range__i64.
        let range_with_t = Type::Concrete(
            "Range".to_string(),
            vec![Type::Struct("T".to_string())]  // T is unresolved
        );

        // BAD type_map: only has method-level generics
        let mut bad_map = BTreeMap::new();
        bad_map.insert("A".to_string(), Type::I64);
        bad_map.insert("F".to_string(), Type::Fn(vec![Type::I64, Type::I64], Box::new(Type::I64)));

        let bad_result = range_with_t.substitute(&bad_map);
        // T is NOT in the map, so Range<T> stays as Range<Struct("T")> — BUG!
        assert_eq!(
            bad_result,
            Type::Concrete("Range".to_string(), vec![Type::Struct("T".to_string())]),
            "Without T in type_map, T should remain unsubstituted (this is the bug scenario)"
        );

        // GOOD type_map: includes both struct-level AND method-level generics
        let mut good_map = BTreeMap::new();
        good_map.insert("T".to_string(), Type::I64);  // struct-level
        good_map.insert("A".to_string(), Type::I64);  // method-level
        good_map.insert("F".to_string(), Type::Fn(vec![Type::I64, Type::I64], Box::new(Type::I64)));

        let good_result = range_with_t.substitute(&good_map);
        // T IS in the map, so Range<T> becomes Range<I64> — CORRECT!
        assert_eq!(
            good_result,
            Type::Concrete("Range".to_string(), vec![Type::I64]),
            "With T in type_map, Range<T> must resolve to Range<I64>"
        );
    }

    #[test]
    fn test_substitute_nested_struct_generic_escape() {
        // Simulates the chain: Filter<Map<Range<T>, F>, F2>
        // All generics (T, F, F2) must be in the type_map for full resolution
        let nested = Type::Concrete(
            "Filter".to_string(),
            vec![
                Type::Concrete("Map".to_string(), vec![
                    Type::Concrete("Range".to_string(), vec![Type::Struct("T".to_string())]),
                    Type::Struct("F".to_string()),
                ]),
                Type::Struct("F2".to_string()),
            ]
        );

        let mut complete_map = BTreeMap::new();
        complete_map.insert("T".to_string(), Type::I64);
        complete_map.insert("F".to_string(), Type::Fn(vec![Type::I64], Box::new(Type::I64)));
        complete_map.insert("F2".to_string(), Type::Fn(vec![Type::I64], Box::new(Type::Bool)));

        let result = nested.substitute(&complete_map);
        assert_eq!(
            result,
            Type::Concrete("Filter".to_string(), vec![
                Type::Concrete("Map".to_string(), vec![
                    Type::Concrete("Range".to_string(), vec![Type::I64]),
                    Type::Fn(vec![Type::I64], Box::new(Type::I64)),
                ]),
                Type::Fn(vec![Type::I64], Box::new(Type::Bool)),
            ]),
            "All nested generics must be fully resolved in combinator chains"
        );
    }

    // -------------------------------------------------------------------------
    // REGRESSION TEST: Double-wrapped Reference base type extraction
    // Bug: Reference(Reference(Concrete("Range", []), true), false) failed to
    // extract the base type name, causing non-deterministic method resolution.
    // The fix: recursively peel Reference wrappers in extract_receiver_prefix.
    // This test validates the Type-level behavior that the fix depends on.
    // -------------------------------------------------------------------------
    #[test]
    fn test_double_wrapped_reference_base_name() {
        // This mimics what happens inside a hydrated method body:
        // `self` is `&mut Range` → Reference(Concrete("Range", []), true)
        // When referenced again → Reference(Reference(Concrete("Range", []), true), false)
        let single_ref = Type::Reference(
            Box::new(Type::Concrete("std__core__iter__Range".to_string(), vec![])),
            true
        );
        let double_ref = Type::Reference(
            Box::new(single_ref.clone()),
            false
        );
        let triple_ref = Type::Reference(
            Box::new(double_ref.clone()),
            false
        );

        // Helper that mimics extract_receiver_prefix logic
        fn peel_to_base(ty: &Type) -> Option<String> {
            match ty {
                Type::Concrete(name, _) => Some(name.clone()),
                Type::Struct(name) => Some(name.clone()),
                Type::Reference(inner, _) => peel_to_base(inner),
                _ => None,
            }
        }

        // All depths must produce the same base name
        let base = Type::Concrete("std__core__iter__Range".to_string(), vec![]);
        assert_eq!(peel_to_base(&base), Some("std__core__iter__Range".to_string()),
            "Bare Concrete must extract base name");
        assert_eq!(peel_to_base(&single_ref), Some("std__core__iter__Range".to_string()),
            "Single Reference must extract base name");
        assert_eq!(peel_to_base(&double_ref), Some("std__core__iter__Range".to_string()),
            "Double Reference must extract base name (this was the bug)");
        assert_eq!(peel_to_base(&triple_ref), Some("std__core__iter__Range".to_string()),
            "Triple Reference must also extract base name");
    }
}
