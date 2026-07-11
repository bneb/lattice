use crate::codegen::context::LoweringContext;
use crate::types::Type;

fn combine_conditions(ctx: &mut LoweringContext, out: &mut String, conds: Vec<String>, is_eq: bool) -> String {
     if conds.is_empty() {
         return if is_eq { "1 : i1".to_string() } else { "0 : i1".to_string() };
     }
     let mut curr = conds[0].clone();
     for next in &conds[1..] {
         let res = format!("%comb_{}", ctx.next_id());
         if is_eq {
             out.push_str(&format!("    {} = arith.andi {}, {} : i1\n", res, curr, next));
         } else {
             out.push_str(&format!("    {} = arith.ori {}, {} : i1\n", res, curr, next));
         }
         curr = res;
     }
     curr
}


pub fn emit_aggregate_eq(ctx: &mut LoweringContext, out: &mut String, op: &syn::BinOp, lhs: &str, rhs: &str, ty: &Type) -> Result<String, String> {
    let is_eq = matches!(op, syn::BinOp::Eq(_));
    
    match ty {
        Type::Struct(name) => emit_struct_eq(ctx, out, op, lhs, rhs, ty, name, is_eq),
        Type::Tuple(elems) => emit_tuple_eq(ctx, out, op, lhs, rhs, ty, elems, is_eq),
        Type::Array(inner, len, _) => emit_array_eq(ctx, out, op, lhs, rhs, ty, inner, *len, is_eq),
        Type::Enum(name) => emit_enum_eq(ctx, out, op, lhs, rhs, ty, name, is_eq),
        _ => Err(format!("Aggregate equality not implemented for type {:?}", ty)),
    }
}

#[allow(clippy::too_many_arguments)]
// REASON: all 8 parameters are independently meaningful for struct equality codegen; bundling would obscure emitter semantics.
fn emit_struct_eq(ctx: &mut LoweringContext, out: &mut String, op: &syn::BinOp, lhs: &str, rhs: &str, ty: &Type, name: &str, is_eq: bool) -> Result<String, String> {
    let canonical = ctx.struct_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).cloned();
    
    if let Some(info) = canonical {
        let mut conds = Vec::new();
        let fields = info.field_order.clone(); 
        
        for (i, field_ty) in fields.iter().enumerate() {
            let l_val = format!("%l_field_{}_{}", i, ctx.next_id());
            let r_val = format!("%r_field_{}_{}", i, ctx.next_id());
            
            let field_storage = field_ty.to_mlir_storage_type(ctx)?;
            let struct_storage = ty.to_mlir_storage_type(ctx)?;
            ctx.emit_extractvalue(out, &l_val, lhs, i, &struct_storage);
            ctx.emit_extractvalue(out, &r_val, rhs, i, &struct_storage);
            
            let is_ptr = matches!(field_ty, Type::Owned(_) | Type::Reference(..) | Type::Fn(..));
            let cond = if field_ty.is_numeric() || is_ptr || *field_ty == Type::Bool {
                let res = format!("%cmp_field_{}", ctx.next_id());
                let op_code = crate::codegen::type_bridge::get_arith_op(op, field_ty);
                let pred = crate::codegen::type_bridge::get_comparison_pred(op, field_ty);
                
                if op_code.contains("cmp") {
                    ctx.emit_cmp(out, &res, &op_code, &pred, &l_val, &r_val, &field_storage);
                } else {
                    return Err(format!("Invalid op for struct field eq: {:?}", op));
                }
                res
            } else {
                emit_aggregate_eq(ctx, out, op, &l_val, &r_val, field_ty)?
            };
            conds.push(cond);
        }
        Ok(combine_conditions(ctx, out, conds, is_eq))
    } else {
        let stripped_name = name.rsplit("__").next().unwrap_or(name);
        
        if let Some(e) = ctx.enum_registry().values().filter(|i| i.name == *name || i.name == stripped_name).min_by_key(|i| &i.name).cloned() {
            let res = format!("%cmp_enum_{}", ctx.next_id());
            let l_val = format!("%l_disc_{}", ctx.next_id());
            let r_val = format!("%r_disc_{}", ctx.next_id());
            let struct_ty = format!("!struct_{}", e.name);
            ctx.emit_extractvalue(out, &l_val, lhs, 0, &struct_ty);
            ctx.emit_extractvalue(out, &r_val, rhs, 0, &struct_ty);
            let pred = if is_eq { "eq" } else { "ne" };
            ctx.emit_cmp(out, &res, "arith.cmpi", pred, &l_val, &r_val, "i32");
            Ok(res)
        } else {
            Err(format!("Cannot compare unknown struct '{}'", name))
        }
    }
}

