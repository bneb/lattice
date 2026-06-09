#include <fcntl.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

static uint8_t *shared_memory = NULL;

typedef struct __attribute__((packed)) {
  uint32_t type;
  uint64_t arg1;
  uint32_t arg2;
  uint32_t payload_len;
  uint8_t payload[1024]; // Simple bounded payload
} IPCCommand;

typedef struct {
  uint32_t type;
  uint64_t arg1;
  uint32_t arg2;
  uint64_t payload_ptr;
  uint32_t payload_len;
} SaltIPCCommand;

// We partition the 256KB into:
// 0    - 128KB: M2R Ring (Inputs, Navigations)
// 128KB - 256KB: R2M Ring (IOSurfaces, Crash states)

#define RING_SIZE 65536
#define CMD_SIZE sizeof(IPCCommand)
#define QUEUE_CAPACITY (RING_SIZE / CMD_SIZE)

// Atomic headers at the very start of each block?
// No, let's put the headers separated or at the start.
// Actually, let's rigidly define the layout:
// 0x0000: uint32_t m2r_head
// 0x0004: uint32_t m2r_tail
// 0x0008: IPCCommand commands[... (fitting in 128KB - 8)]
// 0x20000: uint32_t r2m_head
// 0x20004: uint32_t r2m_tail
// 0x20008: IPCCommand r2m_commands[...]

typedef struct {
  atomic_uint head;
  atomic_uint tail;
  IPCCommand commands[(RING_SIZE - 8) / sizeof(IPCCommand)];
} IPCRing;

