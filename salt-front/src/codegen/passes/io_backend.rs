// =============================================================================
// I/O Backend Abstraction
//
// Hardware-agnostic I/O backend for the KeuOS async engine.
// Provides a unified trait (IoBackend) with platform-specific implementations:
//   - KqueueBackend (macOS/Darwin)
//   - IoUringBackend (Linux)
//
// Adding a new backend (e.g., IOCP, DPDK) requires only implementing IoBackend.
// The intrinsic dispatch layer and CodegenContext remain unchanged.
// =============================================================================

/// Target platform for I/O backend selection.
/// Defaults to host platform; overridable via compiler flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPlatform {
    /// macOS / Darwin — uses kqueue
    Darwin,
    /// Linux — uses io_uring
    Linux,
}

impl TargetPlatform {
    /// Detect the host platform at compile time.
    pub fn host() -> Self {
        if cfg!(target_os = "macos") {
            TargetPlatform::Darwin
        } else {
            TargetPlatform::Linux
        }
    }
}

impl Default for TargetPlatform {
    fn default() -> Self {
        Self::host()
    }
}

/// Trait for platform-specific I/O MLIR emission.
/// Each backend emits MLIR `func.call` instructions targeting
/// platform-specific runtime symbols.
pub trait IoBackend {
    /// Which platform this backend targets.
    fn platform(&self) -> TargetPlatform;

    /// Emit MLIR for submitting I/O requests to the kernel ring.
    /// `ring_var`: SSA value for the ring/kqueue file descriptor
    /// `batch_var`: SSA value for batch size
    /// Returns MLIR string containing the func.call and result binding.
    fn emit_submit(&self, ring_var: &str, batch_var: &str) -> (String, String);

    /// Emit MLIR for reaping I/O completions from the kernel ring.
    /// `ring_var`: SSA value for the ring/kqueue file descriptor
    /// `buffer_var`: SSA value for the reap buffer pointer
    /// `batch_var`: SSA value for max batch size
    /// Returns MLIR string containing the func.call and result binding.
    fn emit_reap(&self, ring_var: &str, buffer_var: &str, batch_var: &str) -> (String, String);

    /// Emit MLIR for tearing down the I/O ring.
    /// `ring_var`: SSA value for the ring/kqueue file descriptor
    /// Returns MLIR string for the teardown call.
    fn emit_teardown(&self, ring_var: &str) -> String;
}

// =============================================================================
// kqueue Backend (macOS / Darwin)
// =============================================================================

/// kqueue-based I/O backend for macOS.
/// Uses kevent() for submission and reaping, with EV_DISPATCH for
/// automatic event masking (prevents double-pulsing across cores).
pub struct KqueueBackend;

impl IoBackend for KqueueBackend {
    fn platform(&self) -> TargetPlatform {
        TargetPlatform::Darwin
    }

    fn emit_submit(&self, ring_var: &str, batch_var: &str) -> (String, String) {
        let res = "%kq_submit_result".to_string();
        let mlir = format!(
            "    {} = func.call @salt_kqueue_submit({}, {}) : (i64, i64) -> i64\n",
            res, ring_var, batch_var
        );
        (mlir, res)
    }

    fn emit_reap(&self, ring_var: &str, buffer_var: &str, batch_var: &str) -> (String, String) {
        let res = "%kq_reap_result".to_string();
        let mlir = format!(
            "    {} = func.call @salt_kqueue_reap({}, {}, {}) : (i64, !llvm.ptr, i64) -> i64\n",
            res, ring_var, buffer_var, batch_var
        );
        (mlir, res)
    }

    fn emit_teardown(&self, ring_var: &str) -> String {
        format!(
            "    func.call @salt_kqueue_teardown({}) : (i64) -> ()\n",
            ring_var
        )
    }
}

// =============================================================================
// io_uring Backend (Linux)
// =============================================================================

/// io_uring-based I/O backend for Linux.
/// Uses io_uring_enter() for submission and direct CQ ring reads for reaping.
/// Supports zero-copy via IORING_OP_READV with DMA arena buffers.
pub struct IoUringBackend;

impl IoBackend for IoUringBackend {
    fn platform(&self) -> TargetPlatform {
        TargetPlatform::Linux
    }

    fn emit_submit(&self, ring_var: &str, batch_var: &str) -> (String, String) {
        let res = "%uring_submit_result".to_string();
        let mlir = format!(
            "    {} = func.call @salt_uring_submit({}, {}) : (i64, i64) -> i64\n",
            res, ring_var, batch_var
        );
        (mlir, res)
    }

    fn emit_reap(&self, ring_var: &str, buffer_var: &str, batch_var: &str) -> (String, String) {
        let res = "%uring_reap_result".to_string();
        let mlir = format!(
            "    {} = func.call @salt_uring_reap({}, {}, {}) : (i64, !llvm.ptr, i64) -> i64\n",
            res, ring_var, buffer_var, batch_var
        );
        (mlir, res)
    }

    fn emit_teardown(&self, ring_var: &str) -> String {
        format!(
            "    func.call @salt_uring_teardown({}) : (i64) -> ()\n",
            ring_var
        )
    }
}

