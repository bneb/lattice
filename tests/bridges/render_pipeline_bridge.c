#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ═══════════════════════════════════════════════════════════════
// Epic 99: Render Pipeline Integration Test Bridge
// Headless validation of: DOM → Layout → Paint → GPU Rect Pack
// ═══════════════════════════════════════════════════════════════

// --- Salt Engine Init Functions ---
extern void ext_salt_airlock_init_allocator(void);
extern void ext_salt_init_arrays(void);
extern void ext_salt_layout_inject_dom_pointers(void);
extern void ext_salt_paint_inject_dom_pointers(void);

// --- DOM ---
extern uint64_t ext_salt_create_node(uint32_t tag);
extern void ext_salt_append_child(uint64_t parent, uint64_t child);
extern void http_set_root_node(uint64_t node_id);
extern void http_set_eof(void);
extern void ext_salt_invalidate_all_layout(void);
extern void js_lex_html_chunk(uint64_t root_id, uint64_t buf_ptr, uint64_t len,
                              uint64_t is_final);
extern uint32_t ext_get_dom_node_count(void);
extern uint64_t resolve_node_by_id(uint64_t ptr, uint32_t len);

// --- CSS ---
extern void user__browser__css__init_css_defaults(void);

// --- Layout ---
extern void ext_layout_tree(void);

// --- Paint ---
extern void ext_paint_begin_frame(void);
extern void ext_paint_tree(void);
extern uint32_t ext_paint_get_cmd_count(void);

// --- Font & Compositior ---
extern void user__browser__font__init_glyphs(void);
extern void user__browser__compositor__load_font_atlas(uint64_t pixels,
                                                       int32_t width,
                                                       int32_t height);
extern uint8_t user__browser__font__SDF_ATLAS[1048576];

// --- Pipeline ---
extern void transpile_dom_tree(uint32_t node_id);
extern void apply_cascade_to_tree(void);
extern uint32_t user__browser__css_lexer__RULE_COUNT;

// --- Compositor (flush_frame + GPU rect buffer) ---
extern void flush_frame(int32_t width, int32_t height);
extern uint64_t get_gpu_buffer_ptr(void);
extern int32_t compositor_get_rect_count(void);

// --- DOM style arrays for setting up the test fixture ---
// Canonical setters through @no_mangle functions
// (avoids weak_odr duplication — these write to the real SoA arrays)
extern void dom_set_style_width(uint32_t idx, int32_t val);
extern void dom_set_style_height(uint32_t idx, int32_t val);
extern void dom_set_style_w_unit(uint32_t idx, uint8_t unit);
extern void dom_set_style_h_unit(uint32_t idx, uint8_t unit);
extern void dom_set_style_display(uint32_t idx, uint8_t val);
// Layout getters through canonical @no_mangle accessors
// (avoids weak_odr duplication — layout writes to injected pointers)
extern float dom_get_layout_w(uint32_t idx);
extern float dom_get_layout_h(uint32_t idx);
// Style getters through canonical @no_mangle accessors
extern int32_t dom_get_style_w(uint32_t idx);
extern int32_t dom_get_style_h(uint32_t idx);
extern uint8_t dom_get_style_display(uint32_t idx);

// --- Paint SoA arrays for direct verification ---
extern uint32_t user__browser__dom__DOM_NODE_COUNT;
extern uint32_t user__browser__dom__DOM_NODE_CURRENT_MAX;

// --- Expose the raw Rust/Salt style node layouts to C bridge ---
// (We provide them here to prevent linker errors on the test suite end)
extern int user__browser__dom__LAYOUT_X[8192];
extern int user__browser__dom__LAYOUT_Y[8192];
extern int user__browser__dom__LAYOUT_W[8192];
extern int user__browser__dom__LAYOUT_H[8192];

// Core node representation
extern int32_t user__browser__paint__CMD_X[8192];
extern int32_t user__browser__paint__CMD_Y[8192];
extern int32_t user__browser__paint__CMD_W[8192];
extern int32_t user__browser__paint__CMD_H[8192];
extern uint8_t user__browser__paint__CMD_R[8192];
extern uint8_t user__browser__paint__CMD_G[8192];
extern uint8_t user__browser__paint__CMD_B[8192];
extern uint8_t user__browser__paint__CMD_A[8192];

