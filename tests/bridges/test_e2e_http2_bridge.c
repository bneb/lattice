#include <stdint.h>
#include <stdio.h>
#include <string.h>

// === Symbols NOT provided by existing bridges ===

// HPACK static table mocks
uint64_t ext_hpack_get_static_key(uint32_t index) { return 0; }
uint64_t ext_hpack_get_static_val(uint32_t index) { return 0; }
void ext_net_route_header_to_stream(uint32_t stream_id, uint64_t key_ptr,
                                    uint32_t key_len, uint64_t val_ptr,
                                    uint32_t val_len) {
  printf("[Telemetry] HPACK Header routed to stream %u\n", stream_id);
}

// Net/Event loop stubs missing from other bridges

void sys_net_init_h2_connection(uint64_t hostname) {}
int32_t check_any_layout_dirty() { return 0; }
void pump_websocket_frames() {}

void set_dom_content_loaded_fired(uint8_t fired) {}
uint8_t get_dom_content_loaded_fired() { return 1; }
void set_frame_count(uint64_t frames) {}
uint64_t get_frame_count() { return 0; }
void set_max_test_frames(uint64_t frames) {}
uint64_t get_max_test_frames() { return 5; }
void sys_browser_navigate(uint64_t url_ptr, uint32_t url_len) {}
void sys_typography_init() {}
void ext_tls_write_bytes(uint64_t data, uint32_t len) {
  printf("[Telemetry] TLS Write: %u bytes\n", len);
}
void sys_ipc_send_r2m_command_with_payload(uint32_t cmd, uint64_t arg1,
                                           uint64_t ptr, uint32_t len) {}
void sys_js_pump_script_queue() {}
