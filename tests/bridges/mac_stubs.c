#include <stdint.h>
#include <stdlib.h>
#include <unistd.h>

// Provide malloc for native/benchmark builds — Salt's extern fn malloc
// resolves to these symbols. The mangled names (_malloc_0, _malloc_1, etc.)
// come from Salt's incremental symbol suffixing.
__attribute__((weak)) void* malloc_0(uint64_t s) { return malloc((size_t)s); }
__attribute__((weak)) void* malloc_1(uint64_t s) { return malloc((size_t)s); }
__attribute__((weak)) void* malloc_2(uint64_t s) { return malloc((size_t)s); }
__attribute__((weak)) void* malloc_3(uint64_t s) { return malloc((size_t)s); }
__attribute__((weak)) void* free_0(void* p) { free(p); }
__attribute__((weak)) void* free_1(void* p) { free(p); }
__attribute__((weak)) void* free_2(void* p) { free(p); }
__attribute__((weak)) void* free_3(void* p) { free(p); }

__attribute__((weak)) void sys_mfence(void) { __sync_synchronize(); }

__attribute__((weak)) void sys_sleep_ms(uint32_t ms) { usleep(ms * 1000); }

// Stub: VFS connection (returns NULL — native builds don't use AOF persistence)
__attribute__((weak)) void* std__fs__fs__vfs_connect(void) { return 0; }

// Stub: yield (no-op on macOS — the kqueue wait handles blocking)
__attribute__((weak)) void r3_sys_yield(void) {}

__attribute__((weak)) unsigned long long
ext_hpack_get_static_key(unsigned int index) {
  return 0;
}
__attribute__((weak)) unsigned long long
ext_hpack_get_static_val(unsigned int index) {
  return 0;
}
__attribute__((weak)) void ext_ipc_send_cdm_command(uint32_t cmd, uint64_t arg1,
                                                    uint64_t p_ptr,
                                                    uint32_t p_len) {}
__attribute__((weak)) void
ext_net_route_header_to_stream(uint32_t stream, uint64_t key_ptr, uint32_t klen,
                               uint64_t val_ptr, uint32_t vlen) {}
__attribute__((weak)) uint32_t dom_get_selection_focus_offset(uint32_t node) {
  return 0;
}
__attribute__((weak)) uint32_t dom_get_selection_anchor_offset(uint32_t node) {
  return 0;
}
__attribute__((weak)) uint32_t dom_get_canvas_surface_id(uint32_t node) {
  return 0;
}
__attribute__((weak)) uint32_t dom_get_selection_anchor_node() { return 0; }
__attribute__((weak)) uint32_t dom_get_selection_focus_node() { return 0; }

__attribute__((weak)) void sys_print_str(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) uint64_t sys_time_now_ms_int(void) { return 0; }

// C-ABI trampoline: Salt cannot call @no_mangle'd flush_frame cross-module
// (see main_bridge.c:122 for the reference implementation)
__attribute__((weak)) void flush_frame(int32_t width, int32_t height) {}
__attribute__((weak)) void ext_flush_frame(int32_t width, int32_t height) {
  flush_frame(width, height);
}

// Missing Symbols Fix (Epic 108)
__attribute__((weak)) uint32_t hash_string(uint64_t ptr, uint32_t len) {
  uint32_t hash = 2166136261U;
  const uint8_t *data = (const uint8_t *)ptr;
  for (uint32_t i = 0; i < len; i++) {
    hash ^= data[i];
    hash *= 16777619U;
  }
  return hash;
}

__attribute__((weak)) void css_arena_inc_count(void) {}
__attribute__((weak)) void css_arena_set_hash(uint32_t slot, uint32_t hash) {}
__attribute__((weak)) void ext_engine_process_mouse_down(float x, float y) {}

__attribute__((weak)) uint64_t keuos_arena_alloc(uint64_t size) {
  return (uint64_t)calloc(1, size);
}

__attribute__((weak)) int64_t ebr_get_global_epoch() { return 0; }
__attribute__((weak)) int64_t ebr_get_core_epoch(int64_t core_id) { return 0; }
__attribute__((weak)) int64_t ebr_get_core_in_epoch(int64_t core_id) { return 0; }
__attribute__((weak)) uint64_t get_ecs_world_ptr() { return 0; }
__attribute__((weak)) int32_t js_quickjs_init() { return 0; }
__attribute__((weak)) void js_quickjs_teardown() {}
