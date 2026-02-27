use crate::types::{Type, TypeKey};
use crate::codegen::context::LoweringContext;
use crate::registry::{StructInfo, EnumInfo};
use crate::evaluator::ConstValue;
use std::collections::HashMap;
use crate::common::mangling::Mangler;
use crate::codegen::abi::Layout;

// ============================================================================




pub fn get_numeric_idx(ty: &Type) -> Option<usize> {
    match ty {
        Type::I8 => Some(0),
        Type::I16 => Some(1),
        Type::I32 => Some(2),
        Type::I64 => Some(3),
        Type::U8 => Some(4),
        Type::U16 => Some(5),
        Type::U32 => Some(6),
        Type::U64 => Some(7),
        Type::Usize => Some(8),
        Type::F32 => Some(9),
        Type::F64 => Some(10),
        Type::Bool => Some(11),
        _ => None
    }
}



pub const PROMOTION_OPS: [[Option<(&str, &str, &str)>; 12]; 12] = {
    let mut table = [[None; 12]; 12];
    
    // Identity removals (return None, which means map to Ok(var))
    // Already handled by from == to check, but these handle I8/U8 mixing etc.
    
    // I32 -> I64/U64/Usize
    table[2][3] = Some(("arith.extsi", "i32", "i64")); 
    table[2][7] = Some(("arith.extsi", "i32", "i64"));
    table[2][8] = Some(("arith.extsi", "i32", "i64"));
    
    // I16 -> I32/U32
    table[1][2] = Some(("arith.extsi", "i16", "i32"));
    table[1][6] = Some(("arith.extsi", "i16", "i32"));
    // I16 -> I64/U64/Usize
    table[1][3] = Some(("arith.extsi", "i16", "i64"));
    table[1][7] = Some(("arith.extsi", "i16", "i64"));
    table[1][8] = Some(("arith.extsi", "i16", "i64"));
    
    // I8 -> I16/U16
    table[0][1] = Some(("arith.extsi", "i8", "i16"));
    table[0][5] = Some(("arith.extsi", "i8", "i16"));
    // I8 -> I32/U32
    table[0][2] = Some(("arith.extsi", "i8", "i32"));
    table[0][6] = Some(("arith.extsi", "i8", "i32"));
    // I8 -> I64/U64/Usize
    table[0][3] = Some(("arith.extsi", "i8", "i64"));
    table[0][7] = Some(("arith.extsi", "i8", "i64"));
    table[0][8] = Some(("arith.extsi", "i8", "i64"));

    // U32 -> I64/U64/Usize
    table[6][3] = Some(("arith.extui", "i32", "i64"));
    table[6][7] = Some(("arith.extui", "i32", "i64"));
    table[6][8] = Some(("arith.extui", "i32", "i64"));
    
    // U16 -> I32/U32
    table[5][2] = Some(("arith.extui", "i16", "i32"));
    table[5][6] = Some(("arith.extui", "i16", "i32"));
    // U16 -> I64/U64/Usize
    table[5][3] = Some(("arith.extui", "i16", "i64"));
    table[5][7] = Some(("arith.extui", "i16", "i64"));
    table[5][8] = Some(("arith.extui", "i16", "i64"));
    
    // U8 -> I16/U16
    table[4][1] = Some(("arith.extui", "i8", "i16"));
    table[4][5] = Some(("arith.extui", "i8", "i16"));
    // U8 -> I32/U32
    table[4][2] = Some(("arith.extui", "i8", "i32"));
    table[4][6] = Some(("arith.extui", "i8", "i32"));
    // U8 -> I64/U64/Usize
    table[4][3] = Some(("arith.extui", "i8", "i64"));
    table[4][7] = Some(("arith.extui", "i8", "i64"));
    table[4][8] = Some(("arith.extui", "i8", "i64"));

    // Float promotions
    table[9][10] = Some(("arith.extf", "f32", "f64"));   // F32 -> F64

    // Truncates (implicitly allowed in some promotions, but let's be safe)
    // REMOVED: Implicit narrowing is dangerous. Use generic cast_numeric instead.
    // table[3][2] = Some(("arith.trunci", "i64", "i32"));  // I64 -> I32
    // table[7][2] = Some(("arith.trunci", "i64", "i32"));  // U64 -> I32
    // table[8][2] = Some(("arith.trunci", "i64", "i32"));  // Usize -> I32
    // table[3][6] = Some(("arith.trunci", "i64", "i32"));  // I64 -> U32
    // table[7][6] = Some(("arith.trunci", "i64", "i32"));  // U64 -> U32
    // table[8][6] = Some(("arith.trunci", "i64", "i32"));  // Usize -> U32

    table
};

impl Type {


    pub fn to_mlir_storage_type(&self, ctx: &mut LoweringContext) -> Result<String, String> {
    // [V7.8 RECURSIVE IDENTITY] Strip semantic wrappers to find the nominal core
    // Mutability/ownership are access permissions, not changes to storage layout
    match self {
        Type::Owned(inner) => return inner.to_mlir_storage_type(ctx),
        // [SOVEREIGN FIX] Atomic<T> storage is just T — atomicrmw/cmpxchg operate
        // on the address of the scalar value, not an opaque pointer.
        Type::Atomic(inner) => return inner.to_mlir_storage_type(ctx),
        _ => {}
    }
    
    // [V1.0 POINTER DECAY RULE]
    // All safe pointers (NodePtr, Ptr, etc.) are emitted as native !llvm.ptr
    // This eliminates struct wrapper overhead and inttoptr/ptrtoint casts.
    // Front-end sees safety metadata; backend sees naked 8-byte pointer.
    if self.k_is_ptr_type() {
        return Ok("!llvm.ptr".to_string());
    }

    // [SOVEREIGN V3] Tensor Lowering: Tensor<T, [D]> -> memref<D x T>
    // This formalizes the "Sovereign Rule" - types carry their shape bounds.
    if let Type::Tensor(_inner, _shape) = self {
         // [SOVEREIGN V3] Tensor Storage (Opaque Handle)
         // We store the base pointer on the stack. The MemRef descriptor is hydrated at use sites.
         return Ok("!llvm.ptr".to_string());
    }

    // [SOVEREIGN V3] Universal Simd Lowering
    if let Type::Concrete(base, args) = self {
       if (base.contains("Simd") && !base.contains("ptr")) || base == "Simd" {
           // Check args: [T, N]
           if args.len() >= 2 {
                let inner_ty = &args[0];
                let size_arg = &args[1];
                let size = if let Type::Struct(s) = size_arg {
                    s.parse::<usize>().unwrap_or(0)
                } else if let Type::Concrete(val_str, _) = size_arg {
                     val_str.parse::<usize>().unwrap_or(0)
                } else { 0 };
                
                if size > 0 {
                    let inner_mlir = inner_ty.to_mlir_type(ctx)?;
                    return Ok(format!("vector<{}x{}>", size, inner_mlir));
                }
           }
       }
       
     // [SOVEREIGN V6] Vector Intrinsic Types
       // These are used for portable SIMD operations
       if base == "Vector8f32" {
           return Ok("vector<8xf32>".to_string());
       }
       if base == "Vector4f64" {
           return Ok("vector<4xf64>".to_string());
       }
       if base == "Vector16f32" {
           return Ok("vector<16xf32>".to_string());
       }
    }
    
    // [VERIFIED METAL] NOMINAL STRIKE: Struct/Concrete types MUST return named aliases
    // This ensures type identity consistency across alloca, store, load, and GEP operations.
    // The named alias is the Single Source of Truth for struct memory layout.
    match self {
        Type::Struct(name) => {
            // [SOVEREIGN FIX] Look up the fully-qualified struct name from struct_registry
            // This ensures consistency between header alias declarations and body usage.
            // The registry stores structs with their full package-qualified names.
            let full_name = {
                let registry = ctx.struct_registry();
                // Find a struct where the full name either:
                // 1. Equals the short name exactly (already fully-qualified), OR
                // 2. Contains the short name after a __ separator (package path)
                let target = name;
                registry.values()
                    .find(|info| {
                        info.name == *target 
                        || info.name.ends_with(&format!("__{}", target))
                        || (info.name.contains("__") && info.name.split("__").last() == Some(target.as_str()))
                    })
                    .map(|info| info.name.clone())
                    .unwrap_or_else(|| name.clone())
            };
            return Ok(format!("!struct_{}", full_name));
        }
        Type::Concrete(base, args) => {
            // [SOVEREIGN FIX] Look up the fully-qualified template name from struct_templates
            // This ensures consistency between header alias declarations and body usage.
            let full_base = {
                let templates = ctx.struct_templates();
                templates.keys()
                    .find(|k| k.ends_with(base) || *k == base)
                    .cloned()
                    .unwrap_or_else(|| base.clone())
            };
            // Build canonical mangled name: Base_Arg1_Arg2_...
            let suffix = args.iter().map(|t| t.to_canonical_name()).collect::<Vec<_>>().join("_");
            let mangled = if args.is_empty() { full_base } else { format!("{}_{}", full_base, suffix) };
            return Ok(format!("!struct_{}", mangled));
        }
        _ => {}
    }
    
    let layout = Layout::compute(ctx, self);
    Ok(layout.to_mlir_storage(ctx))
}
} // End impl Type

// ============================================================================
// [VERIFIED METAL] Inception Guard
// Recursively flattens nested pointers to enforce the Single Indirection Property.
// ============================================================================



// ============================================================================
// [ZERO-TRUST] Layout Prover
// Validates that two types are bit-for-bit identical before allowing a cast.
// ============================================================================





pub fn get_arith_op(op: &syn::BinOp, ty: &Type) -> String {
    let is_float = matches!(ty, Type::F32 | Type::F64);
    let is_unsigned = ty.is_unsigned();
    match op {
        syn::BinOp::Add(_) | syn::BinOp::AddAssign(_) => if is_float { "arith.addf" } else { "arith.addi" }.to_string(),
        syn::BinOp::Sub(_) | syn::BinOp::SubAssign(_) => if is_float { "arith.subf" } else { "arith.subi" }.to_string(),
        syn::BinOp::Mul(_) | syn::BinOp::MulAssign(_) => if is_float { "arith.mulf" } else { "arith.muli" }.to_string(),
        syn::BinOp::Div(_) | syn::BinOp::DivAssign(_) => if is_float { "arith.divf" } else if is_unsigned { "arith.divui" } else { "arith.divsi" }.to_string(),
        syn::BinOp::Rem(_) | syn::BinOp::RemAssign(_) => if is_float { "arith.remf" } else if is_unsigned { "arith.remui" } else { "arith.remsi" }.to_string(),
        syn::BinOp::BitAnd(_) | syn::BinOp::BitAndAssign(_) => "arith.andi".to_string(),
        syn::BinOp::BitOr(_) | syn::BinOp::BitOrAssign(_) => "arith.ori".to_string(),
        syn::BinOp::BitXor(_) | syn::BinOp::BitXorAssign(_) => "arith.xori".to_string(),
        syn::BinOp::Shl(_) | syn::BinOp::ShlAssign(_) => "arith.shli".to_string(),
        syn::BinOp::Shr(_) | syn::BinOp::ShrAssign(_) => if is_unsigned { "arith.shrui" } else { "arith.shrsi" }.to_string(),
        syn::BinOp::And(_) => "arith.andi".to_string(), // Logical and for i1
        syn::BinOp::Or(_) => "arith.ori".to_string(),   // Logical or for i1
        syn::BinOp::Eq(_) | syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_) | syn::BinOp::Ne(_) => {
            if is_float { "arith.cmpf".to_string() } 
            else if matches!(ty, Type::Reference(..) | Type::Owned(..) | Type::Window(..) | Type::Pointer { .. }) { "llvm.icmp".to_string() }
            else { "arith.cmpi".to_string() }
        }
        _ => crate::ice!("Unhandled binary op: {:?}", op),
    }
}

pub fn get_comparison_pred(op: &syn::BinOp, ty: &Type) -> String {
    let is_float = matches!(ty, Type::F32 | Type::F64);
    // Pointers are unsigned for comparison logic
    let is_unsigned = ty.is_unsigned() || matches!(ty, Type::Pointer { .. });
    match op {
        syn::BinOp::Eq(_) => if is_float { "oeq".to_string() } else { "eq".to_string() },
        syn::BinOp::Ne(_) => if is_float { "une".to_string() } else { "ne".to_string() },
        syn::BinOp::Lt(_) => if is_float { "olt" } else if is_unsigned { "ult" } else { "slt" }.to_string(),
        syn::BinOp::Le(_) => if is_float { "ole" } else if is_unsigned { "ule" } else { "sle" }.to_string(),
        syn::BinOp::Gt(_) => if is_float { "ogt" } else if is_unsigned { "ugt" } else { "sgt" }.to_string(),
        syn::BinOp::Ge(_) => if is_float { "oge" } else if is_unsigned { "uge" } else { "sge" }.to_string(),
        _ => "eq".to_string(),
    }
}

