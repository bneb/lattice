#include <fcntl.h>
#ifdef __APPLE__
#include <crt_externs.h> // _NSGetArgc, _NSGetArgv
#include <mach/mach_time.h>
#else
#include <time.h>
#endif
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

// =============================================================================
// SALT RUNTIME: Environment Access (argc/argv/getenv)
// =============================================================================

// Cross-platform argc/argv storage.
// On Apple, we can use _NSGetArgc/_NSGetArgv directly.
// On Linux (and all other POSIX), we capture from a constructor that
// intercepts main's argc/argv before Salt's entry point runs.
#ifndef __APPLE__
static int g_argc = 0;
static char **g_argv = NULL;

// GCC/Clang constructor: runs before main(), captures argc/argv from the
// stack frame. This is the standard, portable mechanism for Linux/BSD.
__attribute__((constructor)) static void
salt_capture_args(int argc, char **argv, char **envp) {
  (void)envp;
  g_argc = argc;
  g_argv = argv;
}
#endif

int32_t salt_get_argc() {
#ifdef __APPLE__
  return (int32_t)*_NSGetArgc();
#else
  return (int32_t)g_argc;
#endif
}

const char *salt_get_argv(int32_t idx) {
#ifdef __APPLE__
  int argc = *_NSGetArgc();
  char **argv = *_NSGetArgv();
#else
  int argc = g_argc;
  char **argv = g_argv;
#endif
  if (idx < 0 || idx >= argc || argv == NULL)
    return NULL;
  return argv[idx];
}

int64_t salt_get_argv_len(int32_t idx) {
#ifdef __APPLE__
  int argc = *_NSGetArgc();
  char **argv = *_NSGetArgv();
#else
  int argc = g_argc;
  char **argv = g_argv;
#endif
  if (idx < 0 || idx >= argc || argv == NULL)
    return 0;
  return (int64_t)strlen(argv[idx]);
}

const char *salt_getenv(const char *name) { return getenv(name); }

int64_t salt_strlen(const char *s) {
  if (!s)
    return 0;
  return (int64_t)strlen(s);
}

// Syscall wrappers for Salt file I/O
int64_t sys_open(const char *path, int64_t flags, int64_t mode) {
  return open(path, (int)flags, (int)mode);
}

// Salt: sys_write(fd: i32, buf: Ptr<u8>, len: i64) -> i64
int64_t sys_write(int32_t fd, const void *buf, int64_t len) {
  return (int64_t)write(fd, buf, (size_t)len);
}

// Salt: sys_read(fd: i32, buf: i64, len: i64) -> i64
int64_t sys_read(int32_t fd, void *buf, int64_t len) {
  return (int64_t)read(fd, buf, (size_t)len);
}

// Salt: sys_close(fd: i32) -> i32
int32_t sys_close(int32_t fd) { return (int32_t)close(fd); }

// Returns address as integer to match Salt's u64 return type
int64_t sys_mmap(int64_t addr, int64_t len, int64_t prot, int64_t flags,
                 int64_t fd, int64_t offset) {
  void *result = mmap((void *)addr, (size_t)len, (int)prot, (int)flags, (int)fd,
                      (off_t)offset);
  return (int64_t)result;
}

// Salt: sys_munmap(addr: i64, len: i64) -> i32
int32_t sys_munmap(int64_t addr, int64_t length) {
  return (int32_t)munmap((void *)(uintptr_t)addr, (size_t)length);
}

// Salt memcpy wrapper - using asm label to provide C symbol 'memcpy' without
// macro conflict.
// IMPORTANT: Cannot use __builtin_memcpy here because at -O3, clang may emit
// a call to memcpy for it, causing infinite recursion since this IS memcpy.
int64_t salt_memcpy_impl(int64_t dst, int64_t src,
                         int64_t len) __asm__("_memcpy");
