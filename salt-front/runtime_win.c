// =============================================================================
// SALT RUNTIME — Windows (x86_64)
// =============================================================================
// Win32 equivalents for all POSIX functions used by the Salt standard library.
// Linked in place of runtime.c when targeting --target windows.
//
// Inclusions minimised to kernel32 + ucrt. No external dependencies.
// =============================================================================

#ifndef _WIN32
#error "This file is only for Windows targets. Use runtime.c for POSIX."
#endif

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <process.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <io.h>
#include <share.h>
#include <errno.h>

// =============================================================================
// Environment access (argc/argv/getenv)
// =============================================================================

int32_t salt_get_argc(void) {
    return (int32_t)__argc;
}

const char *salt_get_argv(int32_t idx) {
    if (idx < 0 || idx >= __argc) return NULL;
    return __argv[idx];
}

int64_t salt_get_argv_len(int32_t idx) {
    if (idx < 0 || idx >= __argc) return 0;
    return (int64_t)strlen(__argv[idx]);
}

const char *salt_getenv(const char *name) { return getenv(name); }

int64_t salt_strlen(const char *s) {
    if (!s) return 0;
    return (int64_t)strlen(s);
}

// =============================================================================
// File I/O — Win32 CRT equivalents of POSIX open/write/read/close
// =============================================================================

int64_t sys_open(const char *path, int64_t flags, int64_t mode) {
    return _open(path, (int)flags, (int)mode);
}

int64_t sys_write(int32_t fd, const void *buf, int64_t len) {
    return (int64_t)_write(fd, buf, (unsigned int)len);
}

int64_t sys_read(int32_t fd, void *buf, int64_t len) {
    return (int64_t)_read(fd, buf, (unsigned int)len);
}

int32_t sys_close(int32_t fd) { return (int32_t)_close(fd); }

// =============================================================================
// Memory management — VirtualAlloc instead of mmap
// =============================================================================

int64_t sys_mmap(int64_t addr, int64_t len, int64_t prot, int64_t flags,
                 int64_t fd, int64_t offset) {
    (void)addr; (void)fd; (void)offset;
    DWORD flProtect = PAGE_READWRITE;
    DWORD flAlloc = MEM_RESERVE | MEM_COMMIT;
    if (flags & 0x800) flAlloc |= MEM_LARGE_PAGES;
    void *result = VirtualAlloc(NULL, (SIZE_T)len, flAlloc, flProtect);
    return (int64_t)result;
}

int32_t sys_munmap(int64_t addr, int64_t length) {
    if (!VirtualFree((void *)(uintptr_t)addr, 0, MEM_RELEASE)) return -1;
    return 0;
}

int64_t salt_mmap(int64_t size) {
    void *result = VirtualAlloc(NULL, (SIZE_T)size, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE);
    return result ? (int64_t)result : 0;
}

int64_t salt_munmap(int64_t addr, int64_t size) {
    (void)size;
    return VirtualFree((void *)addr, 0, MEM_RELEASE) ? 0 : -1;
}

// =============================================================================
// memcpy — identical to POSIX version (no OS dependency)
// =============================================================================

int64_t salt_memcpy_impl(int64_t dst, int64_t src,
                         int64_t len) __asm__("_memcpy");
int64_t salt_memcpy_impl(int64_t dst, int64_t src, int64_t len) {
    unsigned char *d = (unsigned char *)(uintptr_t)dst;
    const unsigned char *s = (const unsigned char *)(uintptr_t)src;
    if (len <= 0) return dst;
    if (d > s && (size_t)(d - s) < (size_t)len) {
        for (int64_t i = len - 1; i >= 0; i--) d[i] = s[i];
    } else {
        for (int64_t i = 0; i < len; i++) d[i] = s[i];
    }
    return dst;
}

// =============================================================================
// Arena allocator — VirtualAlloc-backed bump allocator
// =============================================================================

#define CHUNK_SIZE (2 * 1024 * 1024)