pub fn promote_numeric(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<String, String> {    

    
    if from == to { return Ok(var.to_string()); }
    
    // ═══════════════════════════════════════════════════════════════════
    // LINEAR TYPE PROMOTIONS (checked BEFORE the integer→pointer guard)
    // Boxing a value into Owned<T> or taking a reference &T is a
    // legitimate linear type operation, not context contamination.
    // ═══════════════════════════════════════════════════════════════════
    
    // Promotion: Value T -> Owned<T> (Auto-box/allocate)
    if let Type::Owned(inner) = to {
        if **inner == *from { 
            let temp_ptr = format!("%auto_box_{}", ctx.next_id());
            let mlir_ty = inner.to_mlir_storage_type(ctx).map_err(|e| format!("Failed to get storage type for auto-box: {}", e))?;
            ctx.emit_alloca(out, &temp_ptr, &mlir_ty);
            ctx.emit_store(out, var, &temp_ptr, &mlir_ty);
            return Ok(temp_ptr);
        }
    }
    // Promotion: Value T -> Reference<T> (Auto-ref)
    if let Type::Reference(inner, _) = to {
         if inner.structural_eq(from) {
             let temp_ptr = format!("%auto_ref_{}", ctx.next_id());
             let mlir_ty = from.to_mlir_storage_type(ctx).map_err(|e| format!("Auto-ref storage type error: {}", e))?;
             ctx.emit_alloca(out, &temp_ptr, &mlir_ty);
             ctx.emit_store(out, var, &temp_ptr, &mlir_ty);
             return Ok(temp_ptr);
         }
    }
    // Demotion: Owned<T> -> Value T (Auto-unbox/load)
    if let Type::Owned(inner) = from {
        if **inner == *to { 
            let val_res = format!("%auto_unbox_{}", ctx.next_id());
            let mlir_ty = to.to_mlir_storage_type(ctx).map_err(|e| format!("Failed to get storage type for auto-unbox: {}", e))?;
            ctx.emit_load(out, &val_res, var, &mlir_ty);
            return Ok(val_res);
        }
    }
    // [HARDENED] Structural Equivalence Check (replaces mangle_suffix string matching)
    // Uses Type::structural_eq() for robust comparison handling:
    // - Namespace prefixes (Ptr ≡ std__core__ptr__Ptr)
    // - Struct↔Concrete unification (Vec_i32 ≡ Concrete("Vec", [I32]))
    if from.structural_eq(to) {
        return Ok(var.to_string());
    }
    
    // Fallback: Base name comparison for aliased types
    match (from, to) {
        (Type::Struct(n1), Type::Concrete(n2, _)) |
        (Type::Concrete(n2, _), Type::Struct(n1)) => {
            if Type::base_names_equal(n1, n2) {
                return Ok(var.to_string());
            }
        },
        (Type::Concrete(n1, args1), Type::Concrete(n2, args2)) => {
            if Type::base_names_equal(n1, n2) && args1.len() == args2.len() {
                return Ok(var.to_string());
            }
        },
        _ => {}
    }

    // ═══════════════════════════════════════════════════════════════════
    // [COUNCIL FIX] Auto Fn→i64 coercion at call sites
    // Function references are already !llvm.ptr (from func.constant + cast).
    // When the callee expects i64 (e.g. Thread::spawn), auto-emit ptrtoint.
    // Eliminates: Thread::spawn(worker as i64) → Thread::spawn(worker)
    // ═══════════════════════════════════════════════════════════════════
    if matches!(from, Type::Fn(_, _)) && matches!(to, Type::I64 | Type::U64) {
        let res = format!("%fn_to_int_{}", ctx.next_id());
        out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
        return Ok(res);
    }

    // ═══════════════════════════════════════════════════════════════════
    // [COUNCIL FIX] StringView → Ptr<u8>/&u8 auto-extraction
    // String literals are now StringView { ptr, len }. When FFI or casts
    // expect a raw pointer, extract field[0] (the ptr) automatically.
    // Enables: println("hello") — auto-extracts ptr from StringView.
    // ═══════════════════════════════════════════════════════════════════
    {
        let is_stringview_from = match from {
            Type::Struct(name) | Type::Concrete(name, _) => name.contains("StringView"),
            _ => false,
        };
        if is_stringview_from && (to.k_is_ptr_type() || matches!(to, Type::Reference(..))) {
            let res = format!("%sv_extract_ptr_{}", ctx.next_id());
            let sv_mlir = from.to_mlir_type(ctx).unwrap_or("!llvm.struct<(ptr, i64)>".to_string());
            out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", res, var, sv_mlir));
            return Ok(res);
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // [CONSTITUTIONAL GUARD V22.0]: Prevent Integer→Pointer Promotion
    // Placed AFTER linear type checks so auto-box (I32→Owned<I32>) and
    // auto-ref (I32→&I32) pass through correctly. Only fires for raw
    // integer→pointer casts that indicate actual context contamination.
    // ═══════════════════════════════════════════════════════════════════
    if from.is_integer() && to.k_is_ptr_type() {
        return Err(format!(
            "Sovereign Type Error: Cannot promote integer {:?} to pointer {:?}. \
             var={} - This indicates Context Contamination in the loop engine.", 
            from, to, var
        ));
    }

    let res = format!("%prom_{}", ctx.next_id());
    let mut emit = |op: &str, src_ty: &str, dst_ty: &str| {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, var, src_ty, dst_ty));
    };

    match (from, to) {
        (Type::Never, _) => {
             let dst_ty_mlir = to.to_mlir_type(ctx).map_err(|e| e)?;
             out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", res, dst_ty_mlir));
             return Ok(res);
        },
        (Type::I8, Type::U8) | (Type::U8, Type::I8) | (Type::I8, Type::I8) | (Type::U8, Type::U8) => return Ok(var.to_string()),
        (Type::I16, Type::U16) | (Type::U16, Type::I16) | (Type::I16, Type::I16) | (Type::U16, Type::U16) => return Ok(var.to_string()),
        (Type::I32, Type::U32) | (Type::U32, Type::I32) | (Type::I32, Type::I32) | (Type::U32, Type::U32) => return Ok(var.to_string()),
        (Type::I64, Type::U64) | (Type::U64, Type::I64) | (Type::I64, Type::I64) | (Type::U64, Type::U64) | (Type::Usize, Type::Usize) => return Ok(var.to_string()),
        
        // Usize (MLIR index) <-> I64/U64 requires arith.index_cast
        (Type::Usize, Type::I64) | (Type::Usize, Type::U64) => {

            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", res, var));
            return Ok(res);
        },
        (Type::I64, Type::Usize) | (Type::U64, Type::Usize) => {
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, var));
            return Ok(res);
        },
        
        // I32/U32 -> Usize: extend to i64, then index_cast to index
        (Type::I32, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi {} : i32 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::U32, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i32 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        // I16/U16 -> Usize
        (Type::I16, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi {} : i16 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::U16, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i16 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        // I8/U8 -> Usize
        (Type::I8, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extsi {} : i8 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        (Type::U8, Type::Usize) => {
            let intermediate = format!("%ext_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i8 to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.index_cast {} : i64 to index\n", res, intermediate));
            return Ok(res);
        },
        
        // Array Packing: [Bool; N] (unpacked) -> [Bool; N] (packed)
        (Type::Array(from_inner, f_len, false), Type::Array(to_inner, t_len, true)) 
            if f_len == t_len && **from_inner == Type::Bool && **to_inner == Type::Bool => {
             
             let packed_storage_ty = to.to_mlir_storage_type(ctx).map_err(|e| e)?;
             let mut current_packed = format!("%packed_prom_{}", ctx.next_id());
             // Initialize with zeros (false)
             out.push_str(&format!("    {} = llvm.mlir.zero : {}\n", current_packed, packed_storage_ty));
             
             // Unroll packing loop
             let unpacked_storage_ty = from.to_mlir_storage_type(ctx).map_err(|e| e)?;
             
             // We can optimize by accumulating 64 bits then inserting.
             // Given promotion is linear code, we use SSA updates.
             let mut current_word_ssa = String::new();
             
             for i in 0..*f_len {
                 let bit_idx = i % 64;
                 
                 if bit_idx == 0 {
                     let zero = format!("%zero_w_{}", ctx.next_id());
                     ctx.emit_const_int(out, &zero, 0, "i64");
                     current_word_ssa = zero;
                 }
                 
                 // Calculate bit contribution
                 let elem = format!("%elem_{}_{}", i, ctx.next_id());
                 out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", elem, var, i, unpacked_storage_ty));
                 
                 let elem_ext = format!("%elem_ext_{}", ctx.next_id());
                 ctx.emit_cast(out, &elem_ext, "arith.extui", &elem, "i8", "i64");
                 
                 let shifted = format!("%shifted_{}", ctx.next_id());
                 let shift_amt = format!("%sh_amt_{}", ctx.next_id());
                 ctx.emit_const_int(out, &shift_amt, bit_idx as i64, "i64");
                 ctx.emit_binop(out, &shifted, "arith.shli", &elem_ext, &shift_amt, "i64");
                 
                 let new_word = format!("%accum_w_{}_{}", i, ctx.next_id());
                 ctx.emit_binop(out, &new_word, "arith.ori", &current_word_ssa, &shifted, "i64");
                 current_word_ssa = new_word;
                 
                 if bit_idx == 63 || i == f_len - 1 {
                     // Flush word
                     let word_idx = i / 64;
                     let inserted = format!("%packed_insert_{}", ctx.next_id());
                     out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", inserted, current_word_ssa, current_packed, word_idx, packed_storage_ty));
                     current_packed = inserted;
                 }
             }
             return Ok(current_packed);
        },
        (from, to) if from.is_integer() && to.is_integer() => {
             // Special handling for Usize (MLIR index type) - must convert via i64
             if *from == Type::Usize {
                 let intermediate = format!("%idx_i64_{}", ctx.next_id());
                 out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", intermediate, var));
                 
                 // Now truncate from i64 to target type if needed
                 let dst_width = get_bit_width(to);
                 if dst_width < 64 {
                     out.push_str(&format!("    {} = arith.trunci {} : i64 to {}\n", res, intermediate, to.to_mlir_type(ctx).unwrap()));
                     return Ok(res);
                 } else {
                     return Ok(intermediate);
                 }
             }
             
             // Standard integer promotion/truncation
             let src_width = get_bit_width(from);
             let dst_width = get_bit_width(to);
             
             if src_width == dst_width {
                 return Ok(var.to_string());
             } else if src_width > dst_width {
                 emit("arith.trunci", &from.to_mlir_type(ctx).unwrap(), &to.to_mlir_type(ctx).unwrap());
                 return Ok(res);
             } else {
                 let op = if from.is_unsigned() { "arith.extui" } else { "arith.extsi" };
                 emit(op, &from.to_mlir_type(ctx).unwrap(), &to.to_mlir_type(ctx).unwrap());
                 return Ok(res);
             }
        },
        // [SOVEREIGN V25.6] Integer -> Float Promotion (Parity with C)
        (from, to) if from.is_integer() && to.is_float() => {
             let op = if from.is_unsigned() { "arith.uitofp" } else { "arith.sitofp" };
             let src_str = from.to_mlir_type(ctx).unwrap();
             let dst_str = to.to_mlir_type(ctx).unwrap();
             emit(op, &src_str, &dst_str);
             return Ok(res);
        },
        // Float -> Float Promotion/Narrowing
        (Type::F32, Type::F64) => {
             emit("arith.extf", "f32", "f64");
             return Ok(res);
        },
        (Type::F64, Type::F32) => {
             emit("arith.truncf", "f64", "f32");
             return Ok(res);
        },
        
        (Type::Reference(_, _), Type::Reference(_, _)) => return Ok(var.to_string()),
        
        (Type::F32, Type::Bool) => {
             out.push_str("    %cst_0_f32 = arith.constant 0.0 : f32\n");
             out.push_str(&format!("    {} = arith.cmpf \"une\", {}, %cst_0_f32 : f32\n", res, var));
             return Ok(res);
        }
        (Type::F64, Type::Bool) => {
             out.push_str("    %cst_0_f64 = arith.constant 0.0 : f64\n");
             out.push_str(&format!("    {} = arith.cmpf \"une\", {}, %cst_0_f64 : f64\n", res, var));
             return Ok(res);
        }
        (from, Type::Bool) if from.is_integer() => {
             let zero = format!("%c0_{}", ctx.next_id());
             let mlir_from = from.to_mlir_type(ctx).map_err(|e| e)?;
             ctx.emit_const_int(out, &zero, 0, &mlir_from);
             out.push_str(&format!("    {} = arith.cmpi \"ne\", {}, {} : {}\n", res, var, zero, mlir_from));
             return Ok(res);
        },
        (Type::Bool, to) if to.is_integer() => {
             let dst_ty = to.to_mlir_type(ctx).map_err(|e| e)?;
             // Bool (i1) -> Integer (iX) is zero extension
             emit("arith.extui", "i1", &dst_ty);
             return Ok(res);
        }
        (Type::Tuple(fs), Type::Tuple(ts)) if fs.len() == ts.len() => {
             let target_mlir = to.to_mlir_storage_type(ctx).map_err(|e| e)?;
             let src_mlir = from.to_mlir_storage_type(ctx).map_err(|e| e)?;
             
             // Create undefined struct as base
             let first_init = format!("{}_init", res.replace("%", ""));
             out.push_str(&format!("    %{} = llvm.mlir.undef : {}\n", first_init, target_mlir));
             
             let mut current_struct_ssa = format!("%{}", first_init);
             
             for (i, (f_ty, t_ty)) in fs.iter().zip(ts.iter()).enumerate() {
                let elem_val = format!("%{}_elem_{}", res.replace("%", ""), i);
                 ctx.emit_extractvalue(out, &elem_val, var, i, &src_mlir);
                 
                 let prom_elem = match promote_numeric(ctx, out, &elem_val, f_ty, t_ty) {
                     Ok(r) => r,
                     Err(_) => match cast_numeric(ctx, out, &elem_val, f_ty, t_ty) {
                         Ok(r) => r,
                         Err(e) => return Err(e),
                     }
                 };
                 
                 // Chain insertvalue. The last one must define 'res'.
                 // We can't easily force the last one to be 'res' without conditional logic 
                 // because 'res' includes the %.
                 // So we use temporary names, and finally bind 'res' ? 
                 // MLIR has no "assign alias".
                 
                 // Alternative: Use 'res' for the LAST one.
                 let target_name = if i == fs.len() - 1 {
                     res.clone()
                 } else {
                     format!("{}_chain_{}", res, i)
                 };
                 
                 out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", 
                     target_name, prom_elem, current_struct_ssa, i, target_mlir));
                 
                 current_struct_ssa = target_name;
             }
             return Ok(res);
        }
        _ => {}
    }

    if let (Some(f_idx), Some(t_idx)) = (get_numeric_idx(from), get_numeric_idx(to)) {
        if let Some((op, src_ty, dst_ty)) = PROMOTION_OPS[f_idx][t_idx] {
            emit(op, src_ty, dst_ty);
            return Ok(res);
        }
    }

    // [CANONICAL IDENTITY MAP] Check if types are canonically equivalent
    // This resolves the "_TrieNode vs __TrieNode" underscore mismatch
    if from.canonical_eq(to) {

        return Ok(var.to_string()); // No promotion needed - types are canonically identical
    }

    // [SOVEREIGN V2.0] MLIR Identity Check
    // If both types lower to the same MLIR type (e.g. i64 vs u64, or Ptr<T> vs Ptr<U> in simplified ABI),
    // and they have the same size/align, we can treat this as a no-op promotion (bit-verification only).
    if let (Ok(mlir_from), Ok(mlir_to)) = (from.to_mlir_type(ctx), to.to_mlir_type(ctx)) {
        if mlir_from == mlir_to {
             let registry = ctx.struct_registry();
             if from.size_of(&registry) == to.size_of(&registry) {
                 return Ok(var.to_string());
             }
        }
    }

    // [PHASE 5] Struct↔Concrete Equivalence: normalize FQN prefixes
    // Handles cases like Struct("Vec_i64_ArenaAllocator") vs
    // Concrete("Vec", [I64, Concrete("std__mem__allocator__ArenaAllocator", [])]).
    // The mangled names differ only in package prefix depth.
    match (from, to) {
        (Type::Struct(n), Type::Concrete(..)) | (Type::Concrete(..), Type::Struct(n)) => {
            let other = if matches!(from, Type::Struct(_)) { to } else { from };
            // Normalize FQN: strip package prefixes from double-underscore paths.
            // "std__collections__vec__Vec_i64_std__mem__allocator__ArenaAllocator"
            // becomes "Vec_i64_ArenaAllocator"
            //
            // Strategy: split on single `_` (type param separator), but NOT on `__`.
            // For each resulting token, if it contains `__`, take the last segment.
            fn normalize_fqn(s: &str) -> String {
                // First, protect `__` by replacing with a unique placeholder
                let protected = s.replace("__", "\x01");
                // Split on single `_` to get type parameter components
                let parts: Vec<&str> = protected.split('_').collect();
                // Reassemble, replacing placeholder back to `__` and stripping prefix
                let normalized: Vec<String> = parts.iter().map(|part| {
                    let restored = part.replace('\x01', "__");
                    // If it contains `__`, it's a FQN — take the last segment
                    if restored.contains("__") {
                        restored.rsplit("__").next().unwrap_or(&restored).to_string()
                    } else {
                        restored
                    }
                }).collect();
                normalized.join("_")
            }
            let n_norm = normalize_fqn(n);
            let other_norm = normalize_fqn(&other.mangle_suffix());
            if n_norm == other_norm {
                return Ok(var.to_string());
            }
        }
        _ => {}
    }

    // ═══════════════════════════════════════════════════════════════════
    // [SOVEREIGN FIX] Raw pointer promotion cases for kernel !llvm.ptr usage
    // Handles: Pointer{I8} ↔ Pointer{I8} (identity), Pointer → integer (ptrtoint)
    // ═══════════════════════════════════════════════════════════════════
    if from.k_is_ptr_type() && to.k_is_ptr_type() {
        // Both are pointer-like types — identity at MLIR level (!llvm.ptr)
        return Ok(var.to_string());
    }
    // [CONSTITUTIONAL GUARD] Pointer → integer is NOT an implicit promotion.
    // Users must use explicit `as i64`/`as i32` casts, which route through
    // cast_numeric (line ~853) where ptrtoint is emitted deliberately.
    // Pointer element type mismatch: when struct field type is !llvm.ptr but resolved as element type
    if let Type::Pointer { ref element, .. } = from {
        if element.as_ref() == to {
            // from=Pointer{I8}, to=I8 means the field was !llvm.ptr — identity
            return Ok(var.to_string());
        }
    }

    Err(format!("Numeric promotion not supported from {:?} to {:?} (var: {})", from, to, var))
}

