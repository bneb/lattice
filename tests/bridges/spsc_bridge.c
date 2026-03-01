// =============================================================================
// tests/bridges/spsc_bridge.c
// Userland stubs for kernel-only assembly functions used by SPSC tests
// =============================================================================

#include <stdint.h>

// Volatile memory access stubs
// In kernel: these are in volatile_mem.S with explicit mov (preventing
// reorder). In userland: the volatile qualifier on the pointer achieves the
// same effect.

int64_t volatile_read_i64(uint64_t addr) { return *(volatile int64_t *)addr; }

void volatile_write_i64(uint64_t addr, int64_t val) {
  *(volatile int64_t *)addr = val;
}

// cpu_pause — x86 PAUSE instruction hint for spin-wait loops
void cpu_pause(void) {
#if defined(__x86_64__) || defined(__i386__)
  __asm__ volatile("pause");
#elif defined(__aarch64__)
  __asm__ volatile("yield");
#endif
}

// idle_halt — no-op in userland (in kernel: sti;hlt)
void idle_halt(void) {
  // No-op in userland test context
}
