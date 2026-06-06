// =============================================================================
// Epic 84: Component Matrix Test Bridge
// =============================================================================
// Provides:
//   1. Mangled→unmangled aliases for @no_mangle Salt functions called cross-module
//   2. Weak stubs for hardware-only symbols (GPU, Audio, Network, Filesystem)
//      Weak so the real implementations take precedence if linked.
//
// All DOM, CSS, Layout, Lexer, Custom Element, and JSC logic runs REAL code.
// =============================================================================

#include <stdint.h>
#include <string.h>
#include <stdlib.h>



extern void sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t a1, uint64_t ptr, uint32_t len);
void user__browser__ipc_shared__sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t a1, uint64_t ptr, uint32_t len) {
    sys_ipc_send_r2m_command_with_payload(cmd, a1, ptr, len);
}

// ---- JSC Dispatch Stubs (not implemented in jsc_bindings.m yet) ----

__attribute__((weak)) void js_resolve_fetch_impl(uint64_t fetch_id, uint64_t buf, uint32_t len) {}
__attribute__((weak)) void js_resolve_fetch_chunk(uint32_t slot, uint64_t buf, uint32_t len, uint32_t is_end) {}
__attribute__((weak)) void js_bridge_dispatch_document_event(const char *type, uint32_t len) {}
__attribute__((weak)) void js_bridge_dispatch_message_event(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void js_bridge_dispatch_websocket_message(uint32_t id, uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void js_bridge_dispatch_main_message(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void js_bridge_dispatch_worker_message(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void js_bridge_resolve_idb_promise(uint32_t id, uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void js_execute_worker_jobs(void) {}
__attribute__((weak)) void sys_js_dispatch_popstate(uint64_t ptr, uint32_t len) {}

extern void sys_jsc_evaluate_script(uint64_t code_ptr, uint32_t code_len, uint64_t filename);
__attribute__((weak)) void sys_js_evaluate_script(uint64_t code_ptr, uint32_t len, uint64_t fn_ptr, uint32_t fn_len) {
    sys_jsc_evaluate_script(code_ptr, len, fn_ptr);
}

// ---- Weak Hardware Stubs (overridden by real implementations if linked) ----

// GPU / Metal
__attribute__((weak)) void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}
__attribute__((weak)) void sys_gpu_init_iosurface(void) {}
__attribute__((weak)) int32_t sys_gpu_is_iosurface_mode(void) { return 0; }
__attribute__((weak)) void sys_gpu_rasterize_iosurface(void) {}
__attribute__((weak)) void sys_gpu_commit_iosurface(void) {}
__attribute__((weak)) void sys_canvas_create_backing_store(uint32_t idx, uint32_t w, uint32_t h) {}
__attribute__((weak)) void sys_invalidate_paint(uint32_t idx) {}
__attribute__((weak)) void facet_gpu_upload_image(uint32_t id, uint64_t ptr, uint32_t w, uint32_t h) {}
__attribute__((weak)) void facet_gpu_free_texture(uint32_t id) {}
__attribute__((weak)) void facet_gpu_load_font_atlas(uint64_t ptr, uint32_t w, uint32_t h) {}
__attribute__((weak)) void facet_gpu_rasterize_primitives(void) {}
__attribute__((weak)) void facet_gpu_render_to_buffer(void) {}
__attribute__((weak)) void facet_image_decode(void) {}
__attribute__((weak)) void facet_image_free(void) {}
__attribute__((weak)) void facet_window_init(void) {}
__attribute__((weak)) void* facet_window_next_drawable(void) { return NULL; }
__attribute__((weak)) void facet_window_pump_events(void) {}
__attribute__((weak)) void facet_window_drain_keyboard(void) {}
__attribute__((weak)) void facet_window_get_scroll_delta(void) {}

// Audio
__attribute__((weak)) void sys_hw_audio_init(void) {}
__attribute__((weak)) void sys_hw_decoder_signal_data_ready(void) {}

// Network / TLS
__attribute__((weak)) void sys_tls_upgrade_to_websocket(void) {}
__attribute__((weak)) void ext_tls_write_bytes(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void decode_hpack_block(void) {}
__attribute__((weak)) void ext_hpack_encode_headers(void) {}
__attribute__((weak)) uint64_t ext_hpack_get_buffer_ptr(void) { return 0; }

// Timers
__attribute__((weak)) void ext_timers_add_timeout(void) {}
__attribute__((weak)) void ext_timers_add_raf(void) {}

// OS / IPC
__attribute__((weak)) uint64_t sys_clock_get_ms(void) { return 0; }
__attribute__((weak)) void sys_sleep_ms(uint32_t ms) {}
__attribute__((weak)) void sys_atomic_write_u8(uint64_t ptr, uint8_t val) {}
__attribute__((weak)) void sys_memcpy(uint64_t dst, uint64_t src, uint32_t len) {
    memcpy((void*)(uintptr_t)dst, (void*)(uintptr_t)src, len);
}
__attribute__((weak)) void sys_mmap_file(void) {}
__attribute__((weak)) void ext_mac_update_omnibox(void) {}

// Typography
__attribute__((weak)) void ext_c_shape_and_measure(void) {}

// Custom Elements
__attribute__((weak)) void ext_dom_set_custom_tag(uint64_t node_id, uint32_t hash, uint64_t ptr, uint32_t len) {}

// WebSocket class init
__attribute__((weak)) void sys_init_ws_class(void* ctx) {}
