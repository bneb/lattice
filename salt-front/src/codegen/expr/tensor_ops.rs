//! Tensor Operations Module - In-place mutating tensor methods
//!
//! This module implements PyTorch-style mutating methods with `_` suffix:
//! - `add_(&mut self, other)` - In-place addition
//! - `relu_(&mut self)` - In-place ReLU activation
//! - `scale_(&mut self, s)` - In-place scalar multiplication
//! - `sub_(&mut self, other)` - In-place subtraction
//!
//! These emit linalg.generic ops that use the same buffer for input and output,
//! enabling MLIR fusion passes to merge chained operations.

use crate::types::Type;
use crate::codegen::context::LoweringContext;

/// Check if a method name indicates an in-place mutating operation.
/// Following PyTorch convention: methods ending with `_` mutate in-place.
pub fn is_mutating_method(method_name: &str) -> bool {
    method_name.ends_with('_') && method_name.len() > 1
}

/// Get the base method name without the mutating suffix
pub fn strip_mutating_suffix(method_name: &str) -> &str {
    if is_mutating_method(method_name) {
        &method_name[..method_name.len() - 1]
    } else {
        method_name
    }
}

/// Emit in-place ReLU: max(0, x) for each element
/// 
/// Generates:
/// ```mlir
/// linalg.generic {
///   indexing_maps = [affine_map<(d0) -> (d0)>, affine_map<(d0) -> (d0)>],
///   iterator_types = ["parallel"]
/// } ins(%tensor : memref<Nxf32>) outs(%tensor : memref<Nxf32>) {
///   ^bb0(%in: f32, %out: f32):
///     %zero = arith.constant 0.0 : f32
///     %result = arith.maximumf %in, %zero : f32
///     linalg.yield %result : f32
/// }
/// ```
pub fn emit_inplace_relu(
    ctx: &mut LoweringContext,
    out: &mut String,
    tensor_val: &str,
    tensor_ty: &Type,
) -> Result<String, String> {
    let (elem_ty, shape) = match tensor_ty {
        Type::Tensor(inner, shape) => (inner.as_ref(), shape.clone()),
        _ => return Err(format!("relu_ requires Tensor type, got {:?}", tensor_ty)),
    };

    let elem_mlir = elem_ty.to_mlir_type(ctx)?;
    let shape_str = shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
    let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
    
    let id = ctx.next_id();
    
    // Generate affine maps for the iteration space
    let dims: String = (0..shape.len()).map(|i| format!("d{}", i)).collect::<Vec<_>>().join(", ");
    let indexing = format!("({})", dims);
    
    out.push_str(&format!(
        r#"    // relu_: in-place ReLU activation
    linalg.generic {{
      indexing_maps = [affine_map<{indexing} -> {indexing}>, affine_map<{indexing} -> {indexing}>],
      iterator_types = [{iters}]
    }} ins({tensor} : {ty}) outs({tensor} : {ty}) {{
      ^bb0(%in_{id}: {elem}, %out_{id}: {elem}):
        %zero_{id} = arith.constant 0.0 : {elem}
        %result_{id} = arith.maximumf %in_{id}, %zero_{id} : {elem}
        linalg.yield %result_{id} : {elem}
    }}
"#,
        indexing = indexing,
        iters = (0..shape.len()).map(|_| "\"parallel\"").collect::<Vec<_>>().join(", "),
        tensor = tensor_val,
        ty = memref_ty,
        elem = elem_mlir,
        id = id,
    ));
    
    // Return the same tensor (for chaining)
    Ok(tensor_val.to_string())
}

