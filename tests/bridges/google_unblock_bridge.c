#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ═══════════════════════════════════════════════════════════════
// Google Unblock: Integration Test Bridge (Headless)
// Validates: document.write pipeline, dynamic script element
// creation, base64 data URI decoding, and flex-direction column
// constraint solver with flex-grow + justify-content.
// ═══════════════════════════════════════════════════════════════

// --- Salt Engine Init Functions ---
extern void ext_salt_airlock_init_allocator(void);
extern void ext_salt_init_arrays(void);
extern void layout_inject_dom_pointers(void);
extern void paint_inject_dom_pointers(void);

// --- DOM ---
extern uint64_t ext_salt_create_node(uint32_t tag);
extern void ext_salt_append_child(uint64_t parent, uint64_t child);
extern void http_set_root_node(uint64_t node_id);
extern void http_set_eof(void);
extern void invalidate_all_layout(void);
extern void js_lex_html_chunk(uint64_t root_id, uint64_t buf_ptr, uint64_t len,
                              uint64_t is_final);
extern uint32_t ext_get_dom_node_count(void);
extern uint64_t resolve_node_by_id(uint64_t ptr, uint32_t len);
extern void js_dom_append_child(uint32_t parent_idx, uint32_t child_idx);

// --- Script SoA ---
extern void dom_set_script_src(uint32_t node_idx, uint64_t src_ptr,
                               uint32_t src_len);
extern uint64_t dom_get_script_src_ptr(uint32_t idx);
extern uint32_t dom_get_script_src_len(uint32_t idx);
extern uint64_t queue_script_fetch(uint64_t src_ptr, uint32_t src_len);

// --- Base64 ---
extern int32_t base64_decode(const uint8_t *src, uint32_t src_len,
                             uint8_t *dst, uint32_t dst_max);
extern int32_t is_data_uri(const uint8_t *src, uint32_t len);
extern int32_t is_data_image_uri(const uint8_t *src, uint32_t len);
extern int32_t data_uri_get_payload_offset(const uint8_t *src, uint32_t len);
extern int32_t decode_data_uri(const uint8_t *src, uint32_t src_len,
                               uint8_t *dst, uint32_t dst_max);

// --- CSS ---
extern void user__browser__css__init_css_defaults(void);

// --- Layout ---
extern void user__browser__layout__layout_tree(void);
extern void transpile_dom_tree(uint32_t node_id);
extern void apply_cascade_to_tree(void);

// --- Font ---
extern void user__browser__font__init_glyphs(void);
extern void user__browser__compositor__load_font_atlas(uint64_t pixels,
                                                       int32_t width,
                                                       int32_t height);
extern uint8_t user__browser__font__SDF_ATLAS[1048576];

// --- DOM style arrays ---
extern uint8_t user__browser__dom__STYLE_DISPLAY[65536];
extern int32_t user__browser__dom__STYLE_W[65536];
extern int32_t user__browser__dom__STYLE_H[65536];
extern uint8_t user__browser__dom__STYLE_W_UNIT[65536];
extern uint8_t user__browser__dom__STYLE_H_UNIT[65536];
extern uint8_t user__browser__dom__STYLE_FLEX_DIR[65536];
extern int32_t user__browser__dom__STYLE_FLEX_GROW[65536];
extern int32_t user__browser__dom__STYLE_FLEX_BASIS[65536];
extern uint8_t user__browser__dom__STYLE_JUSTIFY_CONTENT[65536];
extern uint8_t user__browser__dom__STYLE_ALIGN_ITEMS[65536];
extern uint8_t user__browser__dom__STYLE_POSITION[65536];
extern int32_t user__browser__dom__LAYOUT_X[65536];
extern int32_t user__browser__dom__LAYOUT_Y[65536];
extern int32_t user__browser__dom__LAYOUT_W[65536];
extern int32_t user__browser__dom__LAYOUT_H[65536];
extern uint32_t user__browser__dom__DOM_NODE_TAG[65536];
extern uint8_t user__browser__dom__DOM_NODE_FETCH_STATE[65536];

// --- Script fetch SoA (from dom.salt) ---
extern uint8_t user__browser__dom__SCRIPT_FETCH_STATE[64];

// ═══════════════════════════════════════════════════════════════
// Headless Stubs
// ═══════════════════════════════════════════════════════════════

