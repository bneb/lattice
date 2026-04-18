#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>

// =============================================================================
// Epic 85: JSC-Compatible Storage Test Bridge
// Provides sys_mmap_file and hardware stubs for headless IDB testing
// =============================================================================

// --- Storage POSIX mmap ---
uint64_t sys_mmap_file(uint64_t filename_ptr, uint32_t size) {
    const char *filename = (const char *)(uintptr_t)filename_ptr;
    int fd = open(filename, O_RDWR | O_CREAT, 0666);
    if (fd < 0) return 0;
    ftruncate(fd, size);
    void *ptr = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    close(fd);
    if (ptr == MAP_FAILED) return 0;
    return (uint64_t)(uintptr_t)ptr;
}

// --- Mangled→Unmangled Aliases ---
// Salt @no_mangle generates unmangled symbols, but cross-module calls use mangled names
// For functions called from the TEST (unmangled) that exist in Salt IR (mangled):
extern void user__browser__dom__init_arrays(void);
void init_arrays(void) { user__browser__dom__init_arrays(); }

extern int32_t compare_document_position(uint32_t n1, uint32_t n2);
int32_t user__browser__dom__compare_document_position(uint32_t n1, uint32_t n2) {
    return compare_document_position(n1, n2);
}

extern void sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t arg1, uint64_t ptr, uint32_t len);
void user__browser__ipc_shared__sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t arg1, uint64_t ptr, uint32_t len) {
    sys_ipc_send_r2m_command_with_payload(cmd, arg1, ptr, len);
}

extern void user__browser__paint__paint_node(uint32_t node_id, uint64_t focus, uint64_t frame, int32_t scrolly, int32_t vh);
void paint_node(uint32_t node_id, uint64_t focus, uint64_t frame, int32_t scrolly, int32_t vh) {
    user__browser__paint__paint_node(node_id, focus, frame, scrolly, vh);
}

// --- Weak Hardware Stubs ---
// GPU
__attribute__((weak)) void sys_gpu_update_texture_region(int32_t id, int32_t x, int32_t y, int32_t w, int32_t h, uint64_t ptr) {}
__attribute__((weak)) void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}
__attribute__((weak)) void sys_metal_draw_text_run(uint64_t ptr, int32_t x, int32_t y, uint32_t count, uint32_t color, float size) {}
__attribute__((weak)) void sys_metal_fill_rect(int32_t x, int32_t y, int32_t w, int32_t h, uint32_t color) {}
__attribute__((weak)) void sys_metal_fill_rounded_rect(int32_t x, int32_t y, int32_t w, int32_t h, uint32_t color, int32_t r) {}
__attribute__((weak)) void sys_metal_draw_border(int32_t x, int32_t y, int32_t w, int32_t h, uint32_t color, int32_t bw) {}
__attribute__((weak)) void sys_metal_draw_linear_gradient(int32_t x, int32_t y, int32_t w, int32_t h, uint32_t c1, uint32_t c2, uint8_t dir) {}
__attribute__((weak)) void sys_metal_composite_video_frame(int32_t x, int32_t y, int32_t w, int32_t h) {}
__attribute__((weak)) void sys_metal_draw_image(uint32_t img_id, int32_t x, int32_t y, int32_t w, int32_t h) {}
__attribute__((weak)) void sys_metal_draw_box_shadow(int32_t x, int32_t y, int32_t w, int32_t h, int32_t ox, int32_t oy, int32_t blur, int32_t spread, uint32_t color) {}
__attribute__((weak)) void facet_send_repaint_signal(void) {}

// Audio
__attribute__((weak)) void sys_hw_decoder_signal_data_ready(void) {}

// Network
__attribute__((weak)) void ext_ws_connect(uint64_t url_ptr, uint32_t url_len) {}
__attribute__((weak)) void ext_net_start_fetch(uint32_t fetch_id, uint64_t url_ptr, uint32_t url_len) {}
__attribute__((weak)) void ext_tls_write_bytes(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void sys_tls_upgrade_to_websocket(uint64_t ptr, uint32_t len) {}

// IPC (weak stubs — real impl not needed in headless test)
__attribute__((weak)) void sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t arg1, uint64_t ptr, uint32_t len) {}