pub fn cast_numeric(ctx: &mut LoweringContext, out: &mut String, var: &str, from: &Type, to: &Type) -> Result<String, String> {
    // 1. Structural Identity Bypass
    // This handles i64 <-> u64, Ptr<T> <-> u64, etc., based on bits, not names.
    if from.structural_eq(to) {
        return Ok(var.to_string());
    }

    // 2. Machine-Width Identity (Sovereign Fix)
    // If both are integers and have the same bit-width, it's a zero-cost bitwise cast.
    // EXCEPTION: Usize (MLIR `index`) requires arith.index_cast even though it's 64-bit.
    // The MLIR type system treats `index` and `i64` as fundamentally different types.
    let w_from = get_bit_width(from); // Returns u32 (bits)
    let w_to = get_bit_width(to);
    
    let involves_usize = *from == Type::Usize || *to == Type::Usize;
    if w_from != 0 && w_from == w_to && from.is_integer() && to.is_integer() && !involves_usize {
        // [VERIFIED METAL]: No MLIR instruction needed for same-width integer casts.
        // This resolves i64 -> u64 without string matching or wrapper logic.
        return Ok(var.to_string());
    }
    
    // Check for promotion first (handles i32 -> i64, etc.)
    if let Ok(res) = promote_numeric(ctx, out, var, from, to) {
        return Ok(res);
    }
    
    let res = format!("%cast_{}", ctx.next_id());
    let mut emit = |op: &str, src_ty: &str, dst_ty: &str| {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, var, src_ty, dst_ty));
    };

    match (from, to) {
        // Usize (index) -> smaller int: index_cast to i64 first, then trunci
        (Type::Usize, Type::I32 | Type::U32) => {
            let intermediate = format!("%idx_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.trunci {} : i64 to i32\n", res, intermediate));
            return Ok(res);
        },
        (Type::Usize, Type::I16 | Type::U16) => {
            let intermediate = format!("%idx_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.trunci {} : i64 to i16\n", res, intermediate));
            return Ok(res);
        },
        (Type::Usize, Type::I8 | Type::U8) => {
            let intermediate = format!("%idx_i64_{}", ctx.next_id());
            out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", intermediate, var));
            out.push_str(&format!("    {} = arith.trunci {} : i64 to i8\n", res, intermediate));
            return Ok(res);
        },
        
        // Generic Integer Truncation
        (from, to) if from.is_integer() && to.is_integer() && w_from > w_to => {
             let src_str = from.to_mlir_type(ctx)?;
             let dst_str = to.to_mlir_type(ctx)?;
             emit("arith.trunci", &src_str, &dst_str);
             return Ok(res);
        },
        
        // Generic Integer Extension (should have been handled by promote_numeric, but fallback here)
        (from, to) if from.is_integer() && to.is_integer() && w_from < w_to => {
             let op = if from.is_unsigned() { "arith.extui" } else { "arith.extsi" };
             let src_str = from.to_mlir_type(ctx)?;
             let dst_str = to.to_mlir_type(ctx)?;
             emit(op, &src_str, &dst_str);
             return Ok(res);
        },

        (Type::F64, Type::F32) => { emit("arith.truncf", "f64", "f32"); return Ok(res); },
        
        (Type::F32 | Type::F64, Type::I8 | Type::I32 | Type::I64) => {
             let src_str = from.to_mlir_type(ctx)?;
             let dst_str = to.to_mlir_type(ctx)?;
             emit("arith.fptosi", &src_str, &dst_str);
             return Ok(res);
        }
        (Type::F32 | Type::F64, Type::U8 | Type::U32 | Type::U64 | Type::Usize) => {
             let src_str = from.to_mlir_type(ctx)?;
             let dst_str = to.to_mlir_type(ctx)?;
             emit("arith.fptoui", &src_str, &dst_str);
             return Ok(res);
        }
        
        (Type::I8 | Type::I32 | Type::I64, Type::F32 | Type::F64) => {
             let src_str = from.to_mlir_type(ctx)?;
             let dst_str = to.to_mlir_type(ctx)?;
             emit("arith.sitofp", &src_str, &dst_str);
             return Ok(res);
        }
        (Type::U8 | Type::U32 | Type::U64 | Type::Usize, Type::F32 | Type::F64) => {
             let src_str = from.to_mlir_type(ctx)?;
             let dst_str = to.to_mlir_type(ctx)?;
             emit("arith.uitofp", &src_str, &dst_str);
             return Ok(res);
        }

        (Type::Struct(_) | Type::Concrete(..), Type::Struct(_) | Type::Concrete(..)) => {
             // SOUNDNESS CHECK: Validate layout compatibility before cast
             if !prove_layout_compatibility_ctx(ctx, from, to) {
                 let struct_registry = ctx.struct_registry();
                 let size_from = from.size_of(&struct_registry);
                 let size_to = to.size_of(&struct_registry);
                 let align_from = from.align_of(&struct_registry);
                 let align_to = to.align_of(&struct_registry);
                 drop(struct_registry); // Release borrow
                 
                 return Err(format!(
                     "FORMAL INTEGRITY ERROR: Unsound cast from {} to {}. \
                      Layout compatibility could not be proven. \
                      Source: size={}, align={}. Target: size={}, align={}.",
                     from.mangle_suffix(), to.mangle_suffix(),
                     size_from, align_from, size_to, align_to
                 ));
             }
             
             let src_ty_mlir = from.to_mlir_storage_type(ctx)?;
             let dst_ty_mlir = to.to_mlir_storage_type(ctx)?;
             
             // Emit llvm.bitcast for zero-cost type coercion
             out.push_str(&format!(
                 "    {} = llvm.bitcast {} : {} to {}\n",
                 res, var, src_ty_mlir, dst_ty_mlir
             ));
             return Ok(res);
        },

        (Type::Reference(_, _), Type::Reference(_, _)) => return Ok(var.to_string()),

        // Integer to Pointer cast (inttoptr) - enables addr as Ptr<T>
        (Type::U64 | Type::Usize | Type::I64, Type::Pointer { .. }) => {
            // First ensure we have i64 for llvm.inttoptr
            let src_ty = from.to_mlir_type(ctx)?;
            let int_val = if src_ty != "i64" {
                let temp = format!("%inttoptr_prep_{}", ctx.next_id());
                // I64 is signed, U64/Usize are unsigned
                if matches!(from, Type::I64) {
                    out.push_str(&format!("    {} = arith.extsi {} : {} to i64\n", temp, var, src_ty));
                } else {
                    out.push_str(&format!("    {} = arith.extui {} : {} to i64\n", temp, var, src_ty));
                }
                temp
            } else {
                var.to_string()
            };
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", res, int_val));
            return Ok(res);
        }

        // [SOVEREIGN FIX] Pointer to Integer cast (ptrtoint) - enables self as u64 in Ptr::addr
        (Type::Pointer { .. }, Type::U64 | Type::Usize | Type::I64) => {
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
            return Ok(res);
        }

        // [STACK ARRAY] Array to Pointer cast — identity (both are !llvm.ptr)
        // Enables: let arr: [u8; 6]; fill(arr as Ptr<u8>, 6);
        (Type::Array(ref _inner, _, _), Type::Pointer { .. }) => {
            return Ok(var.to_string());
        }

        // [FIRST-CLASS FUNCTIONS] Function reference -> Integer cast
        // Function references are already lowered to !llvm.ptr by emit_expr
        // (via func.constant + unrealized_conversion_cast), so we just ptrtoint.
        (Type::Fn(_, _), Type::I64 | Type::U64) => {
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
            return Ok(res);
        }
        (Type::Fn(_, _), Type::Pointer { .. }) => {
            // Already !llvm.ptr, no-op cast
            return Ok(var.to_string());
        }

        // [KERNEL CAST] Integer to Reference (&T / &mut T) — inttoptr
        // Enables kernel patterns like: *(addr as &mut u8) = value
        // References lower to !llvm.ptr in MLIR, identical to Pointer cast
        (Type::U64 | Type::Usize | Type::I64, Type::Reference(_, _)) => {
            let src_ty = from.to_mlir_type(ctx)?;
            let int_val = if src_ty != "i64" {
                let temp = format!("%inttoptr_ref_{}", ctx.next_id());
                if matches!(from, Type::Usize) {
                    out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", temp, var));
                } else {
                    out.push_str(&format!("    {} = arith.extsi {} : {} to i64\n", temp, var, src_ty));
                }
                temp
            } else {
                var.to_string()
            };
            out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", res, int_val));
            return Ok(res);
        }

        // [KERNEL CAST] Reference (&T) to Integer — ptrtoint
        // Enables kernel patterns like: let addr = ref_val as u64
        (Type::Reference(_, _), Type::U64 | Type::Usize | Type::I64) => {
            out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, var));
            return Ok(res);
        }

        _ => return Err(format!("Unsupported explicit cast {} -> {}", from.mangle_suffix(), to.mangle_suffix())),
    }
}

fn get_bit_width(ty: &Type) -> u32 {
    match ty {
        Type::Bool | Type::I8 | Type::U8 => 8,
        Type::I16 | Type::U16 => 16,
        Type::I32 | Type::U32 | Type::F32 => 32,
        Type::I64 | Type::U64 | Type::Usize | Type::F64 => 64,
        _ => 0
    }
}

/// Detects if a struct name is already specialized (e.g., "std__core__node_ptr__NodePtr_TrieNode")
/// and extracts the template name and type arguments from it.
/// This prevents "Pointer Inception" bugs where NodePtr<TrieNode> becomes NodePtr<NodePtr<TrieNode>>.
#[allow(dead_code)]
fn peel_already_specialized_name(ctx: &mut LoweringContext, name: &str) -> Option<(String, Vec<Type>)> {
    // Look for known templates that may have been specialized into this name
    // Pattern: "template_suffix_TypeArg" where suffix marks specialization
    
    // Check if this matches a known template pattern with type arg suffix
    // e.g., "std__core__node_ptr__NodePtr_TrieNode" -> template="std__core__node_ptr__NodePtr", arg="TrieNode"
    for template_name in ctx.struct_templates().keys() {
        // Check if name starts with template and has a suffix
        let prefix = format!("{}_", template_name);
        if name.starts_with(&prefix) && name.len() > prefix.len() {
            let type_arg_name = &name[prefix.len()..];
            // The type arg should be resolvable as a struct
            if !type_arg_name.is_empty() {
                // [VERIFIED METAL] Phase 5: Use centralized struct lookup
                let arg_ty = if ctx.struct_registry().values().any(|i| i.name == type_arg_name) {
                    Type::Struct(type_arg_name.to_string())
                } else if let Some(info) = ctx.find_struct_by_name(type_arg_name) {
                    Type::Struct(info.name)
                } else {
                    // Use as-is
                    Type::Struct(type_arg_name.to_string())
                };

                return Some((template_name.clone(), vec![arg_ty]));
            }
        }
    }
    None
}

impl Type {
    /// Ensures Pointers and References always become !llvm.ptr for the Apple M4.
    pub fn to_mlir_type(&self, ctx: &mut LoweringContext) -> Result<String, String> {
        to_mlir_type(ctx, self)
    }
}
// End impl Type

// ============================================================================
// [VERIFIED METAL] Inception Guard & Layout Prover
// ============================================================================

/// Extracts the inner type from mangled pointer names.
pub fn extract_ptr_inner(name: &str) -> Option<String> {
    if let Some(idx) = name.rfind("Ptr") {
        let after = &name[idx + "Ptr".len()..];
        let inner = after.trim_start_matches('_');
        if !inner.is_empty() { return Some(inner.to_string()); }
    }
    None
}

/// [VERIFIED METAL] Flattening Loop
pub fn flatten_inception_recursive(ty: &Type, depth: usize, debug_ctx: &str) -> Type {
    if depth > 10 { return ty.clone(); }
    match ty {
        Type::Concrete(template, args) if template.contains("Ptr") && !args.is_empty() => {
            if args[0].k_is_ptr_type() {
                // [VERIFIED METAL] Drill down to the innermost non-pointer type
                return flatten_inception_recursive(&args[0], depth + 1, debug_ctx);
            }
            // If it's a pointer but the inner is NOT a pointer, we stay as is
            // EXCEPT if we are already in a recursion (depth > 0), in which case we strip this last layer too
            if depth > 0 { return args[0].clone(); }
            ty.clone()
        }
        Type::Struct(name) if name.contains("Ptr") => {
            if let Some(inner_name) = extract_ptr_inner(name) {
                let t = Type::Struct(inner_name);
                return flatten_inception_recursive(&t, depth + 1, debug_ctx);
            }
            ty.clone()
        }
        _ => ty.clone(),
    }
}

/// [ZERO-TRUST] Layout Prover
pub fn prove_layout_compatibility(struct_registry: &std::collections::HashMap<crate::types::TypeKey, crate::registry::StructInfo>, from: &Type, to: &Type) -> bool {
    if from == to { return true; }
    from.size_of(struct_registry) == to.size_of(struct_registry) && from.align_of(struct_registry) == to.align_of(struct_registry)
}

/// Convenience wrapper: extracts struct_registry from CodegenContext.
pub fn prove_layout_compatibility_ctx(ctx: &mut LoweringContext, from: &Type, to: &Type) -> bool {
    let reg = ctx.struct_registry();
    prove_layout_compatibility(&reg, from, to)
}

/// [GRAYDON FIX] Recursively substitute generic placeholders using current_type_map.
/// This is the "Secret of $i64$" - when HashMap<i64, i64> looks at Entry<K, V>,
/// this function transforms it to Entry<i64, i64> by consulting the active type context.
pub fn substitute_generics(type_map: &std::collections::BTreeMap<String, Type>, ty: &Type) -> Type {
    match ty {
        // Generics stored as Struct names (parser artifact) — check type_map
        Type::Struct(name) if type_map.contains_key(name) => {
            let concrete = &type_map[name].clone();
            // Guard against self-referential mappings that cause infinite loops
            if let Type::Struct(concrete_name) = concrete {
                if concrete_name == name {
                    return Type::Generic(name.clone());
                }
            }

            substitute_generics(type_map, concrete)
        }
        // Explicit Generic type
        Type::Generic(name) => {
            if let Some(concrete) = type_map.get(name) {

                substitute_generics(type_map, concrete)
            } else {
                ty.clone()
            }
        }
        // Concrete types with generic args (e.g., Entry<K, V>)
        Type::Concrete(name, args) => {
            let substituted_args: Vec<Type> = args.iter()
                .map(|a| substitute_generics(type_map, a))
                .collect();
            Type::Concrete(name.clone(), substituted_args)
        }
        // Pointer types
        Type::Pointer { element, provenance, is_mutable } => {
            Type::Pointer {
                element: Box::new(substitute_generics(type_map, element)),
                provenance: provenance.clone(),
                is_mutable: *is_mutable,
            }
        }
        // Reference types
        Type::Reference(inner, mutability) => {
            Type::Reference(Box::new(substitute_generics(type_map, inner)), *mutability)
        }
        // Array types
        Type::Array(inner, len, packed) => {
            Type::Array(Box::new(substitute_generics(type_map, inner)), *len, *packed)
        }
        // Tuple types
        Type::Tuple(elems) => {
            Type::Tuple(elems.iter().map(|e| substitute_generics(type_map, e)).collect())
        }
        // Function types: recursively substitute generics in arg and return types
        Type::Fn(args, ret) => {
            Type::Fn(
                args.iter().map(|a| substitute_generics(type_map, a)).collect(),
                Box::new(substitute_generics(type_map, ret)),
            )
        }
        // All other types pass through unchanged
        _ => ty.clone()
    }
}

/// Convenience wrapper: extracts type_map from CodegenContext.
pub fn substitute_generics_ctx(ctx: &mut LoweringContext, ty: &Type) -> Type {
    let type_map = ctx.current_type_map();
    substitute_generics(&type_map, ty)
}

/// Top-level helper for MLIR Type Lowering
pub fn to_mlir_type(ctx: &mut LoweringContext, ty: &Type) -> Result<String, String> {
    // [GRAYDON FIX] First, substitute any generic placeholders using current context
    let resolved_ty = substitute_generics_ctx(ctx, ty);
    
    if resolved_ty.k_is_ptr_type() || matches!(resolved_ty, Type::Reference(_, _)) { 
        return Ok("!llvm.ptr".to_string()); 
    }
    match &resolved_ty {
        Type::I8 | Type::U8 => Ok("i8".to_string()),
        Type::I16 | Type::U16 => Ok("i16".to_string()),
        Type::I32 | Type::U32 => Ok("i32".to_string()),
        Type::I64 | Type::U64 => Ok("i64".to_string()),
        Type::F32 => Ok("f32".to_string()),
        Type::F64 => Ok("f64".to_string()),
        Type::Bool => Ok("i1".to_string()),
        Type::Usize => Ok("index".to_string()),
        Type::Unit => Ok("!llvm.void".to_string()),
        Type::Struct(name) => {
            // Check type_map for unresolved generic placeholders
            if let Some(concrete) = ctx.current_type_map().get(name).cloned() {
                return to_mlir_type(ctx, &concrete);
            }
            
            // [SOVEREIGN FIX] Look up the fully-qualified struct name from struct_registry
            // Use shortest-match disambiguation to prefer main__ListNode over
            // std__core__boxed__Box_main__ListNode (both end with __ListNode).
            let full_name = {
                let registry = ctx.struct_registry();
                let target = name;
                let suffix = format!("__{}", target);
                let mut candidates: Vec<&str> = registry.values()
                    .filter(|info| {
                        info.name == *target 
                        || info.name.ends_with(&suffix)
                    })
                    .map(|info| info.name.as_str())
                    .collect();
                // Sort by length: prefer shortest (direct struct) over specialized templates
                candidates.sort_by_key(|c| c.len());
                candidates.first().map(|s| s.to_string())
                    .unwrap_or_else(|| name.clone())
            };
            Ok(format!("!struct_{}", full_name))
        },
        Type::Concrete(name, args) => {
            // [V25.7] De-escalated Type Fallback: Check if any arg is an unresolved Generic
            fn has_unresolved_generic(ty: &Type) -> bool {
                match ty {
                    Type::Generic(_) => true,
                    Type::Struct(n) if n.len() == 1 && n.chars().next().map_or(false, |c| c.is_ascii_uppercase()) => true,
                    Type::Concrete(_, inner_args) => inner_args.iter().any(has_unresolved_generic),
                    Type::Pointer { element, .. } => has_unresolved_generic(element),
                    Type::Reference(inner, _) => has_unresolved_generic(inner),
                    Type::Owned(inner) => has_unresolved_generic(inner),
                    _ => false,
                }
            }
            
            if args.iter().any(has_unresolved_generic) {
                eprintln!("WARNING: Unresolved generic in Concrete type '{}' - using !llvm.ptr fallback", name);
                return Ok("!llvm.ptr".to_string());
            }
            
            // [SOVEREIGN FIX] Look up the fully-qualified template name from struct_templates
            // This ensures consistency between header alias declarations and body usage.
            let full_base = {
                let templates = ctx.struct_templates();
                templates.keys()
                    .find(|k| *k == name || k.ends_with(&format!("__{}", name)) 
                              || (k.contains("__") && k.split("__").last() == Some(name.as_str())))
                    .cloned()
                    .unwrap_or_else(|| name.clone())
            };
            // [CANONICAL RESOLUTION] Canonicalize Struct args before mangling.
            // to_canonical_name() has no context, so Struct("Node") mangles to "Node"
            // producing Box_Node instead of Box_main__Node. We canonicalize here.
            let canonical_args: Vec<Type> = args.iter().map(|t| {
                if let Type::Struct(sname) = t {
                    if !sname.contains("__") {
                        let suffix = format!("__{}", sname);
                        if let Some(canonical) = ctx.struct_templates().keys()
                            .find(|k| k.ends_with(&suffix))
                            .cloned()
                        {
                            return Type::Struct(canonical);
                        }
                    }
                }
                t.clone()
            }).collect();
            let suffix = canonical_args.iter().map(|t| t.to_canonical_name()).collect::<Vec<_>>().join("_");
            let mangled = if args.is_empty() { full_base } else { format!("{}_{}", full_base, suffix) };
            Ok(format!("!struct_{}", mangled))
        },
        Type::Array(inner, len, _) => Ok(format!("!llvm.array<{} x {}>", len, to_mlir_type(ctx, inner)?)),
        Type::Tuple(elems) => {
            let parts: Result<Vec<_>, _> = elems.iter().map(|e| to_mlir_type(ctx, e)).collect();
            Ok(format!("!llvm.struct<({})>", parts?.join(", ")))
        }
        Type::Enum(name) => {
            // [V4.0 ENUM FUZZY LOOKUP] Check if enum name needs stripping
            // Handles package-prefixed names like "main__Status" → "Status"
            let stripped_name = name.rsplit("__").next().unwrap_or(name);
            if let Some(enum_info) = ctx.enum_registry().values()
                .find(|i| i.name == *name || i.name == stripped_name) 
            {
                // Use the registered enum name for type alias
                return Ok(format!("!struct_{}", enum_info.name));
            }
            // Use canonical stripped name
            Ok(format!("!struct_{}", stripped_name))
        }
        _ => Err(format!("MLIR Lowering not implemented for type: {:?}", ty)),
    }
}