int sys_gpu_is_iosurface_mode(void) { return 0; }
void sys_gpu_rasterize_iosurface(void *r, int w, int h, int c, float s) {}
void sys_gpu_commit_iosurface(void) {}
void ext_flush_frame(int32_t w, int32_t h) {}
void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}
void sys_browser_navigate(uint64_t p, uint32_t l) {}
void sys_typography_init(void) {}
void sys_js_pump_script_queue(void) {}
void pump_websocket_frames(void) {}
void sys_net_init_h2_connection(uint64_t h) {}
void ext_tls_write_bytes(uint64_t d, uint32_t l) {}
uint64_t ext_hpack_get_static_key(uint32_t i) { return 0; }
uint64_t ext_hpack_get_static_val(uint32_t i) { return 0; }
void ext_net_route_header_to_stream(uint32_t s, uint64_t kp, uint32_t kl,
                                    uint64_t vp, uint32_t vl) {}
uint64_t get_frame_count(void) { return 0; }
uint64_t get_max_test_frames(void) { return 5; }
void set_frame_count(uint64_t v) {}
void set_max_test_frames(uint64_t v) {}
void set_dom_content_loaded_fired(uint8_t v) {}
uint8_t get_dom_content_loaded_fired(void) { return 1; }



extern uint32_t user__browser__css_lexer__RULE_COUNT;

// ═══════════════════════════════════════════════════════════════
// Initialization Helper
// ═══════════════════════════════════════════════════════════════

static void init_engine(void) {
  airlock_init_allocator();
  init_arrays();
  layout_inject_dom_pointers();
  paint_inject_dom_pointers();
  user__browser__css__init_css_defaults();
  user__browser__font__init_glyphs();
  user__browser__compositor__load_font_atlas(
      (uint64_t)user__browser__font__SDF_ATLAS, 1024, 1024);
}

// ═══════════════════════════════════════════════════════════════
// Test 1: document.write Pipeline
// ═══════════════════════════════════════════════════════════════

static int test_document_write(void) {
  int failures = 0;
  printf("\n[Test 1] document.write Pipeline\n");

  init_engine();

  uint64_t root = create_node(1);
  uint32_t root_idx = (uint32_t)(root & 0xFFFF);
  user__browser__dom__STYLE_W[root_idx] = 1920;
  user__browser__dom__STYLE_H[root_idx] = 1080;
  user__browser__dom__STYLE_W_UNIT[root_idx] = 0;
  user__browser__dom__STYLE_H_UNIT[root_idx] = 0;
  user__browser__dom__STYLE_DISPLAY[root_idx] = 1; // BLOCK

  http_set_root_node(root);
  http_set_eof();

  uint32_t before = ext_get_dom_node_count();
  printf("  DOM nodes before write: %u\n", before);

  // Simulate document.write injecting HTML
  const char *html = "<div id=\"dw-test\"><span>Written</span></div>";
  js_lex_html_chunk(root, (uint64_t)html, strlen(html), 1);

  uint32_t after = ext_get_dom_node_count();
  printf("  DOM nodes after write: %u\n", after);

  if (after <= before) {
    printf("  [FAIL] document.write did not create new DOM nodes\n");
    failures++;
  } else {
    printf("  [OK] document.write created %u new nodes\n", after - before);
  }

  // Verify the injected div is findable by ID
  uint64_t found = resolve_node_by_id((uint64_t) "dw-test", 7);
  if (found == 0) {
    printf("  [FAIL] Could not find #dw-test node\n");
    failures++;
  } else {
    printf("  [OK] Found #dw-test at node 0x%llx\n", found);
  }

  return failures;
}

// ═══════════════════════════════════════════════════════════════
// Test 2: Dynamic <script> createElement + appendChild
// ═══════════════════════════════════════════════════════════════