typedef struct ArenaChunk {
    struct ArenaChunk* next;
    int64_t base;
    int64_t current;
    int64_t end;
} ArenaChunk;

static __declspec(thread) ArenaChunk* tl_head_chunk = NULL;
static __declspec(thread) ArenaChunk* tl_current_chunk = NULL;
static __declspec(thread) ArenaChunk* tl_free_chunks = NULL;

static ArenaChunk* alloc_chunk(int64_t required_size) {
    int64_t alloc_size = CHUNK_SIZE;
    if (required_size + sizeof(ArenaChunk) > alloc_size)
        alloc_size = required_size + sizeof(ArenaChunk);
    if (alloc_size == CHUNK_SIZE && tl_free_chunks) {
        ArenaChunk* chunk = tl_free_chunks;
        tl_free_chunks = chunk->next;
        chunk->next = NULL;
        chunk->current = chunk->base;
        return chunk;
    }
    void* result = VirtualAlloc(NULL, alloc_size, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE);
    if (!result) return NULL;
    ArenaChunk* chunk = (ArenaChunk*)result;
    chunk->base = (int64_t)result + sizeof(ArenaChunk);
    chunk->end = (int64_t)result + alloc_size;
    chunk->current = chunk->base;
    chunk->next = NULL;
    return chunk;
}

int64_t salt_arena_alloc(int64_t size) {
    int64_t aligned_size = (size + 7) & ~7;
    if (!tl_current_chunk) {
        tl_head_chunk = alloc_chunk(aligned_size);
        tl_current_chunk = tl_head_chunk;
        if (!tl_current_chunk) return 0;
    }
    int64_t aligned_addr = (tl_current_chunk->current + 7) & ~7;
    int64_t next = aligned_addr + aligned_size;
    if (next > tl_current_chunk->end) {
        ArenaChunk* new_chunk = alloc_chunk(aligned_size);
        if (!new_chunk) return 0;
        tl_current_chunk->next = new_chunk;
        tl_current_chunk = new_chunk;
        aligned_addr = (tl_current_chunk->current + 7) & ~7;
        tl_current_chunk->current = aligned_addr + aligned_size;
        return aligned_addr;
    }
    tl_current_chunk->current = next;
    return aligned_addr;
}

int64_t salt_arena_mark(void) {
    if (!tl_current_chunk) return 0;
    return tl_current_chunk->current;
}

void salt_arena_reset_to(int64_t mark) {
    if (!tl_head_chunk) return;
    ArenaChunk* curr = tl_head_chunk;
    int found = 0;
    while (curr) {
        if (mark >= curr->base && mark <= curr->end) { found = 1; break; }
        curr = curr->next;
    }
    if (!found) {
        if (mark == 0) curr = NULL;
        else return;
    }
    ArenaChunk* to_free = curr ? curr->next : tl_head_chunk;
    if (curr) {
        curr->next = NULL;
        curr->current = mark;
        tl_current_chunk = curr;
    } else {
        tl_head_chunk = NULL;
        tl_current_chunk = NULL;
    }
    while (to_free) {
        ArenaChunk* nf = to_free->next;
        int64_t cs = to_free->end - (to_free->base - sizeof(ArenaChunk));
        if (cs == CHUNK_SIZE) {
            to_free->next = tl_free_chunks;
            tl_free_chunks = to_free;
        } else {
            VirtualFree(to_free, 0, MEM_RELEASE);
        }
        to_free = nf;
    }
}

// =============================================================================
// High-resolution clock — QueryPerformanceCounter
// =============================================================================

int64_t salt_clock_now(void) {
    static LARGE_INTEGER freq;
    static int initialized = 0;
    if (!initialized) {
        QueryPerformanceFrequency(&freq);
        initialized = 1;
    }
    LARGE_INTEGER counter;
    QueryPerformanceCounter(&counter);
    return (int64_t)((counter.QuadPart * 1000000000LL) / freq.QuadPart);
}

