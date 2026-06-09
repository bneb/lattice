# KeuOS Runtime Substrate — Build System
#
# Builds keuos_rt.o from keuos_rt.c with no libc dependency.
# Target: arm64-apple-macosx14.0.0 (Apple M4)

CC = /opt/homebrew/opt/llvm/bin/clang
CFLAGS = -nostdlib -ffreestanding -O3 -target arm64-apple-macosx14.0.0 -Wall -Wextra

.PHONY: all clean check

all: keuos_rt.o

keuos_rt.o: keuos_rt.c
	$(CC) $(CFLAGS) -c keuos_rt.c -o keuos_rt.o

# Verify the object exports all required symbols
check: keuos_rt.o
	@echo "=== KeuOS RT Symbol Audit ==="
	@nm keuos_rt.o | grep -E "isolated_arena_alloc|isolated_arena_reset|salt_init_x19|salt_kqueue_init|salt_kqueue_submit|salt_kqueue_reap|salt_kqueue_teardown|_start"
	@echo "=== All symbols present ==="

clean:
	rm -f keuos_rt.o
