//! Coroutine-to-State-Machine Transformation (AsyncToState)
//!
//! Transforms `@yielding` functions into stackless state machines that can
//! be suspended and resumed by the KeuOS executor. Each yield point
//! becomes a state transition with variables captured in a heap-allocated
//! TaskFrame.
//!
//! ## Generated MLIR Structure
//! ```mlir
//! // TaskFrame struct: { resume_state: i32, captured_var_1: T1, ... }
//! // Dispatch hub: llvm.switch on resume_state
//! // State N: resume execution from yield point N
//! ```
//!
//! ## Key Invariants
//! - TaskFrame is allocated from the KeuOS Arena (O(1), pointer-bump)
//! - resume_state = 0 means "initial entry"
//! - resume_state = -1 means "completed"
//! - ZST variables (Context) are never stored in the frame

use super::liveness::{LivenessResult, FrameMember, YieldPointInfo};

/// Configuration for state machine emission
#[derive(Debug, Clone)]
pub struct StateMachineConfig {
    /// Function name being transformed
    pub fn_name: String,
    /// Arena allocation function name
    pub arena_alloc: String,
    /// Arena free function name (for completion)
    pub arena_free: String,
}

impl Default for StateMachineConfig {
    fn default() -> Self {
        Self {
            fn_name: "unknown".to_string(),
            arena_alloc: "keuos_arena_alloc".to_string(),
            arena_free: "keuos_arena_free".to_string(),
        }
    }
}

/// State machine emitter — transforms liveness results into MLIR
pub struct StateMachineEmitter {
    pub config: StateMachineConfig,
}

impl StateMachineEmitter {
    pub fn new(config: StateMachineConfig) -> Self {
        Self { config }
    }

    /// Generate the TaskFrame struct type definition
    pub fn generate_task_frame_struct(&self, liveness: &LivenessResult) -> String {
        let mut out = String::new();
        let frame_name = format!("TaskFrame_{}", self.config.fn_name);

        out.push_str(&format!(
            "    // TaskFrame for '{}' ({} captured vars, {} yield points)\n",
            self.config.fn_name,
            liveness.frame_members.len(),
            liveness.yield_points.len(),
        ));

        // Build the struct type
        // Element 0 is always resume_state: i32
        let mut field_types = vec!["i32".to_string()]; // resume_state
        for member in &liveness.frame_members {
            field_types.push(member.ty.clone());
        }

        let fields_str = field_types.join(", ");
        out.push_str(&format!(
            "    !{} = !llvm.struct<\"{}\" ({})> {{alignment = 64 : i64}}\n",
            frame_name, frame_name, fields_str,
        ));

        // Add field comments
        out.push_str("    // Field 0: resume_state (i32)\n");
        for member in &liveness.frame_members {
            out.push_str(&format!(
                "    // Field {}: {} ({})\n",
                member.index + 1,
                member.name,
                member.ty,
            ));
        }

        out
    }

    /// Generate the dispatch hub (entry point that switches on resume_state)
    pub fn generate_dispatch_hub(&self, liveness: &LivenessResult) -> String {
        let mut out = String::new();
        let frame_name = format!("TaskFrame_{}", self.config.fn_name);
        let num_states = liveness.yield_points.len() + 1; // +1 for initial state

        out.push_str(&format!(
            "    // Dispatch hub for '{}' ({})\n",
            self.config.fn_name, num_states,
        ));

        // Load resume_state from frame
        out.push_str(&format!(
            "    %frame_ptr = \"llvm.load\"(%task_frame) : (!llvm.ptr) -> !{}\n",
            frame_name,
        ));
        out.push_str(&format!(
            "    %resume_state = llvm.extractvalue %frame_ptr[0] : !{}\n",
            frame_name,
        ));

        // Generate the switch
        out.push_str("    llvm.switch %resume_state : i32, ^bb_invalid [\n");
        out.push_str("      0: ^bb_state_0,\n"); // Initial entry
        for yp in &liveness.yield_points {
            out.push_str(&format!(
                "      {}: ^bb_state_{},\n",
                yp.index + 1,
                yp.index + 1,
            ));
        }
        out.push_str("    ]\n");

        // Invalid state block
        out.push_str("  ^bb_invalid:\n");
        out.push_str("    \"salt.panic\"() {msg = \"Invalid resume state\"} : () -> ()\n");
        out.push_str("    llvm.unreachable\n");

        out
    }

