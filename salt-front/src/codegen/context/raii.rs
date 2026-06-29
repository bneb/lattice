use crate::types::Type;
use crate::codegen::context::CodegenContext;

impl<'a> CodegenContext<'a> {
    /// Enter a new lexical scope (e.g., function body, loop body)
    pub fn push_cleanup_scope(&self) {
        self.cleanup_stack_mut().push(Vec::new());
    }

    /// Register an owned resource for cleanup at scope exit
    /// Also registers with Z3StateTracker for formal verification
    pub fn register_owned_resource(&self, value: &str, drop_fn: &str, var_name: &str, ty: Type) {
        if let Some(scope) = self.cleanup_stack_mut().last_mut() {
            scope.push(crate::codegen::phases::CleanupTask {
                value: value.to_string(),
                drop_fn: drop_fn.to_string(),
                var_name: var_name.to_string(),
                ty,
            });
        }

        // Z3 Ownership Ledger: Register BIRTH event
        // Use var_name for better error messages (maps to source variable)
        self.ownership_tracker.borrow_mut().register_allocation(
            var_name,
            &self.z3_solver.borrow()
        );
    }

    /// Pop the current scope and emit cleanup calls for all remaining resources
    /// Also marks resources as Released in Z3StateTracker
    pub fn pop_and_emit_cleanup(&self, out: &mut String) -> Result<(), String> {
        if let Some(tasks) = self.cleanup_stack_mut().pop() {
            // Emit in reverse order (LIFO - last allocated, first freed)
            for task in tasks.into_iter().rev() {
                self.ownership_tracker.borrow_mut().mark_released(
                    &task.var_name,
                    &self.z3_solver.borrow()
                )?;

                // Emit the drop function call
                let mlir_ty = self.resolve_mlir_type(&task.ty)?;
                out.push_str(&format!("    func.call @{}({}) : ({}) -> ()\n",
                    task.drop_fn, task.value, mlir_ty));
            }
        }
        Ok(())
    }

    /// Transfer ownership of a resource (e.g., when returning it)
    /// Removes the resource from cleanup tracking so it won't be freed
    /// Also marks as Moved in Z3StateTracker
    pub fn transfer_ownership(&self, value: &str) -> Result<(), String> {
        let mut stack = self.cleanup_stack_mut();
        for scope in stack.iter_mut() {
            if let Some(pos) = scope.iter().position(|t: &crate::codegen::phases::CleanupTask| t.value == value) {
                let _task = scope.remove(pos);

                // Z3 Ownership Ledger: Register MOVE event
                self.ownership_tracker.borrow_mut().mark_moved(
                    value, // Tracks by value name in SSA
                    &self.z3_solver.borrow()
                )?;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Remove a resource from the cleanup stack by its SOURCE variable name.
    /// Called when the user explicitly calls .free() or .drop() on a variable,
    /// so the RAII system won't emit a duplicate cleanup call.
    pub fn release_by_var_name(&self, var_name: &str) {
        let mut stack = self.cleanup_stack_mut();
        for scope in stack.iter_mut() {
            if let Some(pos) = scope.iter().position(|t: &crate::codegen::phases::CleanupTask| t.var_name == var_name) {
                scope.remove(pos);
                return;
            }
        }
    }

    pub fn mk_int(&self, val: i64) -> crate::z3_shim::ast::Int<'a> {
        crate::z3_shim::ast::Int::from_i64(self.z3_ctx, val)
    }

    pub fn mk_var(&self, name: &str) -> crate::z3_shim::ast::Int<'a> {
        crate::z3_shim::ast::Int::new_const(self.z3_ctx, name)
    }

    pub fn push_solver(&self) {
        self.z3_solver.borrow().push();
    }

    pub fn pop_solver(&self) {
        self.z3_solver.borrow().pop(1);
    }

    pub fn add_assertion(&self, expr: &crate::z3_shim::ast::Bool<'a>) {
        self.z3_solver.borrow().assert(expr);
    }

    /// Check if a violation condition is provably unsatisfiable.
    ///
    /// Returns `true` if Z3 can prove the violation is impossible (UNSAT),
    /// meaning the code is provably safe. Returns `false` if Z3 finds a
    /// counterexample (SAT) or times out (Unknown).
    pub fn is_provably_safe(&self, violation: &crate::z3_shim::ast::Bool<'a>) -> bool {
        // Create a fresh solver for this check (isolated from main solver state)
        let solver = crate::z3_shim::Solver::new(self.z3_ctx);

        // Set timeout to 100ms to prevent hangs on complex expressions
        let mut params = crate::z3_shim::Params::new(self.z3_ctx);
        params.set_u32("timeout", 100);
        solver.set_params(&params);

        // Assert the violation and check if it's satisfiable
        solver.assert(violation);

        match solver.check() {
            crate::z3_shim::SatResult::Unsat => {
                // No counterexample exists - code is provably safe
                true
            }
            crate::z3_shim::SatResult::Sat => {
                // Counterexample found - violation is possible
                false
            }
            crate::z3_shim::SatResult::Unknown => {
                false
            }
        }
    }

    pub fn register_symbolic_int(&self, ssa_name: String, val: crate::z3_shim::ast::Int<'a>) {
        self.symbolic_tracker.borrow_mut().insert(ssa_name, val);
    }

    pub fn get_symbolic_int(&self, ssa_name: &str) -> Option<crate::z3_shim::ast::Int<'a>> {
        self.symbolic_tracker.borrow().get(ssa_name).cloned()
    }
}
