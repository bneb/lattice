// tests/bridges/layout_diff_bridge.c
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void sys_log_info(unsigned long long msg_ptr, unsigned int msg_len) {
  uint8_t *msg = (uint8_t *)msg_ptr;
  fwrite(msg, 1, msg_len, stdout);
  fflush(stdout);
}

void sys_print_u32(unsigned int u) { printf("%u", u); }
void sys_print_float(double f) { printf("%g", f); }

// Stubs for layout.salt dependencies (using merged Salt implementations)
// These were duplicates: sys_queue_resize_record, sys_is_node_observed_for,
// sys_push_to_shape_queue, airlock_init_allocator, init_arrays.

void ext_jsc_trigger_connected_callback(unsigned long long id) {}
void ext_jsc_trigger_disconnected_callback(unsigned long long id) {}
void ext_events_free_node_callbacks(unsigned int idx) {}
void apply_rules_to_node(unsigned int idx) {}
void bake_sdf_atlas() {}
float _get_kerning_offset_c(unsigned int c1, unsigned int c2) { return 0.0f; }
unsigned int sys_canvas_create_backing_store(unsigned int id, unsigned int w,
                                             unsigned int h) {
  return 0;
}