/// Create the appropriate I/O backend for a given target platform.
pub fn backend_for_target(target: TargetPlatform) -> Box<dyn IoBackend> {
    match target {
        TargetPlatform::Darwin => Box::new(KqueueBackend),
        TargetPlatform::Linux => Box::new(IoUringBackend),
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // TDD Tests — Written before implementation (now verified)
    // =========================================================================

    #[test]
    fn test_target_platform_default_matches_host() {
        let platform = TargetPlatform::host();
        if cfg!(target_os = "macos") {
            assert_eq!(platform, TargetPlatform::Darwin,
                "Host platform on macOS must default to Darwin");
        } else {
            assert_eq!(platform, TargetPlatform::Linux,
                "Host platform on Linux must default to Linux");
        }
    }

    #[test]
    fn test_kqueue_submit_emits_kqueue_call() {
        let backend = KqueueBackend;
        let (mlir, res) = backend.emit_submit("%ring_fd", "%batch");

        assert!(mlir.contains("@salt_kqueue_submit"),
            "kqueue submit must call @salt_kqueue_submit");
        assert!(mlir.contains("%ring_fd"),
            "kqueue submit must use ring_fd argument");
        assert!(mlir.contains("%batch"),
            "kqueue submit must use batch argument");
        assert!(mlir.contains("-> i64"),
            "kqueue submit must return i64 (count)");
        assert!(!res.is_empty(), "Result SSA value must be non-empty");
    }

    #[test]
    fn test_kqueue_reap_emits_kevent_batch() {
        let backend = KqueueBackend;
        let (mlir, res) = backend.emit_reap("%ring_fd", "%buffer", "%max_batch");

        assert!(mlir.contains("@salt_kqueue_reap"),
            "kqueue reap must call @salt_kqueue_reap");
        assert!(mlir.contains("%ring_fd"),
            "kqueue reap must use ring_fd");
        assert!(mlir.contains("%buffer"),
            "kqueue reap must use buffer pointer");
        assert!(mlir.contains("%max_batch"),
            "kqueue reap must use max_batch");
        assert!(mlir.contains("!llvm.ptr"),
            "kqueue reap must include ptr type for buffer");
        assert!(!res.is_empty(), "Result SSA value must be non-empty");
    }

    #[test]
    fn test_kqueue_teardown_emits_close() {
        let backend = KqueueBackend;
        let mlir = backend.emit_teardown("%ring_fd");

        assert!(mlir.contains("@salt_kqueue_teardown"),
            "kqueue teardown must call @salt_kqueue_teardown");
        assert!(mlir.contains("%ring_fd"),
            "kqueue teardown must use ring_fd");
        assert!(mlir.contains("-> ()"),
            "kqueue teardown must return void");
    }

    #[test]
    fn test_uring_submit_emits_uring_enter() {
        let backend = IoUringBackend;
        let (mlir, res) = backend.emit_submit("%ring_fd", "%batch");

        assert!(mlir.contains("@salt_uring_submit"),
            "io_uring submit must call @salt_uring_submit");
        assert!(mlir.contains("%ring_fd"),
            "io_uring submit must use ring_fd");
        assert!(mlir.contains("-> i64"),
            "io_uring submit must return i64");
        assert!(!res.is_empty(), "Result SSA value must be non-empty");
    }

    #[test]
    fn test_uring_reap_emits_cq_drain() {
        let backend = IoUringBackend;
        let (mlir, res) = backend.emit_reap("%ring_fd", "%cq_buf", "%max_reap");

        assert!(mlir.contains("@salt_uring_reap"),
            "io_uring reap must call @salt_uring_reap");
        assert!(mlir.contains("%cq_buf"),
            "io_uring reap must use CQ buffer pointer");
        assert!(mlir.contains("!llvm.ptr"),
            "io_uring reap must include ptr type for buffer");
        assert!(!res.is_empty(), "Result SSA value must be non-empty");
    }

    #[test]
    fn test_uring_teardown_emits_queue_exit() {
        let backend = IoUringBackend;
        let mlir = backend.emit_teardown("%ring_fd");

        assert!(mlir.contains("@salt_uring_teardown"),
            "io_uring teardown must call @salt_uring_teardown");
        assert!(mlir.contains("-> ()"),
            "io_uring teardown must return void");
    }

    #[test]
    fn test_backend_dispatch_uses_target() {
        // Darwin target should produce kqueue backend
        let darwin_backend = backend_for_target(TargetPlatform::Darwin);
        assert_eq!(darwin_backend.platform(), TargetPlatform::Darwin);
        let (mlir, _) = darwin_backend.emit_submit("%fd", "%n");
        assert!(mlir.contains("kqueue"), "Darwin must use kqueue backend");

        // Linux target should produce io_uring backend
        let linux_backend = backend_for_target(TargetPlatform::Linux);
        assert_eq!(linux_backend.platform(), TargetPlatform::Linux);
        let (mlir, _) = linux_backend.emit_submit("%fd", "%n");
        assert!(mlir.contains("uring"), "Linux must use io_uring backend");
    }
}
