#include <stdint.h>
#include <stdio.h>
#include <string.h>

// Epic 82: Stub missing symbols for E2E interaction test because we link 
// main.salt but don't include the MacOS backend bridges.

uint64_t sys_mmap_file(uint64_t filename_ptr, uint32_t size) { return 0; }
void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}
void sys_memset(uint64_t dst, uint8_t val, uint32_t size) { memset((void*)dst, val, size); }
void sys_memcpy(uint64_t dst, uint64_t src, uint32_t size) { memcpy((void*)dst, (void*)src, size); }
void sys_hw_audio_init() {}
uint32_t sys_clock_get_ms() { return 0; }

// Dummy Globals
uint32_t user__browser__media__MEDIA_HEAD = 0;
uint32_t user__browser__media__MEDIA_TAIL = 0;
uint8_t user__browser__paint__Z_SORT_BUF[65536];

// Unmangled to Mangled wrappers because @no_mangle was leaked/exported from Salt tests
extern int32_t compare_document_position(uint32_t n1, uint32_t n2);
int32_t user__browser__dom__compare_document_position(uint32_t n1, uint32_t n2) { return compare_document_position(n1, n2); }

extern uint32_t dom_find_iframe_slot(uint32_t n);
uint32_t user__browser__dom__dom_find_iframe_slot(uint32_t n) { return dom_find_iframe_slot(n); }

extern void invalidate_layout(uint32_t n);
void user__browser__dom__invalidate_layout(uint32_t n) { invalidate_layout(n); }

extern void enqueue_click(int32_t x, int32_t y);
void user__browser__events__enqueue_click(int32_t x, int32_t y) { enqueue_click(x, y); }

extern void sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t a1, uint64_t ptr, uint32_t len);
void user__browser__ipc_shared__sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t a1, uint64_t ptr, uint32_t len) {
    sys_ipc_send_r2m_command_with_payload(cmd, a1, ptr, len);
}

// Media / Canvas / History Stubs
void sys_gpu_init_iosurface() {}
void sys_gpu_commit_iosurface() {}
void sys_gpu_rasterize_iosurface(uint64_t p, uint64_t s, uint32_t w, uint32_t h) {}
uint32_t sys_gpu_is_iosurface_mode() { return 0; }
void sys_invalidate_paint() {}
void sys_js_dispatch_popstate(uint64_t u, uint32_t ul, uint64_t p, uint32_t pl) {}
void sys_js_evaluate_script(uint64_t c, uint32_t cl, uint64_t f, uint32_t fl) {}

// Missing Globals dropped by LLVM
uint32_t user__browser__dom__EVICTION_QUEUE[1024];
int32_t user__browser__dom__LAYOUT_SCROLL_X = 0;
uint8_t user__browser__dom__STYLE_POINTER_EVENTS[65536];

// Initialization and System Stubs
void init_arrays() {}
int32_t ext_fs_open(uint64_t path_ptr, uint32_t path_len, uint32_t flags) { return -1; }
int32_t ext_tls_read_bytes(int32_t ssl_idx, uint64_t out_buf, uint32_t count) { return -1; }
int32_t ext_tls_write_bytes(int32_t ssl_idx, uint64_t in_buf, uint32_t count) { return -1; }

// JS Bridge QuickJS legacy stubs
void js_bridge_dispatch_document_event(uint64_t t, uint32_t l) {}
void js_bridge_dispatch_main_message(uint64_t t, uint32_t l) {}
void js_bridge_dispatch_message_event(uint64_t t, uint32_t l) {}
void js_bridge_dispatch_websocket_message(uint64_t t, uint32_t l) {}
void js_bridge_dispatch_worker_message(uint64_t t, uint32_t l) {}
void js_bridge_resolve_idb_promise(uint32_t a, uint64_t b, uint32_t c) {}
void js_execute_worker_jobs() {}
void js_resolve_fetch_chunk(uint64_t a, uint64_t b, uint32_t c) {}
__attribute__((weak)) void js_resolve_fetch_impl(uint64_t a, uint64_t b, uint32_t c) {}
void ext_set_media_head(uint32_t v) {}
void ext_set_media_tail(uint32_t v) {}
uint32_t ext_get_media_head() { return 0; }
uint32_t ext_get_media_tail() { return 0; }
void ext_hpack_encode_headers(uint64_t a, uint32_t b) {}
uint64_t ext_hpack_get_buffer_ptr() { return 0; }
void ext_mac_update_omnibox(uint64_t p, uint32_t pl) {}
uint32_t ext_timers_add_raf(uint64_t cb) { return 0; }
uint32_t ext_timers_add_timeout(uint64_t cb, uint32_t t) { return 0; }
void decode_hpack_block(uint64_t a, uint32_t b, uint64_t c) {}


