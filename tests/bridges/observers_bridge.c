// Epic 79 Red Team Observer Test Bridge
// Provides stubs and aliases for the headless test harness.
#include <stdint.h>
#include <stdio.h>
#include <string.h>

// Stubs for symbols from other modules in the transitive dependency chain
uint32_t user__browser__paint__Z_SORT_BUF[8192] = {0};
void user__browser__ipc_shared__sys_ipc_send_r2m_command_with_payload(
    uint32_t cmd, uint64_t ptr, uint32_t len) {}
uint32_t user__browser__media__MEDIA_HEAD = 0;
uint32_t user__browser__media__MEDIA_TAIL = 0;
void sys_hw_audio_init(void) {}


// ============================================================================
// 1. Observer symbol aliases
//    @no_mangle emits e.g. ext_observers_register, but Salt's import mechanism
//    generates calls to user__browser__observers__ext_observers_register.
//    These aliases bridge the gap.
// ============================================================================
extern uint8_t ext_observers_register(uint32_t, uint64_t, uint8_t);
extern void ext_observers_unregister(uint32_t, uint64_t, uint8_t);
extern uint64_t sys_get_callback_for_node(uint32_t, uint8_t);
extern void sys_observers_clear_queues(void);

uint8_t user__browser__observers__ext_observers_register(uint32_t a, uint64_t b, uint8_t c) {
    return ext_observers_register(a, b, c);
}
void user__browser__observers__ext_observers_unregister(uint32_t a, uint64_t b, uint8_t c) {
    ext_observers_unregister(a, b, c);
}
uint64_t user__browser__observers__sys_get_callback_for_node(uint32_t a, uint8_t b) {
    return sys_get_callback_for_node(a, b);
}
void user__browser__observers__sys_observers_clear_queues(void) {
    sys_observers_clear_queues();
}

// ============================================================================
// 2. Pending queue globals (accessed by jsc_bridge.m via extern)
//    The Salt compiler emits these as unmangled names via @no_mangle on the
//    flush path, but some transitive references use mangled prefixes.
// ============================================================================

// ============================================================================
// 3. DOM stubs — transitive references from paint.salt, layout.salt, main.salt
// ============================================================================
uint32_t user__browser__dom__EVICTION_QUEUE[1024] = {0};
int32_t user__browser__dom__LAYOUT_SCROLL_X = 0;

uint32_t user__browser__dom__compare_document_position(uint32_t a, uint32_t b) { return 0; }
uint32_t user__browser__dom__dom_find_iframe_slot(uint32_t a) { return 0; }
uint32_t user__browser__dom__dom_get_selection_anchor_node(void) { return 0; }
uint32_t user__browser__dom__dom_get_selection_anchor_offset(void) { return 0; }
uint32_t user__browser__dom__dom_get_selection_focus_node(void) { return 0; }
uint32_t user__browser__dom__dom_get_selection_focus_offset(void) { return 0; }
void user__browser__dom__invalidate_layout(uint64_t idx) { /* no-op in test */ }

// ============================================================================
// 4. GPU / IOSurface / Compositor stubs
// ============================================================================
void sys_gpu_commit_iosurface(void) {}
void sys_gpu_init_iosurface(uint32_t w, uint32_t h) {}
uint32_t sys_gpu_is_iosurface_mode(void) { return 0; }
void sys_gpu_rasterize_iosurface(uint64_t p, uint32_t n) {}
void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}
void sys_invalidate_paint(void) {}

// IOSurface symbols — provided by IOSurface.framework in real builds
// but not linked in headless tests.
// We only stub the constants; functions shouldn't be called.
const void* kIOSurfaceBytesPerElement = NULL;
const void* kIOSurfaceHeight = NULL;
const void* kIOSurfacePixelFormat = NULL;
const void* kIOSurfaceWidth = NULL;
void* IOSurfaceCreate(void* p) { return NULL; }
void* IOSurfaceGetBaseAddress(void* s) { return NULL; }
uint64_t IOSurfaceGetBytesPerRow(void* s) { return 0; }
uint64_t IOSurfaceGetHeight(void* s) { return 0; }
uint32_t IOSurfaceGetID(void* s) { return 0; }
uint64_t IOSurfaceGetWidth(void* s) { return 0; }
int IOSurfaceLock(void* s, uint32_t o, uint32_t* seed) { return 0; }
void* IOSurfaceLookup(uint32_t id) { return NULL; }
int IOSurfaceUnlock(void* s, uint32_t o, uint32_t* seed) { return 0; }

// ============================================================================
// 5. JS bridge stubs — called from main.salt run_loop, worker, net
// ============================================================================
void js_bridge_dispatch_document_event(uint32_t a, uint64_t b) {}
void js_bridge_dispatch_main_message(uint64_t a, uint32_t b) {}
void js_bridge_dispatch_message_event(uint64_t a, uint32_t b) {}
void js_bridge_dispatch_websocket_message(uint32_t a, uint64_t b, uint32_t c) {}
void js_bridge_dispatch_worker_message(uint64_t a, uint32_t b) {}
void js_bridge_resolve_idb_promise(uint32_t a, uint64_t b, uint32_t c) {}
void js_execute_worker_jobs(void) {}
void js_resolve_fetch_chunk(uint32_t a, uint64_t b, uint32_t c) {}
void js_resolve_fetch_impl(uint32_t a, uint32_t b, uint64_t c, uint32_t d) {}
void sys_js_dispatch_popstate(void) {}
void sys_js_evaluate_script(uint64_t a, uint32_t b, uint64_t c) {}
void sys_on_mouse_click(int32_t x, int32_t y) {}

// ============================================================================
// 6. Timer stubs
// ============================================================================
uint32_t ext_timers_add_raf(void) { return 0; }
uint32_t ext_timers_add_timeout(uint32_t a, uint8_t b) { return 0; }

// ============================================================================
// 7. Media / TLS / Misc stubs
// ============================================================================
uint32_t ext_get_media_head(void) { return 0; }
uint32_t ext_get_media_tail(void) { return 0; }
void ext_set_media_head(uint32_t v) {}
void ext_set_media_tail(uint32_t v) {}
void ext_mac_update_omnibox(uint64_t a, uint32_t b) {}
void ext_tls_write_bytes(uint32_t a, uint64_t b, uint32_t c) {}
void init_arrays(void) {}
void decode_hpack_block(uint64_t a, uint32_t b) {}
void ext_hpack_encode_headers(uint32_t a) {}
uint64_t ext_hpack_get_buffer_ptr(void) { return 0; }

// ============================================================================
// 8. System stubs
// ============================================================================
uint64_t sys_clock_get_ms(void) { return 0; }
void sys_memcpy(uint64_t dst, uint64_t src, uint32_t len) {
    memcpy((void*)(uintptr_t)dst, (void*)(uintptr_t)src, len);
}
uint64_t sys_mmap_file(uint64_t path_ptr, uint32_t path_len, uint64_t out_size) { return 0; }