int64_t rdtsc(void) { return salt_clock_now(); }

// =============================================================================
// syscall6 — Windows doesn't have syscall; stub for KeuOS compatibility
// =============================================================================

int64_t syscall6(int64_t number, int64_t arg1, int64_t arg2, int64_t arg3,
                 int64_t arg4, int64_t arg5, int64_t arg6) {
    (void)number; (void)arg1; (void)arg2;
    (void)arg3; (void)arg4; (void)arg5; (void)arg6;
    return 0;
}

void ___salt_yield_check(void) {}
void __salt_yield_check(void) {}

// =============================================================================
// System allocator — _aligned_malloc / _aligned_free
// =============================================================================

void *salt_sys_alloc(int64_t size, int64_t align) {
    if (size <= 0) return NULL;
    size_t a = (size_t)align;
    if (a < sizeof(void*)) a = sizeof(void*);
    void *ptr = _aligned_malloc((size_t)size, a);
    if (!ptr) fprintf(stderr, "FATAL: Salt runtime failed to allocate %lld bytes\n", (long long)size);
    return ptr;
}

void salt_sys_dealloc(void *ptr, int64_t size, int64_t align) {
    (void)size; (void)align;
    _aligned_free(ptr);
}

void *simple_alloc(int64_t size) { return salt_sys_alloc(size, 8); }
void simple_dealloc(void *ptr, int64_t size) { salt_sys_dealloc(ptr, size, 8); }

void *salt_sys_realloc(void *ptr, int64_t new_size) {
    return realloc(ptr, (size_t)new_size);
}

void salt_sys_free(void *ptr) { free(ptr); }

// =============================================================================
// Debug, contract, and panic handlers (no OS dependency)
// =============================================================================

void debug_print_ptr(void *p, const char *label) {
    printf("[SALT-DEBUG] %s: %p\n", label, p);
}

void __salt_contract_violation(void) {
    fprintf(stderr, "FATAL: Salt contract violation (requires/ensures/invariant)\n");
    abort();
}

void __salt_overflow_panic(void) {
    fprintf(stderr, "FATAL: Salt integer overflow detected\n");
    abort();
}

void __salt_panic(const char *message) {
    fprintf(stderr, "CRITICAL: Salt Runtime Panic: %s\n", message);
    __debugbreak();
}

void printf_shim(const char *fmt, int64_t val) { printf(fmt, val); }

// =============================================================================
// Print hooks (no OS dependency)
// =============================================================================

void __salt_print_literal(const char *str, int64_t len) {
    fwrite(str, 1, (size_t)len, stdout);
}
void __salt_print_i64(int64_t val)   { printf("%lld", val); }
void __salt_print_u64(int64_t val)   { printf("%llu", (unsigned long long)val); }
void __salt_print_f64(double val)    { printf("%g", val); }
void __salt_print_bool(int8_t val)   { printf("%s", val ? "true" : "false"); }
void __salt_print_ptr(int64_t val)   { printf("0x%llx", (unsigned long long)val); }

int64_t __salt_fmt_f64_to_buf(char *buf, double val, int64_t precision) {
    char fmt[8];
    snprintf(fmt, sizeof(fmt), "%%.%lldf", (long long)precision);
    int written = snprintf(buf, 64, fmt, val);
    return (int64_t)(written > 0 ? written : 0);
}

// =============================================================================
// Threading — CreateThread / WaitForSingleObject
// =============================================================================

typedef void (*salt_thread_fn)(void);

struct salt_thread_trampoline_ctx {
    salt_thread_fn fn;
};

static DWORD WINAPI salt_thread_trampoline(void *arg) {
    struct salt_thread_trampoline_ctx *ctx = (struct salt_thread_trampoline_ctx *)arg;
    salt_thread_fn fn = ctx->fn;
    free(ctx);
    fn();
    return 0;
}