    /// Generate suspension logic for a specific yield point
    /// Replaces `salt.yield` with: save state → store captured vars → return
    pub fn generate_suspension(&self, yield_point: &YieldPointInfo, frame_members: &[FrameMember]) -> String {
        let mut out = String::new();
        let frame_name = format!("TaskFrame_{}", self.config.fn_name);
        let next_state = yield_point.index + 1;

        out.push_str(&format!(
            "    // Suspension at {} (→ state {})\n",
            yield_point.label, next_state,
        ));

        // Store the next resume state
        out.push_str(&format!(
            "    %next_state_{idx} = arith.constant {state} : i32\n",
            idx = yield_point.index,
            state = next_state,
        ));
        out.push_str(&format!(
            "    %frame_with_state_{idx} = llvm.insertvalue %next_state_{idx}, %frame_ptr[0] : !{frame}\n",
            idx = yield_point.index,
            frame = frame_name,
        ));

        // Store each captured variable into the frame
        for member in frame_members {
            out.push_str(&format!(
                "    %frame_s{idx}_f{fidx} = llvm.insertvalue %{name}, %frame_with_state_{idx}[{field}] : !{frame}\n",
                idx = yield_point.index,
                fidx = member.index,
                name = member.name,
                field = member.index + 1, // +1 because field 0 is resume_state
                frame = frame_name,
            ));
        }

        // Store frame back and return (yield to executor)
        out.push_str(&format!(
            "    \"llvm.store\"(%frame_s{idx}_final, %task_frame) : (!{frame}, !llvm.ptr) -> ()\n",
            idx = yield_point.index,
            frame = frame_name,
        ));
        out.push_str("    llvm.return\n");

        out
    }

    /// Generate the launcher function that creates the initial TaskFrame
    pub fn generate_launcher(&self, liveness: &LivenessResult) -> String {
        let mut out = String::new();
        let frame_name = format!("TaskFrame_{}", self.config.fn_name);

        out.push_str(&format!(
            "    // Launcher for '{}'\n",
            self.config.fn_name,
        ));

        // Calculate frame size
        let field_count = 1 + liveness.frame_members.len(); // resume_state + captured vars
        out.push_str(&format!(
            "    // Frame: {} fields ({} bytes estimated)\n",
            field_count,
            field_count * 8, // Conservative: 8 bytes per field
        ));

        // Allocate from keuos arena
        let frame_size_bytes = field_count * 8;
        out.push_str(&format!(
            "    %frame_size = arith.constant {} : i64\n",
            frame_size_bytes,
        ));
        out.push_str(&format!(
            "    %task_frame = func.call @{}(%frame_size) : (i64) -> !llvm.ptr\n",
            self.config.arena_alloc,
        ));

        // Initialize resume_state to 0
        out.push_str("    %zero_state = arith.constant 0 : i32\n");
        out.push_str(&format!(
            "    %init_frame = llvm.insertvalue %zero_state, %task_frame[0] : !{}\n",
            frame_name,
        ));
        out.push_str(&format!(
            "    \"llvm.store\"(%init_frame, %task_frame) : (!{}, !llvm.ptr) -> ()\n",
            frame_name,
        ));

        // Return the task frame pointer (executor will call the dispatch hub)
        out.push_str("    llvm.return %task_frame : !llvm.ptr\n");

        out
    }

    /// Generate completion block (resume_state = -1, free frame)
    pub fn generate_completion(&self) -> String {
        let frame_name = format!("TaskFrame_{}", self.config.fn_name);
        let mut out = String::new();

        out.push_str(&format!(
            "    // Completion for '{}'\n",
            self.config.fn_name,
        ));

        // Set resume_state to -1 (completed sentinel)
        out.push_str("    %done_state = arith.constant -1 : i32\n");
        out.push_str(&format!(
            "    %done_frame = llvm.insertvalue %done_state, %frame_ptr[0] : !{}\n",
            frame_name,
        ));
        out.push_str(&format!(
            "    \"llvm.store\"(%done_frame, %task_frame) : (!{}, !llvm.ptr) -> ()\n",
            frame_name,
        ));

        // Free the frame (returns to arena)
        out.push_str(&format!(
            "    func.call @{}(%task_frame) : (!llvm.ptr) -> ()\n",
            self.config.arena_free,
        ));
        out.push_str("    llvm.return\n");

        out
    }

    // =========================================================================
    // Jump Table Dispatch (KeuOS)
    // =========================================================================
    //
    // O(1) dispatch via a global array of function pointers, replacing the
    // linear switch chain. On M4 this lowers to:
    //   ldr x3, [x2, x0, lsl #3]   ;; jump_table[resume_state]
    //   br  x3                       ;; indirect branch

