.PHONY: setup build test clean run-qemu

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
