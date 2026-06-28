// Static constant evaluation for global initializers
//
// This module provides a mechanism to evaluate and emit static initialization
// data for global variables. It supports:
// - Scalar literals (integers, floats)
// - Zero-initialized arrays (splatted for efficiency)
// - Aggregate struct fields (recursive)
// - Symbolic references to other globals (for warm boot patching)

use crate::types::Type;

/// Represents a statically-evaluated value for global initialization
#[derive(Debug, Clone)]
pub enum ConstEvalValue {
    /// A scalar integer value with its type
    Scalar(i64, Type),
    /// A scalar float value (f32 or f64)
    Float(f64, Type),
    /// A boolean value
    Bool(bool),
    /// A zero-initialized array (emits `zeroinitializer` in LLVM)
    ZeroArray { elem_ty: Type, count: usize },
    /// An aggregate (struct) with named fields
    Aggregate(Vec<(String, ConstEvalValue)>),
    /// A symbolic reference to another global (requires warm boot patching)
    SymbolicRef(String),
    /// Null pointer
    Null,
    /// A nested struct initialized via constructor call
    StructCall { struct_name: String, specialized: Option<Vec<Type>> },
}

/// A record of a symbolic reference that needs runtime patching
#[derive(Debug, Clone)]
pub struct BootstrapPatch {
    /// The global variable being patched (mangled name)
    pub global_name: String,
    /// The path of field indices to the pointer field
    pub field_path: Vec<usize>,
    /// The MLIR struct types at each level of nesting (for GEP type specification)
    pub struct_types: Vec<String>,
    /// The target symbol whose address will be stored
    pub target_symbol: String,
}

impl ConstEvalValue {
    /// Check if this value requires warm boot patching
    pub fn requires_bootstrap(&self) -> bool {
        match self {
            ConstEvalValue::SymbolicRef(_) => true,
            ConstEvalValue::Aggregate(fields) => {
                fields.iter().any(|(_, v)| v.requires_bootstrap())
            }
            _ => false,
        }
    }

    /// Collect all bootstrap patches needed for this value
    pub fn collect_patches(&self, global_name: &str, path: &mut Vec<usize>, patches: &mut Vec<BootstrapPatch>) {
        match self {
            ConstEvalValue::SymbolicRef(target) => {
                patches.push(BootstrapPatch {
                    global_name: global_name.to_string(),
                    field_path: path.clone(),
                    struct_types: Vec::new(), // NOTE: Caller should populate types if needed
                    target_symbol: target.clone(),
                });
            }
            ConstEvalValue::Aggregate(fields) => {
                for (idx, (_, v)) in fields.iter().enumerate() {
                    path.push(idx);
                    v.collect_patches(global_name, path, patches);
                    path.pop();
                }
            }
            _ => {}
        }
    }
}

/// Emit a static data attribute for an MLIR global initializer
pub fn emit_static_data(value: &ConstEvalValue, mlir_ty: &str) -> String {
    match value {
        ConstEvalValue::Scalar(v, _) => format!("{}", v),
        ConstEvalValue::Float(v, ty) => {
            if matches!(ty, Type::F32) {
                format!("{:.6e} : f32", v)
            } else {
                format!("{:.15e} : f64", v)
            }
        }
        ConstEvalValue::Bool(b) => if *b { "1 : i8".to_string() } else { "0 : i8".to_string() },
        ConstEvalValue::ZeroArray { .. } => {
            // For zero arrays, we use zeroinitializer via region
            format!("dense<0> : {}", mlir_ty)
        }
        ConstEvalValue::Null => "0 : i64".to_string(), // nullptr represented as 0
        ConstEvalValue::SymbolicRef(_) => {
            // Symbolic refs are patched at runtime; emit nullptr placeholder
            "0 : i64".to_string()
        }
        ConstEvalValue::Aggregate(_) | ConstEvalValue::StructCall { .. } => {
            // Struct types use region-based initialization
            "zeroinitializer".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_no_bootstrap() {
        let v = ConstEvalValue::Scalar(42, Type::I64);
        assert!(!v.requires_bootstrap());
    }

    #[test]
    fn test_symbolic_requires_bootstrap() {
        let v = ConstEvalValue::SymbolicRef("other_global".to_string());
        assert!(v.requires_bootstrap());
    }

    #[test]
    fn test_nested_aggregate_bootstrap() {
        let v = ConstEvalValue::Aggregate(vec![
            ("x".to_string(), ConstEvalValue::Scalar(1, Type::I64)),
            ("p".to_string(), ConstEvalValue::SymbolicRef("target".to_string())),
        ]);
        assert!(v.requires_bootstrap());
    }

    #[test]
    fn test_emit_scalar() {
        let v = ConstEvalValue::Scalar(10, Type::I64);
        assert_eq!(emit_static_data(&v, "i64"), "10");
    }

    #[test]
    fn test_emit_bool() {
        assert_eq!(emit_static_data(&ConstEvalValue::Bool(true), "i8"), "1 : i8");
        assert_eq!(emit_static_data(&ConstEvalValue::Bool(false), "i8"), "0 : i8");
    }

    #[test]
    fn test_emit_null() {
        assert_eq!(emit_static_data(&ConstEvalValue::Null, "i64"), "0 : i64");
    }
}
