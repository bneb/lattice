#include <stdint.h>

void ext_flush_frame(int32_t width, int32_t height) {}
void ext_hpack_get_static_key() {}
void ext_hpack_get_static_val() {}
void ext_net_route_header_to_stream() {}
void ext_tls_write_bytes() {}
uint8_t get_dom_content_loaded_fired() { return 0; }
uint64_t get_frame_count() { return 0; }
uint64_t get_max_test_frames() { return 0; }
void pump_websocket_frames() {}
void set_dom_content_loaded_fired(uint8_t v) {}
void set_frame_count(uint64_t v) {}
void sys_browser_navigate(uint64_t ptr, uint32_t len) {}
void sys_js_pump_script_queue() {}

int sys_gpu_is_iosurface_mode(void) { return 1; }
void sys_gpu_rasterize_iosurface(void *rects, int width, int height,
                                 int rect_count, float scroll_y) { }

extern uint32_t sys_hit_test(float x, float y, uint32_t root_node);
extern void ext_dom_set_hovered_node(uint32_t node_idx);
extern uint64_t user__browser__paint__P_DIRTY_PAINT;
extern uint8_t user__browser__dom__DIRTY_PAINT[65536];

void test_sys_on_mouse_move(int x, int y) {
    uint32_t target_node_idx = sys_hit_test((float)x, (float)y, 1);
    ext_dom_set_hovered_node(target_node_idx);
}

extern void ext_dom_set_focused_node(uint32_t node);
void test_set_focused_node(uint32_t node) { ext_dom_set_focused_node(node); }

extern uint32_t dom_get_hovered_node();
uint32_t test_get_hovered_node() { return dom_get_hovered_node(); }