int64_t salt_thread_spawn(int64_t fn_addr) {
    struct salt_thread_trampoline_ctx *ctx = malloc(sizeof(*ctx));
    if (!ctx) return 0;
    ctx->fn = (salt_thread_fn)fn_addr;
    HANDLE h = CreateThread(NULL, 0, salt_thread_trampoline, ctx, 0, NULL);
    if (!h) { free(ctx); return 0; }
    CloseHandle(h); // detach — join uses the handle we return
    // Re-open as a joinable handle
    HANDLE h2 = CreateThread(NULL, 0, salt_thread_trampoline, ctx, 0, NULL);
    if (!h2) { free(ctx); return 0; }
    return (int64_t)h2;
}

int32_t salt_thread_join(int64_t handle) {
    DWORD rc = WaitForSingleObject((HANDLE)handle, INFINITE);
    CloseHandle((HANDLE)handle);
    return (rc == WAIT_OBJECT_0) ? 0 : -1;
}

// =============================================================================
// Synchronization — SRWLOCK (slim reader/writer lock as mutex)
// =============================================================================

int64_t salt_mutex_create(void) {
    SRWLOCK *lock = (SRWLOCK *)malloc(sizeof(SRWLOCK));
    if (!lock) return 0;
    InitializeSRWLock(lock);
    return (int64_t)lock;
}

void salt_mutex_lock(int64_t handle)   { AcquireSRWLockExclusive((SRWLOCK *)handle); }
void salt_mutex_unlock(int64_t handle) { ReleaseSRWLockExclusive((SRWLOCK *)handle); }
void salt_mutex_destroy(int64_t handle) { free((void *)handle); }

// =============================================================================
// Atomics — same __atomic builtins (Clang on Windows supports these)
// =============================================================================

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
    __atomic_compare_exchange_n(ptr, &expected, desired, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST);
    return expected;
}

// =============================================================================
// Process execution — CreateProcess (simplified)
// =============================================================================

int32_t salt_process_exec(const char *program, const char *arg1,
                          const char *arg2, const char *arg3) {
    char cmd[1024];
    int len = snprintf(cmd, sizeof(cmd), "\"%s\"", program);
    if (arg1) len += snprintf(cmd + len, sizeof(cmd) - len, " \"%s\"", arg1);
    if (arg2) len += snprintf(cmd + len, sizeof(cmd) - len, " \"%s\"", arg2);
    if (arg3) len += snprintf(cmd + len, sizeof(cmd) - len, " \"%s\"", arg3);
    if (len < 0 || len >= (int)sizeof(cmd)) return -1;

    STARTUPINFOA si = { sizeof(si) };
    PROCESS_INFORMATION pi = { 0 };
    si.dwFlags = STARTF_USESTDHANDLES;
    if (!CreateProcessA(NULL, cmd, NULL, NULL, FALSE,
                        CREATE_NO_WINDOW, NULL, NULL, &si, &pi))
        return -1;
    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD exit_code;
    GetExitCodeProcess(pi.hProcess, &exit_code);
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
    return (int32_t)exit_code;
}

int64_t salt_process_exec_capture(const char *program, const char *arg1,
                                  char *out_buf, int64_t buf_size) {
    char cmd[1024];
    int cmd_len;
    if (arg1) cmd_len = snprintf(cmd, sizeof(cmd), "\"%s\" \"%s\"", program, arg1);
    else cmd_len = snprintf(cmd, sizeof(cmd), "\"%s\"", program);
    if (cmd_len < 0 || cmd_len >= (int)sizeof(cmd)) return -1;

    HANDLE hRead, hWrite;
    SECURITY_ATTRIBUTES sa = { sizeof(sa), NULL, TRUE };
    if (!CreatePipe(&hRead, &hWrite, &sa, 0)) return -1;

    STARTUPINFOA si = { sizeof(si) };
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = hWrite;
    si.hStdError = hWrite;
    PROCESS_INFORMATION pi = { 0 };
    if (!CreateProcessA(NULL, cmd, NULL, NULL, TRUE,
                        CREATE_NO_WINDOW, NULL, NULL, &si, &pi)) {
        CloseHandle(hRead); CloseHandle(hWrite);
        return -1;
    }
    CloseHandle(hWrite);
    int64_t total = 0;
    DWORD n;
    while (total < buf_size - 1 && ReadFile(hRead, out_buf + total, (DWORD)(buf_size - 1 - total), &n, NULL) && n > 0)
        total += n;
    out_buf[total] = '\0';
    CloseHandle(hRead);
    WaitForSingleObject(pi.hProcess, INFINITE);
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
    return total;
}

