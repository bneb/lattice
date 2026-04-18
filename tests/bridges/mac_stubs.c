#include <stdint.h>
#include <unistd.h>

__attribute__((weak)) void sys_sleep_ms(uint32_t ms) { usleep(ms * 1000); }

__attribute__((weak)) unsigned long long
ext_hpack_get_static_key(unsigned int index) {
  return 0;
}
__attribute__((weak)) unsigned long long
ext_hpack_get_static_val(unsigned int index) {
  return 0;
}
__attribute__((weak)) void ext_ipc_send_cdm_command(uint32_t cmd, uint64_t arg1,
                                                    uint64_t p_ptr,
                                                    uint32_t p_len) {}
__attribute__((weak)) void
ext_net_route_header_to_stream(uint32_t stream, uint64_t key_ptr, uint32_t klen,
                               uint64_t val_ptr, uint32_t vlen) {}
__attribute__((weak)) uint32_t dom_get_selection_focus_offset(uint32_t node) {
  return 0;
}
__attribute__((weak)) uint32_t dom_get_selection_anchor_offset(uint32_t node) {
  return 0;
}
__attribute__((weak)) uint32_t dom_get_canvas_surface_id(uint32_t node) {
  return 0;
}
__attribute__((weak)) uint32_t dom_get_selection_anchor_node() { return 0; }
__attribute__((weak)) uint32_t dom_get_selection_focus_node() { return 0; }

__attribute__((weak)) void sys_print_str(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) uint64_t sys_time_now_ms_int(void) { return 0; }
