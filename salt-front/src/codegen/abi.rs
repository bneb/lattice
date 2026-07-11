use crate::types::{Type};
use crate::codegen::context::LoweringContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scalar {
    I1,
    I8,
    I16,
    I32,
    I64,
    Index, // MLIR index type for affine loop inductives
    F32,
    F64,
    Ptr,   // First-class Register Pointer
}

impl Scalar {
    pub fn to_mlir(&self, storage: bool) -> &'static str {
        match self {
            Scalar::I1 => if storage { "i8" } else { "i1" },
            Scalar::I8 => "i8",
            Scalar::I16 => "i16",
            Scalar::I32 => "i32",
            Scalar::I64 => "i64",
            Scalar::Index => "index",
            Scalar::F32 => "f32",
            Scalar::F64 => "f64",
            Scalar::Ptr => "!llvm.ptr",
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Scalar::I1 | Scalar::I8 => 1,
            Scalar::I16 => 2,
            Scalar::I32 | Scalar::F32 => 4,
            Scalar::I64 | Scalar::Index | Scalar::F64 | Scalar::Ptr => 8,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LayoutKind {
    Scalar(Scalar),
    Aggregate(Vec<Layout>),
    Array(Box<Layout>, usize),
    PackedArray(usize),
    Void,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub size: usize,
    pub align: usize,
    pub kind: LayoutKind,
}

impl Layout {
    pub fn new(kind: LayoutKind, size: usize, align: usize) -> Self {
        Self { kind, size, align }
    }

    #[allow(clippy::only_used_in_recursion)] // pub API: ctx passed for recursive calls
    pub fn to_mlir_storage(&self, ctx: &mut LoweringContext) -> String {
        match &self.kind {
            LayoutKind::Scalar(s) => s.to_mlir(true).to_string(),
            LayoutKind::Aggregate(fields) => {
                let field_tys: Vec<String> = fields.iter().map(|f| f.to_mlir_storage(ctx)).collect();
                format!("!llvm.struct<({})>", field_tys.join(", "))
            },
            LayoutKind::Array(inner, len) => {
                format!("!llvm.array<{} x {}>", len, inner.to_mlir_storage(ctx))
            },
            LayoutKind::PackedArray(len) => {
                let words = len.div_ceil(64);
                format!("!llvm.array<{} x i64>", words)
            },
            LayoutKind::Void => "!llvm.void".to_string(),
        }
    }
    
    #[allow(clippy::only_used_in_recursion)] // pub API: ctx passed for recursive calls
    pub fn to_mlir_logical(&self, ctx: &mut LoweringContext) -> String {
        match &self.kind {
            LayoutKind::Scalar(s) => s.to_mlir(false).to_string(),
            LayoutKind::Aggregate(fields) => {
                let field_tys: Vec<String> = fields.iter().map(|f| f.to_mlir_logical(ctx)).collect();
                format!("!llvm.struct<({})>", field_tys.join(", "))
            },
            LayoutKind::Array(inner, len) => {
                format!("!llvm.array<{} x {}>", len, inner.to_mlir_logical(ctx))
            },
            LayoutKind::PackedArray(len) => {
                let words = len.div_ceil(64);
                format!("!llvm.array<{} x i64>", words)
            },
            LayoutKind::Void => "!llvm.void".to_string(),
        }
    }

    pub fn compute(ctx: &mut LoweringContext, ty: &Type) -> Self {
        // 
        // We check k_is_ptr_type() FIRST. 
        // This ensures Ptr<T> and Reference(&T) are immediately flattened to scalars.
        if ty.k_is_ptr_type() {
            return Layout::new(LayoutKind::Scalar(Scalar::Ptr), 8, 8);
        }
        
        match ty {
            Type::I8 | Type::U8 => Layout::new(LayoutKind::Scalar(Scalar::I8), 1, 1),
            Type::I16 | Type::U16 => Layout::new(LayoutKind::Scalar(Scalar::I16), 2, 2),
            Type::I32 | Type::U32 => Layout::new(LayoutKind::Scalar(Scalar::I32), 4, 4),
            Type::I64 | Type::U64 => Layout::new(LayoutKind::Scalar(Scalar::I64), 8, 8),
            Type::Usize => Layout::new(LayoutKind::Scalar(Scalar::Index), 8, 8),
            Type::F32 => Layout::new(LayoutKind::Scalar(Scalar::F32), 4, 4),
            Type::F64 => Layout::new(LayoutKind::Scalar(Scalar::F64), 8, 8),
            Type::Bool => Layout::new(LayoutKind::Scalar(Scalar::I1), 1, 1),
            
            // Unify all handle-like types into register-native pointers
            Type::Owned(..) | Type::Fn(..) | Type::Atomic(..) | Type::Window(..) => {
                Layout::new(LayoutKind::Scalar(Scalar::Ptr), 8, 8)
            }
            
            Type::Array(inner, len, packed) => {
                if *packed && **inner == Type::Bool {
                     let size = (*len).div_ceil(8);
                     Layout::new(LayoutKind::PackedArray(*len), size, 8)
                } else {
                    let inner_layout = Layout::compute(ctx, inner);
                    let size = inner_layout.size * len;
                    Layout::new(LayoutKind::Array(Box::new(inner_layout.clone()), *len), size, inner_layout.align)
                }
            }
            
            Type::Tuple(elems) => {
                let mut offset = 0;
                let mut align = 1;
                let mut fields = Vec::new();
                for e in elems {
                    let l = Layout::compute(ctx, e);
                    align = align.max(l.align);
                    offset = (offset + l.align - 1) & !(l.align - 1);
                    offset += l.size;
                    fields.push(l);
                }
                offset = (offset + align - 1) & !(align - 1);
                Layout::new(LayoutKind::Aggregate(fields), offset, align)
            }

            Type::Struct(_name) => {
                // If the StructRegistry knows about this name (as a specialized template or base struct)
                // we resolve the field-by-field layout.
                if let Some(info) = ctx.lookup_struct_by_type(ty) {
                     let mut fields = Vec::new();
                     let mut align = 1;
                     let mut offset = 0;
                     for f_ty in &info.field_order {
                         let l = Layout::compute(ctx, f_ty);
                         align = align.max(l.align);
                         offset = (offset + l.align - 1) & !(l.align - 1);
                         offset += l.size;
                         fields.push(l);
                     }
                     offset = (offset + align - 1) & !(align - 1);
                     return Layout::new(LayoutKind::Aggregate(fields), offset, align);
                }
                
                // If we are here, we have a structural disconnect.
                // Return void sentinel instead of panicking on malformed input.
                Layout::new(LayoutKind::Void, 0, 1)
            }

            Type::Concrete(base, _) => {
                 // Check if it's a known pointer template (in case k_is_ptr_type missed it)
                 if base == "Ptr" || base.contains("std__core__ptr__Ptr") {
                     return Layout::new(LayoutKind::Scalar(Scalar::Ptr), 8, 8);
                 }

                 // Otherwise, treat as struct and lookup
                 let mangled = ty.mangle_suffix();
                  if let Some(_info) = ctx.find_struct_by_name(&mangled) {
                      // Recurse using the struct logic above
                      return Layout::compute(ctx, &Type::Struct(mangled));
                  }

                 // Return void sentinel instead of panicking on malformed input.
                Layout::new(LayoutKind::Void, 0, 1)
            }
            
            Type::Unit | Type::Never => Layout::new(LayoutKind::Void, 0, 1),
            _ => Layout::new(LayoutKind::Scalar(Scalar::I64), 8, 8),
        }
    }
}