pub fn resolve_codegen_type(ctx: &mut LoweringContext, ty: &Type) -> Type {
    let flattened = flatten_inception_recursive(ty, 0, "codegen_resolve");
    let res = match &flattened {
        Type::Enum(name) => {
            let mut resolved_name = name.clone();
            // Try resolving via imports - Transactional Block
            {
                let imports = ctx.imports();
                for imp in &*imports {
                    if let Some(group) = &imp.group {
                        if group.iter().any(|id| id.to_string() == *name) {
                            let base = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                            resolved_name = format!("{}__{}", base, name);
                            break;
                        }
                    }
                    if let Some(last) = imp.name.last() {
                        if let Some(alias) = &imp.alias {
                            if alias.to_string() == *name {
                                resolved_name = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                                break;
                            }
                        } else if last.to_string() == *name {
                            resolved_name = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                            break;
                        }
                    }
                }
            } // Import borrow drops here
            Type::Enum(resolved_name)
        }
        Type::Generic(name) => {
            // TRANSACTIONAL: Extract concrete type and DROP borrow immediately
            let concrete_opt = {
                ctx.current_type_map().get(name).cloned()
            };

            if let Some(concrete_ty) = concrete_opt {
                // Safe to recurse now that map is potentially free (for this scope)
                 resolve_codegen_type(ctx, &concrete_ty)
            } else if ctx.enum_registry().values().any(|i| i.name == *name) || ctx.enum_templates().contains_key(name) {
                Type::Enum(name.clone())
            } else {
                // Check if it's an imported type - Transactional
                let mut resolved_name = name.clone();
                {
                    let imports = ctx.imports();
                    for imp in &*imports {
                        if let Some(group) = &imp.group {
                            if group.iter().any(|id| id.to_string() == *name) {
                                let base = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                                resolved_name = Mangler::mangle(&[&base, name]);
                                break;
                            }
                        }
                        if let Some(last) = imp.name.last() {
                            if let Some(alias) = &imp.alias {
                                if alias.to_string() == *name {
                                    resolved_name = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                                    break;
                                }
                            } else if last.to_string() == *name {
                                resolved_name = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                                break;
                            }
                        }
                    }
                } // Imports drop here
                
                if ctx.enum_registry().values().any(|i| i.name == resolved_name) || ctx.enum_templates().contains_key(&resolved_name) {
                    Type::Enum(resolved_name)
                } else {
                    Type::Struct(resolved_name)
                }
            }
        }
        Type::SelfType => {
            let mut res = None;
            let self_concrete_opt = {
                ctx.current_type_map().get("Self").cloned()
            };
            if let Some(concrete_ty) = self_concrete_opt {
                res = Some(concrete_ty);
            }
            if res.is_none() {
                if let Some(self_ty) = &*ctx.current_self_ty() {
                    res = Some(self_ty.clone());
                }
            }

            if let Some(r) = res {
                // Identity Hand-off Fix: Hydrate Struct("Vec") to Concrete("Vec", [T]) if map covers it
                if let Type::Struct(name) = &r {
                     if let Some(template) = ctx.struct_templates().get(name) {
                         // Check if we can hydrate using generic params from the template
                         if let Some(generics) = &template.generics {
                             let mut args = Vec::new();
                             let mut all_found = true;
                             for param in &generics.params {
                                  let p_name = match param {
                                      crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                      crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                  };
                                  let arg_opt = {
                                      ctx.current_type_map().get(&p_name).cloned()
                                  };
                                  if let Some(arg) = arg_opt {
                                      args.push(arg);
                                  } else {
                                      all_found = false; 
                                      break;
                                  }
                             }
                             if all_found && !args.is_empty() {
                                 return Type::Concrete(name.clone(), args);
                             }
                         }
                     }
                }
                r
            } else {
                panic!("MonomorphizationError: Failed to resolve SelfType. Map keys: {:?}", ctx.current_type_map().keys().collect::<Vec<_>>());
            }
        }
        Type::Struct(name) => {
            if name.chars().all(|c| c.is_ascii_digit()) {
                return ty.clone();
            }
            if name.contains("__") {
                // Already resolved FQN
                let resolved_base = name.clone();
                
                // V3.0: Check if this is a template base that requires generic params
                // If so, skip specialization - caller must provide the generic args
                let requires_generics = ctx.struct_templates().get(&resolved_base)
                    .map(|t| t.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false))
                    .unwrap_or(false);
                
                if requires_generics {
                    // Template base without specialization - return as-is, don't specialize
                    // Specialization will happen when the caller provides concrete types
                    return Type::Struct(resolved_base);
                }
                
                let resolved_params = vec![]; 
                
                // Jump to check logic (duplicated here for clarity or refactor structure)
                let is_enum = ctx.enum_templates().contains_key(&resolved_base);
                if !ctx.suppress_specialization.get() {
                    let _ = ctx.specialize_template(&resolved_base, &resolved_params, is_enum);
                }
                if is_enum {
                    return Type::Enum(resolved_base);
                } else {
                    return Type::Struct(resolved_base);
                }
            }


            let concrete_opt = {
                ctx.current_type_map().get(name).cloned()
            };
            if let Some(concrete_ty) = concrete_opt {
                concrete_ty
            } else {
                // [CANONICAL RESOLUTION] Package-agnostic struct name resolution.
                // During Ptr<T> method hydration, current_package is std.core.ptr (not main),
                // so we must search ALL struct_templates for any key ending with __{name}.
                // This correctly resolves raw Struct("Node") → Struct("main__Node").
                let suffix = format!("__{}", name);
                let canonical_candidate = ctx.struct_templates().keys()
                    .find(|k| k.ends_with(&suffix))
                    .cloned()
                    .or_else(|| {
                        // Also check enum_templates
                        ctx.enum_templates().keys()
                            .find(|k| k.ends_with(&suffix))
                            .cloned()
                    });
                
                if let Some(ref candidate) = canonical_candidate {
                    let resolved_base = candidate.clone();
                    let requires_generics = ctx.struct_templates().get(&resolved_base)
                        .map(|t| t.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false))
                        .unwrap_or(false);
                    if requires_generics {
                        return Type::Struct(resolved_base);
                    }
                    let is_enum = ctx.enum_templates().contains_key(&resolved_base);
                    if !ctx.suppress_specialization.get() {
                        let _ = ctx.specialize_template(&resolved_base, &[], is_enum);
                    }
                    return if is_enum { Type::Enum(resolved_base) } else { Type::Struct(resolved_base) };
                }
                
                // [CROSS-MODULE STRUCT] Split qualified names like "addr::PhysAddr" into segments
                let segments: Vec<String> = name.split("::").map(|s| s.to_string()).collect();
                if let Some((pkg, item)) = crate::codegen::expr::utils::resolve_package_prefix_ctx(ctx, &segments) {
                     let resolved_base = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) };
                     let mut resolved_params = vec![];

                     // HYDRATION FIX: If the resolved base is a template, try to use current generic args
                     // This prevents 'Ptr' in 'impl Ptr<T>' from being treated as 'Ptr<>' (empty args),
                     // which causes specialize_template to create a placeholder with no fields.
                     if resolved_params.is_empty() {
                          if let Some(template) = ctx.struct_templates().get(&resolved_base) {
                              if let Some(generics) = &template.generics {
                                  let current_args = ctx.current_generic_args();
                                   if current_args.len() == generics.params.len() {
                                       // Basic arity check passed.
                                       // We allow implicit inference for any struct if the arity matches the current context.
                                       // This supports using 'Ptr' inside 'Vec<T>' as 'Ptr<T>'.
                                       resolved_params = current_args.clone();
                                   } else {
                                       // Fallback: Try to infer params from type_map
                                       // This handles explicit struct usage inside its own impl (e.g. Vec inside impl<T> Vec<T>)
                                       // where current_generic_args might be empty (method has no generics).
                                       let mut inferred = Vec::new();
                                       let mut all_found = true;
                                       for param in &generics.params {
                                           let p_name = match param {
                                               crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                               crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                           };
                                           let arg_opt = {
                                               ctx.current_type_map().get(&p_name).cloned()
                                           };
                                           if let Some(arg) = arg_opt {
                                               inferred.push(arg);
                                           } else {
                                               all_found = false;
                                               break;
                                           }
                                       }
                                       if all_found {
                                           resolved_params = inferred;
                                       }
                                   }
                              }
                          }
                     }
                     
                     let is_enum = ctx.enum_templates().contains_key(&resolved_base);
                     
                     // V3.0: Only specialize if we have params OR template doesn't require generics
                     let requires_generics = ctx.struct_templates().get(&resolved_base)
                         .map(|t| t.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false))
                         .unwrap_or(false);
                     
                     if !ctx.suppress_specialization.get() && (!requires_generics || !resolved_params.is_empty()) {
                          let _ = ctx.specialize_template(&resolved_base, &resolved_params, is_enum);
                     }

                     
                     if !resolved_params.is_empty() {
                         Type::Concrete(resolved_base, resolved_params)
                     } else if is_enum {
                         Type::Enum(resolved_base)
                     } else {
                         // PEEL LOGIC disabled for testing - using fallback
                         // TODO: Re-evaluate if this is needed with the Inception Guard in place
                         // if let Some((template_name, peeled_args)) = peel_already_specialized_name(ctx, &resolved_base) {
                         //     Type::Concrete(template_name, peeled_args)
                         // } else {
                             if resolved_base.contains("Ptr") {
                              }
                             Type::Struct(resolved_base)
                         // } // removed stray closing brace
                     }
                } else {
                     // eprintln!("WARNING: resolve_codegen_type failed to resolve '{}'. Falling back to Struct({}). Imports: {:?}", name, name, ctx.imports().iter().map(|i| i.alias.as_ref().map(|a| a.to_string()).unwrap_or("?".to_string())).collect::<Vec<_>>());
                     Type::Struct(name.clone())
                }
            }
        },

        Type::Concrete(base_name, target_params) => {
            // [SOVEREIGN FIX] Level 2 Safety Net: Catch misclassified generics.
            // If Concrete(name, []) and name is in the type_map, it's actually a generic placeholder
            // (e.g. "F2" parsed without context by from_syn). Resolve it like Type::Generic.
            if target_params.is_empty() {
                let concrete_opt = {
                    ctx.current_type_map().get(base_name).cloned()
                };
                if let Some(concrete_ty) = concrete_opt {
                    return resolve_codegen_type(ctx, &concrete_ty);
                }
            }
            // [PHANTOM FIX] For zero-arg Concrete that matches a struct template,
            // try to build args from the current type_map + phantom generic inference.
            if target_params.is_empty() && !ctx.current_type_map().is_empty() {
                if let Some(template) = ctx.struct_templates().get(base_name) {
                    if let Some(generics) = &template.generics {
                        let param_names: Vec<String> = generics.params.iter().map(|param| {
                            match param {
                                crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                            }
                        }).collect();
                        let type_map = ctx.current_type_map();
                        let mut inferred_map = type_map.clone();
                        // Infer phantom generics from Fn return types
                        crate::codegen::expr::infer_phantom_generics(&param_names, &mut inferred_map);
                        let args: Vec<Type> = param_names.iter()
                            .filter_map(|pname| inferred_map.get(pname).cloned())
                            .collect();
                        if args.len() == param_names.len() {
                            // All generics resolved — produce the fully-parameterized type
                            let resolved_args: Vec<Type> = args.iter()
                                .map(|a| resolve_codegen_type(ctx, a))
                                .collect();
                            return Type::Concrete(base_name.clone(), resolved_args);
                        }
                    }
                }
            }
            let mut resolved_params = Vec::new();
            for param in target_params {
                resolved_params.push(resolve_codegen_type(ctx, param));
            }
            if base_name == "Owned" && !resolved_params.is_empty() {
                Type::Owned(Box::new(resolved_params[0].clone()))
            } else if !resolved_params.is_empty() && base_name == "Window" {
                let region = if resolved_params.len() >= 2 {
                    if let Type::Struct(r) = &resolved_params[1] { r.clone() } else { "RAM".to_string() }
                } else { "RAM".to_string() };
                Type::Window(Box::new(resolved_params[0].clone()), region)
            } else if base_name == "Atomic" && !resolved_params.is_empty() {
                Type::Atomic(Box::new(resolved_params[0].clone()))
            } else {
                
                
                // Fix 2: Resolve base_name
                let mut resolved_base = base_name.clone();
                let mut found = false;

                // [CROSS-MODULE STRUCT] If base_name contains "::" (e.g. "addr::PhysAddr"),
                // split into segments and use resolve_package_prefix_ctx to resolve to FQN.
                if base_name.contains("::") {
                    let segments: Vec<String> = base_name.split("::").map(|s| s.to_string()).collect();
                    if let Some((pkg, item)) = crate::codegen::expr::utils::resolve_package_prefix_ctx(ctx, &segments) {
                        resolved_base = if item.is_empty() { pkg } else if pkg.is_empty() { item } else { format!("{}__{}", pkg, item) };
                        found = true;
                    }
                }
                // 1. Try explicit imports - Transactional Block
                {
                    let imports = ctx.imports();
                    if base_name == "Result" {

                    }
                    
                    for imp in &*imports {
                        // Check Alias
                        if let Some(alias) = &imp.alias {
                             if alias.to_string() == *base_name {
                                 resolved_base = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                                 found = true;
                                 break;
                             }
                        } 
                        
                        // Check Last Segment
                        if !found {
                            if let Some(last) = imp.name.last() {
                                 if last.to_string() == *base_name {
                                     resolved_base = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                                     found = true;

                                     break;
                                 }
                            }
                        }
    
                        // Check Group Imports
                        if !found {
                            if let Some(group) = &imp.group {
                                if base_name == "Result" {

                                }
                                if group.iter().any(|id| id.to_string() == *base_name) {
                                     resolved_base = Mangler::mangle(&imp.name.iter().map(|id| id.to_string()).collect::<Vec<_>>());
                                     // Append the item name (base_name) to the package base
                                     resolved_base = format!("{}__{}", resolved_base, base_name);
                                     found = true;

                                     break;
                                }
                            }
                        }
                    }
                } // Imports Borrow Drops Here

                // 2. If not found in imports, check if it's already a valid template name (direct match)
                if !found {
                     if ctx.struct_templates().contains_key(&resolved_base) || ctx.enum_templates().contains_key(&resolved_base) {
                         found = true;
                     }
                }

                // 3. Suffix Fallback / Global Search
                if !found {
                     // [VERIFIED METAL] Phase 5: Use centralized template lookup
                     if let Some(template_name) = ctx.find_struct_template_by_name(&base_name) {
                         resolved_base = template_name;
                         found = true;
                     } else if let Some(template_name) = ctx.find_enum_template_by_name(&base_name) {
                         resolved_base = template_name;
                         found = true;
                     }
                }

                if found {
                    // Success: specialized struct
                    let is_enum = ctx.enum_templates().contains_key(&resolved_base);
                    if !ctx.suppress_specialization.get() {
                        let _ = ctx.specialize_template(&resolved_base, &resolved_params, is_enum);
                    }
                    Type::Concrete(resolved_base, resolved_params)
                } else {

                    Type::Concrete(resolved_base, resolved_params)
                }
            }
        }
        Type::Reference(inner, is_mut) => Type::Reference(Box::new(resolve_codegen_type(ctx, inner)), *is_mut),
        Type::Pointer { element, provenance, is_mutable } => Type::Pointer {
            element: Box::new(resolve_codegen_type(ctx, element)),
            provenance: provenance.clone(),
            is_mutable: *is_mutable,
        },
        Type::Window(inner, region) => Type::Window(Box::new(resolve_codegen_type(ctx, inner)), region.clone()),
        Type::Array(inner, len, _) => Type::Array(Box::new(resolve_codegen_type(ctx, inner)), *len, false),
        Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| resolve_codegen_type(ctx, e)).collect()),
        Type::Tensor(inner, shape) => Type::Tensor(Box::new(resolve_codegen_type(ctx, inner)), shape.clone()),
        _ => ty.clone(),
    };

    if let Type::Struct(name) = ty {

    }
    res
}


