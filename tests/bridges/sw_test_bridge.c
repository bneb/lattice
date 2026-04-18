#include <stdint.h>

// Stubs for Service Worker E2E test (Phase 1)
// These symbols are referenced by browser components linked into the test
// harness. Names match Salt's mangling: _package_path_name

uint32_t user__browser__media__MEDIA_HEAD = 0;
uint32_t user__browser__media__MEDIA_TAIL = 0;
uint8_t user__browser__paint__Z_SORT_BUF[65536];

__attribute__((weak)) void
user__browser__dom__invalidate_layout(uint32_t node_id) {}
__attribute__((weak)) void user__browser__paint__paint_node(uint32_t node_id) {}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_selection_anchor_node() {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_selection_anchor_offset() {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_selection_focus_node() {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_selection_focus_offset() {
  return 0;
}
__attribute__((weak)) uint8_t
user__browser__paint__is_node_between_selection(uint32_t node_id) {
  return 0;
}

__attribute__((weak)) uint32_t
user__browser__dom__dom_find_iframe_slot(uint32_t node_id) {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_node_tag(uint32_t node_id) {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_parent(uint32_t node_id) {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_first_child(uint32_t node_id) {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_next_sibling(uint32_t node_id) {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_attr_count(uint32_t node_id) {
  return 0;
}
__attribute__((weak)) uint64_t
user__browser__dom__dom_get_attr_key_ptr(uint32_t node_id, uint32_t index) {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_attr_key_len(uint32_t node_id, uint32_t index) {
  return 0;
}
__attribute__((weak)) uint64_t
user__browser__dom__dom_get_attr_val_ptr(uint32_t node_id, uint32_t index) {
  return 0;
}
__attribute__((weak)) uint32_t
user__browser__dom__dom_get_attr_val_len(uint32_t node_id, uint32_t index) {
  return 0;
}

// Map the mangled name to the unmangled one for the IPC bridge
void user__browser__ipc_shared__sys_ipc_send_r2m_command_with_payload(
    uint32_t cmd_type, uint64_t arg1, uint64_t payload_ptr,
    uint32_t payload_len) {
  extern void sys_ipc_send_r2m_command_with_payload(
      uint32_t cmd_type, uint64_t arg1, uint64_t payload_ptr,
      uint32_t payload_len);
  sys_ipc_send_r2m_command_with_payload(cmd_type, arg1, payload_ptr,
                                        payload_len);
}

void js_quickjs_init() {
  // Stub for legacy call in tests (or redirect to sys_jsc_init)
  extern void sys_jsc_init();
  sys_jsc_init();
}

extern void init_arrays(void);
void sys_init_arrays(void) { init_arrays(); }

void pump_websocket_frames() {}
void set_dom_content_loaded_fired(uint8_t fired) {}
uint8_t get_dom_content_loaded_fired() { return 0; }
void set_frame_count(uint64_t count) {}
uint64_t get_frame_count() { return 0; }
uint64_t get_max_test_frames() { return 60; }
uint8_t check_any_layout_dirty() { return 0; }
void sys_browser_navigate(uint64_t url_ptr, uint32_t url_len) {}
void sys_js_pump_script_queue() {}