// ═══════════════════════════════════════════════════════════════
// GPU / IPC stubs (headless — no real Metal or IPC)
// ═══════════════════════════════════════════════════════════════

static int flush_frame_called = 0;
static int rasterize_called = 0;
static int rasterize_rect_count = 0;
static int commit_called = 0;

int sys_gpu_is_iosurface_mode(void) { return 1; }

void sys_gpu_rasterize_iosurface(void *rects, int width, int height,
                                 int rect_count, float scroll_y) {
  rasterize_called = 1;
  rasterize_rect_count = rect_count;
  printf("  [STUB] sys_gpu_rasterize_iosurface: %d rects, %dx%d\n", rect_count,
         width, height);
}

void sys_gpu_commit_iosurface(void) {
  commit_called = 1;
  printf("  [STUB] sys_gpu_commit_iosurface\n");
}

// C-ABI trampoline has been abstracted strictly to mock_gui_stubs.c

void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}

// Frame/test management
static uint64_t g_frame_count = 0;
static uint64_t g_max_frames = 5;
void set_frame_count(uint64_t v) { g_frame_count = v; }
uint64_t get_frame_count(void) { return g_frame_count; }
void set_max_test_frames(uint64_t v) { g_max_frames = v; }
uint64_t get_max_test_frames(void) { return g_max_frames; }
void set_dom_content_loaded_fired(uint8_t v) {}
uint8_t get_dom_content_loaded_fired(void) { return 1; }

// Misc stubs
__attribute__((weak)) void sys_browser_navigate(uint64_t p, uint32_t l) {}
void sys_typography_init(void) {}
void pump_websocket_frames(void) {}
void sys_net_init_h2_connection(uint64_t h) {}
void ext_tls_write_bytes(uint64_t d, uint32_t l) {}

// ═══════════════════════════════════════════════════════════════
// Component 1: Script Execution Pump (replaces void no-op)
// Drains the script queue populated by the HTML lexer and
// evaluates each script through JavaScriptCore.
// ═══════════════════════════════════════════════════════════════

extern uint64_t js_dequeue_script_ptr(void);
extern uint32_t js_dequeue_script_len(void);
extern void sys_jsc_evaluate_script(uint64_t ptr, uint32_t len, uint64_t f_ptr, uint32_t f_len);

void sys_js_pump_script_queue(void) {
  int scripts_pumped = 0;
  while (1) {
    uint64_t s_ptr = js_dequeue_script_ptr();
    if (s_ptr == 0)
      break;
    uint32_t s_len = js_dequeue_script_len();
    if (s_len == 0)
      continue;
    printf("  [PUMP] Evaluating queued script: %u bytes\n", s_len);
    sys_jsc_evaluate_script(s_ptr, s_len, 0, 0);
    scripts_pumped++;
  }
  if (scripts_pumped > 0) {
    printf("  [PUMP] Pumped %d inline scripts\n", scripts_pumped);
  }
}

// ═══════════════════════════════════════════════════════════════
// Component 3: IPC Mock Router for External Script Fetches
// Intercepts CMD_FETCH_REQUEST (type 12) with script bit 60,
// synthesizes a mock JS response, and re-pumps the script queue.
// ═══════════════════════════════════════════════════════════════

extern int32_t complete_script_fetch(uint64_t fetch_id, uint64_t buf_ptr,
                                     uint32_t buf_len);

// Mock JS payload for external script responses — creates an INPUT element
static const char *mock_external_js =
    "document.body.appendChild(document.createElement('input'));";

