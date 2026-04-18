#include <stdio.h>
#include <stdint.h>
#include <string.h>

// ============================================================================
// JIT Matrix E2E Test Bridge
// Provides stub implementations for all extern symbols required by the
// full browser engine dependency graph when building in test mode.
// ============================================================================

// --- IPC Ring dummy buffer ---
extern uint64_t user__os__ipc_ring__IPC_BUFFER_PTR;
static uint8_t dummy_ipc_ring[65536];

// --- GPU Stubs ---
void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}
void sys_gpu_commit_iosurface(uint32_t id) {}
void sys_gpu_init_iosurface(uint32_t w, uint32_t h) {}
int sys_gpu_is_iosurface_mode(void) { return 0; }
void sys_gpu_rasterize_iosurface(void) {}

// --- Memory Stubs ---
uint64_t sys_mmap_file(uint64_t filename_ptr, uint32_t size) { return 0; }
void sys_memcpy(uint64_t dst, uint64_t src, uint64_t len) {
    memcpy((void*)(uintptr_t)dst, (void*)(uintptr_t)src, (size_t)len);
}

// --- Audio Stubs ---
void sys_hw_audio_init(void) {}

// --- Mouse/Input Stubs ---
void sys_on_mouse_click(int32_t x, int32_t y) {}

// --- TLS Stubs ---
void ext_tls_write_bytes(uint64_t ptr, uint32_t len) {}

// --- HPACK Stubs ---
void decode_hpack_block(uint64_t ptr, uint32_t len, uint64_t out_ptr) {}
void ext_hpack_encode_headers(uint64_t ptr, uint32_t len) {}
uint64_t ext_hpack_get_buffer_ptr(void) { return 0; }

// --- Omnibox Stubs ---
void ext_mac_update_omnibox(uint64_t ptr, uint32_t len) {}

// --- Timer Stubs ---
uint32_t ext_timers_add_timeout(uint32_t delay, uint8_t is_interval) { return 0; }
void ext_timers_remove_timeout(uint32_t id) {}
uint32_t ext_timers_add_raf(void) { return 0; }

// --- Clock Stubs ---
uint64_t sys_clock_get_ms(void) { return 0; }

// --- Paint Stubs ---
void sys_invalidate_paint(void) {}

// --- Script/JS Bridge Stubs ---
void sys_js_evaluate_script(uint64_t code_ptr, uint32_t code_len, uint64_t filename_ptr, uint32_t filename_len) {}
void sys_js_dispatch_popstate(void) {}
void js_bridge_dispatch_document_event(const char *type_ptr, uint32_t type_len) {}
void js_bridge_dispatch_main_message(uint64_t ptr, uint32_t len) {}
void js_bridge_dispatch_message_event(uint64_t ptr, uint32_t len) {}
void js_bridge_dispatch_websocket_message(uint32_t ws_id, uint64_t ptr, uint32_t len) {}
void js_bridge_dispatch_worker_message(uint64_t ptr, uint32_t len) {}
void js_bridge_resolve_idb_promise(uint32_t id, uint64_t ptr, uint32_t len) {}
void js_execute_worker_jobs(void) {}
void js_resolve_fetch_chunk(uint64_t fetch_id, uint64_t ptr, uint32_t len) {}
void init_arrays(void) {}

// --- IOSurface Framework Stubs (not needed in test) ---
// These are real framework symbols, we stub them to avoid linking IOSurface.framework
void* IOSurfaceCreate(void* props) { return NULL; }
void* IOSurfaceGetBaseAddress(void* surface) { return NULL; }
size_t IOSurfaceGetBytesPerRow(void* surface) { return 0; }
size_t IOSurfaceGetHeight(void* surface) { return 0; }
uint32_t IOSurfaceGetID(void* surface) { return 0; }
size_t IOSurfaceGetWidth(void* surface) { return 0; }
int IOSurfaceLock(void* surface, uint32_t options, uint32_t* seed) { return 0; }
void* IOSurfaceLookup(uint32_t csid) { return NULL; }
int IOSurfaceUnlock(void* surface, uint32_t options, uint32_t* seed) { return 0; }
// IOSurface key stubs (these are CFStringRef globals)
void* kIOSurfaceBytesPerElement = NULL;
void* kIOSurfaceHeight = NULL;
void* kIOSurfacePixelFormat = NULL;
void* kIOSurfaceWidth = NULL;

// --- Salt-manespaced globals (replicated for linkage) ---
// These are globals defined in Salt modules that the linker can't find
// because they use module-qualified names. We define them here as weak symbols.
uint64_t user__browser__dom__EVICTION_QUEUE[4096] __attribute__((weak)) = {0};
int32_t user__browser__dom__LAYOUT_SCROLL_X[65536] __attribute__((weak)) = {0};
uint32_t user__browser__paint__Z_SORT_BUF[65536] __attribute__((weak)) = {0};
uint64_t user__browser__media__MEDIA_HEAD __attribute__((weak)) = 0;
uint64_t user__browser__media__MEDIA_TAIL __attribute__((weak)) = 0;

// --- Salt-namespaced function stubs ---
uint32_t user__browser__dom__compare_document_position(uint32_t n1, uint32_t n2) __attribute__((weak));
uint32_t user__browser__dom__compare_document_position(uint32_t n1, uint32_t n2) { return 0; }

uint32_t user__browser__dom__dom_find_iframe_slot(uint32_t idx) __attribute__((weak));
uint32_t user__browser__dom__dom_find_iframe_slot(uint32_t idx) { return 0; }

uint32_t user__browser__dom__dom_get_selection_anchor_node(void) __attribute__((weak));
uint32_t user__browser__dom__dom_get_selection_anchor_node(void) { return 0; }

uint32_t user__browser__dom__dom_get_selection_anchor_offset(void) __attribute__((weak));
uint32_t user__browser__dom__dom_get_selection_anchor_offset(void) { return 0; }

uint32_t user__browser__dom__dom_get_selection_focus_node(void) __attribute__((weak));
uint32_t user__browser__dom__dom_get_selection_focus_node(void) { return 0; }

uint32_t user__browser__dom__dom_get_selection_focus_offset(void) __attribute__((weak));
uint32_t user__browser__dom__dom_get_selection_focus_offset(void) { return 0; }

void user__browser__dom__invalidate_layout(uint32_t node_id) __attribute__((weak));
void user__browser__dom__invalidate_layout(uint32_t node_id) {}

void user__browser__ipc_shared__sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t ptr, uint32_t len) __attribute__((weak));
void user__browser__ipc_shared__sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t ptr, uint32_t len) {}

// Media extern stubs
uint64_t ext_get_media_head(void) { return 0; }
uint64_t ext_get_media_tail(void) { return 0; }
void ext_set_media_head(uint64_t val) {}
void ext_set_media_tail(uint64_t val) {}

// JS resolve fetch stub (normally in jsc_bindings.m, but that's already linked)
// JS resolve fetch stub — weak so the real implementation in jsc_bindings.m wins
__attribute__((weak)) void js_resolve_fetch_impl(uint64_t fetch_id, uint64_t ptr, uint32_t len) {}

// --- Initialization ---
void jit_test_bridge_init(void) {
    user__os__ipc_ring__IPC_BUFFER_PTR = (uint64_t)(uintptr_t)dummy_ipc_ring;
}