#[allow(clippy::too_many_arguments)]
// REASON: all 8 parameters are independently meaningful for tuple equality codegen; bundling would obscure emitter semantics.
fn emit_tuple_eq(ctx: &mut LoweringContext, out: &mut String, op: &syn::BinOp, lhs: &str, rhs: &str, ty: &Type, elems: &[Type], is_eq: bool) -> Result<String, String> {
    let mut conds = Vec::new();
    for (i, elem_ty) in elems.iter().enumerate() {
        let l_val = format!("%tup_l_{}", ctx.next_id());
        let r_val = format!("%tup_r_{}", ctx.next_id());
        
        let storage_ty = ty.to_mlir_storage_type(ctx)?;
        ctx.emit_extractvalue(out, &l_val, lhs, i, &storage_ty);
        ctx.emit_extractvalue(out, &r_val, rhs, i, &storage_ty);
        
        let is_ptr = matches!(elem_ty, Type::Owned(_) | Type::Reference(..) | Type::Fn(..));
        let cond = if elem_ty.is_numeric() || is_ptr || *elem_ty == Type::Bool {
            let res = format!("%cmp_tup_{}", ctx.next_id());
            let op_code = crate::codegen::type_bridge::get_arith_op(op, elem_ty);
            let pred = crate::codegen::type_bridge::get_comparison_pred(op, elem_ty);
            let elem_storage = elem_ty.to_mlir_storage_type(ctx)?;
            ctx.emit_cmp(out, &res, &op_code, &pred, &l_val, &r_val, &elem_storage);
            res
        } else {
            emit_aggregate_eq(ctx, out, op, &l_val, &r_val, elem_ty)?
        };
        conds.push(cond);
    }
    Ok(combine_conditions(ctx, out, conds, is_eq))
}

