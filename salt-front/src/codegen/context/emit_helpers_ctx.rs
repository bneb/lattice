use crate::types::Type;
use crate::codegen::phases::TensorLayout;

impl<'a> crate::codegen::context::CodegenContext<'a> {
    // MLIR Builder Pattern Helpers
    // =========================================================================

    pub fn emit_binop(&self, out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
        out.push_str(&format!("    {} = {} {}, {} : {}\n", res, op, lhs, rhs, ty));
    }
    
    /// Emit binary operation with fast-math attributes for vectorization.
    /// Only use for floating-point operations in reduction loops where reassociation is acceptable.
    /// Attributes: reassoc (allow reordering), contract (allow FMA contraction)
    pub fn emit_binop_fast(&self, out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
        out.push_str(&format!("    {} = {} {}, {} {{fastmath = #arith.fastmath<reassoc, contract>}} : {}\n", 
            res, op, lhs, rhs, ty));
    }

    pub fn emit_const_int(&self, out: &mut String, res: &str, val: i64, ty: &str) {
        out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val, ty));
    }

    pub fn emit_const_float(&self, out: &mut String, res: &str, val: f64, ty: &str) {
        let val_str = if val == 0.0 {
            "0.0".to_string()
        } else {
            format!("{:.17e}", val)
        };
        out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val_str, ty));
    }

    pub fn emit_load(&self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, ptr, ty));
    }

    pub fn emit_load_scoped(&self, out: &mut String, res: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
        if !self.emit_alias_scopes {
            // Fall back to plain load when alias scopes are disabled
            self.emit_load(out, res, ptr, ty);
            return;
        }
        // Use MLIR attribute syntax: { alias_scopes = [...], noalias = [...] }
        out.push_str(&format!("    {} = llvm.load {} {{ alias_scopes = [{}], noalias = [{}] }} : !llvm.ptr -> {}\n", res, ptr, scope, noalias, ty));
    }

    pub fn emit_load_logical(&self, out: &mut String, res: &str, ptr: &str, ty: &Type) -> Result<(), String> {
        self.emit_load_logical_with_scope(out, res, ptr, ty, None)
    }

    pub fn emit_load_logical_with_scope(&self, out: &mut String, res: &str, ptr: &str, ty: &Type, scopes: Option<(&str, &str)>) -> Result<(), String> {
        let storage_ty = self.resolve_mlir_storage_type(ty)?;
        
        if *ty == Type::Bool {
            let load_res = format!("%b_load_{}", self.next_id());
            if let Some((s, n)) = scopes {
                self.emit_load_scoped(out, &load_res, ptr, &storage_ty, s, n);
            } else {
                out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", load_res, ptr, storage_ty));
            }
            self.emit_trunc(out, res, &load_res, "i8", "i1");
        } else if ty.k_is_ptr_type() {
             // Load pointers directly as !llvm.ptr
             // Previously used i64 storage + inttoptr which broke LLVM pointer provenance
             if let Some((s, n)) = scopes {
                 self.emit_load_scoped(out, res, ptr, "!llvm.ptr", s, n);
             } else {
                 out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !llvm.ptr\n", res, ptr));
             }
        } else if let Some((s, n)) = scopes {
            self.emit_load_scoped(out, res, ptr, &storage_ty, s, n);
        } else {
            out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, ptr, storage_ty));
        }
        Ok(())
    }

    pub fn emit_store(&self, out: &mut String, val: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", val, ptr, ty));
    }

    pub fn emit_store_scoped(&self, out: &mut String, val: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
        if !self.emit_alias_scopes {
            // Fall back to plain store when alias scopes are disabled
            self.emit_store(out, val, ptr, ty);
            return;
        }
        out.push_str(&format!("    llvm.store {}, {} {{ alias_scopes = [{}], noalias = [{}] }} : {}, !llvm.ptr\n", val, ptr, scope, noalias, ty));
    }

    pub fn emit_store_logical(&self, out: &mut String, val: &str, ptr: &str, ty: &Type) -> Result<(), String> {
        self.emit_store_logical_with_scope(out, val, ptr, ty, None)
    }

    pub fn emit_store_logical_with_scope(&self, out: &mut String, val: &str, ptr: &str, ty: &Type, scopes: Option<(&str, &str)>) -> Result<(), String> {
        let storage_ty = self.resolve_mlir_storage_type(ty)?;
        if *ty == Type::Bool {
            let zext_res = format!("%b_zext_{}", self.next_id());
            // Boolean Law: i1 -> i8 via arith.extui
            out.push_str(&format!("    {} = arith.extui {} : i1 to i8\n", zext_res, val));
            if let Some((s, n)) = scopes {
                self.emit_store_scoped(out, &zext_res, ptr, &storage_ty, s, n);
            } else {
                out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", zext_res, ptr, storage_ty));
            }
        } else if ty.k_is_ptr_type() {
             // Store pointers directly as !llvm.ptr
             // Previously used ptrtoint + i64 storage which broke LLVM pointer provenance
             if let Some((s, n)) = scopes {
                 self.emit_store_scoped(out, val, ptr, "!llvm.ptr", s, n);
             } else {
                 out.push_str(&format!("    llvm.store {} , {} : !llvm.ptr, !llvm.ptr\n", val, ptr));
             }
        } else if let Some((s, n)) = scopes {
            self.emit_store_scoped(out, val, ptr, &storage_ty, s, n);
        } else {
            out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", val, ptr, storage_ty));
        }
        Ok(())
    }

    pub fn emit_alloca(&self, _out: &mut String, res: &str, ty: &str) {
        self.alloca_out_mut().push_str(&format!("    {} = llvm.alloca %c1_i64 x {} : (i64) -> !llvm.ptr\n", res, ty));
    }

    pub fn emit_gep_field(&self, out: &mut String, res: &str, base: &str, idx: usize, struct_ty: &str) {
        out.push_str(&format!("    {} = llvm.getelementptr {}[0, {}] : (!llvm.ptr) -> !llvm.ptr, {}\n", res, base, idx, struct_ty));
    }

    pub fn emit_gep(&self, out: &mut String, res: &str, base: &str, idx_var: &str, elem_ty: &str) {
        out.push_str(&format!("    {} = llvm.getelementptr {}[{}] : (!llvm.ptr, i64) -> !llvm.ptr, {}\n", res, base, idx_var, elem_ty));
    }

    pub fn emit_extractvalue(&self, out: &mut String, res: &str, val: &str, idx: usize, ty: &str) {
        out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", res, val, idx, ty));
    }

    pub fn emit_extractvalue_logical(&self, out: &mut String, res: &str, val: &str, idx: usize, ty: &str, field_ty: &Type) -> Result<(), String> {
        if *field_ty == Type::Bool {
            let extract_res = format!("%b_extract_{}", self.next_id());
            // Extract as i8 (storage type)
            out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", extract_res, val, idx, ty));
            // Truncate to i1 (logical type)
            self.emit_trunc(out, res, &extract_res, "i8", "i1");
        } else {
            self.emit_extractvalue(out, res, val, idx, ty);
        }
        Ok(())
    }

    pub fn emit_insertvalue(&self, out: &mut String, res: &str, elem: &str, val: &str, idx: usize, ty: &str) {
        out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, elem, val, idx, ty));
    }
    #[allow(clippy::too_many_arguments)] // REASON: all 8 params independently meaningful for MLIR insertvalue emission
    pub fn emit_insertvalue_logical(&self, out: &mut String, res: &str, elem: &str, val: &str, idx: usize, ty: &str, field_ty: &Type) -> Result<(), String> {
        if *field_ty == Type::Bool {
             let zext_res = format!("%b_zext_ins_{}", self.next_id());
             // Promote i1 to i8
             self.emit_cast(out, &zext_res, "arith.extui", elem, "i1", "i8");
             // Insert the i8 into the struct
             out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, zext_res, val, idx, ty));
        } else {
             self.emit_insertvalue(out, res, elem, val, idx, ty);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)] // REASON: all 7 params independently meaningful for MLIR cmp emission
    pub fn emit_cmp(&self, out: &mut String, res: &str, cmp_op: &str, pred: &str, lhs: &str, rhs: &str, ty: &str) {
        let comma = if cmp_op == "llvm.icmp" || cmp_op == "llvm.fcmp" { "" } else { "," };
        out.push_str(&format!("    {} = {} \"{}\"{} {}, {} : {}\n", res, cmp_op, pred, comma, lhs, rhs, ty));
    }

    pub fn emit_cast(&self, out: &mut String, res: &str, op: &str, val: &str, from_ty: &str, to_ty: &str) {
        out.push_str(&format!("    {} = {} {} : {} to {}\n", res, op, val, from_ty, to_ty));
    }

    pub fn emit_trunc(&self, out: &mut String, res: &str, val: &str, from_ty: &str, to_ty: &str) {
        out.push_str(&format!("    {} = arith.trunci {} : {} to {}\n", res, val, from_ty, to_ty));
    }

    pub fn emit_br(&self, out: &mut String, label: &str) {
        out.push_str(&format!("    llvm.br ^{}\n", label));
    }

    pub fn emit_cond_br(&self, out: &mut String, cond: &str, true_label: &str, false_label: &str) {
        out.push_str(&format!("    llvm.cond_br {}, ^{}, ^{}\n", cond, true_label, false_label));
    }

    pub fn emit_label(&self, out: &mut String, label: &str) {
        out.push_str(&format!("  ^{}:\n", label));
    }

    pub fn emit_return(&self, out: &mut String, val: &str, ty: &str) {
        out.push_str(&format!("    llvm.return {} : {}\n", val, ty));
    }

    pub fn emit_return_void(&self, out: &mut String) {
        out.push_str("    llvm.return\n");
    }

    pub fn emit_load_exclusive(&self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    {} = \"llvm.load\"({}) {{salt.access = \"exclusive\"}} : (!llvm.ptr) -> {}\n", res, ptr, ty));
    }

    pub fn emit_load_atomic(&self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        // Atomic load via atomicrmw or ptr, 0 — identity operation that returns current value.
        // This avoids MLIR version incompatibility with atomic_memory_order attributes on llvm.load.
        let zero = format!("%atomic_zero_{}", self.next_id());
        out.push_str(&format!("    {} = arith.constant 0 : {}\n", zero, ty));
        out.push_str(&format!("    {} = llvm.atomicrmw _or {}, {} seq_cst : !llvm.ptr, {}\n", res, ptr, zero, ty));
    }

    pub fn emit_store_atomic(&self, out: &mut String, val: &str, ptr: &str, ty: &str) {
        // Atomic store via atomicrmw xchg ptr, value — discards old value.
        // This avoids MLIR version incompatibility with atomic_memory_order attributes on llvm.store.
        let discard = format!("%atomic_discard_{}", self.next_id());
        out.push_str(&format!("    {} = llvm.atomicrmw xchg {}, {} seq_cst : !llvm.ptr, {}\n", discard, ptr, val, ty));
    }

    pub fn emit_atomicrmw(&self, out: &mut String, res: &str, op: &str, ptr: &str, val: &str, ty: &str) {
        out.push_str(&format!("    {} = llvm.atomicrmw {} {}, {} seq_cst : !llvm.ptr, {}\n", res, op, ptr, val, ty));
    }

    pub fn emit_call(&self, out: &mut String, res: Option<&str>, func_name: &str, args: &str, arg_tys: &str, ret_ty: &str) {
        let mangled_func_name = self.mangle_fn_name(func_name);
        if let Some(r) = res {
            out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", r, mangled_func_name, args, arg_tys, ret_ty));
        } else {
            out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled_func_name, args, arg_tys));
        }
    }

    pub fn emit_addressof(&self, out: &mut String, res: &str, name: &str) -> Result<(), String> {
        // Check if the symbol is a function (Local, External, or from Registry)
        let is_func = self.defined_functions().contains(name) 
            || self.external_decls().contains(name)
            || matches!(self.resolve_global(name), Some(Type::Fn(_, _)));

        if is_func {
            // Retrieve type to construct signature
            let ty = self.resolve_global(name).unwrap_or(Type::Unit);
            if let Type::Fn(args, ret) = ty {
                 let mut arg_code = Vec::new();
                 for t in args {
                     arg_code.push(self.resolve_mlir_type(&t)?);
                 }
                 let arg_str = arg_code.join(", ");
                 let ret_str = if let Type::Unit = *ret { "()".to_string() } else { self.resolve_mlir_type(&ret)? };
                 let signature = format!("({}) -> {}", arg_str, ret_str);
                 
                 let tmp = format!("{}__fn", res);
                 out.push_str(&format!("    {} = func.constant @{} : {}\n", tmp, name, signature));
                 out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : {} to !llvm.ptr\n", res, tmp, signature));
            } else {
                 // Function is declared but type not in globals (e.g., assembly-only extern fn).
                 // Use func.constant with () -> () signature (safe for addressof + ptrtoint).
                 let tmp = format!("{}__fn", res);
                 out.push_str(&format!("    {} = func.constant @{} : () -> ()\n", tmp, name));
                 out.push_str(&format!("    {} = builtin.unrealized_conversion_cast {} : () -> () to !llvm.ptr\n", res, tmp));
            }
        } else {
             // For global variables, use llvm.mlir.addressof
             out.push_str(&format!("    {} = llvm.mlir.addressof @{} : !llvm.ptr\n", res, name));
        }
        Ok(())
    }

    pub fn emit_inttoptr(&self, out: &mut String, res: &str, val: &str, from_ty: &str) {
        out.push_str(&format!("    {} = llvm.inttoptr {} : {} to !llvm.ptr\n", res, val, from_ty));
    }

    pub fn emit_verify(&self, out: &mut String, cond: &str, _msg: &str) {
        // Lower to standard MLIR: scf.if with inverted condition + panic
        let true_const = format!("%verify_true_{}", self.next_id());
        let violated = format!("%verify_violated_{}", self.next_id());
        out.push_str(&format!("    {} = arith.constant true\n", true_const));
        out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", violated, cond, true_const));
        out.push_str(&format!("    scf.if {} {{\n", violated));
        out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
        out.push_str("      scf.yield\n");
        out.push_str("    }\n");
    }
    pub fn ensure_struct_exists(&self, base_name: &str, params: &[Type]) -> Result<String, String> {
        if base_name == "GlobalSlabAlloc" {
             panic!("Short Name Leak detected in ensure_struct_exists!");
        }
        let full_params = params.to_vec();
        let key = (base_name.to_string(), full_params.clone());
        if let Some(mangled) = self.specializations().get(&key) {
            let m_res: String = mangled.clone();
            return Ok(m_res);
        }

        // Delegate to type_bridge specialized logic which handles template instantiation
        Ok(self.specialize_template(base_name, params, false)?.mangle())
    }

    pub fn ensure_enum_exists(&self, base_name: &str, params: &[Type]) -> Result<String, String> {
        let full_params = params.to_vec();
        let key = (base_name.to_string(), full_params.clone());
        if let Some(mangled) = self.specializations().get(&key) {
            let m_res: String = mangled.clone();
            return Ok(m_res);
        }

        Ok(self.specialize_template(base_name, params, true)?.mangle())
    }

    pub fn get_tensor_layout(&self, ty: &Type) -> Result<TensorLayout, String> {
        if let Some(layout) = self.tensor_layout_cache().get(ty) {
            return Ok(layout.clone());
        }
        if let Type::Tensor(_, shape) = ty {
            let mut strides = vec![1; shape.len()];
            for i in (0..shape.len() - 1).rev() {
                strides[i] = strides[i+1] * shape[i+1];
            }
            let layout = TensorLayout {
                shape: shape.clone(),
                strides,
                is_row_major: true,
            };
            self.tensor_layout_cache_mut().insert(ty.clone(), layout.clone());
            Ok(layout)
        } else {
            Err(format!("Type {:?} is not a tensor", ty))
        }
    }

    pub fn emit_linalg_generic(
        &self,
        out: &mut String,
        inputs: Vec<String>,
        outputs: Vec<String>,
        indexing_maps: Vec<String>,
        iterator_types: Vec<String>,
    ) -> Result<String, String> {
        let res = format!("%linalg_res_{}", self.next_id());
        let ins = inputs.join(", ");
        let outs = outputs.join(", ");
        let maps = indexing_maps.iter().map(|s| format!("affine_map<{}>", s)).collect::<Vec<_>>().join(", ");
        let iter_tys = iterator_types.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
        
        out.push_str(&format!("    {} = linalg.generic {{indexing_maps = [{}], iterator_types = [{}]}} ins({}) outs({}) \n", 
            res, maps, iter_tys, ins, outs));
        Ok(res)
    }
    #[allow(clippy::too_many_arguments)] // REASON: all 7 params independently meaningful for MLIR matmul emission
    pub fn emit_linalg_matmul(
        &self,
        out: &mut String,
        lhs: &str,
        _lhs_ty: &str,
        rhs: &str,
        _rhs_ty: &str,
        acc: &str,
        acc_ty: &str,
    ) -> Result<String, String> {
        let res = format!("%matmul_res_{}", self.next_id());
        out.push_str(&format!("    {} = linalg.matmul ins({}, {} : {}, {}) outs({} : {}) -> {}\n", 
            res, lhs, rhs, _lhs_ty, _rhs_ty, acc, acc_ty, acc_ty));
        Ok(res)
    }

    pub fn emit_noalias_metadata(&self, _out: &mut String, region_name: &str) -> (String, String) {
        let id = self.next_metadata_id();
        let scope_domain = format!("@alias_domain_{}", id);
        let scope_id = format!("@alias_scope_{}_{}", region_name, id);
        
        // This is a simplification. Real MLIR would need these in the metadata section.
        // For now, store them in CodegenContext to be emitted later if needed,
        // or emit them as LLVM IR metadata if targeting LLVM directly.
        // In MLIR, these are often dialect attributes.
        (scope_id, scope_domain)
    }
}