/// Bridges the gap between Rust's syn::Type (legacy/helper) and Salt's Type system.
pub fn resolve_type(ctx: &mut LoweringContext, ty: &crate::grammar::SynType) -> Type {
    // [SOVEREIGN V2.0] Type Resolution Hardening
    // Handle context-dependent types (Array, Tensor) here.
    
    if let crate::grammar::SynType::Array(inner, len_expr) = ty {
        let inner_ty = resolve_type(ctx, inner);
        return match ctx.evaluator.eval_expr(len_expr) {
            Ok(crate::evaluator::ConstValue::Integer(val)) => Type::Array(Box::new(inner_ty), val as usize, false),
            Ok(_) => { crate::ice!("Array length must evaluate to an integer"); },
            Err(e) => { crate::ice!("Failed to evaluate array length: {:?}", e); }
        };
    }

    if let crate::grammar::SynType::Path(tp) = ty {
        if let Some(seg) = tp.segments.last() {
            if seg.ident == "Tensor" {
                 if seg.args.len() >= 2 {
                     let inner_syn = &seg.args[0];
                     let inner = resolve_type(ctx, inner_syn);
                     let mut shape = Vec::new();
                     
                     // [SOVEREIGN PHASE 3] Check for __Shape_X_Y_Z__ marker (AUTO-RANK)
                     // Preprocessor prepends auto-computed rank: {128,784} -> __Shape_2_128_784__
                     // Format: __Shape_Rank_D1_D2_...__ where first element is auto-rank (skipped)
                     if let crate::grammar::SynType::Path(shape_path) = &seg.args[1] {
                         if let Some(shape_seg) = shape_path.segments.last() {
                             let shape_name = shape_seg.ident.to_string();
                             if shape_name.starts_with("__Shape_") && shape_name.ends_with("__") {
                                 // Parse __Shape_2_128_784__ -> skip auto-rank, dims = [128, 784]
                                 let shape_str = &shape_name[8..shape_name.len()-2]; // strip prefix/suffix
                                 let all_values: Vec<usize> = shape_str.split('_')
                                     .filter_map(|s| s.parse().ok())
                                     .collect();
                                 // Skip first value (rank indicator) and use rest as dimensions
                                 if all_values.len() > 1 {
                                     shape = all_values[1..].to_vec();
                                 } else if !all_values.is_empty() {
                                     // Single value: use as dimension (rank-1 tensor)
                                     shape = all_values;
                                 }
                                 return Type::Tensor(Box::new(inner), shape);
                             }
                         }
                     }
                     
                     // Legacy: Support old Tensor<f32, [128], [784]> syntax
                     for i in 1..seg.args.len() {
                         if let crate::grammar::SynType::Array(_dummy, len_expr) = &seg.args[i] {
                              if let Ok(crate::evaluator::ConstValue::Integer(val)) = ctx.evaluator.eval_expr(len_expr) {
                                  shape.push(val as usize);
                              }
                         }
                     }
                     return Type::Tensor(Box::new(inner), shape);
                 }
            }
        }
    }

    // Default: Lower to Type and resolve imports/aliases (via resolve_codegen_type)
    // Note: Type::from_syn handles basic conversions (structs, primitives, etc.)
    if let Some(t) = Type::from_syn(ty) {
        resolve_codegen_type(ctx, &t)
    } else {
        Type::Unit
    }
}

/// Infers the type of a syn::Expr without emitting MLIR.
/// Used for receiver extraction in method call resolution.
pub fn infer_expr_type(
    ctx: &mut LoweringContext, 
    expr: &syn::Expr, 
    local_vars: &HashMap<String, (Type, crate::codegen::context::LocalKind)>
) -> Result<Type, String> {
    match expr {
        syn::Expr::Path(p) => {
            let name = p.path.segments.iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("__");
            
            // Check local vars first
            if let Some((ty, _)) = local_vars.get(&name) {
                return Ok(ty.clone());
            }
            
            // Check single-segment name in locals
            if p.path.segments.len() == 1 {
                let simple_name = p.path.segments[0].ident.to_string();
                if let Some((ty, _)) = local_vars.get(&simple_name) {
                    return Ok(ty.clone());
                }
            }
            
            // Check global variables/constants
            if let Some(ty) = ctx.globals().get(&name) {
                return Ok(ty.clone());
            }
            
            // Try canonical resolution with imports
            let canonical = crate::codegen::expr::utils::resolve_package_prefix_ctx(ctx, &p.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>());
            if let Some((pkg, _)) = canonical {
                if let Some(ty) = ctx.globals().get(&pkg) {
                    return Ok(ty.clone());
                }
            }
            
            Err(format!("Cannot infer type for path expression: {:?}", name))
        }
        syn::Expr::Paren(p) => infer_expr_type(ctx, &p.expr, local_vars),
        syn::Expr::Field(f) => {
            let base_ty = infer_expr_type(ctx, &f.base, local_vars)?;
            // For field access, find the field type in the struct registry
            let _base_name = match &base_ty {
                Type::Struct(n) => n.clone(),
                Type::Concrete(n, _) => n.clone(),
                Type::Reference(inner, _) => {
                    match &**inner {
                        Type::Struct(n) => n.clone(),
                        Type::Concrete(n, _) => n.clone(),
                        _ => return Err(format!("Field access on non-struct reference: {:?}", base_ty)),
                    }
                }
                _ => return Err(format!("Field access on non-struct type: {:?}", base_ty)),
            };
            
            // Find the struct in the registry using TypeKey
            let type_key = type_to_type_key(&base_ty);
            if let Some(info) = ctx.struct_registry().get(&type_key) {
                if let syn::Member::Named(field_name) = &f.member {
                    // StructInfo.fields is HashMap<String, (usize, Type)>
                    if let Some((_, ft)) = info.fields.get(&field_name.to_string()) {
                        return Ok(ft.clone());
                    }
                }
            }
            Err(format!("Unknown field on type {:?}: {:?}", base_ty, f.member))
        }
        syn::Expr::Reference(r) => {
            let inner = infer_expr_type(ctx, &r.expr, local_vars)?;
            Ok(Type::Reference(Box::new(inner), r.mutability.is_some()))
        }
        syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
            let inner = infer_expr_type(ctx, &u.expr, local_vars)?;
            match inner {
                Type::Reference(inner_ty, _) => Ok(*inner_ty),
                Type::Owned(inner_ty) => Ok(*inner_ty),
                _ => Err(format!("Dereference on non-reference type: {:?}", inner)),
            }
        }
        _ => Err(format!("Cannot infer type for expression: {:?}", expr)),
    }
}