// =============================================================================
// Filesystem bridges — errno + FindFirstFile/FindNextFile
// =============================================================================

int32_t salt_errno(void) { return (int32_t)errno; }

typedef struct {
    HANDLE handle;
    WIN32_FIND_DATAA data;
    int first;
} salt_dir_t;

int64_t salt_opendir(const char *path) {
    char pattern[MAX_PATH];
    snprintf(pattern, sizeof(pattern), "%s\\*", path);
    salt_dir_t *dir = (salt_dir_t *)malloc(sizeof(salt_dir_t));
    if (!dir) return 0;
    dir->handle = FindFirstFileA(pattern, &dir->data);
    if (dir->handle == INVALID_HANDLE_VALUE) {
        free(dir);
        return 0;
    }
    dir->first = 1;
    return (int64_t)dir;
}

int32_t salt_readdir(int64_t handle, char *name_buf, int64_t buf_cap,
                     int64_t *out_name_len, int32_t *out_is_dir) {
    salt_dir_t *dir = (salt_dir_t *)handle;
    WIN32_FIND_DATAA *entry;
    while (1) {
        if (!dir->first) {
            if (!FindNextFileA(dir->handle, &dir->data)) goto done;
        }
        dir->first = 0;
        entry = &dir->data;
        if (entry->cFileName[0] == '.' && entry->cFileName[1] == '\0') continue;
        if (entry->cFileName[0] == '.' && entry->cFileName[1] == '.' && entry->cFileName[2] == '\0') continue;
        int64_t name_len = (int64_t)strlen(entry->cFileName);
        if (name_len >= buf_cap) name_len = buf_cap - 1;
        for (int64_t i = 0; i < name_len; i++) name_buf[i] = entry->cFileName[i];
        name_buf[name_len] = '\0';
        *out_name_len = name_len;
        *out_is_dir = (entry->dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) ? 1 : 0;
        return 1;
    }
done:
    *out_name_len = 0;
    *out_is_dir = 0;
    return 0;
}

void salt_closedir(int64_t handle) {
    if (!handle) return;
    salt_dir_t *dir = (salt_dir_t *)handle;
    FindClose(dir->handle);
    free(dir);
}

// =============================================================================
// WASM stubs (native-mode no-ops — identical to POSIX)
// =============================================================================

int64_t salt_get_prompt_count(void) { return 0; }
int64_t salt_get_prompt_token(int64_t idx) { (void)idx; return 0; }

static void *__basalt_engine_ptr = NULL;
static int64_t __basalt_pos = 0;
static int64_t __basalt_token = 1;
static int32_t __basalt_initialized = 0;

void wasm_set_engine_ptr(void *ptr)         { __basalt_engine_ptr = ptr; }
void *wasm_get_engine_ptr(void)             { return __basalt_engine_ptr; }
void wasm_set_pos(int64_t pos)              { __basalt_pos = pos; }
int64_t wasm_get_pos(void)                  { return __basalt_pos; }
void wasm_set_token(int64_t tok)            { __basalt_token = tok; }
int64_t wasm_get_token(void)                { return __basalt_token; }
void wasm_set_initialized(int32_t v)        { __basalt_initialized = v; }
int32_t wasm_get_initialized(void)          { return __basalt_initialized; }
