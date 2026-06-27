use crate::types::Type;
use crate::codegen::context::LoweringContext;

impl<'a, 'ctx> LoweringContext<'a, 'ctx> {
    // MLIR Builder Pattern Helpers (zero RefCell)
    // =========================================================================

    pub fn emit_binop(&self, out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
        out.push_str(&format!("    {} = {} {}, {} : {}\n", res, op, lhs, rhs, ty));
    }
    pub fn emit_binop_fast(&self, out: &mut String, res: &str, op: &str, lhs: &str, rhs: &str, ty: &str) {
        out.push_str(&format!("    {} = {} {}, {} {{fastmath = #arith.fastmath<reassoc, contract>}} : {}\n", res, op, lhs, rhs, ty));
    }
    pub fn emit_const_int(&self, out: &mut String, res: &str, val: i64, ty: &str) {
        out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val, ty));
    }
    pub fn emit_const_float(&self, out: &mut String, res: &str, val: f64, ty: &str) {
        let val_str = if val == 0.0 { "0.0".to_string() } else { format!("{:.17e}", val) };
        out.push_str(&format!("    {} = arith.constant {} : {}\n", res, val_str, ty));
    }
    pub fn emit_load(&self, out: &mut String, val: &str, ptr: &str, ty: &str) {
        let scope_attr = "";
        // Disable alias_scopes to prevent LLVM from deleting loop conditions
        // if ptr.contains("local") || ptr.contains("spill") {
        //      scope_attr = " { alias_scopes = [#scope_local], noalias = [#scope_global] }";
        // } else if ptr.contains("global") {
        //      scope_attr = " { alias_scopes = [#scope_global], noalias = [#scope_local] }";
        // }
        out.push_str(&format!("    {} = llvm.load {}{} : !llvm.ptr -> {}\n", val, ptr, scope_attr, ty));
    }
    pub fn emit_load_scoped(&self, out: &mut String, res: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
        if !self.config.emit_alias_scopes { self.emit_load(out, res, ptr, ty); return; }
        out.push_str(&format!("    {} = llvm.load {} {{ alias_scopes = [{}], noalias = [{}] }} : !llvm.ptr -> {}\n", res, ptr, scope, noalias, ty));
    }
    pub fn emit_load_logical_with_scope(&mut self, out: &mut String, res: &str, ptr: &str, ty: &Type, scopes: Option<(&str, &str)>) -> Result<(), String> {
        let storage_ty = ty.to_mlir_storage_type_simple();
        if *ty == Type::Bool {
            let load_res = format!("%b_load_{}", self.next_id());
            if let Some((s, n)) = scopes { self.emit_load_scoped(out, &load_res, ptr, &storage_ty, s, n); }
            else { out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", load_res, ptr, storage_ty)); }
            self.emit_trunc(out, res, &load_res, "i8", "i1");
        } else if ty.k_is_ptr_type() {
            if let Some((s, n)) = scopes { self.emit_load_scoped(out, res, ptr, "!llvm.ptr", s, n); }
            else { out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> !llvm.ptr\n", res, ptr)); }
        } else if let Some((s, n)) = scopes { self.emit_load_scoped(out, res, ptr, &storage_ty, s, n); }
        else { out.push_str(&format!("    {} = llvm.load {} : !llvm.ptr -> {}\n", res, ptr, storage_ty)); }
        Ok(())
    }
    pub fn emit_store(&self, out: &mut String, val: &str, ptr: &str, ty: &str) {
        out.push_str(&format!("    llvm.store {}, {} : {}, !llvm.ptr\n", val, ptr, ty));
    }
    pub fn emit_store_scoped(&self, out: &mut String, val: &str, ptr: &str, ty: &str, scope: &str, noalias: &str) {
        if !self.config.emit_alias_scopes { self.emit_store(out, val, ptr, ty); return; }
        out.push_str(&format!("    llvm.store {}, {} {{ alias_scopes = [{}], noalias = [{}] }} : {}, !llvm.ptr\n", val, ptr, scope, noalias, ty));
    }
    pub fn emit_store_logical_with_scope(&mut self, out: &mut String, val: &str, ptr: &str, ty: &Type, scopes: Option<(&str, &str)>) -> Result<(), String> {
        let storage_ty = ty.to_mlir_storage_type_simple();
        if *ty == Type::Bool {
            let zext_res = format!("%b_zext_{}", self.next_id());
            out.push_str(&format!("    {} = arith.extui {} : i1 to i8\n", zext_res, val));
            if let Some((s, n)) = scopes { self.emit_store_scoped(out, &zext_res, ptr, &storage_ty, s, n); }
            else { out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", zext_res, ptr, storage_ty)); }
        } else if ty.k_is_ptr_type() {
            if let Some((s, n)) = scopes { self.emit_store_scoped(out, val, ptr, "!llvm.ptr", s, n); }
            else { out.push_str(&format!("    llvm.store {} , {} : !llvm.ptr, !llvm.ptr\n", val, ptr)); }
        } else if let Some((s, n)) = scopes { self.emit_store_scoped(out, val, ptr, &storage_ty, s, n); }
        else { out.push_str(&format!("    llvm.store {} , {} : {}, !llvm.ptr\n", val, ptr, storage_ty)); }
        Ok(())
    }
    pub fn emit_alloca(&mut self, _out: &mut String, res: &str, ty: &str) {
        self.emission.alloca_out.push_str(&format!("    {} = llvm.alloca %c1_i64 x {} : (i64) -> !llvm.ptr\n", res, ty));
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
    pub fn emit_extractvalue_logical(&mut self, out: &mut String, res: &str, val: &str, idx: usize, ty: &str, field_ty: &Type) -> Result<(), String> {
        if *field_ty == Type::Bool {
            let extract_res = format!("%b_extract_{}", self.next_id());
            out.push_str(&format!("    {} = llvm.extractvalue {}[{}] : {}\n", extract_res, val, idx, ty));
            self.emit_trunc(out, res, &extract_res, "i8", "i1");
        } else { self.emit_extractvalue(out, res, val, idx, ty); }
        Ok(())
    }
    pub fn emit_insertvalue(&self, out: &mut String, res: &str, elem: &str, val: &str, idx: usize, ty: &str) {
        out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, elem, val, idx, ty));
    }
    #[allow(clippy::too_many_arguments)]
    // REASON: all 8 parameters are independently meaningful for MLIR insertvalue emission; bundling would obscure emitter semantics.
    pub fn emit_insertvalue_logical(&mut self, out: &mut String, res: &str, elem: &str, val: &str, idx: usize, ty: &str, field_ty: &Type) -> Result<(), String> {
        if *field_ty == Type::Bool {
            let zext_res = format!("%b_zext_ins_{}", self.next_id());
            self.emit_cast(out, &zext_res, "arith.extui", elem, "i1", "i8");
            out.push_str(&format!("    {} = llvm.insertvalue {}, {}[{}] : {}\n", res, zext_res, val, idx, ty));
        } else { self.emit_insertvalue(out, res, elem, val, idx, ty); }
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
    pub fn emit_load_atomic(&mut self, out: &mut String, res: &str, ptr: &str, ty: &str) {
        let zero = format!("%atomic_zero_{}", self.next_id());
        out.push_str(&format!("    {} = arith.constant 0 : {}\n", zero, ty));
        out.push_str(&format!("    {} = llvm.atomicrmw _or {}, {} seq_cst : !llvm.ptr, {}\n", res, ptr, zero, ty));
    }
    pub fn emit_store_atomic(&mut self, out: &mut String, val: &str, ptr: &str, ty: &str) {
        let discard = format!("%atomic_discard_{}", self.next_id());
        out.push_str(&format!("    {} = llvm.atomicrmw xchg {}, {} seq_cst : !llvm.ptr, {}\n", discard, ptr, val, ty));
    }
    pub fn emit_atomicrmw(&self, out: &mut String, res: &str, op: &str, ptr: &str, val: &str, ty: &str) {
        out.push_str(&format!("    {} = llvm.atomicrmw {} {}, {} seq_cst : !llvm.ptr, {}\n", res, op, ptr, val, ty));
    }
    pub fn emit_inttoptr(&self, out: &mut String, res: &str, val: &str, from_ty: &str) {
        out.push_str(&format!("    {} = llvm.inttoptr {} : {} to !llvm.ptr\n", res, val, from_ty));
    }
    pub fn emit_verify(&mut self, out: &mut String, cond: &str, _msg: &str) {
        let true_const = format!("%verify_true_{}", self.next_id());
        let violated = format!("%verify_violated_{}", self.next_id());
        out.push_str(&format!("    {} = arith.constant true\n", true_const));
        out.push_str(&format!("    {} = arith.xori {}, {} : i1\n", violated, cond, true_const));
        out.push_str(&format!("    scf.if {} {{\n", violated));
        out.push_str("      func.call @__salt_contract_violation() : () -> ()\n");
        out.push_str("      scf.yield\n");
        out.push_str("    }\n");
    }
    pub fn emit_noalias_metadata(&self, _out: &mut String, region_name: &str) -> (String, String) {
        let id = self.next_metadata_id();
        let scope_domain = format!("@alias_domain_{}", id);
        let scope_id = format!("@alias_scope_{}_{}", region_name, id);
        (scope_id, scope_domain)
    }
    pub fn emit_call(&self, out: &mut String, res: Option<&str>, func_name: &str, args: &str, arg_tys: &str, ret_ty: &str) {
        let mangled_func_name = self.mangle_fn_name(func_name);
        if let Some(r) = res {
            out.push_str(&format!("    {} = func.call @{}({}) : ({}) -> {}\n", r, mangled_func_name, args, arg_tys, ret_ty));
        } else {
            out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n", mangled_func_name, args, arg_tys));
        }
    }

}
