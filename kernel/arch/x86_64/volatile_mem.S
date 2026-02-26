// Volatile memory access helpers for IPC signaling.
// These prevent the compiler from hoisting loads/stores out of spin-wait loops.
// Assembly functions cannot be optimized away by salt-opt.

.section .text

// volatile_read_i32(addr: *i32) -> i32
// Reads a 32-bit value from memory without optimization.
.global volatile_read_i32
volatile_read_i32:
    movl (%rdi), %eax
    ret

// volatile_write_i32(addr: *i32, val: i32)
// Writes a 32-bit value to memory without optimization.
.global volatile_write_i32
volatile_write_i32:
    movl %esi, (%rdi)
    ret

// volatile_read_i64(addr: *i64) -> i64
// Reads a 64-bit value from memory without optimization.
.global volatile_read_i64
volatile_read_i64:
    movq (%rdi), %rax
    ret

// volatile_write_i64(addr: *i64, val: i64)
// Writes a 64-bit value to memory without optimization.
.global volatile_write_i64
volatile_write_i64:
    movq %rsi, (%rdi)
    ret