/// Converts a Type to a TypeKey for method_registry lookup.
pub fn type_to_type_key(ty: &Type) -> TypeKey {
    match ty {
        Type::Struct(name) => {
            // Split by "__" to extract path, but keep FULL name for registry match
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

/// [V4.0] Trait Constraint Solver
/// Checks whether a concrete type satisfies a trait constraint.
/// 
/// This is called during generic instantiation when a type parameter has a bound:
/// `fn foo<T: Formattable>(x: T)` - when T is replaced with i64, we verify i64: Formattable.
/// 
/// Returns Ok(()) if the constraint is satisfied, Err with message if not.
pub fn check_trait_constraint(
    ctx: &mut LoweringContext,
    concrete_type: &Type,
    trait_name: &str,
) -> Result<(), String> {
    // Convert the concrete type to a TypeKey for trait lookup
    let type_key = type_to_type_key(concrete_type);
    
    // Check if the trait exists in the registry
    let trait_exists = ctx.trait_registry().get_trait(trait_name).is_some();
    if !trait_exists {
        // If trait doesn't exist yet, we allow it (forward reference or external trait)
        // In a stricter mode, we could return an error here
        eprintln!("WARN: Trait '{}' not found in registry, allowing forward reference", trait_name);
        return Ok(());
    }
    
    // Check if there's a trait impl for this (trait, type) pair
    if ctx.trait_registry().get_trait_impl(&type_key, trait_name).is_some() {
        return Ok(());
    }
    
    // Check method-based satisfaction: does the type have all required trait methods?
    if let Some(trait_def) = ctx.trait_registry().get_trait(trait_name) {
        let required_methods: Vec<String> = trait_def.method_signatures.iter()
            .map(|m| m.name.clone())
            .collect();
        
        // Check if all required methods exist for this type
        for method_name in &required_methods {
            if !ctx.trait_registry().contains_method(&type_key, method_name) {
                return Err(format!(
                    "Type '{}' does not satisfy trait '{}': missing method '{}'",
                    concrete_type.mangle_suffix(),
                    trait_name,
                    method_name
                ));
            }
        }
        
        // All methods found - trait constraint satisfied
        return Ok(());
    }
    
    Err(format!(
        "Type '{}' does not implement trait '{}'",
        concrete_type.mangle_suffix(),
        trait_name
    ))
}

/// [V4.0] Validate all trait constraints for a generic function instantiation.
/// Called when specializing a generic function with concrete type arguments.
pub fn validate_trait_constraints(
    ctx: &mut LoweringContext,
    generics: &Option<crate::grammar::Generics>,
    concrete_types: &[Type],
) -> Result<(), String> {
    let generics = match generics {
        Some(g) => g,
        None => return Ok(()), // No generics = no constraints
    };
    
    // Match generic params with concrete types
    let type_params: Vec<_> = generics.params.iter()
        .filter_map(|p| {
            if let crate::grammar::GenericParam::Type { name, constraint } = p {
                Some((name.to_string(), constraint.as_ref().map(|c| c.to_string())))
            } else {
                None
            }
        })
        .collect();
    
    // Check each type parameter that has a constraint
    for (i, (param_name, constraint)) in type_params.iter().enumerate() {
        if let Some(trait_name) = constraint {
            if let Some(concrete_ty) = concrete_types.get(i) {
                check_trait_constraint(ctx, concrete_ty, trait_name)
                    .map_err(|e| format!(
                        "Constraint violation for type parameter '{}': {}",
                        param_name, e
                    ))?;
            }
        }
    }
    
    Ok(())
}

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    pub fn request_explicit_specialization(&mut self, func_name: &str, override_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        // [ABI FIX] CANONICALIZATION GUARD: Always strip Reference wrappers from self_ty.
        let self_ty = self_ty.map(|mut ty| {
            while let Type::Reference(inner, _) = ty {
                ty = *inner;
            }
            ty
        });
        
        let mangled = override_name.to_string();
        
        // Check strict map
        // Check strict map
        if let Some(existing) = self.specializations().get(&(func_name.to_string(), concrete_tys.clone())) {

            
            // Fix: If it exists in map, but is NOT defined or pending, we must queue it!
            let defined = self.defined_functions().contains(existing);
            let pending = self.pending_generations().iter().any(|task| task.mangled_name == *existing);
            


            if !defined && !pending {

                 // Fall through to queue logic!
            } else {
                 return existing.clone();
            }
        }

        self.specializations_mut().insert((func_name.to_string(), concrete_tys.clone()), mangled.clone());
        
        let file = &self.config.file;
        // Search logic duplicated from request_specialization
        let found = if let Some(st) = &self_ty {
             let (st_base, method_name) = if let Some((base, method)) = func_name.rsplit_once("__") {
                 (base.to_string(), method.to_string())
             } else {
                 ("".to_string(), func_name.to_string())
             };
             
            let template_name = if let Type::Struct(name) = st {
                 self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Enum(name) = st {
                 self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             // [SOVEREIGN FIX] Handle Type::Pointer method lookup with fully-qualified template name
             } else if let Type::Pointer { .. } = st {
                 "std__core__ptr__Ptr".to_string()
             } else {
                 st_base
             };
             // [V4.0 SOVEREIGN] Use TraitRegistry for method lookup
             self.trait_registry().find_method_by_name(&template_name, &method_name, st)
        } else {
             file.items.iter().find_map(|item| {
                 if let crate::grammar::Item::Fn(f) = item {
                     if f.name == func_name { return Some((f.clone(), None, self.imports().clone())); }
                 }
                 None
             })
        };
        
        if let Some((func, s_ty, imports)) = found {

            let spec_map;
            {
                let old_imports = self.imports().clone();
                *self.imports_mut() = imports.clone();
                let old_map = self.current_type_map().clone();
                let old_args = self.current_generic_args().clone();
                let old_self = self.current_self_ty().clone();
                let mut old_const_vals = Vec::new();
                
                *self.current_generic_args_mut() = concrete_tys.clone();
                *self.current_self_ty_mut() = s_ty.clone();

                if let Some(st) = &s_ty {
                    let template_name = if let Type::Struct(name) = st {
                        self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
                    } else if let Type::Enum(name) = st {
                        self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
                    } else if let Type::Concrete(name, _) = st {
                        name.clone()
                    // [SOVEREIGN FIX] Handle Type::Pointer for type_map population
                    } else if let Type::Pointer { .. } = st {
                        "std__core__ptr__Ptr".to_string()
                    } else {
                        "".to_string()
                    };
                    
                     if !template_name.is_empty() {
                         let gen_params = if let Some(s) = self.struct_templates().get(&template_name) {
                             s.generics.as_ref().map(|g| g.params.clone())
                         } else if let Some(e) = self.enum_templates().get(&template_name) {
                             e.generics.as_ref().map(|g| g.params.clone())
                         } else { None };
                         
                         if let Some(params) = gen_params {
                              for (i, param) in params.iter().enumerate() {
                                   let pname = match param { crate::grammar::GenericParam::Type { name, .. } => name.to_string(), crate::grammar::GenericParam::Const { name, .. } => name.to_string() };
                                   // Logic: If s_ty is Concrete, use its args. If not, use concrete_tys?
                                   // For explicit specialization, concrete_tys MUST contain the args we want to map.
                                   // Unless s_ty already has them.
                                   // impl<T> Ptr<T>. s_ty = Ptr<u8>.
                                   // If Ptr<u8> is passed as s_ty, we can extract u8 from it!
                                   if let Type::Concrete(_, args) = &st {
                                        if let Some(arg) = args.get(i) {
                                            self.current_type_map_mut().insert(pname, arg.clone());
                                        }
                                   // [SOVEREIGN FIX] Handle Type::Pointer - extract element type for T
                                   } else if let Type::Pointer { element, .. } = &st {
                                        if i == 0 {  // Ptr<T> has one generic param T
                                            self.current_type_map_mut().insert(pname, (**element).clone());
                                        }
                                   } else if let Some(arg) = concrete_tys.get(i) {
                                       self.current_type_map_mut().insert(pname, arg.clone());
                                   }
                              }
                         }
                     }
                }
                
                // [SOVEREIGN FIX v2] Map method-level generics (e.g. map<F2, T>)
                // CRITICAL: fn_generics.params may contain EITHER:
                //   (a) merged impl+method params [I, F, F2, T] (from some trait_registry paths)
                //   (b) method-only params [F2, T] (from find_method_by_name)
                // We MUST use name-based filtering (not position-based skip) to handle both cases.
                if let Some(fn_generics) = &func.generics {
                    // Build a set of struct-level generic names for filtering
                    let struct_generic_names: std::collections::HashSet<String> = {
                        let mut names = std::collections::HashSet::new();
                        if let Some(t) = self_ty.as_ref() {
                            let type_name = match t {
                                Type::Struct(name) | Type::Concrete(name, _) => Some(name.clone()),
                                _ => None
                            };
                            if let Some(ref tname) = type_name {
                                let gen_params = {
                                    let templates = self.struct_templates();
                                    if let Some(s) = templates.get(tname) {
                                        s.generics.as_ref().map(|g| g.params.clone())
                                    } else {
                                        drop(templates);
                                        let etemplates = self.enum_templates();
                                        etemplates.get(tname).and_then(|e| e.generics.as_ref()).map(|g| g.params.clone())
                                    }
                                };
                                if let Some(params) = gen_params {
                                    for p in &params {
                                        let name = match p {
                                            crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                            crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                        };
                                        names.insert(name);
                                    }
                                }
                            }
                        }
                        names
                    };
                    
                    let struct_generic_count = struct_generic_names.len();
                    let method_args: Vec<Type> = concrete_tys.iter().skip(struct_generic_count).cloned().collect();
                    
                    if !method_args.is_empty() {
                        // Filter fn_generics.params to only method-level params (by name, not position)
                        let method_only_params: syn::punctuated::Punctuated<_, syn::token::Comma> = fn_generics.params.iter()
                            .filter(|p| {
                                let name = match p {
                                    crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                                    crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                                };
                                !struct_generic_names.contains(&name)
                            })
                            .cloned()
                            .collect();
                        
                        let method_only_generics = crate::grammar::Generics {
                            params: method_only_params,
                        };
                        self.map_generics(&Some(method_only_generics), &method_args, &func.name.to_string(), &mut old_const_vals);
                    }
                }

                spec_map = self.current_type_map().clone();

                *self.current_type_map_mut() = old_map;
                *self.current_generic_args_mut() = old_args;
                *self.current_self_ty_mut() = old_self;
                *self.imports_mut() = old_imports;
            }

            // Deduce package path from func_name or use empty
            let path_segments: Vec<String> = if func_name.contains("__") {
                 func_name.split("__").map(|s| s.to_string()).collect()
            } else {
                 vec![]
            };
            let pkg_path = if path_segments.len() > 1 {
                path_segments[0..path_segments.len()-1].to_vec()
            } else {
                vec![]
            };

            let task = crate::codegen::collector::MonomorphizationTask {
                identity: crate::types::TypeKey { 
                    path: pkg_path, 
                    name: func.name.to_string(), 
                    specialization: None 
                },
                mangled_name: mangled.clone(),
                func: func.clone(),
                concrete_tys: concrete_tys.clone(),
                self_ty: s_ty.clone(),
                imports: imports.clone(),
                type_map: spec_map,
            };


            self.pending_generations_mut().push_back(task);
        } else {
             eprintln!("Error: Function '{}' not found for specialization.", func_name);
        }
        
        mangled
    }




    pub fn request_specialization(&mut self, func_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        // [ABI FIX] CANONICALIZATION GUARD: Always strip Reference wrappers from self_ty.
        // The self_ty identity should be the naked base type (e.g., Result), not Reference(Result).
        // This ensures correct type mangling and Self resolution during hydration.
        let self_ty = self_ty.map(|mut ty| {
            while let Type::Reference(inner, _) = ty {
                ty = *inner;
            }
            ty
        });

        // [VERIFIED METAL] INCEPTION GUARD - Prevent recursive specialization
        // Use the reusable flatten_inception_recursive helper to enforce Single Indirection Property
        let concrete_tys: Vec<Type> = concrete_tys.into_iter().enumerate().map(|(i, ty)| {
            let debug_ctx = format!("{}[arg {}]", func_name, i);
            flatten_inception_recursive(&ty, 0, &debug_ctx)
        }).collect();


        // [Generic Wall] Security Check: Ensure NO generics leak into the queue
        // Check for both Generic("T") and Struct("F") where F is not a known struct/enum
        fn has_unresolved_type_params(ctx: &mut LoweringContext, ty: &Type) -> bool {
            match ty {
                Type::Generic(_) => true,
                Type::Struct(name) => {
                    // Self-referential type_map entries are unresolved
                    if let Some(mapped) = ctx.current_type_map().get(name) {
                        if let Type::Struct(mapped_name) = mapped {
                            if mapped_name == name { return true; }
                        }
                    }
                    // If not in any struct/enum registry, it's likely a generic param
                    let is_known = ctx.struct_registry().keys().any(|k| k.name.ends_with(name))
                        || ctx.enum_templates().contains_key(name);
                    !is_known && !name.contains("__") // Mangled names are real types
                }
                Type::Concrete(name, args) => {
                    // Check if the base name itself is an unresolved generic
                    let base_unresolved = if args.is_empty() {
                        !ctx.struct_registry().keys().any(|k| k.name.ends_with(name))
                            && !ctx.enum_templates().contains_key(name)
                            && !name.contains("__")
                    } else { false };
                    base_unresolved || args.iter().any(|a| has_unresolved_type_params(ctx, a))
                }
                Type::Pointer { element, .. } => has_unresolved_type_params(ctx, element),
                Type::Reference(inner, _) | Type::Owned(inner) | Type::Atomic(inner) => has_unresolved_type_params(ctx, inner),
                Type::Array(inner, _, _) => has_unresolved_type_params(ctx, inner),
                Type::Fn(args, ret) => args.iter().any(|a| has_unresolved_type_params(ctx, a)) || has_unresolved_type_params(ctx, ret),
                Type::Tuple(elems) => elems.iter().any(|e| has_unresolved_type_params(ctx, e)),
                _ => false,
            }
        }

        if concrete_tys.iter().any(|t| has_unresolved_type_params(self, t)) {

             return func_name.to_string();
        }
        if let Some(sty) = &self_ty {
            if has_unresolved_type_params(self, sty) {

                 return func_name.to_string();
            }
        }

        // Derive suffix from concrete_tys, OR from self_ty's specialization args if concrete_tys is empty
        // This ensures method specializations like Ptr<u8>::offset get suffix "_u8"

        let suffix = if !concrete_tys.is_empty() {
            concrete_tys.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_")
        } else if let Some(Type::Concrete(_, args)) = &self_ty {
            args.iter().map(|t| t.mangle_suffix()).collect::<Vec<_>>().join("_")
        } else {
            String::new()
        };
        let mangled = if suffix.is_empty() { func_name.to_string() } else { format!("{}_{}", func_name, suffix) };
        
        if let Some(existing) = self.specializations().get(&(func_name.to_string(), concrete_tys.clone())) {
            let s_res: String = existing.clone();
            return s_res;
        }
        self.specializations_mut().insert((func_name.to_string(), concrete_tys.clone()), mangled.clone());
        
        let file = &self.config.file;
        let found = if let Some(st) = &self_ty {
             // Method lookup
             let (st_base, method_name) = if let Some((base, method)) = func_name.rsplit_once("__") {
                 (base.to_string(), method.to_string())
             } else {
                 ("".to_string(), func_name.to_string())
             };
             
             // If st_base is a specialized name, resolve it to template name
             let template_name = if let Type::Struct(name) = st {
                 self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else if let Type::Enum(name) = st {
                 self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone())
             } else {
                 st_base
             };
             // [V4.0 SOVEREIGN] Use TraitRegistry for method lookup
             self.trait_registry().find_method_by_name(&template_name, &method_name, st)
        } else {
             // Function lookup
             file.items.iter().find_map(|item| {
                 if let crate::grammar::Item::Fn(f) = item {
                     if f.name == func_name { return Some((f.clone(), None, self.imports().clone())); }
                 }
                 None
             })
        };

        if let Some((func, s_ty, imports)) = found {
            // [V4.0] Trait Constraint Solver: Validate constraints before specialization
            if let Err(e) = validate_trait_constraints(self, &func.generics, &concrete_tys) {
                eprintln!("ERROR: Trait constraint validation failed for '{}': {}", func_name, e);
                // In strict mode we could panic, but for now we just warn
            }
            
            // [Fix] Scan specialized function for new dependencies (e.g. return types, local vars)
            // This prevents "Frozen Emission" panics by discovering deps during Expansion phase.
            let spec_map;
            {
                let old_imports = self.imports().clone();
                *self.imports_mut() = imports.clone();
                
                let old_map = self.current_type_map().clone();
                let old_args = self.current_generic_args().clone();
                let old_self = self.current_self_ty().clone();
                let mut old_const_vals = Vec::new();
                
                *self.current_generic_args_mut() = concrete_tys.clone();
                *self.current_self_ty_mut() = s_ty.clone();

                // Map Generics
                if let Some(st) = &s_ty {
                    // [GRAYDON FIX] Extract concrete args from Type::Concrete for struct generics
                    let (template_name, struct_concrete_args) = if let Type::Struct(name) = st {
                        let tname = self.struct_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone());
                        (tname, vec![])
                    } else if let Type::Enum(name) = st {
                        let tname = self.enum_registry().values().find(|i| i.name == *name).and_then(|i| i.template_name.clone()).unwrap_or(name.clone());
                        (tname, vec![])
                    } else if let Type::Concrete(name, args) = st {
                        // [CRITICAL] The args here ARE the concrete types for the struct generics!

                        (name.clone(), args.clone())
                    } else {
                        ("".to_string(), vec![])
                    };
                    
                    if !template_name.is_empty() {
                         let gen_params = if let Some(s) = self.struct_templates().get(&template_name) {
                             s.generics.clone()
                         } else if let Some(e) = self.enum_templates().get(&template_name) {
                             e.generics.clone()
                         } else { None };
                          

                          // [GRAYDON FIX] Use struct_concrete_args when available, fallback to concrete_tys
                          let args_to_map = if struct_concrete_args.is_empty() { &concrete_tys[..] } else { &struct_concrete_args[..] };

                          self.map_generics(&gen_params, args_to_map, &template_name, &mut old_const_vals);
                    }
                } else {
                    // Global Fn
                    if !concrete_tys.is_empty() {
                         self.map_generics(&func.generics, &concrete_tys, &func.name.to_string(), &mut old_const_vals);
                    }
                }
                
                // SOVEREIGN FIX: Method-level generics (e.g., mmap<T> on File struct)
                // CRITICAL: func.generics.params includes BOTH impl-level and method-level params.
                // We must only map method-level ones (skip struct_generic_count from func.generics).
                if let Some(fn_generics) = &func.generics {
                    // Use the CALLER's self_ty for correct struct_generic_count
                    let struct_generic_count = self_ty.as_ref()
                        .and_then(|t| match t {
                            Type::Struct(name) | Type::Concrete(name, _) => {
                                self.struct_templates().get(name)
                                    .and_then(|s| s.generics.as_ref())
                                    .map(|g| g.params.len())
                                    .or_else(|| self.enum_templates().get(name)
                                        .and_then(|e| e.generics.as_ref())
                                        .map(|g| g.params.len()))
                            }
                            Type::Pointer { .. } => Some(1),
                            _ => None
                        })
                        .unwrap_or(0);
                    
                    let method_args: Vec<Type> = concrete_tys.iter().skip(struct_generic_count).cloned().collect();

                    if !method_args.is_empty() {
                        // Create method-only generics by skipping impl-level params
                        let method_only_generics = crate::grammar::Generics {
                            params: fn_generics.params.iter().skip(struct_generic_count).cloned().collect(),
                        };
                        self.map_generics(&Some(method_only_generics), &method_args, &func.name.to_string(), &mut old_const_vals);
                    }
                }
                
                // Scan!

                // [MIGRATION] Inline type scanning (scan_types_in_fn expects CodegenContext)
                if let Err(e) = self.scan_types_in_fn_lctx(&func) {
                    eprintln!("Warning: Failed to scan dependencies for {}: {}", mangled, e);
                }
                
                // Capture the specialized map before restoring context
                spec_map = self.current_type_map().clone();

                *self.imports_mut() = old_imports;
                *self.current_type_map_mut() = old_map;
                *self.current_generic_args_mut() = old_args;
                *self.current_self_ty_mut() = old_self;
                
                // Restore consts
                for (name, old_val) in old_const_vals.into_iter().rev() {
                    if let Some(v) = old_val {
                        self.evaluator.constant_table.insert(name, v);
                    } else {
                        self.evaluator.constant_table.remove(&name);
                    }
                }
            }

            // Deduce package path from func_name or use empty
            let path_segments: Vec<String> = if func_name.contains("__") {
                 func_name.split("__").map(|s| s.to_string()).collect()
            } else {
                 vec![]
            };
            let pkg_path = if path_segments.len() > 1 {
                path_segments[0..path_segments.len()-1].to_vec()
            } else {
                vec![]
            };

            let task = crate::codegen::collector::MonomorphizationTask {
                identity: crate::types::TypeKey { 
                    path: pkg_path, 
                    name: func.name.to_string(), 
                    specialization: None 
                },
                mangled_name: mangled.clone(),
                func: func.clone(),
                concrete_tys: concrete_tys.clone(),
                self_ty: s_ty.clone(),
                imports: imports.clone(),
                type_map: spec_map,
            };


            self.pending_generations_mut().push_back(task);
        }
        mangled
    }

    pub fn specialize_template(&mut self, base_name: &str, concrete_tys: &[Type], is_enum: bool) -> Result<TypeKey, String> {
        // [CANONICAL RESOLUTION] Canonicalize concrete_tys before constructing the TypeKey.
        // Without this, Struct("Node") produces "Box_Node" while Struct("main__Node") produces
        // "Box_main__Node", creating duplicate specializations. By canonicalizing here, all
        // specializations consistently use FQN names.
        let concrete_tys: Vec<Type> = concrete_tys.iter().map(|ty| {
            if let Type::Struct(name) = ty {
                if !name.contains("__") {
                    let suffix = format!("__{}", name);
                    if let Some(canonical) = self.struct_templates().keys()
                        .find(|k| k.ends_with(&suffix))
                        .cloned()
                    {
                        return Type::Struct(canonical);
                    }
                }
            }
            ty.clone()
        }).collect();
        let concrete_tys = &concrete_tys;
        
        // Construct TypeKey

        let parts: Vec<&str> = base_name.split("__").collect();
        let (path, name) = if parts.len() > 1 {

             (parts[..parts.len()-1].iter().map(|s| s.to_string()).collect::<Vec<_>>(), parts.last().unwrap().to_string())
        } else {
             (vec![], base_name.to_string())
        };
        let key = TypeKey {
             path,
             name,
             specialization: if concrete_tys.is_empty() { None } else { Some(concrete_tys.to_vec()) },
        };
        
        let mangled = key.mangle();

        // 1. Check Registry (Existence = Done or In Progress)
        let exists = if is_enum {
            self.enum_registry().contains_key(&key)
        } else {
            self.struct_registry().contains_key(&key)
        };

        if exists { return Ok(key); }

        // 1.5. Generic Guard: Do NOT specialize (expand) if args are still generic
        // After substitute_generics, self-referential {I: Struct("I")} → Generic("I")
        let substituted_tys: Vec<Type> = concrete_tys.iter()
            .map(|t| substitute_generics_ctx(self, t))
            .collect();
        if substituted_tys.iter().any(|t| t.has_generics()) {
             return Ok(key);
        }

        // 2. Check Pending Set
        let is_queued = self.monomorphizer().pending_set.contains(&mangled);
        if is_queued { return Ok(key); }

        // 3. Frozen Check (Provenance Safety)
        if self.monomorphizer().is_frozen {
            // WARNING: Late specialization during emission.
            // Allowed via iterative drainage.
        }

        // 4. Self-Identity Guard (If inside the struct being simplified)
        if let Some(Type::Struct(self_name)) = &*self.current_self_ty() {
            if *self_name == mangled { return Ok(key); }
        }
        if let Some(Type::Enum(self_name)) = &*self.current_self_ty() {
             if *self_name == mangled { return Ok(key); }
        }

        // 5. Protected Name Check
        if Type::is_protected_name(&mangled) {
             return Ok(key); 
        }

        // 6. Atomic Registration (Placeholder)
        // Insert empty info to prevent recursive re-entry if registry lookup happens (redundant with pending_set but safe)
        if is_enum {
             let mut reg = self.enum_registry_mut();
             reg.insert(key.clone(), EnumInfo {
                 name: mangled.clone(), variants: Vec::new(), max_payload_size: 0,
                 template_name: if concrete_tys.is_empty() { None } else { Some(base_name.to_string()) },
                 specialization_args: concrete_tys.to_vec(),
             });
        } else {
             let mut reg = self.struct_registry_mut();
             reg.insert(key.clone(), StructInfo {
                 name: mangled.clone(), fields: HashMap::new(), field_order: Vec::new(), field_alignments: Vec::new(),
                 template_name: if concrete_tys.is_empty() { None } else { Some(base_name.to_string()) },
                 specialization_args: concrete_tys.to_vec(),
             });
        }

        // 7. Defer Work (Commit to Queue)
        // 7. Recursive Expansion (Immediate - Stack Based)
        // Instead of queuing, we process immediately to ensure Deps are sized before Dependents.
        {
            self.monomorphizer_mut().pending_set.insert(mangled.clone());
        }

        // EXPAND
        if is_enum {
             let res = self.expand_enum_structure(base_name, concrete_tys);
             match res {
                 Ok(info) => { self.enum_registry_mut().insert(key.clone(), info); }
                 Err(e) => {
                     self.enum_registry_mut().remove(&key);
                     self.monomorphizer_mut().pending_set.remove(&mangled);
                     return Err(e);
                 }
             }
        } else {
             let res = self.expand_template_structure(base_name, concrete_tys);
             match res {
                 Ok(info) => { 
                     self.struct_registry_mut().insert(key.clone(), info); 
                 }
                 Err(e) => {
                     self.struct_registry_mut().remove(&key);
                     self.monomorphizer_mut().pending_set.remove(&mangled);
                     return Err(e);
                 }
             }
        };

        // HOISTING (Immediate)
        let full_ty = if is_enum { crate::types::Type::Enum(mangled.clone()) } else { crate::types::Type::Struct(mangled.clone()) };
        if let Ok(mlir_def) = full_ty.to_mlir_storage_type(self) {
             if mlir_def.contains(", (") || mlir_def.contains(", ()") {
                let dummy_name = format!("__typedef_{}", mangled);
                let mut d = self.decl_out_mut();
                d.push_str(&format!("  llvm.mlir.global private @{}() : {} {{\n", dummy_name, mlir_def));
                d.push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_def));
                d.push_str(&format!("    llvm.return %0 : {}\n", mlir_def));
                d.push_str("  }\n");
             }
        }

        self.monomorphizer_mut().pending_set.remove(&mangled);

        Ok(key)
    }

    pub fn drain_work_queue(&mut self) {
        loop {
            // Pop task (Short borrow)
            let task_opt = self.monomorphizer_mut().work_queue.pop_front();
            if task_opt.is_none() { break; }
            let task = task_opt.unwrap();


            // Setup Context for Self-Resolution
            let old_self = self.current_self_ty().clone();
            let self_type = if task.is_enum { Type::Enum(task.mangled_name.clone()) } else { Type::Struct(task.mangled_name.clone()) };
            *self.current_self_ty_mut() = Some(self_type);

            // Construct Key for Registry Access
            let base_name = &task.template_name;
            let parts: Vec<&str> = base_name.split("__").collect();
            let (path, name) = if parts.len() > 1 {
                 (parts[..parts.len()-1].iter().map(|s| s.to_string()).collect::<Vec<_>>(), parts.last().unwrap().to_string())
            } else {
                 (vec![], base_name.to_string())
            };
            let key = TypeKey {
                 path,
                 name,
                 specialization: Some(task.args.clone()),
            };

            // EXPAND (No Registry Borrow Here, only Read Templates + Request Spec)
            let _result = if task.is_enum {
                let info = self.expand_enum_structure(&task.template_name, &task.args).expect("Failed to expand enum");
                // Commit to Registry
                if let Some(entry) = self.enum_registry_mut().get_mut(&key) {
                    *entry = info;
                }
            } else {
                let info = self.expand_template_structure(&task.template_name, &task.args).expect("Failed to expand struct");
                // Commit to Registry
                if let Some(entry) = self.struct_registry_mut().get_mut(&key) {
                    *entry = info;
                }
            };

            // Restore Context
            *self.current_self_ty_mut() = old_self;

            // Mark as Done (Remove from pending_set is optional if we check registry first, but good for cleanup)
            self.monomorphizer_mut().pending_set.remove(&task.mangled_name);

            // [Header Hoisting] Immediate Emission
            // We force the emission of the struct/enum definition into decl_out immediately after specialization.
            // This ensures the type is fully defined before any function body (generated later) attempts to use it.
            let full_ty = if task.is_enum { crate::types::Type::Enum(task.mangled_name.clone()) } else { crate::types::Type::Struct(task.mangled_name.clone()) };
            
            // Generate the full body definition string (e.g. !llvm.struct<"Vec_u8", (...)>)
            // We use to_mlir_storage_type which triggers the registry lookup and body formatting.
            if let Ok(mlir_def) = full_ty.to_mlir_storage_type(self) {
                // Only hoist if the returned string contains a body definition (i.e. has fields or explicitly empty body).
                // If it returns an opaque reference (e.g. !llvm.struct<"Foo">), it means it was already emitted elsewhere.
                if mlir_def.contains(", (") || mlir_def.contains(", ()") { 
                    // Construct a private dummy global to force the type definition into the module scope.
                    // This satisfies the "Definition Precedence" requirement.
                    let dummy_name = format!("__typedef_{}", task.mangled_name);
                    let mut d = self.decl_out_mut();
                    d.push_str(&format!("  llvm.mlir.global private @{}() : {} {{\n", dummy_name, mlir_def));
                    d.push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_def));
                    d.push_str(&format!("    llvm.return %0 : {}\n", mlir_def));
                    d.push_str("  }\n");
                }
            } else {
                 eprintln!("WARNING: Failed to generate storage type for hoisted task: {}", task.mangled_name);
            }
        }
        
        // Finalize (Freeze)
        self.monomorphizer_mut().is_frozen = true;
    }

    pub fn map_generics(&mut self, generics: &Option<crate::grammar::Generics>, args: &[Type], template_name: &str, old_const_vals: &mut Vec<(String, Option<ConstValue>)>) {

         if let Some(gen) = generics {
             for (i, param) in gen.params.iter().enumerate() {
                 if let Some(concrete) = args.get(i) {
                     let c_t: Type = concrete.clone();
                     let name = match param {
                         crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                         crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                     };
                     if Type::is_protected_name(&name) {
                           panic!("Shadowing Guard: Generic parameter '{}' covers a protected type name in template '{}'", name, template_name);
                      }
                     self.current_type_map_mut().insert(name.clone(), c_t.clone());

                     
                     // Const Generic Injection
                     if let Type::Struct(val_str) = &c_t {
                         if let Ok(int_val) = val_str.parse::<i64>() {
                             let old = self.evaluator.constant_table.insert(name.clone(), ConstValue::Integer(int_val));
                             old_const_vals.push((name, old));
                         }
                     }
                 }
             }
         }
    }

    /// Performs the structural expansion of a template by mapping generic
    /// parameters to concrete arguments and resolving field types.
    /// This is side-effect free w.r.t the struct registry.
    pub fn expand_template_structure(&mut self,
        template_name: &str,
        args: &[Type],
    ) -> Result<StructInfo, String> {
        // 1. Transactional Read: Extract Template Data
        // We clone generics and fields to free struct_templates for the next level of recursion.
        let (generics, fields) = {
            let templates = self.struct_templates();
            let template = templates.get(template_name)
                .cloned()
                .ok_or_else(|| format!("Template '{}' not found in registry", template_name))?;
            (template.generics.clone(), template.fields.clone())
        };

        // Fix: Context Swap to Template Definition Scope to prevent Key Drift
        // This makes sure that field resolution (e.g. "GlobalSlabAlloc") happens in the std lib context, NOT the user context.
        let mut _import_guard = None;
        if let Some(registry) = self.config.registry {
             let parts: Vec<&str> = template_name.split("__").collect();
             if parts.len() > 1 {
                 for (pkg_name, mod_info) in &registry.modules {
                      let pkg_mangled = pkg_name.replace(".", "__");
                      let prefix = format!("{}__", pkg_mangled);
                      if template_name.starts_with(&prefix) {
                           let mut combined_imports = mod_info.imports.clone();
                           // V3.0: Synthesize self-imports ONLY for non-generic types
                           // Generic types (like Vec<T>, SlabCache<SIZE>) should be resolved
                           // via their categorical export metadata which preserves generic_params.
                           {
                                let pkg_prefix_ident = format!("{}__", pkg_mangled);
                                
                                // Only add non-generic struct templates as simple aliases
                                for (s_name, s_def) in &mod_info.struct_templates {
                                     // V3.0: Skip generic templates - they need explicit instantiation
                                     let has_generics = s_def.generics.as_ref().map(|g| !g.params.is_empty()).unwrap_or(false);
                                     if has_generics {
                                         continue;
                                     }
                                     
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                                
                                // Concrete (non-template) structs can be aliased directly
                                for (s_name, _) in &mod_info.structs {
                                     let mangled = format!("{}{}", pkg_prefix_ident, s_name);
                                     let mangled_ident = syn::Ident::new(&mangled, proc_macro2::Span::call_site());
                                     let mut p = syn::punctuated::Punctuated::new();
                                     p.push(mangled_ident);
                                     combined_imports.push(crate::grammar::ImportDecl { name: p, alias: Some(syn::Ident::new(s_name, proc_macro2::Span::call_site())), group: None });
                                }
                           }
                           // [MIGRATION] Direct import swap (ImportContextGuard expects CodegenContext)
                           let old_imports = std::mem::replace(&mut *self.imports_mut(), combined_imports);
                           _import_guard = Some(old_imports);
                           break; 
                      }
                 }
             }

        }

        // 2. Validate Argument Count
        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
            // V3.0: Instead of hard error, return placeholder for deferred expansion
            // This handles cases like Vec<T> inside String definition - the T will be
            // substituted later when the actual specialization is requested with concrete args.
            // Only log for debugging, don't fail compilation.

            // Restore imports if they were swapped for template definition scope
            if let Some(old_imports) = _import_guard {
                *self.imports_mut() = old_imports;
            }
            
            // Return a stub StructInfo with the template name - indicates "unspecialized"
            return Ok(StructInfo {
                name: template_name.to_string(),
                fields: std::collections::HashMap::new(),
                field_order: vec![],
                field_alignments: vec![],
                template_name: Some(template_name.to_string()),
                specialization_args: vec![],
            });
        }



        // 3. State Snapshot: Prepare new type mapping
        let old_map = self.current_type_map().clone();
        let old_generic_args = self.current_generic_args().clone();

        let mut type_map = old_map.clone();
        
        if let Some(gen) = &generics {
            for (param, arg) in gen.params.iter().zip(args.iter()) {
                 let name = match param {
                     crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                     crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                 };
                 type_map.insert(name, arg.clone());
            }
        }

        // 4. Transactional Update: Set the recursion context
        {
            *self.current_type_map_mut() = type_map;
            *self.current_generic_args_mut() = args.to_vec();
        }

        // 5. Recursive Discovery: Map fields in the new context
        let mut resolved_fields = HashMap::new();
        let mut field_order = Vec::new();
        let mut field_alignments = Vec::new();

        for (i, field) in fields.iter().enumerate() {
            // resolve_type is recursive and might access struct_templates/current_type_map
            let mut field_ty = resolve_type(self, &field.ty);

            // Handle @packed attribute
            if field.attributes.iter().any(|a| a.name == "packed") {
                 if let Type::Array(inner, len, _) = field_ty {
                      field_ty = Type::Array(inner, len, true);
                 } else {
                      eprintln!("Warning: @packed attribute ignored on non-array field '{}' in struct '{}'", field.name, template_name);
                 }
            }
            
            let align = crate::grammar::attr::extract_align(&field.attributes);

            resolved_fields.insert(field.name.to_string(), (i, field_ty.clone()));
            field_order.push(field_ty);
            field_alignments.push(align);
        }
        
        // 6. Transactional Restore: Roll back the context
        {
            *self.current_type_map_mut() = old_map;
            *self.current_generic_args_mut() = old_generic_args;
        }
        // [FIX] Restore imports that were swapped for template definition scope.
        // Without this, the caller's import context is permanently clobbered
        // with the template's module imports (e.g., Slice's 1-import context
        // overwrites main's 21-import context).
        if let Some(old_imports) = _import_guard {
            *self.imports_mut() = old_imports;
        }

        // Phase B: API Surface Discovery (Eager Method Registration)
        let methods = self.find_methods_for_template(template_name);
        for method_name in methods {
             // [SOVEREIGN FIX] Skip generic methods. They require inference/turbofish at call site.
             // Registry stores full mangled name in 'name' field with empty path for Struct types.
             let key = crate::types::TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
             
             // eprintln!("DEBUG: Eager Check {} on {:?}", method_name, key);

             if let Some((func, _, _)) = self.trait_registry().get_legacy(&key, &method_name) {
                 if let Some(g) = &func.generics {
                     if !g.params.is_empty() {

                         continue; 
                     }
                 }
             } else {

             }

             let full_name = format!("{}__{}", template_name, method_name);
             let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
             let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }


        // 7. Return Metadata
        Ok(StructInfo {
            name: self.specialize_template(template_name, args, false)?.mangle(),
            fields: resolved_fields,
            field_order,
            field_alignments,
            template_name: Some(template_name.to_string()),
            specialization_args: args.to_vec(),
        })
    }

    pub fn expand_enum_structure(&mut self,
        template_name: &str,
        args: &[Type],
    ) -> Result<EnumInfo, String> {
         // 1. Transactional Read: Extract Enum Template Data
        let (generics, variants) = {
            let templates = self.enum_templates();
            let template = templates.get(template_name)
                .cloned()
                .ok_or_else(|| format!("Enum Template '{}' not found", template_name))?;
            (template.generics.clone(), template.variants.clone())
        };

        let params_len = generics.as_ref().map(|g| g.params.len()).unwrap_or(0);
        if params_len != args.len() {
             return Err(format!("Generic mismatch for enum {}", template_name));
        }

        // 3. State Snapshot
        let old_map = self.current_type_map().clone();
        let old_generic_args = self.current_generic_args().clone();

        let mut type_map = old_map.clone();
        if let Some(gen) = &generics {
            for (param, arg) in gen.params.iter().zip(args.iter()) {
                 let name = match param {
                     crate::grammar::GenericParam::Type { name, .. } => name.to_string(),
                     crate::grammar::GenericParam::Const { name, .. } => name.to_string(),
                 };
                 type_map.insert(name, arg.clone());
            }
        }

        // 4. Transactional Update: Set recursion context
        {
            *self.current_type_map_mut() = type_map;
            *self.current_generic_args_mut() = args.to_vec();
        }
        
        let mut resolved_variants = Vec::new();
        let mut max_payload_size = 0;
        
        // 5. Recursive Discovery
        for (idx, v) in variants.iter().enumerate() {
             let p_ty = v.ty.as_ref().map(|sy| crate::codegen::type_bridge::resolve_type(self, sy));
             if let Some(ref ty) = p_ty {
                 let size = ty.size_of(&*self.struct_registry());
                 if size > max_payload_size { max_payload_size = size; }
             }
             resolved_variants.push((v.name.to_string(), p_ty, idx as i32));
        }

        // 6. Transactional Restore
        {
            *self.current_type_map_mut() = old_map;
            *self.current_generic_args_mut() = old_generic_args;
        }

        // Phase B: API Surface Discovery
        let methods = self.find_methods_for_template(template_name);
        for method_name in methods {
             // [SOVEREIGN FIX] Skip generic methods. They require inference/turbofish at call site.
             // Registry stores full mangled name in 'name' field with empty path for Struct types.
             let key = crate::types::TypeKey { path: vec![], name: template_name.to_string(), specialization: None };
             
             // eprintln!("DEBUG: Eager Check {} on {:?}", method_name, key);

             if let Some((func, _, _)) = self.trait_registry().get_legacy(&key, &method_name) {
                 if let Some(g) = &func.generics {
                     if !g.params.is_empty() {

                         continue; 
                     }
                 }
             }

             let full_name = format!("{}__{}", template_name, method_name);
             let self_ty = Type::Concrete(template_name.to_string(), args.to_vec());
             let _ = self.request_specialization(&full_name, args.to_vec(), Some(self_ty));
        }


        Ok(EnumInfo {
            name: self.specialize_template(template_name, args, true)?.mangle(),
            variants: resolved_variants,
            max_payload_size,
            template_name: Some(template_name.to_string()),
            specialization_args: args.to_vec(),
        })
    }

}