// Process
__attribute__((weak)) uint32_t ext_process_spawn_renderer(uint64_t url_ptr, uint32_t url_len) { return 0; }

// iframe rendering
__attribute__((weak)) uint32_t dom_find_iframe_slot(uint32_t node_idx) { return 0; }

// Layout invalidation
__attribute__((weak)) void invalidate_layout(void) {}

// HarfBuzz typography
__attribute__((weak)) void sys_harfbuzz_shape(uint64_t text_ptr, uint32_t text_len, uint64_t font_ptr, float font_size_px, uint64_t out_glyphs_ptr, uint64_t out_count_ptr) {}
__attribute__((weak)) void ext_c_shape_and_measure(uint64_t ptr, uint32_t len, float size, uint64_t out_w) {}

// HPACK (HTTP/2)
__attribute__((weak)) int32_t decode_hpack_block(uint64_t ptr, uint32_t len, uint64_t out, uint32_t cap) { return 0; }
__attribute__((weak)) int32_t ext_hpack_encode_headers(uint64_t ptr, uint32_t count, uint64_t out, uint32_t cap) { return 0; }
__attribute__((weak)) uint64_t ext_hpack_get_buffer_ptr(void) { return 0; }

// Timers/RAF
__attribute__((weak)) uint32_t ext_timers_add_timeout(uint32_t delay_ms, uint8_t is_interval) { return 0; }
__attribute__((weak)) uint32_t ext_timers_add_raf(void) { return 0; }

// GPU / IOSurface
__attribute__((weak)) void sys_gpu_commit_iosurface(void) {}
__attribute__((weak)) void sys_gpu_init_iosurface(int32_t w, int32_t h) {}
__attribute__((weak)) uint8_t sys_gpu_is_iosurface_mode(void) { return 0; }
__attribute__((weak)) void sys_gpu_rasterize_iosurface(void) {}

// Audio
__attribute__((weak)) void sys_hw_audio_init(void) {}

// WebSocket JSC class
__attribute__((weak)) void sys_init_ws_class(void* ctx) {}

// Canvas
__attribute__((weak)) void sys_invalidate_paint(void) {}

// macOS UI
__attribute__((weak)) void ext_mac_update_omnibox(uint64_t url_ptr, uint32_t url_len) {}

// Custom elements DOM
__attribute__((weak)) void ext_dom_set_custom_tag(uint32_t idx, uint64_t ptr, uint32_t len) {}

// Worker/message dispatch
__attribute__((weak)) void js_bridge_dispatch_document_event(uint32_t type_hash) {}
__attribute__((weak)) void js_bridge_dispatch_main_message(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void js_bridge_dispatch_message_event(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void js_bridge_dispatch_websocket_message(uint32_t ws_id, uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void js_bridge_dispatch_worker_message(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) int32_t js_execute_worker_jobs(void) { return 0; }

// Fetch
__attribute__((weak)) void js_resolve_fetch_impl(uint64_t fetch_id, uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void js_resolve_fetch_chunk(uint64_t fetch_id, uint64_t ptr, uint32_t len) {}

// JSC bridge functions
__attribute__((weak)) void sys_js_dispatch_popstate(uint64_t state_ptr, uint32_t state_len) {}
__attribute__((weak)) void sys_js_evaluate_script(uint64_t code_ptr, uint32_t code_len, uint64_t fn_ptr, uint32_t fn_len) {}

// System
__attribute__((weak)) void sys_atomic_write_u8(uint64_t ptr, uint8_t val) {}
__attribute__((weak)) uint64_t sys_clock_get_ms(void) { return 0; }
__attribute__((weak)) void sys_memcpy(uint64_t dst, uint64_t src, uint32_t len) { memcpy((void*)(uintptr_t)dst, (void*)(uintptr_t)src, len); }
__attribute__((weak)) void sys_sleep_ms(uint32_t ms) {}