static int test_dynamic_script_element(void) {
  int failures = 0;
  printf("\n[Test 2] Dynamic Script Element Creation\n");

  init_engine();

  uint64_t root = create_node(1);
  uint32_t root_idx = (uint32_t)(root & 0xFFFF);
  user__browser__dom__STYLE_DISPLAY[root_idx] = 1;
  http_set_root_node(root);
  http_set_eof();

  // Create a <script> element (tag_id=99)
  uint64_t script_node = create_node(99);
  uint32_t script_idx = (uint32_t)(script_node & 0xFFFF);

  uint32_t tag = user__browser__dom__DOM_NODE_TAG[script_idx];
  if (tag != 99) {
    printf("  [FAIL] Script node tag is %u (expected 99)\n", tag);
    failures++;
  } else {
    printf("  [OK] Script node created with tag=99\n");
  }

  // Set src attribute
  const char *src_url = "https://www.google.com/xjs/_/js/main.js";
  uint32_t src_len = (uint32_t)strlen(src_url);
  dom_set_script_src(script_idx, (uint64_t)(uintptr_t)src_url, src_len);

  // Verify src was stored
  uint64_t stored_ptr = dom_get_script_src_ptr(script_idx);
  uint32_t stored_len = dom_get_script_src_len(script_idx);
  if (stored_ptr == 0 || stored_len != src_len) {
    printf("  [FAIL] Script src not stored (ptr=%llu len=%u)\n", stored_ptr,
           stored_len);
    failures++;
  } else {
    printf("  [OK] Script src stored: len=%u\n", stored_len);
  }

  // Queue a script fetch and verify state transition
  uint64_t fetch_id = queue_script_fetch((uint64_t)(uintptr_t)src_url, src_len);
  if (fetch_id == 0) {
    printf("  [FAIL] queue_script_fetch returned 0\n");
    failures++;
  } else {
    printf("  [OK] queue_script_fetch returned fetch_id=%llu\n", fetch_id);

    // Check SCRIPT_FETCH_STATE went to PENDING (1)
    int found_pending = 0;
    for (int i = 0; i < 64; i++) {
      if (user__browser__dom__SCRIPT_FETCH_STATE[i] == 1) {
        found_pending = 1;
        break;
      }
    }
    if (!found_pending) {
      printf("  [FAIL] No SCRIPT_FETCH_STATE entry in PENDING state\n");
      failures++;
    } else {
      printf("  [OK] SCRIPT_FETCH_STATE has PENDING entry\n");
    }
  }

  return failures;
}

// ═══════════════════════════════════════════════════════════════
// Test 3: Base64 Data URI Decoder
// ═══════════════════════════════════════════════════════════════

static int test_base64_decoder(void) {
  int failures = 0;
  printf("\n[Test 3] Base64 Data URI Decoder\n");

  // Test basic base64 decode: "SGVsbG8=" -> "Hello"
  const char *b64 = "SGVsbG8=";
  uint8_t decoded[64];
  int32_t len = base64_decode((const uint8_t *)b64, strlen(b64), decoded, 64);

  if (len != 5) {
    printf("  [FAIL] Decoded length %d (expected 5)\n", len);
    failures++;
  } else if (memcmp(decoded, "Hello", 5) != 0) {
    printf("  [FAIL] Decoded content mismatch\n");
    failures++;
  } else {
    printf("  [OK] base64_decode: 'SGVsbG8=' -> 'Hello'\n");
  }

  // Test is_data_uri
  const char *uri = "data:image/png;base64,iVBORw0KGgo=";
  if (!is_data_uri((const uint8_t *)uri, strlen(uri))) {
    printf("  [FAIL] is_data_uri failed to detect data: prefix\n");
    failures++;
  } else {
    printf("  [OK] is_data_uri detected data: prefix\n");
  }

  // Test is_data_image_uri
  if (!is_data_image_uri((const uint8_t *)uri, strlen(uri))) {
    printf("  [FAIL] is_data_image_uri failed\n");
    failures++;
  } else {
    printf("  [OK] is_data_image_uri detected data:image/ prefix\n");
  }

  // Test payload offset detection
  int32_t offset =
      data_uri_get_payload_offset((const uint8_t *)uri, strlen(uri));
  if (offset < 0) {
    printf("  [FAIL] data_uri_get_payload_offset returned %d\n", offset);
    failures++;
  } else {
    printf("  [OK] Payload starts at offset %d\n", offset);

    // Verify we can decode the payload from that offset
    uint8_t img_decoded[64];
    int32_t img_len =
        base64_decode((const uint8_t *)uri + offset,
                      (uint32_t)(strlen(uri) - offset), img_decoded, 64);
    if (img_len <= 0) {
      printf("  [FAIL] Could not decode payload after offset\n");
      failures++;
    } else {
      printf("  [OK] Decoded %d bytes from data URI payload\n", img_len);
    }
  }

  // Test full decode_data_uri convenience function
  uint8_t full_decoded[64];
  int32_t full_len = decode_data_uri((const uint8_t *)uri, strlen(uri),
                                     full_decoded, 64);
  if (full_len <= 0) {
    printf("  [FAIL] decode_data_uri failed (len=%d)\n", full_len);
    failures++;
  } else {
    printf("  [OK] decode_data_uri decoded %d bytes\n", full_len);
  }

  // Test non-data URI returns 0
  const char *http_uri = "https://example.com/img.png";
  if (is_data_uri((const uint8_t *)http_uri, strlen(http_uri))) {
    printf("  [FAIL] is_data_uri false positive on https:// URL\n");
    failures++;
  } else {
    printf("  [OK] is_data_uri correctly rejected https:// URL\n");
  }

  return failures;
}

