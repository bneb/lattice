#include <JavaScriptCore/JavaScript.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern void sys_jsc_init();
extern void sys_jsc_evaluate_script(uint64_t script_ptr, uint32_t script_len,
                                    const char *filename);

void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}

// Stubs for missing symbols from unrelated incomplete Engine modules
void user__browser__dom__dom_find_iframe_slot() {}
void user__browser__dom__invalidate_layout() {}
void user__browser__dom__compare_document_position() {}
uint8_t user__browser__dom__STYLE_POINTER_EVENTS[1] = {0};
void user__browser__ipc_shared__sys_ipc_send_r2m_command_with_payload() {}
uint32_t user__browser__media__MEDIA_HEAD = 0;
uint32_t user__browser__media__MEDIA_TAIL = 0;

uint32_t user__browser__dom__EVICTION_QUEUE[1] = {0};
int32_t user__browser__dom__LAYOUT_SCROLL_X[1] = {0};

void sys_hw_decoder_init() {}
void sys_hw_decoder_push_nalu() {}
void sys_invalidate_paint() {}
void sys_js_dispatch_popstate() {}
void sys_js_evaluate_script() {}
void sys_memcpy() {}
void sys_mmap_file() {}
void sys_sleep_ms() {}
void sys_tls_upgrade_to_websocket() {}
void sys_tls_start_ws_streaming_loop() {}

// Round 3 Stubs
void decode_hpack_block() {}
void ext_c_shape_and_measure() {}
uint32_t ext_get_media_head() { return 0; }
uint32_t ext_get_media_tail() { return 0; }
void ext_hpack_encode_headers() {}
void ext_hpack_get_buffer_ptr() {}
void ext_mac_update_omnibox() {}
void ext_set_media_head() {}
void ext_set_media_tail() {}
void ext_timers_add_raf() {}
void ext_timers_add_timeout() {}
void ext_tls_write_bytes() {}
void init_arrays() {}
void js_bridge_dispatch_document_event() {}
void js_bridge_dispatch_main_message() {}
void js_bridge_dispatch_message_event() {}
void js_bridge_dispatch_websocket_message() {}
void js_bridge_dispatch_worker_message() {}
void js_bridge_resolve_idb_promise() {}
void js_execute_worker_jobs() {}
void js_resolve_fetch_chunk() {}
void js_resolve_fetch_impl() {}
void sys_atomic_write_u8() {}
uint64_t sys_clock_get_ms() { return 0; }
void sys_gpu_commit_iosurface() {}
void sys_gpu_init_iosurface() {}
int sys_gpu_is_iosurface_mode() { return 0; }
void sys_gpu_rasterize_iosurface() {}
void sys_hw_audio_init() {}

JSGlobalContextRef global_ctx;

int c_bridge_websockets_e2e_test() {
  airlock_init_allocator();
  init_arrays();
  sys_jsc_init();

  // Test sets up a WS connection
  const char *script =
      "globalThis.latestMessage = '';\n"
      "const ws = new WebSocket('wss://localhost:8080');\n"
      "ws.onmessage = (e) => { globalThis.latestMessage = e.data; };\n";

  sys_jsc_evaluate_script((uint64_t)script, strlen(script), "test.js");

  return 0;
}

extern void ext_ws_push_bytes(uint32_t socket_id, uint64_t data_ptr,
                              uint32_t data_len);

int c_bridge_push_ws(uint32_t socket_id, uint64_t data_ptr, uint32_t data_len) {
  ext_ws_push_bytes(socket_id, data_ptr, data_len);
  return 0;
}