int32_t ext_ipc_init_shared_memory(int32_t fd) {
  if (fd < 0)
    return -1;

  struct stat st;
  if (fstat(fd, &st) < 0) {
    perror("fstat shared memory");
    return -1;
  }

  void *map_ptr =
      mmap(NULL, st.st_size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
  if (map_ptr == MAP_FAILED) {
    perror("mmap shared memory");
    return -1;
  }
  shared_memory = (uint8_t *)map_ptr;
  fprintf(stderr,
          "[IPC-DIAG] Shared memory mapped %lld bytes at %p from fd=%d\n",
          (long long)st.st_size, shared_memory, fd);
  return 0;
}

// Alias for Salt ABI
int32_t ext_ipc_init_shm(int32_t fd) { return ext_ipc_init_shared_memory(fd); }

uint64_t sys_ipc_get_bulk_ingress_ptr() {
  if (!shared_memory)
    return 0;
  return (uint64_t)(shared_memory + 131072);
}

uint64_t ext_ipc_get_bulk_ingress_ptr() {
  return sys_ipc_get_bulk_ingress_ptr();
}

// Epic 99: Open shared memory by name (bypasses fd inheritance across exec)
int32_t ext_ipc_init_by_name(void) {
  int fd = shm_open("/keuos_tab", O_RDWR, 0666);
  if (fd < 0) {
    perror("[IPC] shm_open by name failed");
    return -1;
  }
  fprintf(stderr, "[IPC-DIAG] shm_open by name succeeded, fd=%d\n", fd);
  return ext_ipc_init_shared_memory(fd);
}

// Write to Renderer -> Main
void sys_ipc_send_r2m_command(uint32_t cmd_type, uint64_t arg1) {
  if (!shared_memory)
    return;
  IPCRing *ring = (IPCRing *)(shared_memory + 65536);
  uint32_t head = atomic_load(&ring->head);
  uint32_t next = (head + 1) % ((RING_SIZE - 8) / sizeof(IPCCommand));
  // Lock-free: we overwrite if full, or we drop. Let's drop if full.
  if (next == atomic_load(&ring->tail)) {
    printf("[IPC] R2M Queue Full! (cmd=%u)\n", cmd_type);
    return;
  }
  ring->commands[head].type = cmd_type;
  ring->commands[head].arg1 = arg1;
  ring->commands[head].arg2 = 0;
  ring->commands[head].payload_len = 0;
  atomic_store(&ring->head, next);
}

// Epic 69: Write to R2M ring WITH variable-length payload (for iframe src URLs,
// postMessage data)
__attribute__((weak))
void sys_ipc_send_r2m_command_with_payload(uint32_t cmd_type, uint64_t arg1,
                                           uint64_t payload_ptr,
                                           uint32_t payload_len) {
  if (!shared_memory)
    return;
  IPCRing *ring = (IPCRing *)(shared_memory + 65536);
  uint32_t head = atomic_load(&ring->head);
  uint32_t next = (head + 1) % ((RING_SIZE - 8) / sizeof(IPCCommand));
  if (next == atomic_load(&ring->tail)) {
    printf("[IPC] R2M Queue Full (payload)! (cmd=%u)\n", cmd_type);
    return;
  }
  ring->commands[head].type = cmd_type;
  ring->commands[head].arg1 = arg1;
  ring->commands[head].arg2 = 0;
  // Copy payload into the ring buffer's inline payload field
  if (payload_ptr != 0 && payload_len > 0) {
    uint32_t copy_len = payload_len < 1024 ? payload_len : 1024;
    memcpy(ring->commands[head].payload, (void *)payload_ptr, copy_len);
    ring->commands[head].payload_len = copy_len;
  } else {
    ring->commands[head].payload_len = 0;
  }
  atomic_store(&ring->head, next);
}

__attribute__((weak))
void user__browser__ipc_shared__ext_ipc_send_r2m_command_with_payload(
    uint32_t cmd_type, uint64_t arg1, uint64_t p_ptr, uint32_t p_len) {
  sys_ipc_send_r2m_command_with_payload(cmd_type, arg1, p_ptr, p_len);
}

__attribute__((weak))
void ext_ipc_send_r2m_command_with_payload(uint32_t cmd_type, uint64_t arg1,
                                           uint64_t p_ptr, uint32_t p_len) {
  sys_ipc_send_r2m_command_with_payload(cmd_type, arg1, p_ptr, p_len);
}

// Epic 69: Read any R2M command (returns full struct, not filtered by
// target_cmd) Returns pointer to static SaltIPCCommand, or NULL if empty
static SaltIPCCommand global_r2m_read_cmd;

SaltIPCCommand *sys_ipc_read_r2m_command_full(void) {
  if (!shared_memory)
    return NULL;
  IPCRing *ring = (IPCRing *)(shared_memory + 65536);
  uint32_t tail = atomic_load(&ring->tail);
  if (tail == atomic_load(&ring->head))
    return NULL; // Empty

  IPCCommand *cmd = &ring->commands[tail];
  global_r2m_read_cmd.type = cmd->type;
  global_r2m_read_cmd.arg1 = cmd->arg1;
  global_r2m_read_cmd.arg2 = cmd->arg2;
  global_r2m_read_cmd.payload_ptr = (uint64_t)cmd->payload;
  global_r2m_read_cmd.payload_len = cmd->payload_len;

  atomic_store(&ring->tail,
               (tail + 1) % ((RING_SIZE - 8) / sizeof(IPCCommand)));
  return &global_r2m_read_cmd;
}

// Read Renderer -> Main (For Cocoa UI)
uint32_t sys_ipc_read_r2m_command(uint32_t target_cmd) {
  if (!shared_memory)
    return 0;
  IPCRing *ring = (IPCRing *)(shared_memory + 65536);
  uint32_t tail = atomic_load(&ring->tail);
  if (tail == atomic_load(&ring->head))
    return 0; // Empty

  IPCCommand cmd = ring->commands[tail];
  atomic_store(&ring->tail,
               (tail + 1) % ((RING_SIZE - 8) / sizeof(IPCCommand)));

  if (cmd.type == target_cmd) {
    return (uint32_t)cmd.arg1;
  }
  return 0;
}

// Write Main -> Renderer (Inputs, Navs)
void sys_ipc_push_command(uint32_t cmd_type, uint64_t arg1, uint32_t arg2) {
  if (!shared_memory)
    return;
  IPCRing *ring = (IPCRing *)shared_memory;
  uint32_t head = atomic_load(&ring->head);
  uint32_t next = (head + 1) % ((RING_SIZE - 8) / sizeof(IPCCommand));
  if (next == atomic_load(&ring->tail)) {
    printf("[IPC] M2R Queue Full!\n");
    return;
  }
  ring->commands[head].type = cmd_type;
  ring->commands[head].arg1 = arg1;
  ring->commands[head].arg2 = arg2;
  // For navigation, arg1 and arg2 are the pointer/len to a temporary string.
  // Wait! Pointers across processes are invalid!
  // We must copy the payload INTO the ring buffer.
  if (cmd_type == 1 /* NAVIGATE */) {
    if (arg2 < 1024) {
      memcpy(ring->commands[head].payload, (void *)arg1, arg2);
      ring->commands[head].payload_len = arg2;
    }
  } else {
    ring->commands[head].payload_len = 0;
  }
  atomic_store(&ring->head, next);
}

void sys_ipc_push_command_with_payload(uint32_t cmd_type, uint64_t arg1,
                                       uint64_t payload_ptr,
                                       uint32_t payload_len) {
  printf("[C-IPC] PUSH received: type=%u, arg1=%llu, p_ptr=%llu, p_len=%u\n", cmd_type, arg1, payload_ptr, payload_len);
  
  if (!shared_memory)
    return;
  IPCRing *ring = (IPCRing *)shared_memory;
  uint32_t head = atomic_load(&ring->head);
  uint32_t next = (head + 1) % ((RING_SIZE - 8) / sizeof(IPCCommand));
  if (next == atomic_load(&ring->tail)) {
    printf("[IPC] M2R Queue Full (payload)!\n");
    return;
  }
  ring->commands[head].type = cmd_type;
  ring->commands[head].arg1 = arg1;
  ring->commands[head].arg2 = 0;
  if (payload_ptr != 0 && payload_len > 0) {
    uint32_t copy_len = payload_len < 1024 ? payload_len : 1024;
    memcpy(ring->commands[head].payload, (void *)payload_ptr, copy_len);
    ring->commands[head].payload_len = copy_len;
  } else {
    ring->commands[head].payload_len = 0;
  }
  atomic_store(&ring->head, next);
}

// Read Main -> Renderer (Salt Runtime)
// We need to return multiple values. Salt FFI uses multiple return registers
// but easiest is to pass pointers or return a packed u64 or struct pointer.
// Since Salt run_loop needs the command type and args, we can populate a global
// struct or memory buffer.
SaltIPCCommand global_ipc_read_cmd;

SaltIPCCommand *sys_ipc_read_m2r_command(void) {
  if (!shared_memory)
    return NULL;
  IPCRing *ring = (IPCRing *)shared_memory;
  uint32_t tail = atomic_load(&ring->tail);
  if (tail == atomic_load(&ring->head))
    return NULL; // Empty

  IPCCommand *cmd = &ring->commands[tail];
  // Copy out to safe static buffer for Salt
  global_ipc_read_cmd.type = cmd->type;
  global_ipc_read_cmd.arg1 = cmd->arg1;
  global_ipc_read_cmd.arg2 = cmd->arg2;
  global_ipc_read_cmd.payload_ptr = (uint64_t)cmd->payload;
  global_ipc_read_cmd.payload_len = cmd->payload_len;

  printf("[C-IPC] READ returning: ptr=%p (type=%u, len=%u)\n", &global_ipc_read_cmd, global_ipc_read_cmd.type, global_ipc_read_cmd.payload_len);
  uint8_t *b = (uint8_t*)&global_ipc_read_cmd;
  printf("[C-IPC] BYTES: ");
  for (int i=0; i<40; i++) { printf("%02x ", b[i]); }
  printf("\n");

  atomic_store(&ring->tail,
               (tail + 1) % ((RING_SIZE - 8) / sizeof(IPCCommand)));
  return &global_ipc_read_cmd;
}

// ============================================================================
// Stubs for headless testing
// ============================================================================
__attribute__((weak)) uint64_t sys_time_get_ticks(void) { return 0; }

__attribute__((weak)) void sys_tls_upgrade_to_websocket(uint32_t conn_id,
                                                        uint64_t path_ptr,
                                                        uint32_t path_len,
                                                        uint64_t origin_ptr,
                                                        uint32_t origin_len) {
  // Stub
}

static int gpu_mode_log_once = 0;
__attribute__((weak)) uint8_t sys_gpu_is_iosurface_mode(void) {
  uint8_t mode = shared_memory ? 1 : 0;
  if (!gpu_mode_log_once) {
    fprintf(stderr,
            "[GPU-DIAG] sys_gpu_is_iosurface_mode() = %d (shared_memory=%p)\n",
            mode, shared_memory);
    gpu_mode_log_once = 1;
  }
  return mode;
}
__attribute__((weak)) void sys_gpu_rasterize_iosurface(uint64_t rects,
                                                       int32_t w, int32_t h,
                                                       int32_t count,
                                                       float scrollY) {
  fprintf(stderr, "[GPU-DIAG] Rasterizing %d quads to IOSurface\n", count);
}
__attribute__((weak)) void sys_gpu_set_scissor_rect(float x, float y, float w,
                                                    float h) {}
__attribute__((weak)) void sys_hw_audio_init(void) {}
__attribute__((weak)) void sys_init_vsync(void) {}
__attribute__((weak)) void sys_init_ws_class(void *env) {}
void sys_invalidate_paint(void) {}
void sys_js_dispatch_popstate(void) {}
void sys_js_evaluate_script(uint64_t ptr, uint32_t len) {}
void sys_memcpy(uint64_t dst, uint64_t src, uint32_t len) {
  memcpy((void *)dst, (void *)src, len);
}
uint64_t sys_mmap_file(uint64_t path_ptr, uint32_t path_len,
                       uint64_t *out_len) {
  return 0;
}
__attribute__((weak)) void sys_sleep_ms(uint32_t ms) { usleep(ms * 1000); }
__attribute__((weak)) void js_resolve_fetch_impl(uint32_t id, uint64_t head_ptr, uint32_t head_len,
                           uint64_t bd_ptr, uint32_t bd_len, uint32_t c) {}
__attribute__((weak)) void sys_atomic_write_u8(uint8_t *ptr, uint8_t val) {}
float sys_clock_get_ms(void) { return 0.0f; }
void sys_flag_gpu_redraw(void) {}
static uint32_t fake_iosurface_id = 42;
__attribute__((weak)) void sys_gpu_commit_iosurface(void) {
  if (shared_memory) {
    fprintf(stderr,
            "[GPU-DIAG] sys_gpu_commit_iosurface -> sending CMD_NEW_FRAME\n");
    sys_ipc_send_r2m_command(1, (uint64_t)fake_iosurface_id);
  }
}
__attribute__((weak)) void sys_gpu_init_iosurface(int32_t w, int32_t h) {
  fprintf(stderr, "[GPU-DIAG] IOSurface mode initialized (%dx%d)\n", w, h);
}
void js_bridge_dispatch_event(uint32_t node_id, uint32_t type) {}
void js_bridge_dispatch_worker_message(uint64_t ptr, uint32_t len) {}
void js_execute_worker_jobs(void) {}
void js_resolve_fetch_chunk(uint32_t id, uint64_t ptr, uint32_t len) {}
void js_bridge_dispatch_main_message(uint64_t ptr, uint32_t len) {}
void js_bridge_dispatch_message_event(uint32_t node_id, uint64_t ptr,
                                      uint32_t len) {}
void js_bridge_dispatch_websocket_message(uint32_t conn_id, uint64_t ptr,
                                          uint32_t len, uint32_t is_binary) {}
void ext_net_tls_handshake(uint32_t fd, uint32_t id) {}
void js_bridge_dispatch_document_event(uint32_t node_id, uint32_t type) {}
void disabled_ext_tls_write_bytes(uint64_t data_ptr, uint32_t len) {}

const char *mock_http =
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n"
    "<!DOCTYPE html><html>"
    "<head><style>"
    "body { margin: 0; background-color: #0f172a; color: #f8fafc; font-family: "
    "-apple-system, sans-serif; display: flex; flex-direction: column; "
    "align-items: center; justify-content: center; height: 1080px; }"
    ".container { text-align: center; background: rgba(30, 41, 59, 0.7); "
    "padding: 40px; border-radius: 16px; border: 1px solid #334155; }"
    "h1 { color: #38bdf8; font-size: 48px; margin-bottom: 10px; }"
    "h2 { color: #94a3b8; font-size: 24px; font-weight: normal; margin-top: 0; "
    "}"
    ".ring { width: 64px; height: 64px; border: 4px solid #38bdf8; "
    "border-top-color: transparent; border-radius: 50%; margin: 40px auto; }"
    "</style></head>"
    "<body>"
    "<div class=\"container\">"
    "<h1>KeuOS Matrix</h1>"
    "<h2>Multiprocess Engine Active • Hardware VSync 60fps • No DOM Polyfill "
    "Loaded</h2>"
    "<div class=\"ring\"></div>"
    "</div>"
    "</body></html>";

__attribute__((weak)) uint64_t ext_get_mock_http_ptr(uint64_t url_ptr, uint32_t url_len) {
  fprintf(stderr, "[MOCK-DIAG] ext_get_mock_http_ptr() called, len=%lu\n",
          strlen(mock_http));
  return (uint64_t)mock_http;
}

__attribute__((weak)) uint32_t ext_get_mock_http_len() { return (uint32_t)strlen(mock_http); }
void ext_dom_set_custom_tag(uint32_t node_id, uint64_t ptr, uint32_t len) {}
uint32_t disabled_ext_hpack_encode_headers(uint64_t m_ptr, uint64_t p_ptr,
                                           uint32_t sid) {
  return 0;
}
uint64_t disabled_ext_hpack_get_buffer_ptr(void) { return 0; }
__attribute__((weak)) void ext_mac_update_omnibox(uint64_t ptr, uint32_t len) {}
void disabled_decode_hpack_block(uint32_t stream_id, uint64_t ptr,
                                 uint32_t len) {}
typedef struct {
  uint32_t glyph_id;
  float x_advance;
  float y_advance;
  float x_offset;
  float y_offset;
} ShapedGlyph_IPC;
__attribute__((weak)) uint32_t sys_shape_text(const char *text, uint32_t len,
                                              ShapedGlyph_IPC *out_buffer,
                                              uint32_t max_glyphs) {
  return 0;
}

__attribute__((weak)) float ext_c_shape_and_measure(uint32_t node_id,
                                                    uint64_t text_ptr,
                                                    uint32_t text_len) {
  if (text_ptr == 0 || text_len == 0)
    return 0.0f;
  if (!sys_shape_text)
    return 0.0f; // HarfBuzz not linked (mac_app process)
  ShapedGlyph_IPC buf[1024];
  uint32_t count = sys_shape_text((const char *)text_ptr, text_len, buf, 1024);
  float total_w = 0.0f;
  for (uint32_t i = 0; i < count; i++) {
    total_w += buf[i].x_advance;
  }
  return total_w;
}

// Epic 90: Telemetry runtime stubs
#include <time.h>
void sys_print_str(const char *ptr, uint32_t len) {
  if (ptr == 0 || len == 0)
    return;
  fwrite(ptr, 1, len, stdout);
  fflush(stdout);
}
void sys_log_int(int32_t val) {
  printf("%d", val);
  fflush(stdout);
}
void sys_print_float(double f) {
  printf("%.2f", f);
  fflush(stdout);
}

void sys_layout_dump(uint32_t n, uint32_t tag, uint8_t disp, int32_t sh, int32_t sw, uint32_t par) {
  printf("[LAYOUT DUMP] Node:%-4u Tag:%2u Disp:%u w:%-5d h:%-5d parent:%u\n", n, tag, disp, sw, sh, par);
  fflush(stdout);
}

#include <mach/mach_time.h>

double sys_time_now_ms() {
  static mach_timebase_info_data_t timebase_info = {0, 0};

  // Initialize the timebase fraction exactly once
  if (timebase_info.denom == 0) {
    mach_timebase_info(&timebase_info);
  }

  uint64_t ticks = mach_absolute_time();

  // Convert ticks to pure nanoseconds
  double nanoseconds =
      (double)ticks * (double)timebase_info.numer / (double)timebase_info.denom;

  // The Crucial Division: Downshift to Milliseconds for the Salt matrix
  return nanoseconds / 1000000.0;
}

uint64_t sys_time_now_ms_int() {
  static mach_timebase_info_data_t timebase_info = {0, 0};
  if (timebase_info.denom == 0) {
    mach_timebase_info(&timebase_info);
  }
  uint64_t ticks = mach_absolute_time();
  double nanoseconds =
      (double)ticks * (double)timebase_info.numer / (double)timebase_info.denom;
  return (uint64_t)(nanoseconds / 1000000.0);
}

void sys_media_pump_decrypt_queue(void) {}
void ext_cdm_sandbox_init(void) {}
static int commit_log_count = 0;
void sys_gpu_commit(void) {
  if (shared_memory) {
    if (commit_log_count < 5) {
      fprintf(stderr,
              "[GPU-DIAG] sys_gpu_commit -> sending CMD_NEW_FRAME (frame %d)\n",
              commit_log_count);
    }
    commit_log_count++;
    sys_ipc_send_r2m_command(1, (uint64_t)fake_iosurface_id);
  }
}

// Stubs for main.salt functions (not in TEST_DEPS)
__attribute__((weak)) void sys_browser_navigate(uint64_t ptr, uint32_t len) {}
__attribute__((weak)) void sys_js_pump_script_queue(void) {}
__attribute__((weak)) void set_frame_count(int32_t count) {}
__attribute__((weak)) int32_t get_frame_count(void) { return 0; }
__attribute__((weak)) int32_t get_max_test_frames(void) { return 0; }
__attribute__((weak)) void construct_search_url_and_navigate(uint64_t a_ptr, uint32_t a_len, uint64_t n_ptr, uint32_t n_len, uint64_t v_ptr, uint32_t v_len) {}
__attribute__((weak)) int32_t check_any_layout_dirty(void) { return 0; }
__attribute__((weak)) void set_dom_content_loaded_fired(void) {}
__attribute__((weak)) int32_t get_dom_content_loaded_fired(void) { return 0; }
__attribute__((weak)) void pump_websocket_frames(void) {}
__attribute__((weak)) void pump_storage_queue(void) {}
__attribute__((weak)) void js_engine_pump_microtasks(void) {}
__attribute__((weak)) void app_run_loop(void) {}
__attribute__((weak)) int32_t find_form_ancestor(uint32_t n) { return 0; }
__attribute__((weak)) int32_t dom_get_attr_action_idx(uint32_t n) { return -1; }
__attribute__((weak)) int32_t dom_get_attr_name_idx(uint32_t n) { return -1; }
__attribute__((weak)) uint64_t js_get_attr_val_ptr(uint32_t i) { return 0; }
__attribute__((weak)) uint32_t js_get_attr_val_len(uint32_t i) { return 0; }
__attribute__((weak)) void ext_engine_dispatch_js_keyboard_event(uint32_t n, uint8_t k) {}
__attribute__((weak)) void apply_rules_to_node(uint32_t idx) {}

// Stubs for layout.salt FFI wrappers
__attribute__((weak)) void ext_flush_frame(int32_t w, int32_t h) {}
__attribute__((weak)) int32_t ext_get_layout_x(uint32_t n) { return 0; }
__attribute__((weak)) int32_t ext_get_layout_y(uint32_t n) { return 0; }
__attribute__((weak)) int32_t ext_get_layout_w(uint32_t n) { return 0; }
__attribute__((weak)) int32_t ext_get_layout_h(uint32_t n) { return 0; }
__attribute__((weak)) void ext_salt_invalidate_layout(uint32_t n) {}
__attribute__((weak)) void ext_salt_invalidate_all_layout(void) {}
__attribute__((weak)) uint32_t ext_salt_resolve_node(uint64_t id) { return 0; }
__attribute__((weak)) uint64_t ext_salt_create_text_node(uint64_t _p, uint32_t _l) { return 0; }

// Stubs for net/hpack
__attribute__((weak)) void ext_tls_write_bytes(uint64_t p, uint32_t l) {}
__attribute__((weak)) uint64_t ext_hpack_get_static_key(uint32_t i) { return 0; }
__attribute__((weak)) uint64_t ext_hpack_get_static_val(uint32_t i) { return 0; }
__attribute__((weak)) void ext_net_route_header_to_stream(uint32_t sid, uint64_t kp, uint32_t kl, uint64_t vp, uint32_t vl) {}

// Stubs for http/lexer
__attribute__((weak)) void http_reset_state(void) {}
__attribute__((weak)) void http_set_root_node(uint64_t id) {}
__attribute__((weak)) void http_process_ingress(uint64_t p, uint32_t l) {}
__attribute__((weak)) uint8_t http_get_eof_reached(void) { return 0; }
__attribute__((weak)) void http_set_eof(void) {}
__attribute__((weak)) void js_lex_html_chunk(uint64_t rid, uint64_t p, uint32_t l, uint8_t e) {}
__attribute__((weak)) uint64_t dom_alloc_text(uint32_t l) { return 0; }
__attribute__((weak)) void js_resolve_fetch(uint64_t id, uint64_t p, uint32_t l) {}
__attribute__((weak)) int32_t complete_script_fetch(uint64_t id, uint64_t p, uint32_t l) { return -1; }
__attribute__((weak)) void dom_add_scroll_y(int32_t d) {}
__attribute__((weak)) void native_go_back(void) {}

// JSC stubs
__attribute__((weak)) void sys_jsc_init(void) {}
__attribute__((weak)) void sys_jsc_evaluate_script(uint64_t p, uint32_t l, uint64_t f) {}
__attribute__((weak)) void sys_jsc_flush_microtasks(void) {}
__attribute__((weak)) void sys_typography_init(void) {}


void sys_ipc_recv_full(unsigned long msg_out) {}