// ═══════════════════════════════════════════════════════════════
// Test 4: Flex-Direction Column with Flex-Grow
// ═══════════════════════════════════════════════════════════════

static int test_flex_column_grow(void) {
  int failures = 0;
  printf("\n[Test 4] Flex Column with Flex-Grow\n");

  init_engine();

  // Create root
  uint64_t root = create_node(1);
  uint32_t root_idx = (uint32_t)(root & 0xFFFF);
  user__browser__dom__STYLE_W[root_idx] = 400;
  user__browser__dom__STYLE_H[root_idx] = 600;
  user__browser__dom__STYLE_W_UNIT[root_idx] = 0;
  user__browser__dom__STYLE_H_UNIT[root_idx] = 0;
  user__browser__dom__STYLE_DISPLAY[root_idx] = 2;     // FLEX
  user__browser__dom__STYLE_FLEX_DIR[root_idx] = 1; // COLUMN
  http_set_root_node(root);

  // Create 3 children: grow=1, grow=2, grow=1
  uint64_t c1 = create_node(4);
  uint32_t c1_idx = (uint32_t)(c1 & 0xFFFF);
  user__browser__dom__STYLE_DISPLAY[c1_idx] = 1;
  user__browser__dom__STYLE_FLEX_GROW[c1_idx] = 1;
  user__browser__dom__STYLE_FLEX_BASIS[c1_idx] = 20;

  uint64_t c2 = create_node(4);
  uint32_t c2_idx = (uint32_t)(c2 & 0xFFFF);
  user__browser__dom__STYLE_DISPLAY[c2_idx] = 1;
  user__browser__dom__STYLE_FLEX_GROW[c2_idx] = 2;
  user__browser__dom__STYLE_FLEX_BASIS[c2_idx] = 20;

  uint64_t c3 = create_node(4);
  uint32_t c3_idx = (uint32_t)(c3 & 0xFFFF);
  user__browser__dom__STYLE_DISPLAY[c3_idx] = 1;
  user__browser__dom__STYLE_FLEX_GROW[c3_idx] = 1;
  user__browser__dom__STYLE_FLEX_BASIS[c3_idx] = 20;

  js_dom_append_child(root_idx, c1_idx);
  js_dom_append_child(root_idx, c2_idx);
  js_dom_append_child(root_idx, c3_idx);

  http_set_eof();

  // Run layout
  invalidate_all_layout();
  user__browser__layout__layout_tree();

  // Container: 600px height, 3 children with basis=20, total_basis=60
  // Free space = 600 - 60 = 540
  // Grow sum = 1+2+1 = 4
  // Child1 height = 20 + (540*1/4) = 20 + 135 = 155
  // Child2 height = 20 + (540*2/4) = 20 + 270 = 290
  // Child3 height = 20 + (540*1/4) = 20 + 135 = 155

  int32_t h1 = user__browser__dom__LAYOUT_H[c1_idx];
  int32_t h2 = user__browser__dom__LAYOUT_H[c2_idx];
  int32_t h3 = user__browser__dom__LAYOUT_H[c3_idx];

  printf("  Child heights: %d, %d, %d\n", h1, h2, h3);

  // Child2 should be approximately 2x child1/child3
  if (h2 <= h1 || h2 <= h3) {
    printf("  [FAIL] Child2 (grow=2) should be taller than child1/3 (grow=1)\n");
    printf("  Heights: c1=%d, c2=%d, c3=%d\n", h1, h2, h3);
    failures++;
  } else {
    printf("  [OK] Flex-grow proportional: c2 > c1, c2 > c3\n");
  }

  // Verify Y positions are monotonically increasing
  int32_t y1 = user__browser__dom__LAYOUT_Y[c1_idx];
  int32_t y2 = user__browser__dom__LAYOUT_Y[c2_idx];
  int32_t y3 = user__browser__dom__LAYOUT_Y[c3_idx];

  printf("  Child Y positions: %d, %d, %d\n", y1, y2, y3);

  if (y2 <= y1 || y3 <= y2) {
    printf("  [FAIL] Y positions not monotonically increasing\n");
    failures++;
  } else {
    printf("  [OK] Y positions monotonically increasing\n");
  }

  // Verify total height approximately fills the container
  int32_t total = h1 + h2 + h3;
  printf("  Total children height: %d (container: 600)\n", total);
  if (total < 500 || total > 700) {
    printf("  [FAIL] Total height %d is far from container height 600\n",
           total);
    failures++;
  } else {
    printf("  [OK] Total height %d approximately fills container\n", total);
  }

  return failures;
}

