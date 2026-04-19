#include <stdint.h>
#include <string.h>

extern void sys_on_mouse_click(int32_t x, int32_t y);
extern void sys_net_pump_ingress(void);
extern void sys_ipc_push_command(uint32_t type, uint64_t arg1, uint32_t arg2);

static uint64_t last_nav_url_ptr = 0;
static uint32_t last_nav_url_len = 0;

void sys_browser_navigate(uint64_t ptr, uint32_t len) {
    last_nav_url_ptr = ptr;
    last_nav_url_len = len;
}

uint64_t test_get_last_nav_url_ptr(void) { return last_nav_url_ptr; }
uint32_t test_get_last_nav_url_len(void) { return last_nav_url_len; }

extern void dom_add_scroll_y(int32_t delta_y);

void test_push_scroll_ipc(int32_t delta_y) {
    dom_add_scroll_y(delta_y);
}

void test_simulate_click(int32_t x, int32_t y) {
    sys_on_mouse_click(x, y);
}

// ── Linker Stubs for test environment ──
#define WEAK __attribute__((weak))
WEAK void ext_net_route_header_to_stream(uint32_t s, uint64_t kp, uint32_t kl, uint64_t vp, uint32_t vl) {}
WEAK void ext_tls_write_bytes(uint64_t p, uint32_t l) {}
WEAK uint32_t get_dom_content_loaded_fired(void) { return 1; }
WEAK void set_dom_content_loaded_fired(uint32_t v) {}
WEAK uint64_t ext_hpack_get_static_key(uint32_t i) { return 0; }
WEAK uint64_t ext_hpack_get_static_val(uint32_t i) { return 0; }
WEAK void ext_flush_frame(void) {}
WEAK int32_t check_any_layout_dirty(void) { return 0; }
WEAK uint32_t get_frame_count(void) { return 1; }
WEAK void set_frame_count(uint32_t v) {}
WEAK uint32_t get_max_test_frames(void) { return 1; }
WEAK void pump_websocket_frames(void) {}
WEAK void sys_js_pump_script_queue(void) {}
WEAK void js_bridge_dispatch_main_message(uint64_t p, uint32_t l) {}
WEAK void sys_jsc_flush_microtasks(void) {}
WEAK void js_resolve_fetch(uint64_t id, uint64_t bp, uint32_t len) {}
WEAK void complete_script_fetch(uint64_t id, uint64_t bp, uint32_t len) {}
WEAK void complete_fetch(uint64_t id, uint64_t bp, uint32_t len) {}
WEAK uint64_t sys_ipc_get_bulk_ingress_ptr(void) { return 0; }
WEAK uint32_t compositor_decode_and_upload_image(uint64_t p, uint32_t len, uint64_t w_p, uint64_t h_p) { return 0; }

