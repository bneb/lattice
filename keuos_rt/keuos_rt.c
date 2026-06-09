// =============================================================================
// KeuOS Runtime Substrate — keuos_rt.c
//
// Minimal, libc-free runtime for KeuOS binaries on Darwin/arm64.
// Implements the symbols that compiler-emitted MLIR calls:
//
//   isolated_arena_alloc   — O(1) bump allocator on mmap'd region
//   isolated_arena_reset   — Reset bump pointer (epoch boundary)
//   salt_init_x19           — Prime the deadline register
//   salt_kqueue_submit      — Thin kevent() write wrapper
//   salt_kqueue_reap        — Batch kevent() read
//   salt_kqueue_teardown    — Close kqueue fd
//   _start                  — Entry point (bypasses C runtime)
//
// Build: clang -nostdlib -ffreestanding -target arm64-apple-macosx14.0.0
//        -c keuos_rt.c -o keuos_rt.o
//
// Binary footprint target: < 4KB
// =============================================================================

#include <stddef.h>
#include <stdint.h>
#include <sys/event.h>
#include <sys/mman.h>
#include <sys/time.h>
#include <sys/types.h>

// =============================================================================
// 1. The KeuOS Arena — O(1) Allocation
// =============================================================================

#define ARENA_SIZE (16ULL * 1024 * 1024 * 1024) // 16GB virtual reservation
#define ARENA_ALIGN 64                          // Cache-line alignment

static uint8_t *arena_base = 0;
static uint8_t *arena_ptr = 0;

/// Allocate `size` bytes from the arena with 64-byte alignment.
/// First call lazily initializes the arena via mmap.
void *isolated_arena_alloc(uint64_t size) {
  if (__builtin_expect(!arena_base, 0)) {
    arena_base = (uint8_t *)mmap(NULL, ARENA_SIZE, PROT_READ | PROT_WRITE,
                                 MAP_ANON | MAP_PRIVATE, -1, 0);
    // Pin initial working set to physical RAM
    // (only first 2MB — rest faults in on demand)
    mlock(arena_base, 2ULL * 1024 * 1024);
    arena_ptr = arena_base;
  }
  void *alloc = arena_ptr;
  arena_ptr += (size + (ARENA_ALIGN - 1)) & ~((uint64_t)(ARENA_ALIGN - 1));
  return alloc;
}

/// Reset arena to initial state (epoch boundary).
/// All prior allocations become invalid.
void isolated_arena_reset(void) { arena_ptr = arena_base; }

// =============================================================================
// 2. Deadline Register Initialization
// =============================================================================

/// Prime x19 with "infinite" deadline (all bits set).
/// This must be called before any KeuOS dispatch.
void salt_init_x19(void) { __asm__ volatile("mov x19, #-1" ::: "x19"); }

// =============================================================================
// 3. kqueue I/O Backend (Darwin)
// =============================================================================

static int kq_fd = -1;

/// Initialize kqueue file descriptor.
/// Returns the kqueue fd or -1 on failure.
int salt_kqueue_init(void) {
  kq_fd = kqueue();
  return kq_fd;
}

/// Submit a read interest for the given socket fd.
/// Uses EV_ADD | EV_ENABLE with the connection pointer as udata.
int salt_kqueue_submit(int sock_fd, void *conn_ptr) {
  struct kevent ev;
  EV_SET(&ev, sock_fd, EVFILT_READ, EV_ADD | EV_ENABLE, 0, 0, conn_ptr);
  return kevent(kq_fd, &ev, 1, NULL, 0, NULL);
}

/// Batch reap ready events from kqueue.
/// Returns number of events, fills `events` array.
/// `max_events` should match the io_batch_size (typically 256).
int salt_kqueue_reap(struct kevent *events, int max_events) {
  // Non-blocking poll (timeout = 0)
  struct timespec ts = {0, 0};
  return kevent(kq_fd, NULL, 0, events, max_events, &ts);
}

/// Teardown: close the kqueue fd.
void salt_kqueue_teardown(void) {
  // Darwin syscall: close(fd)
  // Using inline asm to avoid libc dependency
  register int fd __asm__("w0") = kq_fd;
  __asm__ volatile("mov x16, #6\n" // SYS_close = 6 on Darwin
                   "svc #0x80\n"
                   :
                   : "r"(fd)
                   : "x16");
  kq_fd = -1;
}

// =============================================================================
// 4. Entry Point — The KeuOS Handshake
// =============================================================================

/// External: the compiler-generated launcher function.
extern void _salt_main_launcher(void);

/// _start: replaces the C runtime's entry point.
/// Sequence: x19 init → arena warmup → launch → exit
void _start(void) {
  // 1. Prime the deadline register
  salt_init_x19();

  // 2. Launch the compiler-generated entry point
  _salt_main_launcher();

  // 3. Clean exit via syscall (no atexit, no destructors)
  __asm__ volatile("mov x0, #0\n"  // exit code 0
                   "mov x16, #1\n" // SYS_exit = 1 on Darwin
                   "svc #0x80\n");
}