// ═══════════════════════════════════════════════════════════════
// Test 5: Flex Column Justify-Content Center
// ═══════════════════════════════════════════════════════════════

static int test_flex_column_justify_center(void) {
  int failures = 0;
  printf("\n[Test 5] Flex Column Justify-Content Center\n");

  init_engine();

  // Create root
  uint64_t root = create_node(1);
  uint32_t root_idx = (uint32_t)(root & 0xFFFF);
  user__browser__dom__STYLE_W[root_idx] = 400;
  user__browser__dom__STYLE_H[root_idx] = 600;
  user__browser__dom__STYLE_W_UNIT[root_idx] = 0;
  user__browser__dom__STYLE_H_UNIT[root_idx] = 0;
  user__browser__dom__STYLE_DISPLAY[root_idx] = 2;          // FLEX
  user__browser__dom__STYLE_FLEX_DIR[root_idx] = 1;   // COLUMN
  user__browser__dom__STYLE_JUSTIFY_CONTENT[root_idx] = 1;  // CENTER
  http_set_root_node(root);

  // Single child: 100px tall
  uint64_t c1 = create_node(4);
  uint32_t c1_idx = (uint32_t)(c1 & 0xFFFF);
  user__browser__dom__STYLE_DISPLAY[c1_idx] = 1;
  user__browser__dom__STYLE_H[c1_idx] = 100;
  user__browser__dom__STYLE_H_UNIT[c1_idx] = 0;

  js_dom_append_child(root_idx, c1_idx);
  http_set_eof();

  invalidate_all_layout();
  user__browser__layout__layout_tree();

  // Container: 600px. Child: 100px. Free space: 500px.
  // justify-content: center → offset = 500/2 = 250
  // Child Y should be ~250 + content_y(8) = ~258
  int32_t y1 = user__browser__dom__LAYOUT_Y[c1_idx];
  int32_t root_y = user__browser__dom__LAYOUT_Y[root_idx];
  printf("  Root Y: %d, Child Y: %d\n", root_y, y1);

  // The child should be roughly centered vertically
  int32_t expected_center = root_y + 300; // center of 600px
  int32_t child_center = y1 + 50;        // center of 100px child
  int32_t deviation = abs(expected_center - child_center);

  printf("  Expected center: %d, Actual child center: %d, Deviation: %d\n",
         expected_center, child_center, deviation);

  if (deviation > 30) {
    printf("  [FAIL] Child not vertically centered (deviation=%d)\n",
           deviation);
    failures++;
  } else {
    printf("  [OK] Child vertically centered (deviation=%d)\n", deviation);
  }

  return failures;
}

// ═══════════════════════════════════════════════════════════════
// Main Test Entry Point
// ═══════════════════════════════════════════════════════════════

int google_unblock_test(void) {
  int total_failures = 0;

  printf("╔════════════════════════════════════════════════════════╗\n");
  printf("║   Google Unblock Integration Test Suite               ║\n");
  printf("╚════════════════════════════════════════════════════════╝\n");

  total_failures += test_document_write();
  total_failures += test_dynamic_script_element();
  total_failures += test_base64_decoder();
  total_failures += test_flex_column_grow();
  total_failures += test_flex_column_justify_center();

  printf("\n══════════════════════════════════════════════════════════\n");
  printf("  Results: %d failures\n", total_failures);
  printf("══════════════════════════════════════════════════════════\n");

  return total_failures;
}
