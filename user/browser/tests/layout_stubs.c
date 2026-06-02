#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

void css_arena_inc_count() {}
void css_arena_set_hash(uint32_t idx, uint32_t val) {}
void ext_engine_process_mouse_down() {}
void facet_gpu_free_texture(int32_t tex_idx) {}
void facet_gpu_load_font_atlas(uint64_t pixels, int32_t width, int32_t height) {}
void facet_gpu_rasterize_primitives(uint64_t native_drawable, uint64_t rects, int32_t width, int32_t height, int32_t rect_count, float scroll_y) {}
void facet_gpu_render_to_buffer(uint64_t out_buffer, uint64_t rects, int32_t width, int32_t height, int32_t rect_count, float scroll_y) {}
int32_t facet_gpu_upload_image(uint64_t rgba, int32_t width, int32_t height) { return 0; }
uint64_t facet_image_decode(uint64_t bytes, int32_t len, uint64_t out_w, uint64_t out_h) { return 0; }
void facet_image_free(uint64_t pixels) {}
uint32_t facet_window_drain_keyboard(uint64_t target) { return 0; }
float facet_window_get_scroll_delta() { return 0.0f; }
void facet_window_init(int32_t width, int32_t height) {}
uint64_t facet_window_next_drawable() { return 0; }
void facet_window_pump_events() {}
void ext_events_free_node_callbacks(uint32_t node_idx) {}
void ext_events_invoke(uint64_t node_id, uint32_t type_hash) {}
void ext_events_register(uint64_t node_id, uint32_t type_hash, uint64_t cb_ptr) {}
void ext_events_remove(uint64_t node_id, uint32_t type_hash, uint64_t cb_ptr) {}
uint32_t get_active_focus_node() { return 0; }
uint32_t hash_string(uint64_t ptr, uint32_t len) { return 0; }
uint64_t sovereign_arena_alloc(uint64_t size) { return (uint64_t)malloc(size); }
uint32_t sys_canvas_create_backing_store(uint32_t node_id, uint32_t width, uint32_t height) { return 0; }
void sys_exit(int32_t code) { exit(code); }
void sys_hw_decoder_signal_data_ready() {}
void sys_mfence() { __sync_synchronize(); }