    /// Generate a global jump table: array of function pointers, one per state
    pub fn generate_jump_table(&self, liveness: &LivenessResult) -> String {
        let mut out = String::new();
        let table_name = format!("L_dispatch_table_{}", self.config.fn_name);
        let num_states = liveness.yield_points.len() + 1; // +1 for initial state

        out.push_str(&format!(
            "    // Jump table for '{}' ({} states, O(1) dispatch)\n",
            self.config.fn_name, num_states,
        ));

        // Emit global: llvm.mlir.global internal constant
        out.push_str(&format!(
            "    llvm.mlir.global internal constant @{}() : !llvm.array<{} x !llvm.ptr> {{\n",
            table_name, num_states,
        ));

        // Build the array of function pointers
        let mut fn_names = Vec::new();
        fn_names.push(format!("@{}_state_0", self.config.fn_name));
        for yp in &liveness.yield_points {
            fn_names.push(format!("@{}_state_{}", self.config.fn_name, yp.index + 1));
        }

        let fn_refs: Vec<String> = fn_names.iter()
            .map(|f| format!("!llvm.ptr {}", f))
            .collect();

        out.push_str(&format!(
            "      %0 = llvm.mlir.constant dense<[{}]> : !llvm.array<{} x !llvm.ptr>\n",
            fn_refs.join(", "), num_states,
        ));
        out.push_str("      llvm.return %0\n");
        out.push_str("    }\n");

        out
    }

    /// Generate O(1) indirect dispatch using GEP into the jump table
    pub fn generate_indirect_dispatch(&self, liveness: &LivenessResult) -> String {
        let mut out = String::new();
        let table_name = format!("L_dispatch_table_{}", self.config.fn_name);
        let frame_name = format!("TaskFrame_{}", self.config.fn_name);
        let num_states = liveness.yield_points.len() + 1;

        out.push_str(&format!(
            "    // Indirect dispatch for '{}' (GEP + br)\n",
            self.config.fn_name,
        ));

        // Load resume_state
        out.push_str(&format!(
            "    %frame_ptr = \"llvm.load\"(%task_frame) : (!llvm.ptr) -> !{}\n",
            frame_name,
        ));
        out.push_str(&format!(
            "    %resume_state = llvm.extractvalue %frame_ptr[0] : !{}\n",
            frame_name,
        ));

        // GEP into the jump table
        out.push_str(&format!(
            "    %table_base = llvm.mlir.addressof @{} : !llvm.ptr\n",
            table_name,
        ));
        out.push_str("    %state_ext = arith.extsi %resume_state : i32 to i64\n");
        out.push_str(&format!(
            "    %fn_ptr_addr = llvm.getelementptr %table_base[0, %state_ext] : (!llvm.ptr, i64) -> !llvm.ptr, !llvm.array<{} x !llvm.ptr>\n",
            num_states,
        ));
        out.push_str(
            "    %fn_ptr = \"llvm.load\"(%fn_ptr_addr) : (!llvm.ptr) -> !llvm.ptr\n",
        );

        // MustTail indirect call — zero stack growth, pure jump
        out.push_str(
            "    llvm.call tail %fn_ptr(%task_frame) {tail_call_kind = #llvm.tailcall<musttail>} : (!llvm.ptr) -> ()\n",
        );

        out
    }

    /// Generate per-state entry point functions (placeholder bodies for testing)
    pub fn generate_state_functions(&self, liveness: &LivenessResult) -> Vec<String> {
        let num_states = liveness.yield_points.len() + 1;
        let placeholders: Vec<String> = (0..num_states)
            .map(|_| "      // ... state body ...\n".to_string())
            .collect();
        self.generate_state_functions_with_bodies(liveness, &placeholders)
    }

    /// Generate per-state entry point functions with real body MLIR.
    /// `state_bodies[i]` is the pre-generated MLIR for state i's body segment.
    /// The emitter wraps each body with reload (for resume states) and spill
    /// (for non-final states), producing complete state functions.
    pub fn generate_state_functions_with_bodies(
        &self,
        liveness: &LivenessResult,
        state_bodies: &[String],
    ) -> Vec<String> {
        let mut functions = Vec::new();
        let num_states = liveness.yield_points.len() + 1;

        for state_idx in 0..num_states {
            let mut out = String::new();
            let fn_name = format!("{}_state_{}", self.config.fn_name, state_idx);

            out.push_str(&format!(
                "    // State {} entry point\n",
                state_idx,
            ));
            out.push_str(&format!(
                "    func.func @{}(%task_frame: !llvm.ptr) attributes {{passthrough = [\"noinline\"]}} {{\n",
                fn_name,
            ));

            if state_idx == 0 {
                out.push_str("      // Initial entry: execute function body\n");
            } else {
                let yp_idx = state_idx - 1;
                if let Some(yp) = liveness.yield_points.get(yp_idx) {
                    out.push_str(&format!(
                        "      // Resume from '{}' (yield point {})\n",
                        yp.label, yp.index,
                    ));
                }
            }

            // Reload live-in variables for resume states
            if state_idx > 0 {
                out.push_str(&self.generate_reload(&liveness.frame_members, state_idx));
            }

            // Emit the state body (real codegen or placeholder)
            if let Some(body) = state_bodies.get(state_idx) {
                out.push_str(body);
            } else {
                out.push_str("      // ... state body ...\n");
            }

            // If this isn't the last state, emit spill + state transition
            if state_idx < num_states - 1 {
                out.push_str(&self.generate_spill(&liveness.frame_members, state_idx));
            }

            out.push_str("      llvm.return\n");
            out.push_str("    }\n");

            functions.push(out);
        }

        functions
    }