void sys_ipc_send_r2m_command_with_payload(uint32_t cmd_type, uint64_t arg1,
                                           uint64_t payload_ptr,
                                           uint32_t payload_len) {
  if (cmd_type == 12 /* CMD_FETCH_REQUEST */) {
    uint64_t script_bit = (uint64_t)1 << 60;
    if (arg1 & script_bit) {
      // External script fetch — extract fetch_id and synthesize response
      uint64_t fetch_id = arg1 & ~script_bit;
      printf("  [IPC-MOCK] CMD_FETCH_REQUEST script fetch_id=%llu\n", fetch_id);

      uint32_t mock_len = (uint32_t)strlen(mock_external_js);
      int32_t slot =
          complete_script_fetch(fetch_id, (uint64_t)mock_external_js, mock_len);
      if (slot >= 0) {
        printf("  [IPC-MOCK] Script fetch completed in slot %d, "
               "queuing for execution\n",
               slot);
        // The script content is now in the fetch queue; we also need to
        // enqueue it into the execution queue so it gets pumped
        extern void js_queue_script(uint64_t ptr, uint32_t len);
        js_queue_script((uint64_t)mock_external_js, mock_len);
      }
      return;
    }
    // Non-script fetch (e.g. navigation) — log and drop
    printf("  [IPC-MOCK] CMD_FETCH_REQUEST (non-script) arg1=0x%llx — "
           "dropped\n",
           arg1);
    return;
  }
  // All other commands — silent drop in headless mode
}

// Mangled alias for Salt --lib linkage compatibility
void user__browser__ipc_shared__sys_ipc_send_r2m_command_with_payload(
    uint32_t cmd_type, uint64_t arg1, uint64_t payload_ptr,
    uint32_t payload_len) {
  sys_ipc_send_r2m_command_with_payload(cmd_type, arg1, payload_ptr,
                                       payload_len);
}

// HPACK stubs
uint64_t ext_hpack_get_static_key(uint32_t i) { return 0; }
uint64_t ext_hpack_get_static_val(uint32_t i) { return 0; }
void ext_net_route_header_to_stream(uint32_t s, uint64_t kp, uint32_t kl,
                                    uint64_t vp, uint32_t vl) {}

// ═══════════════════════════════════════════════════════════════
// RenderPrimitive struct — must match facet_gpu.m layout (80 bytes)
// ═══════════════════════════════════════════════════════════════

typedef struct __attribute__((packed)) {
  float x, y, w, h;
  float uv_x, uv_y, uv_w, uv_h;
  uint32_t color;
  uint32_t type;
  float border_radius;
  uint32_t shadow_color;
  float shadow_x, shadow_y, shadow_blur, shadow_spread;
  float transform_x, transform_y;
  float opacity;
  float pad;
} RenderPrimitive;

// ═══════════════════════════════════════════════════════════════
// Main Test Routine
// ═══════════════════════════════════════════════════════════════

// Dummy implementations for linker
__attribute__((weak)) void css_arena_inc_count(void) {}
__attribute__((weak)) void css_arena_set_hash(void) {}
__attribute__((weak)) void ext_engine_process_key_down(void) {}
__attribute__((weak)) void ext_engine_process_mouse_down(void) {}
__attribute__((weak)) void ext_salt_paint_inject_dom_pointers(void) {}
__attribute__((weak)) uint32_t hash_string(uint64_t ptr, uint32_t len) { return 0; }
__attribute__((weak)) void user__browser__compositor__load_font_atlas(uint64_t pixels, int32_t width, int32_t height) {}

