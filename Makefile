.PHONY: setup build test clean run-qemu lettuce lettuce-verify lettuce-run

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

lettuce-run: lettuce
	@echo "Starting LETTUCE on port 6379..."
	@echo "Connect with: redis-cli -p 6379 PING"
	@echo ""
	@zsh scripts/run_test.sh $(LETTUCE_SRC) --compile-only 2>&1 | tail -20
	@echo ""
	@echo "Binary at /tmp/salt_build/server"
	@echo "Run: DYLD_LIBRARY_PATH=/opt/homebrew/lib /tmp/salt_build/server"