    // =========================================================================
    // Spill/Reload (KeuOS — Codegen Wiring)
    // =========================================================================
    //
    // Spill: store live variables from SSA registers into the TaskFrame via GEP.
    // Reload: load live variables from the TaskFrame back into SSA values.
    // ZST variables (Context, Unit) are materialized as `undef` instead of loaded.

    /// Generate spill logic: GEP + store for each live variable at a yield point.
    /// `state_idx` is the current state; the transition targets state_idx + 1.
    pub fn generate_spill(&self, frame_members: &[FrameMember], state_idx: usize) -> String {
        let mut out = String::new();
        let next_state = state_idx + 1;

        out.push_str(&format!(
            "      // Spill: save live vars before yield (state {} → {})\n",
            state_idx, next_state,
        ));

        // Store resume_state = next_state at field offset 0
        out.push_str(&format!(
            "      %next_state_{idx} = arith.constant {next} : i32\n",
            idx = state_idx, next = next_state,
        ));
        out.push_str(&format!(
            "      %state_ptr_{idx} = llvm.getelementptr %task_frame[0, 0] : (!llvm.ptr) -> !llvm.ptr, !TaskFrame_{fn_name}\n",
            idx = state_idx, fn_name = self.config.fn_name,
        ));
        out.push_str(&format!(
            "      llvm.store %next_state_{idx}, %state_ptr_{idx} : i32, !llvm.ptr\n",
            idx = state_idx,
        ));

        // Store each live variable via GEP
        for member in frame_members {
            let field_idx = member.index + 1; // +1 because field 0 is resume_state
            out.push_str(&format!(
                "      %spill_{name}_ptr = llvm.getelementptr %task_frame[0, {field}] : (!llvm.ptr) -> !llvm.ptr, !TaskFrame_{fn_name}\n",
                name = member.name, field = field_idx, fn_name = self.config.fn_name,
            ));
            out.push_str(&format!(
                "      llvm.store %{name}, %spill_{name}_ptr : {ty}, !llvm.ptr\n",
                name = member.name, ty = member.ty,
            ));
        }

        out
    }

    /// Generate reload logic: GEP + load for each live variable at a resume point.
    /// ZST types (Context, Unit) are materialized via `llvm.mlir.undef` instead of loaded.
    pub fn generate_reload(&self, frame_members: &[FrameMember], state_idx: usize) -> String {
        let mut out = String::new();

        out.push_str(&format!(
            "      // Reload: restore live vars for state {}\n",
            state_idx,
        ));

        for member in frame_members {
            // ZST check: Context, Unit, () are never stored in the frame
            let is_zst = member.ty == "Context" || member.ty == "()" || member.ty == "Unit";
            if is_zst {
                out.push_str(&format!(
                    "      %{name} = llvm.mlir.undef : {ty}\n",
                    name = member.name, ty = member.ty,
                ));
                continue;
            }

            let field_idx = member.index + 1; // +1 because field 0 is resume_state
            out.push_str(&format!(
                "      %reload_{name}_ptr = llvm.getelementptr %task_frame[0, {field}] : (!llvm.ptr) -> !llvm.ptr, !TaskFrame_{fn_name}\n",
                name = member.name, field = field_idx, fn_name = self.config.fn_name,
            ));
            out.push_str(&format!(
                "      %{name} = llvm.load %reload_{name}_ptr : !llvm.ptr -> {ty}\n",
                name = member.name, ty = member.ty,
            ));
        }

        out
    }

    /// Full async MLIR emission pipeline — orchestrates all components.
    /// Produces: TaskFrame struct + jump table + state functions (with spill/reload)
    ///         + dispatch hub + launcher + completion
    pub fn emit_full_async_mlir(&self, liveness: &LivenessResult) -> String {
        // Delegate to placeholder-body version (for tests/backward compat)
        let num_states = liveness.yield_points.len() + 1;
        let placeholders: Vec<String> = (0..num_states)
            .map(|_| "      // ... state body ...\n".to_string())
            .collect();
        self.emit_full_async_mlir_with_bodies(liveness, &placeholders)
    }