#[allow(clippy::too_many_arguments)]
// REASON: all 9 parameters are independently meaningful for array equality codegen; bundling would obscure emitter semantics.
fn emit_array_eq(ctx: &mut LoweringContext, out: &mut String, op: &syn::BinOp, lhs: &str, rhs: &str, ty: &Type, inner: &Type, len: usize, is_eq: bool) -> Result<String, String> {
    if len <= 16 {
        let mut conds = Vec::new();
        for i in 0..len {
            let l_val = format!("%arr_l_{}", ctx.next_id());
            let r_val = format!("%arr_r_{}", ctx.next_id());
            
            let storage_ty_l = ty.to_mlir_storage_type(ctx)?;
            ctx.emit_extractvalue(out, &l_val, lhs, i, &storage_ty_l);
            let storage_ty_r = ty.to_mlir_storage_type(ctx)?;
            ctx.emit_extractvalue(out, &r_val, rhs, i, &storage_ty_r);
            
            let is_ptr = matches!(inner, Type::Owned(_) | Type::Reference(..) | Type::Fn(..));
            let cond = if inner.is_numeric() || is_ptr || *inner == Type::Bool {
                let res = format!("%cmp_arr_{}", ctx.next_id());
                let op_code = crate::codegen::type_bridge::get_arith_op(op, inner);
                let pred = crate::codegen::type_bridge::get_comparison_pred(op, inner);
                let inner_storage = inner.to_mlir_storage_type(ctx)?;
                ctx.emit_cmp(out, &res, &op_code, &pred, &l_val, &r_val, &inner_storage);
                res
            } else {
                emit_aggregate_eq(ctx, out, op, &l_val, &r_val, inner)?
            };
            conds.push(cond);
        }
        Ok(combine_conditions(ctx, out, conds, is_eq))
    } else {
        let loop_res = format!("%loop_res_{}", ctx.next_id());
        let c_len = format!("%c_len_{}", ctx.next_id());
        let c_step = format!("%c_step_{}", ctx.next_id());
        let c_start = format!("%c_start_{}", ctx.next_id());
        let c_init = format!("%c_init_{}", ctx.next_id());
        
        ctx.emit_const_int(out, &c_start, 0, "index");
        ctx.emit_const_int(out, &c_len, len as i64, "index");
        ctx.emit_const_int(out, &c_step, 1, "index");
        
        let (init_val, op_combine) = if is_eq { (1, "arith.andi") } else { (0, "arith.ori") };
        ctx.emit_const_int(out, &c_init, init_val, "i1");
        
        let ty_storage = ty.to_mlir_storage_type(ctx)?;
        
        let stack_l = format!("%stack_l_{}", ctx.next_id());
        let stack_r = format!("%stack_r_{}", ctx.next_id());
        ctx.emit_alloca(out, &stack_l, &ty_storage);
        ctx.emit_store(out, lhs, &stack_l, &ty_storage);
        
        ctx.emit_alloca(out, &stack_r, &ty_storage);
        ctx.emit_store(out, rhs, &stack_r, &ty_storage);
        
        out.push_str(&format!("    {} = scf.for %idx = {} to {} step {} iter_args(%acc = {}) -> (i1) {{
", 
            loop_res, c_start, c_len, c_step, c_init));
        
        let inner_storage = inner.to_mlir_storage_type(ctx)?;
        
        let gep_l = format!("%gep_l_{}", ctx.next_id());
        let gep_r = format!("%gep_r_{}", ctx.next_id());
        
        out.push_str(&format!("      {} = llvm.getelementptr {}_{}[0, %idx] : (!llvm.ptr, i64) -> !llvm.ptr, {}
", 
            gep_l, stack_l, "", inner_storage));
        out.push_str(&format!("      {} = llvm.getelementptr {}_{}[0, %idx] : (!llvm.ptr, i64) -> !llvm.ptr, {}
", 
            gep_r, stack_r, "", inner_storage));
            
        let v_l = format!("%v_l_{}", ctx.next_id());
        let v_r = format!("%v_r_{}", ctx.next_id());
        ctx.emit_load(out, &v_l, &gep_l, &inner_storage);
        ctx.emit_load(out, &v_r, &gep_r, &inner_storage);
        
        let is_ptr = matches!(inner, Type::Owned(_) | Type::Reference(..) | Type::Fn(..));
        let cond_val = if inner.is_numeric() || is_ptr || *inner == Type::Bool {
           let res = format!("%cmp_iter_{}", ctx.next_id());
           let op_code = crate::codegen::type_bridge::get_arith_op(op, inner);
           let pred = crate::codegen::type_bridge::get_comparison_pred(op, inner);
           ctx.emit_cmp(out, &res, &op_code, &pred, &v_l, &v_r, &inner_storage);
           res
        } else {
           emit_aggregate_eq(ctx, out, op, &v_l, &v_r, inner)?
        };
        
        let next_acc = format!("%next_acc_{}", ctx.next_id());
        out.push_str(&format!("      {} = {} %acc, {} : i1
", next_acc, op_combine, cond_val));
        
        out.push_str(&format!("      scf.yield {} : i1
", next_acc));
        out.push_str("    }
");
        
        Ok(loop_res)
    }
}

#[allow(clippy::too_many_arguments)]
// REASON: all 8 parameters are independently meaningful for enum equality codegen; bundling would obscure emitter semantics.
fn emit_enum_eq(ctx: &mut LoweringContext, out: &mut String, op: &syn::BinOp, lhs: &str, rhs: &str, ty: &Type, name: &str, is_eq: bool) -> Result<String, String> {
    if let Some(info) = ctx.enum_registry().values().filter(|i| i.name == *name).min_by_key(|i| &i.name).cloned() {
        let mut conds = Vec::new();
        let mlir_ty = ty.to_mlir_storage_type(ctx)?;
        
        let l_tag = format!("%tag_l_{}", ctx.next_id());
        let r_tag = format!("%tag_r_{}", ctx.next_id());
        ctx.emit_extractvalue(out, &l_tag, lhs, 0, &mlir_ty);
        ctx.emit_extractvalue(out, &r_tag, rhs, 0, &mlir_ty);
        
        let tag_res = format!("%cmp_tag_{}", ctx.next_id());
        ctx.emit_cmp(out, &tag_res, "arith.cmpi", "eq", &l_tag, &r_tag, "i32");
        conds.push(tag_res);
        
        if info.max_payload_size > 0 {
            let pad_ty = Type::Array(Box::new(Type::U8), 4, false);
            let l_pad = format!("%pad_l_{}", ctx.next_id());
            let r_pad = format!("%pad_r_{}", ctx.next_id());
            ctx.emit_extractvalue(out, &l_pad, lhs, 1, &mlir_ty);
            ctx.emit_extractvalue(out, &r_pad, rhs, 1, &mlir_ty);
            conds.push(emit_aggregate_eq(ctx, out, op, &l_pad, &r_pad, &pad_ty)?);
            
            let pay_ty = Type::Array(Box::new(Type::U8), info.max_payload_size, false);
            let l_pay = format!("%pay_l_{}", ctx.next_id());
            let r_pay = format!("%pay_r_{}", ctx.next_id());
            ctx.emit_extractvalue(out, &l_pay, lhs, 2, &mlir_ty);
            ctx.emit_extractvalue(out, &r_pay, rhs, 2, &mlir_ty);
            conds.push(emit_aggregate_eq(ctx, out, op, &l_pay, &r_pay, &pay_ty)?);
        }
        Ok(combine_conditions(ctx, out, conds, is_eq))
    } else {
        Err(format!("Unknown enum {}", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::context::CodegenContext;
    use crate::grammar::SaltFile;

    #[test]
    fn test_nested_tuple_array_equality() {
        let file: SaltFile = syn::parse_str("fn main() {}").expect("hardcoded fn main is valid SaltFile");
        let z3_cfg = crate::z3_shim::Config::new();
        let _z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let z3_cfg = crate::z3_shim::Config::new();
        let z3_ctx = crate::z3_shim::Context::new(&z3_cfg);
        let ctx = CodegenContext::new(&file, false, None, &z3_ctx);
        
        let inner_tuple = Type::Tuple(vec![Type::I64, Type::I64]);
        let arr = Type::Array(Box::new(Type::U8), 4, false);
        let ty = Type::Tuple(vec![Type::I32, arr, inner_tuple]);
        
        let mut out = String::new();
        let op: syn::BinOp = syn::parse_str("==").expect("hardcoded == is valid BinOp");
        let res = ctx.with_lowering_ctx(|lctx| emit_aggregate_eq(lctx, &mut out, &op, "%lhs", "%rhs", &ty));
        
        assert!(res.is_ok());
        assert!(out.contains("arith.cmpi \"eq\","));
    }
}