/// Emit in-place scalar multiplication: x * s for each element
pub fn emit_inplace_scale(
    ctx: &mut LoweringContext,
    out: &mut String,
    tensor_val: &str,
    tensor_ty: &Type,
    scalar_val: &str,
    _scalar_ty: &Type,
) -> Result<String, String> {
    let (elem_ty, shape) = match tensor_ty {
        Type::Tensor(inner, shape) => (inner.as_ref(), shape.clone()),
        _ => return Err(format!("scale_ requires Tensor type, got {:?}", tensor_ty)),
    };

    let elem_mlir = elem_ty.to_mlir_type(ctx)?;
    let shape_str = shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
    let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
    
    let id = ctx.next_id();
    let dims: String = (0..shape.len()).map(|i| format!("d{}", i)).collect::<Vec<_>>().join(", ");
    let indexing = format!("({})", dims);
    
    out.push_str(&format!(
        r#"    // scale_: in-place scalar multiplication
    linalg.generic {{
      indexing_maps = [affine_map<{indexing} -> {indexing}>, affine_map<{indexing} -> {indexing}>],
      iterator_types = [{iters}]
    }} ins({tensor} : {ty}) outs({tensor} : {ty}) {{
      ^bb0(%in_{id}: {elem}, %out_{id}: {elem}):
        %result_{id} = arith.mulf %in_{id}, {scalar} : {elem}
        linalg.yield %result_{id} : {elem}
    }}
"#,
        indexing = indexing,
        iters = (0..shape.len()).map(|_| "\"parallel\"").collect::<Vec<_>>().join(", "),
        tensor = tensor_val,
        ty = memref_ty,
        elem = elem_mlir,
        scalar = scalar_val,
        id = id,
    ));
    
    Ok(tensor_val.to_string())
}

/// Emit in-place addition: self[i] += other[i] for each element
pub fn emit_inplace_add(
    ctx: &mut LoweringContext,
    out: &mut String,
    tensor_val: &str,
    tensor_ty: &Type,
    other_val: &str,
    _other_ty: &Type,
) -> Result<String, String> {
    let (elem_ty, shape) = match tensor_ty {
        Type::Tensor(inner, shape) => (inner.as_ref(), shape.clone()),
        _ => return Err(format!("add_ requires Tensor type, got {:?}", tensor_ty)),
    };

    let elem_mlir = elem_ty.to_mlir_type(ctx)?;
    let shape_str = shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
    let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
    
    let id = ctx.next_id();
    let dims: String = (0..shape.len()).map(|i| format!("d{}", i)).collect::<Vec<_>>().join(", ");
    let indexing = format!("({})", dims);
    
    out.push_str(&format!(
        r#"    // add_: in-place element-wise addition
    linalg.generic {{
      indexing_maps = [affine_map<{indexing} -> {indexing}>, affine_map<{indexing} -> {indexing}>, affine_map<{indexing} -> {indexing}>],
      iterator_types = [{iters}]
    }} ins({tensor} : {ty}, {other} : {ty}) outs({tensor} : {ty}) {{
      ^bb0(%self_{id}: {elem}, %other_{id}: {elem}, %out_{id}: {elem}):
        %result_{id} = arith.addf %self_{id}, %other_{id} : {elem}
        linalg.yield %result_{id} : {elem}
    }}
"#,
        indexing = indexing,
        iters = (0..shape.len()).map(|_| "\"parallel\"").collect::<Vec<_>>().join(", "),
        tensor = tensor_val,
        other = other_val,
        ty = memref_ty,
        elem = elem_mlir,
        id = id,
    ));
    
    Ok(tensor_val.to_string())
}

/// Emit in-place subtraction: self[i] -= other[i] for each element
pub fn emit_inplace_sub(
    ctx: &mut LoweringContext,
    out: &mut String,
    tensor_val: &str,
    tensor_ty: &Type,
    other_val: &str,
    _other_ty: &Type,
) -> Result<String, String> {
    let (elem_ty, shape) = match tensor_ty {
        Type::Tensor(inner, shape) => (inner.as_ref(), shape.clone()),
        _ => return Err(format!("sub_ requires Tensor type, got {:?}", tensor_ty)),
    };

    let elem_mlir = elem_ty.to_mlir_type(ctx)?;
    let shape_str = shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
    let memref_ty = format!("memref<{}x{}>", shape_str, elem_mlir);
    
    let id = ctx.next_id();
    let dims: String = (0..shape.len()).map(|i| format!("d{}", i)).collect::<Vec<_>>().join(", ");
    let indexing = format!("({})", dims);
    
    out.push_str(&format!(
        r#"    // sub_: in-place element-wise subtraction
    linalg.generic {{
      indexing_maps = [affine_map<{indexing} -> {indexing}>, affine_map<{indexing} -> {indexing}>, affine_map<{indexing} -> {indexing}>],
      iterator_types = [{iters}]
    }} ins({tensor} : {ty}, {other} : {ty}) outs({tensor} : {ty}) {{
      ^bb0(%self_{id}: {elem}, %other_{id}: {elem}, %out_{id}: {elem}):
        %result_{id} = arith.subf %self_{id}, %other_{id} : {elem}
        linalg.yield %result_{id} : {elem}
    }}
"#,
        indexing = indexing,
        iters = (0..shape.len()).map(|_| "\"parallel\"").collect::<Vec<_>>().join(", "),
        tensor = tensor_val,
        other = other_val,
        ty = memref_ty,
        elem = elem_mlir,
        id = id,
    ));
    
    Ok(tensor_val.to_string())
}