    /// Full async MLIR emission with real per-state body code.
    /// `state_bodies[i]` contains pre-generated MLIR for state i's body segment,
    /// produced by splitting the original function body at yield points and
    /// calling `emit_block()` on each slice.
    pub fn emit_full_async_mlir_with_bodies(
        &self,
        liveness: &LivenessResult,
        state_bodies: &[String],
    ) -> String {
        let mut out = String::new();

        // 1. TaskFrame struct definition
        out.push_str(&self.generate_task_frame_struct(liveness));
        out.push('\n');

        // 2. Jump table (global function pointer array)
        out.push_str(&self.generate_jump_table(liveness));
        out.push('\n');

        // 3. Per-state entry point functions (with embedded reload/spill + real bodies)
        for state_fn in &self.generate_state_functions_with_bodies(liveness, state_bodies) {
            out.push_str(state_fn);
            out.push('\n');
        }

        // 4. Dispatch hub (O(1) indirect call via GEP)
        out.push_str(&self.generate_indirect_dispatch(liveness));
        out.push('\n');

        // 5. Launcher (arena alloc + init frame)
        out.push_str(&self.generate_launcher(liveness));
        out.push('\n');

        // 6. Completion (sentinel + arena free)
        out.push_str(&self.generate_completion());

        out
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> StateMachineConfig {
        StateMachineConfig {
            fn_name: "handler".to_string(),
            ..Default::default()
        }
    }

    fn test_liveness() -> LivenessResult {
        LivenessResult {
            yield_points: vec![
                YieldPointInfo { index: 0, position: 5, label: "yield_0".into() },
                YieldPointInfo { index: 1, position: 12, label: "loop_back_edge".into() },
            ],
            frame_members: vec![
                FrameMember { index: 0, ty: "i64".into(), name: "buffer_len".into() },
                FrameMember { index: 1, ty: "!llvm.ptr".into(), name: "conn_ptr".into() },
            ],
            frame_size: 2,
            needs_transform: true,
        }
    }

    #[test]
    fn test_task_frame_generation() {
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_task_frame_struct(&test_liveness());

        assert!(mlir.contains("TaskFrame_handler"), "Frame name should include function name");
        assert!(mlir.contains("i32"), "Must contain resume_state field (i32)");
        assert!(mlir.contains("i64"), "Must contain captured i64 var");
        assert!(mlir.contains("!llvm.ptr"), "Must contain captured ptr var");
        assert!(mlir.contains("resume_state"), "Should document resume_state field");
        assert!(mlir.contains("buffer_len"), "Should document captured var names");
        assert!(mlir.contains("conn_ptr"), "Should document captured var names");
    }

    #[test]
    fn test_dispatch_hub_mlir() {
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_dispatch_hub(&test_liveness());

        assert!(mlir.contains("llvm.switch"), "Must contain switch dispatch");
        assert!(mlir.contains("0: ^bb_state_0"), "Must have initial state entry");
        assert!(mlir.contains("1: ^bb_state_1"), "Must have state 1 (yield_0 resume)");
        assert!(mlir.contains("2: ^bb_state_2"), "Must have state 2 (yield_1 resume)");
        assert!(mlir.contains("^bb_invalid"), "Must have invalid state handler");
        assert!(mlir.contains("salt.panic"), "Invalid state should panic");
    }

    #[test]
    fn test_suspension_lowering() {
        let lr = test_liveness();
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_suspension(&lr.yield_points[0], &lr.frame_members);

        assert!(mlir.contains("arith.constant 1 : i32"), "Next state should be 1");
        assert!(mlir.contains("llvm.insertvalue"), "Must store vars into frame");
        assert!(mlir.contains("buffer_len"), "Must store captured variable");
        assert!(mlir.contains("conn_ptr"), "Must store captured variable");
        assert!(mlir.contains("llvm.return"), "Must return to executor");
    }

    #[test]
    fn test_launcher_mlir() {
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_launcher(&test_liveness());

        assert!(mlir.contains("keuos_arena_alloc"), "Must use arena allocator");
        assert!(mlir.contains("arith.constant 0 : i32"), "Initial state must be 0");
        assert!(mlir.contains("llvm.insertvalue"), "Must initialize frame");
        assert!(mlir.contains("llvm.return %task_frame"), "Must return frame pointer");
    }

    #[test]
    fn test_completion_mlir() {
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_completion();

        assert!(mlir.contains("arith.constant -1 : i32"), "Completion sentinel = -1");
        assert!(mlir.contains("keuos_arena_free"), "Must free frame");
        assert!(mlir.contains("llvm.return"), "Must return after cleanup");
    }

    #[test]
    fn test_empty_frame() {
        let lr = LivenessResult {
            yield_points: vec![
                YieldPointInfo { index: 0, position: 3, label: "yield_0".into() },
            ],
            frame_members: vec![], // No captured vars (all ZSTs or short-lived)
            frame_size: 0,
            needs_transform: true,
        };

        let emitter = StateMachineEmitter::new(test_config());
        let frame_mlir = emitter.generate_task_frame_struct(&lr);
        let hub_mlir = emitter.generate_dispatch_hub(&lr);

        // Even with no captured vars, we still need resume_state
        assert!(frame_mlir.contains("i32"), "Must have resume_state even with empty frame");
        assert!(hub_mlir.contains("0: ^bb_state_0"), "Must have initial state");
        assert!(hub_mlir.contains("1: ^bb_state_1"), "Must have resume state for yield_0");
    }

    #[test]
    fn test_default_config() {
        let cfg = StateMachineConfig::default();
        assert_eq!(cfg.arena_alloc, "keuos_arena_alloc");
        assert_eq!(cfg.arena_free, "keuos_arena_free");
    }

    // =========================================================================
    // PR 3: Jump Table Tests (TDD)
    // =========================================================================

    #[test]
    fn test_jump_table_mlir_format() {
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_jump_table(&test_liveness());

        // Must emit a global constant array
        assert!(mlir.contains("llvm.mlir.global"),
            "Jump table must use llvm.mlir.global");
        assert!(mlir.contains("!llvm.array<3 x !llvm.ptr>"),
            "Must be array of 3 function pointers (1 initial + 2 yield points)");
        assert!(mlir.contains("L_dispatch_table_handler"),
            "Table name must include function name");
        assert!(mlir.contains("@handler_state_0"),
            "Must reference state_0 function");
        assert!(mlir.contains("@handler_state_1"),
            "Must reference state_1 function");
        assert!(mlir.contains("@handler_state_2"),
            "Must reference state_2 function");
    }

    #[test]
    fn test_dispatch_hub_uses_indirect_branch() {
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_indirect_dispatch(&test_liveness());

        // Must use GEP to index into table
        assert!(mlir.contains("getelementptr"),
            "Must use GEP to index into jump table");
        // Must load function pointer
        assert!(mlir.contains("llvm.load"),
            "Must load function pointer from table");
        // Must call through function pointer (indirect, musttail)
        assert!(mlir.contains("llvm.call tail %fn_ptr"),
            "Must use indirect musttail call via function pointer");
        // Must reference the global table
        assert!(mlir.contains("llvm.mlir.addressof @L_dispatch_table_handler"),
            "Must address the global jump table");
    }

    #[test]
    fn test_dispatch_hub_uses_musttail() {
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_indirect_dispatch(&test_liveness());

        // Must use musttail for zero stack growth during dispatch
        assert!(mlir.contains("musttail"),
            "Dispatch hub must use musttail call for zero stack growth");
        assert!(mlir.contains("tail_call_kind"),
            "Dispatch hub must specify tail_call_kind attribute");
    }

    #[test]
    fn test_state_functions_have_noinline() {
        let emitter = StateMachineEmitter::new(test_config());
        let functions = emitter.generate_state_functions(&test_liveness());

        for (i, func) in functions.iter().enumerate() {
            assert!(func.contains("noinline"),
                "State function {} must have noinline attribute", i);
            assert!(func.contains("passthrough"),
                "State function {} must use passthrough attribute format", i);
        }
    }

    #[test]
    fn test_state_functions_generated() {
        let emitter = StateMachineEmitter::new(test_config());
        let functions = emitter.generate_state_functions(&test_liveness());

        // 1 initial state + 2 yield points = 3 state functions
        assert_eq!(functions.len(), 3,
            "Must generate one function per state (1 initial + 2 yields)");

        // Each function should declare as func.func
        assert!(functions[0].contains("func.func @handler_state_0"),
            "State 0 must be named handler_state_0");
        assert!(functions[1].contains("func.func @handler_state_1"),
            "State 1 must be named handler_state_1");
        assert!(functions[2].contains("func.func @handler_state_2"),
            "State 2 must be named handler_state_2");

        // State 0 should mention initial entry
        assert!(functions[0].contains("Initial entry"),
            "State 0 should document initial entry");

        // States 1-2 should reference their yield points
        assert!(functions[1].contains("Resume from"),
            "Resume states should document resumption");
    }

    // =========================================================================
    // PR 9: Spill/Reload + Pipeline Tests (TDD)
    // =========================================================================

    #[test]
    fn test_spill_generates_gep_store() {
        let emitter = StateMachineEmitter::new(test_config());
        let lr = test_liveness();
        let mlir = emitter.generate_spill(&lr.frame_members, 0);

        // Must store resume_state = next via GEP
        assert!(mlir.contains("arith.constant 1 : i32"),
            "Spill must set next state to 1");
        assert!(mlir.contains("llvm.getelementptr %task_frame[0, 0]"),
            "Must GEP to resume_state field (offset 0)");
        assert!(mlir.contains("llvm.store %next_state_0, %state_ptr_0"),
            "Must store next state via GEP");

        // Must store each captured variable via GEP
        assert!(mlir.contains("llvm.getelementptr %task_frame[0, 1]"),
            "Must GEP to buffer_len field (offset 1)");
        assert!(mlir.contains("llvm.store %buffer_len, %spill_buffer_len_ptr"),
            "Must store buffer_len");
        assert!(mlir.contains("llvm.getelementptr %task_frame[0, 2]"),
            "Must GEP to conn_ptr field (offset 2)");
        assert!(mlir.contains("llvm.store %conn_ptr, %spill_conn_ptr_ptr"),
            "Must store conn_ptr");
    }

    #[test]
    fn test_reload_generates_gep_load() {
        let emitter = StateMachineEmitter::new(test_config());
        let lr = test_liveness();
        let mlir = emitter.generate_reload(&lr.frame_members, 1);

        // Must reload each captured variable via GEP + load
        assert!(mlir.contains("llvm.getelementptr %task_frame[0, 1]"),
            "Must GEP to buffer_len field");
        assert!(mlir.contains("%buffer_len = llvm.load %reload_buffer_len_ptr"),
            "Must load buffer_len from frame");
        assert!(mlir.contains("llvm.getelementptr %task_frame[0, 2]"),
            "Must GEP to conn_ptr field");
        assert!(mlir.contains("%conn_ptr = llvm.load %reload_conn_ptr_ptr"),
            "Must load conn_ptr from frame");
    }

    #[test]
    fn test_reload_zst_materialization() {
        let lr = LivenessResult {
            yield_points: vec![
                YieldPointInfo { index: 0, position: 3, label: "yield_0".into() },
            ],
            frame_members: vec![
                FrameMember { index: 0, ty: "i64".into(), name: "data".into() },
                FrameMember { index: 1, ty: "Context".into(), name: "ctx".into() },
                FrameMember { index: 2, ty: "Unit".into(), name: "unit_val".into() },
            ],
            frame_size: 3,
            needs_transform: true,
        };

        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_reload(&lr.frame_members, 1);

        // data (i64) should be loaded via GEP
        assert!(mlir.contains("%data = llvm.load"),
            "Non-ZST var should be loaded from frame");

        // ctx (Context) should be materialized as undef, NOT loaded
        assert!(mlir.contains("%ctx = llvm.mlir.undef : Context"),
            "Context ZST should be materialized, not loaded");
        assert!(!mlir.contains("reload_ctx_ptr"),
            "Context should NOT generate a GEP");

        // unit_val (Unit) should be materialized as undef
        assert!(mlir.contains("%unit_val = llvm.mlir.undef : Unit"),
            "Unit ZST should be materialized, not loaded");
    }

    #[test]
    fn test_task_frame_64_byte_alignment() {
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.generate_task_frame_struct(&test_liveness());

        assert!(mlir.contains("alignment = 64"),
            "TaskFrame must be 64-byte aligned for M4 cache-line optimization");
    }

    #[test]
    fn test_emit_full_async_mlir_pipeline() {
        let emitter = StateMachineEmitter::new(test_config());
        let mlir = emitter.emit_full_async_mlir(&test_liveness());

        // Must contain all major sections
        assert!(mlir.contains("TaskFrame_handler"),
            "Full pipeline must emit TaskFrame struct");
        assert!(mlir.contains("L_dispatch_table_handler"),
            "Full pipeline must emit jump table");
        assert!(mlir.contains("func.func @handler_state_0"),
            "Full pipeline must emit state 0 function");
        assert!(mlir.contains("func.func @handler_state_1"),
            "Full pipeline must emit state 1 function");
        assert!(mlir.contains("func.func @handler_state_2"),
            "Full pipeline must emit state 2 function");
        assert!(mlir.contains("llvm.getelementptr"),
            "Full pipeline must emit GEP for dispatch");
        assert!(mlir.contains("keuos_arena_alloc"),
            "Full pipeline must emit launcher");
        assert!(mlir.contains("keuos_arena_free"),
            "Full pipeline must emit completion");

        // Verify reload appears in resume states (state 1, 2) but not state 0
        assert!(mlir.contains("Reload: restore live vars for state 1"),
            "State 1 must reload live variables");
        assert!(mlir.contains("Reload: restore live vars for state 2"),
            "State 2 must reload live variables");

        // Verify spill appears in state 0, 1 but not state 2 (last state)
        assert!(mlir.contains("Spill: save live vars before yield (state 0"),
            "State 0 must spill before yield");
        assert!(mlir.contains("Spill: save live vars before yield (state 1"),
            "State 1 must spill before yield");
    }

    // =========================================================================
    // PR 10: Body Splitting TDD Tests
    // =========================================================================

    #[test]
    fn test_body_injection_replaces_placeholder() {
        // Given: an emitter with 2 yield points (3 states) and custom body strings
        let emitter = StateMachineEmitter::new(test_config());
        let liveness = test_liveness(); // 2 yield points → 3 states
        let bodies = vec![
            "      %x = arith.constant 42 : i32\n".to_string(),
            "      %y = arith.addi %x, %x : i32\n".to_string(),
            "      func.return %y : i32\n".to_string(),
        ];

        let fns = emitter.generate_state_functions_with_bodies(&liveness, &bodies);

        // Then: each state function must contain its body, not the placeholder
        assert_eq!(fns.len(), 3, "Should produce 3 state functions");
        assert!(fns[0].contains("arith.constant 42"),
            "State 0 must contain its body code");
        assert!(!fns[0].contains("... state body ..."),
            "State 0 must NOT contain placeholder");
        assert!(fns[1].contains("arith.addi %x"),
            "State 1 must contain its body code");
        assert!(fns[2].contains("func.return %y"),
            "State 2 must contain its body code");
    }

    #[test]
    fn test_body_split_preserves_reload_spill_wrapping() {
        // Given: 3 states with custom bodies
        let emitter = StateMachineEmitter::new(test_config());
        let liveness = test_liveness(); // 2 yield points, frame_members with "conn" and "count"
        let bodies = vec![
            "      // state 0 body\n".to_string(),
            "      // state 1 body\n".to_string(),
            "      // state 2 body\n".to_string(),
        ];

        let fns = emitter.generate_state_functions_with_bodies(&liveness, &bodies);

        // State 0: body + spill, NO reload (initial entry)
        assert!(fns[0].contains("// state 0 body"), "State 0 body present");
        assert!(fns[0].contains("Spill: save live vars"), "State 0 must spill");
        assert!(!fns[0].contains("Reload:"), "State 0 must NOT reload");

        // State 1: reload + body + spill (resume state, not final)
        assert!(fns[1].contains("// state 1 body"), "State 1 body present");
        assert!(fns[1].contains("Reload: restore live vars for state 1"),
            "State 1 must reload");
        assert!(fns[1].contains("Spill: save live vars"),
            "State 1 must spill");

        // State 2: reload + body, NO spill (final state)
        assert!(fns[2].contains("// state 2 body"), "State 2 body present");
        assert!(fns[2].contains("Reload: restore live vars for state 2"),
            "State 2 must reload");
        assert!(!fns[2].contains("Spill:"),
            "State 2 (final) must NOT spill");
    }

    #[test]
    fn test_full_pipeline_with_bodies_no_placeholders() {
        // Given: full pipeline with real body strings
        let emitter = StateMachineEmitter::new(test_config());
        let liveness = test_liveness();
        let bodies = vec![
            "      %status = llvm.load %buf_ptr : !llvm.ptr -> i8\n".to_string(),
            "      %new_status = arith.addi %status, %one : i8\n".to_string(),
            "      llvm.store %new_status, %buf_ptr : i8, !llvm.ptr\n".to_string(),
        ];

        let mlir = emitter.emit_full_async_mlir_with_bodies(&liveness, &bodies);

        // Must contain real body code
        assert!(mlir.contains("llvm.load %buf_ptr"),
            "Pipeline must contain state 0 body (load)");
        assert!(mlir.contains("arith.addi %status"),
            "Pipeline must contain state 1 body (add)");
        assert!(mlir.contains("llvm.store %new_status"),
            "Pipeline must contain state 2 body (store)");

        // Must NOT contain any placeholder
        assert!(!mlir.contains("... state body ..."),
            "Pipeline with bodies must NOT contain any placeholder");

        // Must still contain infrastructure
        assert!(mlir.contains("TaskFrame_handler"),
            "Must still emit TaskFrame struct");
        assert!(mlir.contains("L_dispatch_table_handler"),
            "Must still emit jump table");
        assert!(mlir.contains("keuos_arena_alloc"),
            "Must still emit launcher");
    }
}
