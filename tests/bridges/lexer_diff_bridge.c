// tests/bridges/lexer_diff_bridge.c
// Provides mock HTML payload for the headless lexer differential test.
// Reads HTML file from LEXER_TEST_HTML env var.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char html_buffer[1048576]; // 1MB max
static unsigned int html_len = 0;

__attribute__((constructor)) static void load_html(void) {
  const char *path = getenv("LEXER_TEST_HTML");
  FILE *f = NULL;
  if (path) {
    f = fopen(path, "rb");
    if (!f) {
      fprintf(stderr, "[BRIDGE] Failed to open %s\n", path);
      return;
    }
  } else {
    f = stdin;
  }
  html_len = (unsigned int)fread(html_buffer, 1, sizeof(html_buffer), f);
  if (path && f)
    fclose(f);
}

unsigned long long ext_get_mock_html_ptr(void) {
  return (unsigned long long)html_buffer;
}

unsigned int ext_get_mock_html_len(void) { return html_len; }

// Weak stubs: only linked if no real implementation exists.
// The run_test.sh pipeline brings in jsc_bridge.m, ipc_bridge.c etc. which
// provide the real versions — these are purely fallbacks to avoid linker
// errors.
#define WEAK __attribute__((weak))

WEAK void sys_typography_init(void) {}
WEAK void sys_hw_audio_init(void) {}
WEAK void sys_init_vsync(void) {}
WEAK void ext_cdm_sandbox_init(void) {}
WEAK int ext_get_ipc_fd(void) { return -1; }
WEAK void sys_gpu_init_iosurface(int w, int h) {}
WEAK void sys_gpu_set_scissor_rect(int x, int y, int w, int h) {}
WEAK void sys_gpu_commit(void) {}
WEAK void sys_gpu_emit_video_frame(unsigned int n, int x, int y, int w, int h) {
}
WEAK void sys_gpu_emit_external_surface(unsigned int n, unsigned int sid, int x,
                                        int y, int w, int h) {}
WEAK unsigned long long get_active_focus_node(void) { return 0; }
WEAK int drain_scroll_input(void) { return 0; }
WEAK unsigned long long get_dom_content_loaded_fired(void) { return 0; }
WEAK void set_dom_content_loaded_fired(unsigned char v) {}
WEAK unsigned long long get_frame_count(void) { return 0; }
WEAK void set_frame_count(unsigned long long v) {}
WEAK unsigned long long get_max_test_frames(void) { return 0; }
WEAK void pump_websocket_frames(void) {}
WEAK void sys_browser_navigate(unsigned long long ptr, unsigned int len) {}
WEAK void sys_js_pump_script_queue(void) {}
WEAK void ext_jsc_trigger_connected_callback(unsigned long long id) {}
WEAK void ext_jsc_trigger_disconnected_callback(unsigned long long id) {}
WEAK void css_lex_stylesheet(unsigned long long ptr, unsigned int len,
                             unsigned int scope_id) {}
WEAK void parse_hex_to_rgb(unsigned long long ptr, unsigned int len,
                           unsigned char *r, unsigned char *g,
                           unsigned char *b) {}
WEAK void sys_ipc_send_r2m_command_with_payload(unsigned int cmd,
                                                unsigned long long arg1,
                                                unsigned long long ptr,
                                                unsigned int len) {}

WEAK void sys_log_info(unsigned long long msg_ptr, unsigned int msg_len) {
  uint8_t *msg = (uint8_t *)msg_ptr;
  fwrite(msg, 1, msg_len, stdout);
  fflush(stdout);
}

void sys_print_float(double f) { printf("%g", f); }
void sys_print_u32(unsigned int u) { printf("%u", u); }
void sys_print_i32(int i) { printf("%d", i); }

void ext_events_free_node_callbacks(unsigned int idx) {}