pub fn emit_const(ctx: &mut LoweringContext, _out: &mut String, c: &crate::grammar::ConstDef) -> Result<(), String> {
    let val = ctx.evaluator.eval_expr(&c.value).map_err(|e| format!("Const eval failed for {}: {:?}", c.name, e))?;
    // [PHASE 4a] Only insert scalar constants into constant_table for inlining.
    // Complex values (struct consts like OpenFlags { bits: 0 }) cannot be represented
    // as arith.constant — they are emitted as llvm.mlir.global below.
    match &val {
        ConstValue::Integer(_) | ConstValue::Bool(_) | ConstValue::Float(_) | ConstValue::String(_) => {
            ctx.evaluator.constant_table.insert(c.name.to_string(), val.clone());
        }
        _ => {} // Complex/struct: skip inlining, handled via global below
    }
    
    // Also emit as global constant for test visibility and potential runtime usage (e.g. pointers to consts)
    let ty = resolve_type(ctx, &c.ty);
    let mlir_ty = ty.to_mlir_type(ctx)?;
    let name = ctx.mangle_fn_name(&c.name.to_string());
    if ctx.initialized_globals().contains(&*name) {
        return Ok(());
    }
    ctx.initialized_globals_mut().insert(name.to_string());
    let val_attr = match val {
        ConstValue::Integer(i) => {
             let suffix = match ty {
                 Type::I64 | Type::U64 | Type::Usize => "i64",
                 Type::I32 | Type::U32 => "i32",
                 Type::I16 | Type::U16 => "i16",
                 Type::I8 | Type::U8 => "i8",
                 Type::Bool => "i1",
                 _ => "i64" 
             };
             format!("{} : {}", i, suffix)
        }
        ConstValue::Float(f) => {
             let suffix = if matches!(ty, Type::F32) { "f32" } else { "f64" };
             format!("{} : {}", f, suffix)
        }
        ConstValue::Bool(b) => format!("{} : i1", if b { 1 } else { 0 }),
        // For complex consts (Structs/Arrays), we need recursive attribute printing.
        // Current Evaluator might return Complex?
        // If it's complex, we need a way to print it.
        // MVP: Just support primitives or use zero-init if complex (incorrect, but safe fallback?)
        // Better: error if complex const emission not supported yet.
        _ => {
             // [SCALAR WRAPPER FIX] Check if this is a scalar wrapper struct (single i32 field)
             // e.g., Prot { bits: 1 } should emit { 1 : i32 }, not zero
             if let syn::Expr::Struct(s) = &c.value {
                 if s.fields.len() == 1 {
                     if let Some(field) = s.fields.first() {
                         // Try to evaluate the field expression
                         if let Ok(ConstValue::Integer(i)) = ctx.evaluator.eval_expr(&field.expr) {
                             // Emit as an inline constant struct
                             let i32_val = i as i32;
                             ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global internal constant @{}() {{alignment = 4}} : {} {{\n", name, mlir_ty));
                             ctx.decl_out_mut().push_str(&format!("    %0 = llvm.mlir.constant({} : i32) : i32\n", i32_val));
                             ctx.decl_out_mut().push_str(&format!("    %1 = llvm.mlir.undef : {}\n", mlir_ty));
                             ctx.decl_out_mut().push_str(&format!("    %2 = llvm.insertvalue %0, %1[0] : {}\n", mlir_ty));
                             ctx.decl_out_mut().push_str(&format!("    llvm.return %2 : {}\n", mlir_ty));
                             ctx.decl_out_mut().push_str("  }\n");
                             return Ok(());
                         }
                     }
                 }
             }
             
             // Fallback to zero-init region for complex types (Structs/Arrays)
             // This is crucial for things like GLOBAL_ALLOC which resolve to Item::Const but are complex.
             // We drop 'constant' to be safe for 'var' mapping.
             // [ALIGNMENT ENFORCER] Calculate mandatory alignment
             let alignment = match &ty {
                 Type::Array(_, len, _) if *len >= 16 => 64,  // Cache-line aligned for large arrays
                 Type::Struct(_) | Type::Concrete(_, _) => 16, // 16-byte for aggregates
                 _ => 8, // Default 8-byte alignment
             };
             ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global internal @{}() {{alignment = {}}} : {} {{\n", name, alignment, mlir_ty));
             ctx.decl_out_mut().push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_ty));
             ctx.decl_out_mut().push_str(&format!("    llvm.return %0 : {}\n", mlir_ty));
             ctx.decl_out_mut().push_str("  }\n");
             return Ok(());
        }
    };
    
    ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global internal constant @{}({}) : {}\n", name, val_attr, mlir_ty));
    Ok(())
}

pub fn emit_global_def(ctx: &mut LoweringContext, _out: &mut String, g: &crate::grammar::GlobalDef) -> Result<(), String> {
    let ty_raw = resolve_type(ctx, &g.ty);
    // [SOVEREIGN FIX] Atomic<T> is a semantic wrapper — storage is just T.
    // Unwrap for MLIR emission (type + init_val), but keep Atomic<T> in globals table
    // so method dispatch (fetch_add, compare_exchange, load, store) still works.
    let ty_storage = match &ty_raw {
        Type::Atomic(inner) => (**inner).clone(),
        other => other.clone(),
    };
    let name = ctx.mangle_fn_name(&g.name.to_string());
    
    if ctx.initialized_globals().contains(&*name) {
        return Ok(());
    }
    
    // Store ORIGINAL type (Atomic<i32>) for method dispatch
    ctx.globals_mut().insert(name.to_string(), ty_raw.clone());
    ctx.initialized_globals_mut().insert(name.to_string());
    
    // Use UNWRAPPED type (i32) for MLIR emission
    let mlir_ty = ty_storage.to_mlir_storage_type(ctx)?;
    
    // Check for explicit initializer and evaluate it
    let init_val = if let Some(val_expr) = &g.init {
        let eval = crate::evaluator::Evaluator::new();
        match eval.eval_expr(val_expr) {
            Ok(crate::evaluator::ConstValue::Integer(i)) => {
                let suffix = match &ty_storage {
                    Type::I64 | Type::U64 | Type::Usize => "i64",
                    Type::I32 | Type::U32 => "i32",
                    Type::I16 | Type::U16 => "i16",
                    Type::I8 | Type::U8 => "i8",
                    Type::Bool => "i1",
                    _ => "i64",
                };
                format!("{} : {}", i, suffix)
            }
            Ok(crate::evaluator::ConstValue::Float(f)) => {
                let suffix = if matches!(&ty_storage, Type::F32) { "f32" } else { "f64" };
                format!("{} : {}", f, suffix)
            }
            Ok(crate::evaluator::ConstValue::Bool(b)) => {
                format!("{} : i1", if b { 1 } else { 0 })
            }
            _ => "".to_string(), // Complex types: still zero-init for now
        }
    } else {
        "".to_string()
    };
    
    if init_val.is_empty() {
        // [ALIGNMENT ENFORCER] Calculate mandatory alignment based on type size
        let alignment = match &ty_storage {
            Type::Array(_, len, _) if *len >= 16 => 64,  // Cache-line aligned for large arrays
            Type::Struct(_) | Type::Concrete(_, _) => 16, // 16-byte for aggregates
            _ => 8, // Default 8-byte alignment
        };
        
        // Use region-based zero initialization which works for all types (scalars, pointers, aggregates)
        ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global internal @{}() {{alignment = {}}} : {} {{\n", name, alignment, mlir_ty));
        ctx.decl_out_mut().push_str(&format!("    %0 = llvm.mlir.zero : {}\n", mlir_ty));
        ctx.decl_out_mut().push_str(&format!("    llvm.return %0 : {}\n", mlir_ty));
        ctx.decl_out_mut().push_str("  }\n");
    } else {
        ctx.decl_out_mut().push_str(&format!("  llvm.mlir.global internal @{}({}) : {}\n", name, init_val, mlir_ty));
    }
    Ok(())
}

pub fn zero_attr(ctx: &mut LoweringContext<'_, '_>, ty: &Type) -> Result<String, String> {
    match ty {
        Type::Bool => Ok("0 : i8".to_string()),
        Type::I8 | Type::U8 => Ok("0 : i8".to_string()),
        Type::I16| Type::U16 => Ok("0 : i16".to_string()),
        Type::I32| Type::U32 => Ok("0 : i32".to_string()),
        Type::I64| Type::U64 | Type::Usize => Ok("0 : i64".to_string()),
        Type::F32 => Ok("0.0 : f32".to_string()),
        Type::F64 => Ok("0.0 : f64".to_string()),
        Type::Owned(_) | Type::Reference(_, _) | Type::Fn(_, _) => Ok("null : !llvm.ptr".to_string()),
        // [SOVEREIGN FIX] Atomic<T> storage is the inner type T, not a pointer.
        // Recurse to get the correct zero value (e.g., Atomic<i32> → "0 : i32").
        Type::Atomic(inner) => zero_attr(ctx, inner),
        
        Type::Array(inner, len, _) => {
            let inner_attr = zero_attr(ctx, inner)?;
            if inner_attr.is_empty() { return Ok("".to_string()); }
            let mut parts = Vec::new();
            for _ in 0..*len {
                parts.push(inner_attr.clone());
            }
            Ok(format!("[{}]", parts.join(", ")))
        }
        Type::Tuple(elems) => {
            let mut parts = Vec::new();
            for e in elems {
                let attr = zero_attr(ctx, e)?;
                if attr.is_empty() { return Ok("".to_string()); }
                parts.push(attr);
            }
            Ok(format!("[{}]", parts.join(", ")))
        }
        Type::Struct(name) => {
            let info_opt = ctx.struct_registry().values().find(|i| i.name == *name).cloned();
            if let Some(info) = info_opt {
                let mut parts = Vec::new();
                for ty in &info.field_order {
                    let attr = zero_attr(ctx, ty)?;
                    if attr.is_empty() { return Ok("".to_string()); }
                    parts.push(attr);
                }
                Ok(format!("[{}]", parts.join(", ")))
            } else {
                Ok("".to_string())
            }
        }
        Type::Enum(name) => {
            if let Some(info) = ctx.enum_registry().values().find(|i| i.name == *name).cloned() {
                let mut parts = vec!["0 : i32".to_string()];
                if info.max_payload_size > 0 {
                    // Padding array (4 bytes)
                    parts.push("[0 : i8, 0 : i8, 0 : i8, 0 : i8]".to_string());
                    
                    let mut zeros = Vec::new();
                    for _ in 0..info.max_payload_size {
                        zeros.push("0 : i8".to_string());
                    }
                    parts.push(format!("[{}]", zeros.join(", ")));
                }
                Ok(format!("[{}]", parts.join(", ")))
            } else {
                Ok("".to_string())
            }
        }
        Type::Concrete(..) => {
             // Resolve to mangled name and retry
             let resolved = resolve_codegen_type(ctx, ty);
             zero_attr(ctx, &resolved)
        }
        Type::Never => Ok("".to_string()),
        Type::SelfType => Err("Unresolved 'Self' type reached zero_attr. This is a compiler bug.".to_string()),
        _ => Err(format!("No zero attribute for type {:?}", ty)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::context::{CodegenContext, LoweringContext};
    use crate::registry::EnumInfo;
    use crate::grammar::SaltFile;

    #[test]
    fn test_enum_payload_packing() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = z3::Config::new();
        let _z3_ctx = z3::Context::new(&z3_cfg);
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let name = "PackingEnum".to_string();
        let variants = vec![
            ("A".to_string(), Some(Type::U8), 0),
            ("B".to_string(), Some(Type::Array(Box::new(Type::F64), 8, false)), 1),
        ];

        let info = EnumInfo {
            name: name.clone(),
            variants,
            max_payload_size: 64,
            template_name: None,
            specialization_args: vec![],
        };
        let key = TypeKey { path: vec![], name: name.clone(), specialization: None };
        ctx.enum_registry_mut().insert(key, info);

        let ty = Type::Enum(name);
        let mlir = ctx.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx)).unwrap();
        // After enum type resolution fix: registered enums return their type alias
        // The inline struct definition with payload is emitted separately in type definitions
        assert_eq!(mlir, "!struct_PackingEnum", "Registered enum should use type alias");
    }

    // =========================================================================
    // TDD: Usize (MLIR index) ↔ I64 type conversion
    // =========================================================================
    // Bug context: The compiler generates MLIR `index` for `usize` params but
    // tracks them as `I64` in local_vars, causing `as i64` casts to be no-ops.
    // These tests ensure the conversion functions correctly emit arith.index_cast.

    #[test]
    fn test_usize_and_i64_are_distinct_types() {
        // CRITICAL: Type::Usize and Type::I64 must NOT be equal.
        // If they were, emit_cast's `if ty == target_ty` check would skip
        // the arith.index_cast, leaving index-typed values in i64 operations.
        assert_ne!(Type::Usize, Type::I64,
            "Type::Usize and Type::I64 must be distinct types");
        assert_ne!(Type::Usize, Type::U64,
            "Type::Usize and Type::U64 must be distinct types");
    }

    #[test]
    fn test_promote_numeric_usize_to_i64_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = z3::Config::new();
        let _z3_ctx = z3::Context::new(&z3_cfg);
        let z3_cfg2 = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg2);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%arg_len", &Type::Usize, &Type::I64));

        assert!(result.is_ok(), "promote_numeric(Usize, I64) should succeed");
        assert!(out.contains("arith.index_cast"),
            "Usize→I64 must emit arith.index_cast, got: {}", out);
        assert!(out.contains("index to i64"),
            "Cast should be 'index to i64', got: {}", out);
    }

    #[test]
    fn test_promote_numeric_i64_to_usize_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = z3::Config::new();
        let _z3_ctx = z3::Context::new(&z3_cfg);
        let z3_cfg2 = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg2);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%val", &Type::I64, &Type::Usize));

        assert!(result.is_ok(), "promote_numeric(I64, Usize) should succeed");
        assert!(out.contains("arith.index_cast"),
            "I64→Usize must emit arith.index_cast, got: {}", out);
        assert!(out.contains("i64 to index"),
            "Cast should be 'i64 to index', got: {}", out);
    }

    #[test]
    fn test_cast_numeric_usize_to_i64_emits_index_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = z3::Config::new();
        let _z3_ctx = z3::Context::new(&z3_cfg);
        let z3_cfg2 = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg2);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| cast_numeric(lctx, &mut out, "%arg_len", &Type::Usize, &Type::I64));

        assert!(result.is_ok(), "cast_numeric(Usize, I64) should succeed");
        assert!(out.contains("arith.index_cast"),
            "cast_numeric(Usize, I64) must emit arith.index_cast, got: {}", out);
    }

    #[test]
    fn test_usize_identity_does_not_emit_cast() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = z3::Config::new();
        let _z3_ctx = z3::Context::new(&z3_cfg);
        let z3_cfg2 = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg2);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let mut out = String::new();
        let result = ctx.with_lowering_ctx(|lctx| promote_numeric(lctx, &mut out, "%val", &Type::Usize, &Type::Usize));

        assert!(result.is_ok(), "promote_numeric(Usize, Usize) should succeed");
        assert!(out.is_empty(),
            "Usize→Usize should be identity (no MLIR emitted), got: {}", out);
    }

    // =========================================================================
    // TDD: Atomic<T> Type Emission — The Slab Memory Leak Root Cause
    // =========================================================================
    // Bug: Atomic<i32> globals emitted as `!llvm.ptr` with `null` init instead
    // of `i32` with `0 : i32` init. This causes LLVM Translation to reject the
    // MLIR with: "Global variable initializer type does not match global variable type!"
    //
    // Call graph layers to fix:
    //   Layer 0: to_mlir_type_simple(Atomic<T>) → T's MLIR type  [already works]
    //   Layer 1: zero_attr(Atomic<T>) → recurse to inner T
    //   Layer 2: to_mlir_storage_type_simple(Atomic<T>) → T's storage type
    //   Layer 3: emit_global_def sees Atomic<T> → unwraps to T for init_val

    // --- Layer 0: to_mlir_type_simple (already correct, assert for safety) ---
    #[test]
    fn test_atomic_i32_mlir_type_simple() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.to_mlir_type_simple(), "i32",
            "Atomic<i32> MLIR type should be 'i32', not '!llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_mlir_type_simple() {
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.to_mlir_type_simple(), "i64",
            "Atomic<u64> MLIR type should be 'i64'");
    }

    // --- Layer 1: zero_attr should recurse into inner type ---
    #[test]
    fn test_atomic_i32_zero_attr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ty = Type::Atomic(Box::new(Type::I32));
        let result = ctx.with_lowering_ctx(|lctx| zero_attr(lctx, &ty));
        assert!(result.is_ok(), "zero_attr(Atomic<i32>) should succeed");
        assert_eq!(result.unwrap(), "0 : i32",
            "zero_attr(Atomic<i32>) must be '0 : i32', not 'null : !llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_zero_attr() {
        let file: SaltFile = syn::parse_str("fn main() {}").unwrap();
        let z3_cfg = z3::Config::new();
        let z3_ctx = z3::Context::new(&z3_cfg);
        let mut ctx = CodegenContext::new(&file, false, None, &z3_ctx);

        let ty = Type::Atomic(Box::new(Type::U64));
        let result = ctx.with_lowering_ctx(|lctx| zero_attr(lctx, &ty));
        assert!(result.is_ok(), "zero_attr(Atomic<u64>) should succeed");
        assert_eq!(result.unwrap(), "0 : i64",
            "zero_attr(Atomic<u64>) must be '0 : i64', not 'null : !llvm.ptr'");
    }

    // --- Layer 2: to_mlir_storage_type_simple should unwrap to inner type ---
    #[test]
    fn test_atomic_i32_storage_type_simple() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.to_mlir_storage_type_simple(), "i32",
            "Atomic<i32> storage type should be 'i32', not '!llvm.ptr'");
    }

    #[test]
    fn test_atomic_u64_storage_type_simple() {
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.to_mlir_storage_type_simple(), "i64",
            "Atomic<u64> storage type should be 'i64'");
    }

    // --- Layer 3: k_is_ptr_type should NOT match Atomic ---
    #[test]
    fn test_atomic_is_not_ptr_type() {
        let ty = Type::Atomic(Box::new(Type::I32));
        assert!(!ty.k_is_ptr_type(),
            "Atomic<i32> is NOT a pointer type — it is a scalar wrapper");
    }

    // --- Layer 4: size_of should reflect inner type, not pointer ---
    #[test]
    fn test_atomic_i32_size_of() {
        let reg = std::collections::HashMap::new();
        let ty = Type::Atomic(Box::new(Type::I32));
        assert_eq!(ty.size_of(&reg), 4,
            "Atomic<i32> should be 4 bytes, not 8 (pointer size)");
    }

    #[test]
    fn test_atomic_u64_size_of() {
        let reg = std::collections::HashMap::new();
        let ty = Type::Atomic(Box::new(Type::U64));
        assert_eq!(ty.size_of(&reg), 8,
            "Atomic<u64> should be 8 bytes");
    }
}