int render_pipeline_e2e_test(void) {
  int failures = 0;

  // ─── Phase 1: Initialize Engine ───
  printf("[Phase 1] Initializing engine subsystems...\n");
  ext_salt_airlock_init_allocator();
  ext_salt_init_arrays();
  ext_salt_layout_inject_dom_pointers();
  ext_salt_paint_inject_dom_pointers();
  user__browser__css__init_css_defaults();
  user__browser__font__init_glyphs();
  user__browser__compositor__load_font_atlas(
      (uint64_t)user__browser__font__SDF_ATLAS, 1024, 1024);

  // ─── Phase 2: Create DOM tree with styled content ───
  printf("[Phase 2] Creating styled DOM tree...\n");

  // Root node (TAG_HTML = 1)
  uint64_t root = ext_salt_create_node(1);
  uint32_t root_idx =
      (uint32_t)(root & 0xFFFF); // Extract node index from packed ID
  printf("  Root packed ID: 0x%llx, index: %u\n", root, root_idx);

  dom_set_style_width(root_idx, 1920);
  dom_set_style_height(root_idx, 1080);
  dom_set_style_w_unit(root_idx, 0);  // PX
  dom_set_style_h_unit(root_idx, 0);  // PX
  dom_set_style_display(root_idx, 1); // BLOCK

  // Inject minimal HTML: <div
  // style="background:red;width:200px;height:100px">Test</div>
  const char *html =
      "<div style=\"background-color:rgb(255,0,0);width:200px;height:100px\">"
      "Hello</div>";
  http_set_root_node(root);
  js_lex_html_chunk(root, (uint64_t)html, strlen(html), 1);
  http_set_eof();

  uint32_t node_count = ext_get_dom_node_count();
  printf("  DOM nodes created: %u\n", node_count);
  if (node_count < 2) {
    printf("  [FAIL] Expected >= 2 DOM nodes, got %u\n", node_count);
    failures++;
  } else {
    printf("  [OK] DOM nodes: %u\n", node_count);
  }

  // ─── Phase 3: Layout ───
  printf("[Phase 3] Running layout solver...\n");
  
  ext_salt_invalidate_all_layout();
  ext_layout_tree();

  int32_t root_w = (int32_t)dom_get_layout_w(root_idx);
  int32_t root_h = (int32_t)dom_get_layout_h(root_idx);
  printf("  Root layout: %dx%d\n", root_w, root_h);

  if (root_w <= 0) {
    printf("  [FAIL] Root layout width = %d (expected > 0)\n", root_w);
    failures++;
  } else {
    printf("  [OK] Root layout width: %d\n", root_w);
  }

  // ─── Phase 4: Paint ───
  printf("[Phase 4] Running paint phase...\n");
  ext_paint_begin_frame();
  ext_paint_tree();

  uint32_t cmd_count = ext_paint_get_cmd_count();
  printf("  Paint commands: %u\n", cmd_count);

  if (cmd_count == 0) {
    printf("  [FAIL] Paint produced 0 commands\n");
    failures++;
  } else {
    printf("  [OK] Paint produced %u commands\n", cmd_count);

    // Verify first paint command has valid dimensions
    int32_t first_w = user__browser__paint__CMD_W[0];
    int32_t first_h = user__browser__paint__CMD_H[0];
    printf("  First rect: w=%d h=%d\n", first_w, first_h);

    if (first_w <= 0 || first_h <= 0) {
      printf("  [FAIL] First paint rect has zero dimensions\n");
      failures++;
    } else {
      printf("  [OK] First paint rect: %dx%d\n", first_w, first_h);
    }
  }

  // ─── Phase 5: Compositor flush_frame (GPU rect buffer pack) ───
  printf("[Phase 5] Testing flush_frame (compositor pack + IOSurface "
         "dispatch)...\n");

  // Reset tracking
  flush_frame_called = 0;
  rasterize_called = 0;
  rasterize_rect_count = 0;
  commit_called = 0;

  // This is the critical path we fixed:
  // flush_frame packs SoA → GPU_RECT_BUF → sys_gpu_rasterize_iosurface →
  // sys_gpu_commit_iosurface
  flush_frame(1920, 1080);

  if (!rasterize_called) {
    printf("  [FAIL] sys_gpu_rasterize_iosurface was NOT called\n");
    failures++;
  } else {
    printf("  [OK] sys_gpu_rasterize_iosurface called with %d rects\n",
           rasterize_rect_count);
  }

  if (!commit_called) {
    printf("  [FAIL] sys_gpu_commit_iosurface was NOT called\n");
    failures++;
  } else {
    printf("  [OK] sys_gpu_commit_iosurface called\n");
  }

  if (rasterize_rect_count != (int)cmd_count) {
    printf("  [FAIL] Rasterized %d rects but paint produced %u\n",
           rasterize_rect_count, cmd_count);
    failures++;
  } else {
    printf("  [OK] Rect count matches: %d\n", rasterize_rect_count);
  }

  // ─── Phase 6: Verify GPU_RECT_BUF packed data ───
  printf("[Phase 6] Verifying GPU_RECT_BUF packed primitives...\n");

  uint64_t buf_ptr = get_gpu_buffer_ptr();
  if (buf_ptr == 0) {
    printf("  [FAIL] GPU_RECT_BUF pointer is NULL\n");
    failures++;
  } else {
    RenderPrimitive *prims = (RenderPrimitive *)buf_ptr;
    int valid_prims = 0;

    for (int i = 0; i < rasterize_rect_count && i < 32; i++) {
      if (prims[i].w > 0.0f && prims[i].h > 0.0f) {
        valid_prims++;
      }
    }

    printf("  Valid primitives (w>0, h>0): %d / %d\n", valid_prims,
           rasterize_rect_count);

    if (valid_prims == 0) {
      printf("  [FAIL] No valid primitives in GPU_RECT_BUF\n");
      failures++;
    } else {
      printf("  [OK] %d valid primitives packed into GPU_RECT_BUF\n",
             valid_prims);

      // Log first primitive for diagnostics
      printf("  Prim[0]: x=%.0f y=%.0f w=%.0f h=%.0f color=0x%08X "
             "type=%u opacity=%.2f\n",
             prims[0].x, prims[0].y, prims[0].w, prims[0].h, prims[0].color,
             prims[0].type, prims[0].opacity);
    }
  }

  // ─── Phase 7: Google Fixture Render & Rect Dump ───
  printf("[Phase 7] Rendering Google fixture for visual comparison...\n");

  // Re-init for Google fixture
  ext_salt_airlock_init_allocator();
  ext_salt_init_arrays();
  ext_salt_layout_inject_dom_pointers();
  ext_salt_paint_inject_dom_pointers();
  user__browser__css__init_css_defaults();
  user__browser__font__init_glyphs();
  user__browser__compositor__load_font_atlas(
      (uint64_t)user__browser__font__SDF_ATLAS, 1024, 1024);

  uint64_t g_root = ext_salt_create_node(1);
  uint32_t g_root_idx = (uint32_t)(g_root & 0xFFFF);
  dom_set_style_width(g_root_idx, 1920);
  dom_set_style_height(g_root_idx, 1080);
  dom_set_style_w_unit(g_root_idx, 0);
  dom_set_style_h_unit(g_root_idx, 0);
  dom_set_style_display(g_root_idx, 1);

  http_set_root_node(g_root);

  // Load Google HTML fixture from disk
  FILE *f = fopen("tests/fixtures/google_snapshot.html", "rb");
  if (!f) {
    printf("  [SKIP] Google fixture not found\n");
  } else {
    fseek(f, 0, SEEK_END);
    long fsize = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *html_buf = (char *)malloc(fsize + 1);
    fread(html_buf, 1, fsize, f);
    html_buf[fsize] = 0;
    fclose(f);

    printf("  Loaded %ld bytes of Google HTML\n", fsize);
    js_lex_html_chunk(g_root, (uint64_t)html_buf, fsize, 1);
    http_set_eof();
    free(html_buf);

    uint32_t g_nodes = ext_get_dom_node_count();
    printf("  DOM nodes: %u\n", g_nodes);

    transpile_dom_tree(g_root_idx);
    printf("  [CSS] Rule count before cascade: %u\n",
           user__browser__css_lexer__RULE_COUNT);
    apply_cascade_to_tree();

    ext_salt_invalidate_all_layout();
    ext_layout_tree();

    int32_t g_rw = (int32_t)dom_get_layout_w(g_root_idx);
    printf("  Layout root width: %d\n", g_rw);

    FILE *layout_csv = fopen("tests/output/prisimi_layout_bounds.csv", "w");
    if (layout_csv) {
      fprintf(layout_csv, "x,y,w,h,tag\n");
      extern uint32_t user__browser__dom__DOM_NODE_TAG[65536];
      for (uint32_t i = 1; i <= g_nodes; i++) {
        int w = (int)dom_get_layout_w(i);
        int h = (int)dom_get_layout_h(i);
        if (w > 0 && h > 0) {
          int x = user__browser__dom__LAYOUT_X[i];
          int y = user__browser__dom__LAYOUT_Y[i];
          uint32_t tag = user__browser__dom__DOM_NODE_TAG[i];
          fprintf(layout_csv, "%d,%d,%d,%d,%u\n", x, y, w, h, tag);
        }
      }
      fclose(layout_csv);
      printf("  [OK] Dumped layout bounds to tests/output/prisimi_layout_bounds.csv\n");
    }

    extern uint32_t user__browser__dom__DOM_NODE_TAG[65536];
    int text_nodes = 0;
    int text_laid_out = 0;
    for (uint32_t i = 1; i <= g_nodes; i++) {
      if (user__browser__dom__DOM_NODE_TAG[i] == 0) { // TAG_TEXT == 0
        text_nodes++;
        if (dom_get_layout_w(i) > 0 &&
            dom_get_layout_h(i) > 0) {
          text_laid_out++;
        }
      }
    }
    printf("  [DIAG] Text nodes: %d (%d with W>0 H>0)\n", text_nodes,
           text_laid_out);

    ext_paint_begin_frame();
    ext_paint_tree();
    uint32_t g_cmd = ext_paint_get_cmd_count();
    printf("  Paint rects: %u\n", g_cmd);

    // Pack rects so GPU_RECT_BUF is filled
    flush_frame(1920, 1080);

    // Dump all rects to CSV for Python comparison rendering
    uint64_t g_buf = get_gpu_buffer_ptr();
    FILE *csv = fopen("tests/output/prisimi_google_rects.csv", "w");
    if (csv && g_buf) {
      fprintf(csv, "x,y,w,h,r,g,b,a,type,opacity\n");
      RenderPrimitive *p = (RenderPrimitive *)g_buf;
      for (uint32_t i = 0; i < g_cmd && i < 2000; i++) {
        uint8_t r = (p[i].color >> 24) & 0xFF;
        uint8_t g = (p[i].color >> 16) & 0xFF;
        uint8_t b = (p[i].color >> 8) & 0xFF;
        uint8_t a = (p[i].color) & 0xFF;
        fprintf(csv, "%.0f,%.0f,%.0f,%.0f,%u,%u,%u,%u,%u,%.2f\n", p[i].x,
                p[i].y, p[i].w, p[i].h, r, g, b, a, p[i].type, p[i].opacity);
      }
      fclose(csv);
      printf(
          "  [OK] Dumped %u rects to tests/output/prisimi_google_rects.csv\n",
          g_cmd);
    }
  }
  // ─── Phase 8: Adversarial Inline Parsing (Integer Wrap) ───
  printf("\n[Phase 8] Testing adversarial inline height parser... \n");
  uint64_t adv_root = ext_salt_create_node(1); // HEAD/BODY reset
  uint32_t adv_root_idx = (uint32_t)(adv_root & 0xFFFF);
  dom_set_style_width(adv_root_idx, 1920);
  dom_set_style_height(adv_root_idx, 1080);
  dom_set_style_w_unit(adv_root_idx, 0);
  dom_set_style_h_unit(adv_root_idx, 0);
  dom_set_style_display(adv_root_idx, 1);
  http_set_root_node(adv_root);
  const char* adv_html = "<div id=\"hero\" style=\"width: 100%; height: 50%; padding: 10px; margin: 5px;\"></div>";
  js_lex_html_chunk(adv_root, (uint64_t)adv_html, strlen(adv_html), 1);
  http_set_eof();

  // Find hero ID
  uint64_t found_hero = resolve_node_by_id((uint64_t)"hero", 4);
  if (found_hero == 0) {
    printf("  [FAIL] Failed to find hero parsing adversarial html\n");
    failures++;
  } else {
    uint32_t hero_idx = (uint32_t)(found_hero & 0xFFFF);
    transpile_dom_tree(adv_root_idx);
    apply_cascade_to_tree();
    
    int hero_h = dom_get_style_h(hero_idx);
    if (hero_h == 960) {
        printf("  [OK] Hero STYLE_H cleanly bounded to 960 (no array wraparound)\n");
    } else {
        printf("  [FAIL] Hero STYLE_H wrapped bounds (%d)\n", hero_h);
        failures++;
    }
  }

  // ─── Phase 9: Dynamic JS Execution — Inline Script Creates INPUT ───
  printf("\n[Phase 9] Dynamic JS Execution Pipeline...\n");

  // Re-init for a clean DOM
  ext_salt_airlock_init_allocator();
  ext_salt_init_arrays();
  ext_salt_layout_inject_dom_pointers();
  ext_salt_paint_inject_dom_pointers();
  user__browser__css__init_css_defaults();
  user__browser__font__init_glyphs();
  user__browser__compositor__load_font_atlas(
      (uint64_t)user__browser__font__SDF_ATLAS, 1024, 1024);

  // Initialize JSC so scripts can actually execute
  extern void sys_jsc_init(void);
  sys_jsc_init();

  uint64_t js_root = ext_salt_create_node(1); // TAG_HTML
  uint32_t js_root_idx = (uint32_t)(js_root & 0xFFFF);
  dom_set_style_width(js_root_idx, 1920);
  dom_set_style_height(js_root_idx, 1080);
  dom_set_style_w_unit(js_root_idx, 0);
  dom_set_style_h_unit(js_root_idx, 0);
  dom_set_style_display(js_root_idx, 1);

  http_set_root_node(js_root);

  // HTML with an inline <script> that dynamically creates an INPUT element
  const char *js_html =
      "<body>"
      "<script>document.body.appendChild(document.createElement('input'));"
      "</script>"
      "</body>";

  uint32_t nodes_before = ext_get_dom_node_count();
  printf("  DOM nodes before JS: %u\n", nodes_before);

  js_lex_html_chunk(js_root, (uint64_t)js_html, strlen(js_html), 1);

  // CRITICAL: Pump the script queue — this is the core of Component 1.
  // Without this, the queued inline script is never evaluated.
  sys_js_pump_script_queue();

  http_set_eof();

  uint32_t nodes_after = ext_get_dom_node_count();
  printf("  DOM nodes after JS: %u\n", nodes_after);

  // Assert: JS execution must have created at least one new DOM node
  if (nodes_after <= nodes_before) {
    printf("  [FAIL] JS execution did not create new DOM nodes "
           "(before=%u, after=%u)\n",
           nodes_before, nodes_after);
    failures++;
  } else {
    printf("  [OK] JS created %u new DOM nodes\n", nodes_after - nodes_before);
  }

  // Assert: Find the INPUT tag (tag_id=18) in the DOM
  extern uint32_t user__browser__dom__DOM_NODE_TAG[65536];
  int input_node_idx = -1;
  for (uint32_t ni = 1; ni < nodes_after; ni++) {
    if (user__browser__dom__DOM_NODE_TAG[ni] == 18) { // TAG_INPUT
      input_node_idx = (int)ni;
      break;
    }
  }

  if (input_node_idx < 0) {
    printf("  [FAIL] No INPUT tag (tag_id=18) found in DOM after JS "
           "execution\n");
    failures++;
  } else {
    printf("  [OK] INPUT tag found at node index %d\n", input_node_idx);

    // Run layout and verify the INPUT gets valid bounds
    transpile_dom_tree(js_root_idx);
    apply_cascade_to_tree();
    ext_salt_invalidate_all_layout();
    ext_layout_tree();

    int32_t input_w = (int32_t)dom_get_layout_w(input_node_idx);
    int32_t input_h = (int32_t)dom_get_layout_h(input_node_idx);
    printf("  INPUT layout: w=%d h=%d\n", input_w, input_h);

    if (input_w < 0) {
      printf("  [FAIL] INPUT layout width = %d (expected >= 0)\n", input_w);
      failures++;
    } else {
      printf("  [OK] INPUT layout width: %d (intrinsic form width TBD)\n",
             input_w);
    }

    if (input_h <= 0) {
      printf("  [FAIL] INPUT layout height = %d (expected > 0)\n", input_h);
      failures++;
    } else {
      printf("  [OK] INPUT layout height: %d\n", input_h);
    }
  }

  // Teardown JSC to avoid leaking the context
  extern void sys_jsc_teardown(void);
  sys_jsc_teardown();

  // ─── Summary ───
  printf("\n=== Results: %d failures ===\n", failures);
  return failures;
}