int64_t salt_memcpy_impl(int64_t dst, int64_t src, int64_t len) {
  unsigned char *d = (unsigned char *)(uintptr_t)dst;
  const unsigned char *s = (const unsigned char *)(uintptr_t)src;
  if (len <= 0)
    return dst;
  // memmove semantics: copy backwards when dst > src and regions overlap.
  // This prevents silent memory corruption on overlapping regions.
  if (d > s && (size_t)(d - s) < (size_t)len) {
    for (int64_t i = len - 1; i >= 0; i--) {
      d[i] = s[i];
    }
  } else {
    for (int64_t i = 0; i < len; i++) {
      d[i] = s[i];
    }
  }
  return dst;
}

// [SOVEREIGN FIX] Salt allocator interfaces - simple mmap/munmap wrappers
// Called by std/core/mem.salt Alloc functions via MLIR extern declarations
int64_t salt_mmap(int64_t size) {
  void *result = mmap(NULL, (size_t)size, PROT_READ | PROT_WRITE,
                      MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  return (result == MAP_FAILED) ? 0 : (int64_t)result;
}

int64_t salt_munmap(int64_t addr, int64_t size) {
  return (int64_t)munmap((void *)addr, (size_t)size);
}

// =============================================================================
// [SOVEREIGN V4.0] Arena Allocator - O(1) bump allocation with mark/reset
// =============================================================================
// Global arena state - 256MB for benchmark workloads
#define ARENA_SIZE (256 * 1024 * 1024)

static int64_t arena_base = 0;
static int64_t arena_current = 0;
static int64_t arena_end = 0;

// Initialize: allocate 64MB arena via mmap
static void arena_init() {
  if (arena_base == 0) {
    void *result = mmap(NULL, ARENA_SIZE, PROT_READ | PROT_WRITE,
                        MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (result == MAP_FAILED) {
      arena_base = 0;
    } else {
      arena_base = (int64_t)result;
      arena_current = arena_base;
      arena_end = arena_base + ARENA_SIZE;
    }
  }
}

// Allocate from arena with 8-byte alignment (bump allocator - O(1))
// Returns 0 (null) if arena is exhausted.
int64_t salt_arena_alloc(int64_t size) {
  arena_init();
  // Align to 8 bytes using Sovereign Bitmask pattern
  int64_t aligned_addr = (arena_current + 7) & ~7;
  int64_t next = aligned_addr + size;
  if (next > arena_end)
    return 0; // OOM: return null
  arena_current = next;
  return aligned_addr;
}

// Mark current position for later reset
int64_t salt_arena_mark() {
  arena_init();
  return arena_current;
}

// Reset to a previous mark (O(1) memory reclamation)
// In debug mode (SALT_DEBUG), poison freed memory with 0xDD to catch UAF.
void salt_arena_reset_to(int64_t mark) {
#ifdef SALT_DEBUG
  // Poison the region being freed: fill with 0xDD ("Dead Data")
  // Any dangling pointer dereference will read 0xDDDDDDDDDDDDDDDD
  // instead of stale valid data, converting silent corruption into
  // immediate, debuggable garbage.
  if (arena_current > mark) {
    size_t poison_size = (size_t)(arena_current - mark);
    memset((void *)mark, 0xDD, poison_size);
  }
#endif
  arena_current = mark;
}

// High-resolution monotonic clock — returns nanoseconds
int64_t salt_clock_now() {
#ifdef __APPLE__
  static mach_timebase_info_data_t timebase_info;
  static int timebase_initialized = 0;
  if (!timebase_initialized) {
    mach_timebase_info(&timebase_info);
    timebase_initialized = 1;
  }
  uint64_t ticks = mach_absolute_time();
  return (int64_t)(ticks * timebase_info.numer / timebase_info.denom);
#else
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (int64_t)ts.tv_sec * 1000000000LL + ts.tv_nsec;
#endif
}

// Backward compat alias for c10m/gauntlet benchmarks
int64_t rdtsc() { return salt_clock_now(); }

/**
 * SALT RUNTIME SHIM - Linux x86_64 / macOS
 * Bridges @no_mangle Salt symbols to the Host OS.
 */

// Your Salt code calls '@no_mangle extern func syscall6'
// We provide the implementation here using the standard C library.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
int64_t syscall6(int64_t number, int64_t arg1, int64_t arg2, int64_t arg3,
                 int64_t arg4, int64_t arg5, int64_t arg6) {
  // printf("DEBUG: syscall6 called with %lld\n", number);
  return syscall(number, arg1, arg2, arg3, arg4, arg5, arg6);
}
#pragma clang diagnostic pop

void ___salt_yield_check() {
  // No-op for benchmark
}

void __salt_yield_check() {}

// Debug function to trace allocator pointers
void debug_print_ptr(void *p, const char *label) {
  printf("[SALT-DEBUG] %s: %p\n", label, p);
}

// Contract violation handler — called when Z3 couldn't prove a requires/ensures
// clause at compile time and the runtime check fails.
void __salt_contract_violation() {
  fprintf(stderr,
          "FATAL: Salt contract violation (requires/ensures/invariant)\n");
  abort();
}

// Salt panic hook - explicit failure with clear message
void __salt_panic(const char *message) {
  fprintf(stderr, "CRITICAL: Salt Runtime Panic: %s\n", message);
  __builtin_trap(); // Triggers SIGTRAP, caught by lldb/gdb
}

// Printf shim for benchmarks - TODO: Replace with proper println! macro
void printf_shim(const char *fmt, int64_t val) { printf(fmt, val); }

// =============================================================================
// SALT RUNTIME: Print Hooks (for println! macro)
// =============================================================================

void __salt_print_literal(const char *str, int64_t len) {
  fwrite(str, 1, (size_t)len, stdout);
}

void __salt_print_i64(int64_t val) { printf("%lld", val); }

void __salt_print_u64(int64_t val) { printf("%llu", (unsigned long long)val); }

void __salt_print_f64(double val) { printf("%g", val); }

void __salt_print_bool(int8_t val) { printf("%s", val ? "true" : "false"); }

void __salt_print_ptr(int64_t val) {
  printf("0x%llx", (unsigned long long)val);
}

// =============================================================================
// SALT RUNTIME: F-String Formatting Hooks
// =============================================================================

// Format f64 with specified precision directly into buffer.
// Returns number of bytes written (for buffer position advancement).
// Uses snprintf for portability; can be replaced with NEON-optimized dtoa
// later.
int64_t __salt_fmt_f64_to_buf(char *buf, double val, int64_t precision) {
  // Use snprintf to format with given precision
  // Format: "%.Nf" where N is precision
  char fmt[8];
  snprintf(fmt, sizeof(fmt), "%%.%lldf", (long long)precision);
  int written = snprintf(buf, 64, fmt, val); // Assume max 64 bytes for any f64
  return (int64_t)(written > 0 ? written : 0);
}

// =============================================================================
// SALT MEMORY ARCHITECTURE V4.0 - "Memory Sovereignty"
// =============================================================================

/**
 * Salt System Allocator Hook
 * Called by the Salt core (std/core/mem.salt) to request aligned memory.
 * Uses posix_memalign for alignment guarantees.
 */
void *salt_sys_alloc(int64_t size, int64_t align) {
  if (size <= 0)
    return NULL;

  void *ptr = NULL;
  // Ensure alignment is at least sizeof(void*)
  size_t actual_align = (size_t)align;
  if (actual_align < sizeof(void *)) {
    actual_align = sizeof(void *);
  }

  if (posix_memalign(&ptr, actual_align, (size_t)size) != 0) {
    // Allocation failed - this is a critical error
    fprintf(stderr, "FATAL: Salt runtime failed to allocate %lld bytes\n",
            (long long)size);
    return NULL;
  }
  return ptr;
}

/**
 * Salt System Deallocator Hook
 * Frees memory allocated by salt_sys_alloc.
 */
void salt_sys_dealloc(void *ptr, int64_t size, int64_t align) {
  (void)size;  // Unused - posix_memalign doesn't need size for free
  (void)align; // Unused
  free(ptr);
}

/**
 * Simple Alloc Helper - Direct allocation for Vec and String
 * This is the legacy entry point that the stdlib calls.
 */
void *simple_alloc(int64_t size) {
  return salt_sys_alloc(size, 8); // Default 8-byte alignment
}

void simple_dealloc(void *ptr, int64_t size) { salt_sys_dealloc(ptr, size, 8); }

/**
 * Salt System Realloc Hook
 * Called by HeapAllocator::realloc to resize heap allocations.
 */
void *salt_sys_realloc(void *ptr, int64_t new_size) {
  return realloc(ptr, (size_t)new_size);
}

/**
 * Salt System Free Hook
 * Called by HeapAllocator::free to release heap allocations.
 */
void salt_sys_free(void *ptr) { free(ptr); }

// =============================================================================
// SALT RUNTIME: Threading (pthread bridge for std.thread)
// =============================================================================
#include <pthread.h>

// Thread entry: Salt passes a function pointer (void -> void).
// pthread_create requires (void* -> void*), so we wrap it.
typedef void (*salt_thread_fn)(void);

struct salt_thread_trampoline_ctx {
  salt_thread_fn fn;
};

static void *salt_thread_trampoline(void *arg) {
  struct salt_thread_trampoline_ctx *ctx =
      (struct salt_thread_trampoline_ctx *)arg;
  salt_thread_fn fn = ctx->fn;
  free(ctx);
  fn();
  return NULL;
}

// salt_thread_spawn(fn_addr) -> thread_handle (as i64, actually pthread_t)
// Takes function address as i64 since Salt casts fn to i64.
int64_t salt_thread_spawn(int64_t fn_addr) {
  pthread_t tid;
  struct salt_thread_trampoline_ctx *ctx = malloc(sizeof(*ctx));
  ctx->fn = (salt_thread_fn)fn_addr;
  int ret = pthread_create(&tid, NULL, salt_thread_trampoline, ctx);
  if (ret != 0) {
    free(ctx);
    return 0; // error
  }
  return (int64_t)tid;
}

// salt_thread_join(handle) -> i32 (0 = success)
int32_t salt_thread_join(int64_t handle) {
  return (int32_t)pthread_join((pthread_t)handle, NULL);
}

// =============================================================================
// SALT RUNTIME: Synchronization (pthread mutex + atomics for std.sync)
// =============================================================================

// Mutex: opaque handle (pointer to pthread_mutex_t)
int64_t salt_mutex_create(void) {
  pthread_mutex_t *m = (pthread_mutex_t *)malloc(sizeof(pthread_mutex_t));
  pthread_mutex_init(m, NULL);
  return (int64_t)m;
}

void salt_mutex_lock(int64_t handle) {
  pthread_mutex_lock((pthread_mutex_t *)handle);
}

void salt_mutex_unlock(int64_t handle) {
  pthread_mutex_unlock((pthread_mutex_t *)handle);
}

void salt_mutex_destroy(int64_t handle) {
  pthread_mutex_destroy((pthread_mutex_t *)handle);
  free((void *)handle);
}

// Atomics: 64-bit atomic operations via C11 builtins
int64_t salt_atomic_load_i64(int64_t *ptr) {
  return __atomic_load_n(ptr, __ATOMIC_SEQ_CST);
}

void salt_atomic_store_i64(int64_t *ptr, int64_t val) {
  __atomic_store_n(ptr, val, __ATOMIC_SEQ_CST);
}

int64_t salt_atomic_add_i64(int64_t *ptr, int64_t val) {
  return __atomic_fetch_add(ptr, val, __ATOMIC_SEQ_CST);
}

int64_t salt_atomic_cas_i64(int64_t *ptr, int64_t expected, int64_t desired) {
  __atomic_compare_exchange_n(ptr, &expected, desired, 0, __ATOMIC_SEQ_CST,
                              __ATOMIC_SEQ_CST);
  return expected;
}

// =============================================================================
// SALT RUNTIME: Process execution (posix_spawn bridge for std.process)
// =============================================================================
#include <spawn.h>
#include <sys/wait.h>

// salt_process_exec(program, arg0, arg1, ...) -> exit code
// Simple: runs a program with up to 4 args, waits, returns exit code.
int32_t salt_process_exec(const char *program, const char *arg1,
                          const char *arg2, const char *arg3) {
  pid_t pid;
  char *argv[5];
  argv[0] = (char *)program;
  argv[1] = (char *)arg1;
  argv[2] = (char *)arg2;
  argv[3] = (char *)arg3;
  argv[4] = NULL;

  // Trim NULL args
  int argc = 1;
  if (arg1)
    argc++;
  if (arg2)
    argc++;
  if (arg3)
    argc++;
  argv[argc] = NULL;

  extern char **environ;
  int status;
  int ret = posix_spawn(&pid, program, NULL, NULL, argv, environ);
  if (ret != 0)
    return -1;

  waitpid(pid, &status, 0);
  if (WIFEXITED(status))
    return WEXITSTATUS(status);
  return -1;
}

// salt_process_exec_capture: run program, capture stdout into buffer
// Returns bytes written to buffer, or -1 on error
int64_t salt_process_exec_capture(const char *program, const char *arg1,
                                  char *out_buf, int64_t buf_size) {
  // Build command string for popen — enforce strict bounds checking.
  // If the combined command exceeds the buffer, abort safely (return -1)
  // rather than executing a truncated/malformed command.
  char cmd[1024];
  int cmd_len;
  if (arg1) {
    cmd_len = snprintf(cmd, sizeof(cmd), "%s %s", program, arg1);
  } else {
    cmd_len = snprintf(cmd, sizeof(cmd), "%s", program);
  }
  if (cmd_len < 0 || cmd_len >= (int)sizeof(cmd)) {
    // Command was truncated or snprintf failed — refuse to execute
    return -1;
  }

  FILE *fp = popen(cmd, "r");
  if (!fp)
    return -1;

  int64_t total = 0;
  while (total < buf_size - 1) {
    int ch = fgetc(fp);
    if (ch == EOF)
      break;
    out_buf[total++] = (char)ch;
  }
  out_buf[total] = '\0';

  pclose(fp);
  return total;
}

// =============================================================================
// SALT RUNTIME: Filesystem Bridges (errno + directory iteration)
// =============================================================================
#include <dirent.h>
#include <errno.h>

// Salt: salt_errno() -> i32
// Returns the current thread-local errno value for Status mapping.
int32_t salt_errno() { return (int32_t)errno; }

// Salt: salt_opendir(path: Ptr<u8>) -> i64
// Returns DIR* as i64, or 0 on failure (sets errno).
int64_t salt_opendir(const char *path) {
  DIR *dir = opendir(path);
  return (int64_t)dir; // NULL (0) on failure
}

// Salt: salt_readdir(handle, name_buf, buf_cap, out_name_len, out_is_dir) ->
// i32 Reads the next directory entry, skipping "." and "..". Returns 1 if an
// entry was read, 0 if iteration is complete.
int32_t salt_readdir(int64_t handle, char *name_buf, int64_t buf_cap,
                     int64_t *out_name_len, int32_t *out_is_dir) {
  DIR *dir = (DIR *)handle;
  struct dirent *entry;

  while ((entry = readdir(dir)) != NULL) {
    // Skip "." and ".."
    if (entry->d_name[0] == '.') {
      if (entry->d_name[1] == '\0')
        continue;
      if (entry->d_name[1] == '.' && entry->d_name[2] == '\0')
        continue;
    }

    int64_t name_len = (int64_t)strlen(entry->d_name);
    if (name_len >= buf_cap)
      name_len = buf_cap - 1;

    // Manual copy — avoids conflict with Salt's _memcpy asm label override
    for (int64_t i = 0; i < name_len; i++) {
      name_buf[i] = entry->d_name[i];
    }
    name_buf[name_len] = '\0';

    *out_name_len = name_len;
    *out_is_dir = (entry->d_type == DT_DIR) ? 1 : 0;
    return 1;
  }

  *out_name_len = 0;
  *out_is_dir = 0;
  return 0; // No more entries
}

// Salt: salt_closedir(handle: i64)
void salt_closedir(int64_t handle) {
  if (handle != 0) {
    closedir((DIR *)handle);
  }
}
