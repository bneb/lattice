.PHONY: setup build test test-userspace clean run-qemu lettuce lettuce-verify lettuce-run bench

# =============================================================================
# Salt + KeuOS — Top-Level Makefile
# =============================================================================

setup:
	@bash scripts/bootstrap.sh

build:
	cd salt-front && cargo build --release
	@echo "Compiler built: salt-front/target/release/salt-front"

test:
	cd salt-front && cargo test
	@echo "Tests complete."

test-userspace: build
	@echo "=== KeuOS User Program Test Suite ==="
	@python3 tools/runner_qemu.py test
	@echo ""

.PHONY: test-userspace

clean:
	cd salt-front && cargo clean
	rm -f /tmp/salt_hello /tmp/salt_build/*
	@echo "Clean."

run-qemu:
	qemu-system-x86_64 -cdrom keuos.iso -m 512M -serial stdio -no-reboot

# =============================================================================
# LETTUCE — Verified HTTP/Redis Server
# =============================================================================

SALT_FRONT := salt-front/target/release/salt-front
LETTUCE_SRC := lettuce/src/server.salt
LETTUCE_MLIR := /tmp/lettuce_server.mlir

lettuce: build lettuce-verify
	@echo ""
	@echo "============================================"
	@echo "  LETTUCE — Verified HTTP Server"
	@echo "============================================"
	@echo ""
	@echo "  ✓ Compiler built"
	@echo "  ✓ Z3 contracts verified (resp, aof, store)"
	@echo "  ✓ MLIR emitted: $(LETTUCE_MLIR)"
	@echo ""
	@echo "  Run with: make lettuce-run"
	@echo "  Test with: redis-cli -p 6379 PING"

lettuce-verify: build
	@echo "=== Lettuce: Z3 Contract Verification ==="
	@bash lettuce/tests/test_verified_http.sh
	@echo ""
	@echo "=== Lettuce: Compiling with --verify ==="
	@$(SALT_FRONT) $(LETTUCE_SRC) --verify -o $(LETTUCE_MLIR) 2>&1 | grep -v 'GENERIC WARNING'
	@echo ""

lettuce-run: build
	@echo "=== Building LETTUCE server binary ==="
	@zsh scripts/run_test.sh $(LETTUCE_SRC) --compile-only 2>&1 | grep -v 'GENERIC WARNING\|zoxide\|_ZO_DOCTOR' | tail -15
	@echo ""
	@echo "Binary: /tmp/salt_build/server"
	@echo "Target: KeuOS (QEMU/KVM)"
	@echo "Run in QEMU: make run-qemu"

bench: build
	@bash benchmarks/lettuce_bench.sh 2>&1 | grep -v 'zoxide\|_ZO_DOCTOR\|GENERIC WARNING\|Blocking functions'

bench-long: build
	@bash benchmarks/lettuce_bench.sh --long 2>&1 | grep -v 'zoxide\|_ZO_DOCTOR\|GENERIC WARNING\|Blocking functions'
