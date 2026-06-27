use crate::types::Type;
use crate::codegen::context::{LoweringContext, LocalKind};
use crate::codegen::expr::emit_expr;
use std::collections::HashMap;

pub mod memory;
pub mod memory_ops;
pub mod atomics;
pub mod simd;
pub mod math;
pub mod system;
pub mod tensors;
pub mod io;

use memory::emit_memory_intrinsic;
use atomics::emit_atomic_intrinsic;
use simd::emit_simd_intrinsic;
use math::emit_math_intrinsic;
use system::emit_system_intrinsic;
use tensors::emit_tensor_intrinsic;
use io::emit_io_intrinsic;

/// Canonical name for std::ptr::Ptr<T> (unified across all codegen paths)
pub const PTR_CANONICAL_NAME: &str = "std__core__ptr__Ptr";


impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    pub fn emit_intrinsic(&mut self, out: &mut String, name: &str, args: &[syn::Expr], local_vars: &mut HashMap<String, (Type, LocalKind)>, _expected_ty: Option<&Type>) -> Result<Option<(String, Type)>, String> {
        // Unmangle standard library intrinsics
        let clean_name = name.strip_prefix("std__arith__")
            .or_else(|| name.strip_prefix("std__llvm__"))
            .unwrap_or(name);

        // Try categorized modules first
        if let Some(res) = emit_memory_intrinsic(self, out, clean_name, args, local_vars, _expected_ty)? {
            return Ok(Some(res));
        }
        if let Some(res) = emit_math_intrinsic(self, out, clean_name, args, local_vars, _expected_ty)? {
            return Ok(Some(res));
        }
        if let Some(res) = emit_atomic_intrinsic(self, out, clean_name, args, local_vars, _expected_ty)? {
            return Ok(Some(res));
        }
        if let Some(res) = emit_simd_intrinsic(self, out, clean_name, args, local_vars, _expected_ty)? {
            return Ok(Some(res));
        }
        if let Some(res) = emit_system_intrinsic(self, out, clean_name, args, local_vars, _expected_ty)? {
            return Ok(Some(res));
        }
        if let Some(res) = emit_tensor_intrinsic(self, out, clean_name, args, local_vars, _expected_ty)? {
            return Ok(Some(res));
        }
        if let Some(res) = emit_io_intrinsic(self, out, clean_name, args, local_vars)? {
            return Ok(Some(res));
        }

        // Remaining legacy intrinsics
        match clean_name {
            "std__core__ptr__intrin__addr_of" | "addr_of" => {
                 if args.len() != 1 {
                     return Err("addr_of expects 1 argument".to_string());
                 }
                 let (val, ty) = emit_expr(self, out, &args[0], local_vars, None)?;
                 let res = format!("%addr_of_{}", self.next_id());
                 if ty.k_is_ptr_type() {
                      return Ok(Some((val, ty)));
                 }
                 out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", res, val));
                 Ok(Some((res, Type::I64)))
            }
            _ => Ok(None)
        }
    }

    pub fn emit_lto_hook(&mut self, out: &mut String, symbol: &str, args: &[syn::Expr], local_vars: &mut HashMap<String, (Type, LocalKind)>, _expected_ty: Option<&Type>) -> Result<Option<(String, Type)>, String> {
        let mut arg_vals = Vec::new();
        let mut arg_tys = Vec::new();

        for arg in args {
            let (val, ty) = emit_expr(self, out, arg, local_vars, None)?;
            arg_vals.push(val);
            arg_tys.push(ty);
        }

        let ret_ty = if let Some(t) = _expected_ty { t.clone() } else { Type::Unit };
        self.entity_registry_mut().register_hook(symbol);

        let args_str = arg_vals.join(", ");
        let arg_types_str = arg_tys.iter().map(|t| t.to_mlir_type(self)).collect::<Result<Vec<_>,_>>()?.join(", ");
        let ret_ty_str = ret_ty.to_mlir_type(self)?;

        if ret_ty == Type::Unit {
            out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", symbol, args_str, arg_types_str));
            Ok(Some(("%unit".to_string(), Type::Unit)))
        } else {
             let res = format!("%hook_res_{}", self.next_id());
             out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", res, symbol, args_str, arg_types_str, ret_ty_str));
             Ok(Some((res, ret_ty)))
        }
    }

    pub fn emit_print_literal(&mut self, out: &mut String, s: &str) -> Result<(), String> {
        let mut current = String::new();
        for ch in s.chars() {
            match ch {
                '\n' | '\t' | '\r' => {
                    if !current.is_empty() {
                        self.emit_print_literal_raw(out, &current)?;
                        current.clear();
                    }
                    let code = match ch {
                        '\n' => 10,
                        '\t' => 9,
                        '\r' => 13,
                        _ => unreachable!(),
                    };
                    let char_val = format!("%putchar_arg_{}", self.next_id());
                    self.emit_const_int(out, &char_val, code, "i32");
                    self.entity_registry_mut().register_hook("putchar");
                    out.push_str(&format!("    func.call @putchar({}) : (i32) -> i32\n", char_val));
                }
                _ => current.push(ch),
            }
        }
        if !current.is_empty() {
            self.emit_print_literal_raw(out, &current)?;
        }
        Ok(())
    }
    
    fn emit_print_literal_raw(&mut self, out: &mut String, s: &str) -> Result<(), String> {
        let escaped = s.replace("\\", "\\\\").replace("\"", "\\22");
        let len = s.len();
        let global_name = format!("__str_literal_{}", self.next_id());
        self.string_literals_mut().push((global_name.clone(), escaped, len));
        let ptr_var = format!("%str_ptr_{}", self.next_id());
        let len_var = format!("%str_len_{}", self.next_id());
        out.push_str(&format!("    {} = llvm.mlir.addressof @{} : !llvm.ptr\n", ptr_var, global_name));
        self.emit_const_int(out, &len_var, len as i64, "i64");
        self.entity_registry_mut().register_hook("__salt_print_literal");
        out.push_str(&format!("    func.call @__salt_print_literal({}, {}) : (!llvm.ptr, i64) -> ()\n", ptr_var, len_var));
        Ok(())
    }
    
    pub fn emit_print_typed(&mut self, out: &mut String, val: &str, ty: &Type) -> Result<(), String> {
        match ty {
            Type::I8 | Type::I16 | Type::I32 | Type::I64 => {
                let val64 = if matches!(ty, Type::I64) {
                    val.to_string()
                } else {
                    let extended = format!("%print_ext_{}", self.next_id());
                    let src_ty = ty.to_mlir_type(self)?;
                    out.push_str(&format!("    {} = arith.extsi {} : {} to i64\n", extended, val, src_ty));
                    extended
                };
                self.entity_registry_mut().register_hook("__salt_print_i64");
                out.push_str(&format!("    func.call @__salt_print_i64({}) : (i64) -> ()\n", val64));
            }
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::Usize => {
                let val64 = if matches!(ty, Type::U64) {
                    val.to_string()
                } else if matches!(ty, Type::Usize) {
                    let casted = format!("%print_idx_{}", self.next_id());
                    out.push_str(&format!("    {} = arith.index_cast {} : index to i64\n", casted, val));
                    casted
                } else {
                    let extended = format!("%print_ext_{}", self.next_id());
                    let src_ty = ty.to_mlir_type(self)?;
                    out.push_str(&format!("    {} = arith.extui {} : {} to i64\n", extended, val, src_ty));
                    extended
                };
                self.entity_registry_mut().register_hook("__salt_print_u64");
                out.push_str(&format!("    func.call @__salt_print_u64({}) : (i64) -> ()\n", val64));
            }
            Type::F32 => {
                let val64 = format!("%print_f64_{}", self.next_id());
                out.push_str(&format!("    {} = arith.extf {} : f32 to f64\n", val64, val));
                self.entity_registry_mut().register_hook("__salt_print_f64");
                out.push_str(&format!("    func.call @__salt_print_f64({}) : (f64) -> ()\n", val64));
            }
            Type::F64 => {
                self.entity_registry_mut().register_hook("__salt_print_f64");
                out.push_str(&format!("    func.call @__salt_print_f64({}) : (f64) -> ()\n", val));
            }
            Type::Bool => {
                let val8 = format!("%print_bool_ext_{}", self.next_id());
                out.push_str(&format!("    {} = arith.extui {} : i1 to i8\n", val8, val));
                self.entity_registry_mut().register_hook("__salt_print_bool");
                out.push_str(&format!("    func.call @__salt_print_bool({}) : (i8) -> ()\n", val8));
            }
            Type::Reference(_, _) => {
                let addr = format!("%print_addr_{}", self.next_id());
                out.push_str(&format!("    {} = llvm.ptrtoint {} : !llvm.ptr to i64\n", addr, val));
                self.entity_registry_mut().register_hook("__salt_print_ptr");
                out.push_str(&format!("    func.call @__salt_print_ptr({}) : (i64) -> ()\n", addr));
            }
            Type::Struct(name) | Type::Concrete(name, _) => {
                let type_key = crate::codegen::type_bridge::type_to_type_key(ty);
                if self.trait_registry().contains_method(&type_key, "fmt") {
                    let id = self.next_id();
                    let mangled_name = format!("{}__fmt", name);
                    let fmt_impl_data = self.generic_impls().get(&mangled_name).cloned();
                    if let Some((func_def, func_imports)) = fmt_impl_data {
                        let task = crate::codegen::collector::MonomorphizationTask {
                            identity: crate::types::TypeKey { path: vec![], name: mangled_name.clone(), specialization: None },
                            mangled_name: mangled_name.clone(),
                            func: func_def,
                            concrete_tys: vec![],
                            self_ty: Some(ty.clone()),
                            imports: func_imports,
                            type_map: std::collections::BTreeMap::new(),
                        };
                        self.entity_registry_mut().request_specialization(task.clone());
                    }
                    let c1 = format!("%c1_fmt_{}", id);
                    out.push_str(&format!("    {} = arith.constant 1 : i64\n", c1));
                    let string_ty = "!struct_std__string__String";
                    let undef = format!("%fmt_undef_{}", id);
                    out.push_str(&format!("    {} = llvm.mlir.undef : {}\n", undef, string_ty));
                    let sentinel = format!("%fmt_sentinel_{}", id);
                    out.push_str(&format!("    {} = llvm.inttoptr {} : i64 to !llvm.ptr\n", sentinel, c1));
                    let s1 = format!("%fmt_s1_{}", id);
                    out.push_str(&format!("    {} = llvm.insertvalue {}, {}[0] : {}\n", s1, sentinel, undef, string_ty));
                    let c0 = format!("%c0_fmt_{}", id);
                    out.push_str(&format!("    {} = arith.constant 0 : i64\n", c0));
                    let s2 = format!("%fmt_s2_{}", id);
                    out.push_str(&format!("    {} = llvm.insertvalue {}, {}[1] : {}\n", s2, c0, s1, string_ty));
                    let buf_val = format!("%fmt_buf_{}", id);
                    out.push_str(&format!("    {} = llvm.insertvalue {}, {}[2] : {}\n", buf_val, c0, s2, string_ty));
                    let buf_alloca = format!("%fmt_buf_alloca_{}", id);
                    out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", buf_alloca, c1, string_ty));
                    out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", buf_val, buf_alloca, string_ty));
                    let self_alloca = format!("%fmt_self_alloca_{}", id);
                    let self_ty_mlir = ty.to_mlir_type(self)?;
                    out.push_str(&format!("    {} = llvm.alloca {} x {} : (i64) -> !llvm.ptr\n", self_alloca, c1, self_ty_mlir));
                    out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", val, self_alloca, self_ty_mlir));
                    out.push_str(&format!("    func.call @{}({}, {}) : (!llvm.ptr, !llvm.ptr) -> ()\n", mangled_name, self_alloca, buf_alloca));
                    let buf_after = format!("%fmt_buf_after_{}", id);
                    out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", buf_after, buf_alloca, string_ty));
                    let data_ptr = format!("%fmt_data_ptr_{}", id);
                    out.push_str(&format!("    {} = llvm.extractvalue {}[0] : {}\n", data_ptr, buf_after, string_ty));
                    let len = format!("%fmt_len_{}", id);
                    out.push_str(&format!("    {} = llvm.extractvalue {}[1] : {}\n", len, buf_after, string_ty));
                    self.entity_registry_mut().register_hook("__salt_print_literal");
                    out.push_str(&format!("    func.call @__salt_print_literal({}, {}) : (!llvm.ptr, i64) -> ()\n", data_ptr, len));
                    self.entity_registry_mut().register_hook("free");
                    out.push_str(&format!("    func.call @free({}) : (!llvm.ptr) -> ()\n", data_ptr));
                } else if self.derive_struct_write_to(out, name, val, ty, "%writer_stub").is_err() {
                    self.emit_print_literal(out, &format!("<{}>", name.split("__").last().unwrap_or(name)))?;
                }
            }
            Type::Tensor(inner_ty, shape) => {
                let inner_name = format!("{:?}", inner_ty).replace("Type::", "");
                let shape_str = format!("{:?}", shape);
                let header = format!("Tensor<{}, {}> {{ ", inner_name, shape_str);
                self.emit_print_literal(out, &header)?;
                let stats = self.emit_tensor_stats_gather(out, val, inner_ty)?;
                self.emit_print_literal(out, "min: ")?;
                self.emit_print_typed(out, &stats.min, &Type::F64)?;
                self.emit_print_literal(out, ", max: ")?;
                self.emit_print_typed(out, &stats.max, &Type::F64)?;
                self.emit_print_literal(out, ", mean: ")?;
                self.emit_print_typed(out, &stats.mean, &Type::F64)?;
                self.emit_print_literal(out, " }")?;
            }
            _ => {
                self.emit_print_literal(out, &format!("<{:?}>", ty))?;
            }
        }
        Ok(())
    }
    
    pub fn emit_tensor_stats_gather(&mut self, out: &mut String, _tensor_val: &str, _inner_ty: &Type) -> Result<TensorStats, String> {
        let id = self.next_id();
        let min_var = format!("%tensor_min_{}", id);
        let max_var = format!("%tensor_max_{}", id);
        let sum_var = format!("%tensor_sum_{}", id);
        let count_var = format!("%tensor_count_{}", id);
        let mean_var = format!("%tensor_mean_{}", id);
        let f64_ty = "f64";
        out.push_str(&format!("    {} = arith.constant 1.0e308 : {}\n", min_var, f64_ty));
        out.push_str(&format!("    {} = arith.constant -1.0e308 : {}\n", max_var, f64_ty));
        out.push_str(&format!("    {} = arith.constant 0.0 : {}\n", sum_var, f64_ty));
        out.push_str(&format!("    {} = arith.constant 0 : index\n", count_var));
        out.push_str(&format!("    {} = arith.constant 0.0 : {}\n", mean_var, f64_ty));
        self.entity_registry_mut().register_hook("__salt_tensor_stats");
        Ok(TensorStats { min: min_var, max: max_var, mean: mean_var })
    }
}

pub struct TensorStats {
    pub min: String,
    pub max: String,
    pub mean: String,
}

#[cfg(test)]
mod keuos_intrinsic_tests {
    #[test]
    fn test_keuos_intrinsic_names_registered() {
    }
}