/// Emit matrix multiplication using linalg.matmul
///
/// For 2D tensors: C = A @ B where A is MxK and B is KxN, result is MxN
#[allow(clippy::too_many_arguments)]
// REASON: all 8 parameters are independently meaningful; bundling would obscure intent
pub fn emit_matmul(
    ctx: &mut LoweringContext,
    out: &mut String,
    a_val: &str,
    a_ty: &Type,
    b_val: &str,
    b_ty: &Type,
    c_val: &str,  // Output buffer (must be pre-allocated)
    c_ty: &Type,
) -> Result<String, String> {
    let (a_elem, a_shape) = match a_ty {
        Type::Tensor(inner, shape) if shape.len() == 2 => (inner.as_ref(), shape.clone()),
        _ => return Err(format!("matmul requires 2D Tensor for A, got {:?}", a_ty)),
    };
    
    let (_b_elem, b_shape) = match b_ty {
        Type::Tensor(inner, shape) if shape.len() == 2 => (inner.as_ref(), shape.clone()),
        _ => return Err(format!("matmul requires 2D Tensor for B, got {:?}", b_ty)),
    };
    
    let (_c_elem, c_shape) = match c_ty {
        Type::Tensor(inner, shape) if shape.len() == 2 => (inner.as_ref(), shape.clone()),
        _ => return Err(format!("matmul requires 2D Tensor for C, got {:?}", c_ty)),
    };
    
    // Verify dimensions: A[M,K] @ B[K,N] = C[M,N]
    let m = a_shape[0];
    let k = a_shape[1];
    let n = b_shape[1];
    
    if b_shape[0] != k {
        return Err(format!("matmul dimension mismatch: A is {}x{}, B is {}x{}", 
            m, k, b_shape[0], b_shape[1]));
    }
    if c_shape[0] != m || c_shape[1] != n {
        return Err(format!("matmul output dimension mismatch: expected {}x{}, got {}x{}", 
            m, n, c_shape[0], c_shape[1]));
    }
    
    let elem_mlir = a_elem.to_mlir_type(ctx)?;
    let a_memref = format!("memref<{}x{}x{}>", m, k, elem_mlir);
    let b_memref = format!("memref<{}x{}x{}>", k, n, elem_mlir);
    let c_memref = format!("memref<{}x{}x{}>", m, n, elem_mlir);
    
    out.push_str(&format!(
        r#"    // matmul: C = A @ B using linalg.matmul
    linalg.matmul ins({a} : {a_ty}, {b} : {b_ty}) outs({c} : {c_ty})
"#,
        a = a_val,
        b = b_val,
        c = c_val,
        a_ty = a_memref,
        b_ty = b_memref,
        c_ty = c_memref,
    ));
    
    Ok(c_val.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_mutating_method() {
        assert!(is_mutating_method("add_"));
        assert!(is_mutating_method("relu_"));
        assert!(is_mutating_method("scale_"));
        assert!(!is_mutating_method("add"));
        assert!(!is_mutating_method("relu"));
        assert!(!is_mutating_method("_")); // Single underscore is not mutating
        assert!(!is_mutating_method("")); 
    }

    #[test]
    fn test_strip_mutating_suffix() {
        assert_eq!(strip_mutating_suffix("add_"), "add");
        assert_eq!(strip_mutating_suffix("relu_"), "relu");
        assert_eq!(strip_mutating_suffix("add"), "add");
    }
}
