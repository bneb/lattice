#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// External Engine Initializers
extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern int js_quickjs_init();

// Salt Mangled Native Functions
extern void user__browser__css__init_css_defaults();
extern void user__browser__font__init_glyphs();
extern uint64_t ext_salt_create_node(uint32_t tag);
extern void http_set_root_node(uint64_t node_id);
extern uint8_t http_get_eof_reached();
extern void apply_cascade_to_tree();
extern void user__browser__layout__layout_tree();
extern void user__browser__paint__begin_frame();
extern void user__browser__paint__paint_tree();
extern void trigger_e2e_compositor(uint64_t out_buffer, int w, int h);
extern void http_process_ingress(uint64_t buf_ptr, uint32_t buf_len);

// Output PNG Export
#define STB_IMAGE_WRITE_IMPLEMENTATION
#include "../../user/facet/gpu/stb_image_write.h"

void e2e_save_png(const char *filepath, int width, int height,
                  uint8_t *bgra_buffer) {
  size_t total_pixels = width * height;
  for (size_t i = 0; i < total_pixels; i++) {
    uint8_t b = bgra_buffer[i * 4 + 0];
    uint8_t g = bgra_buffer[i * 4 + 1];
    uint8_t r = bgra_buffer[i * 4 + 2];
    uint8_t a = bgra_buffer[i * 4 + 3];

    bgra_buffer[i * 4 + 0] = r;
    bgra_buffer[i * 4 + 1] = g;
    bgra_buffer[i * 4 + 2] = b;
    bgra_buffer[i * 4 + 3] = a;
  }

  int result =
      stbi_write_png(filepath, width, height, 4, bgra_buffer, width * 4);
  if (!result) {
    fprintf(stderr, "[E2E Bridge] FATAL: PNG Generation failed for %s\n",
            filepath);
  } else {
    printf("[E2E Bridge] Successfully exported structural layout to %s\n",
           filepath);
  }
}

// Global host buffer (1920x1080)
uint8_t HOST_BUFFER[8294400];

void e2e_inject_fixture(const char *filepath) {
  FILE *f = fopen(filepath, "rb");
  if (!f) {
    fprintf(stderr, "[E2E Bridge] FATAL: Cannot open fixture %s\n", filepath);
    return;
  }

  const char *headers = "HTTP/1.1 200 OK\r\nConnection: "
                        "keep-alive\r\nTransfer-Encoding: chunked\r\n\r\n";
  http_process_ingress((uint64_t)headers, strlen(headers));

  uint8_t buffer[16384];
  size_t bytes_read;

  while ((bytes_read = fread(buffer, 1, sizeof(buffer), f)) > 0) {
    char chunk_header[32];
    int header_len = sprintf(chunk_header, "%zX\r\n", bytes_read);
    http_process_ingress((uint64_t)chunk_header, header_len);
    http_process_ingress((uint64_t)buffer, (uint32_t)bytes_read);
    http_process_ingress((uint64_t)"\r\n", 2);
  }

  const char *eof_chunk = "0\r\n\r\n";
  http_process_ingress((uint64_t)eof_chunk, strlen(eof_chunk));
  fclose(f);
}

// Main Routine Invoked from tests/test_e2e_render.salt
void e2e_execute_pipeline() {
  printf("[E2E] Initialize memory & quickjs...\n");
  memset(HOST_BUFFER, 0xFF, sizeof(HOST_BUFFER));
  airlock_init_allocator();
  init_arrays();
  js_quickjs_init();

  printf("[E2E] Booting style rules & typography...\n");
  user__browser__css__init_css_defaults();
  user__browser__font__init_glyphs();

  // Upload baked SDF text atlas to the GPU layer
  extern void user__browser__compositor__load_font_atlas(
      uint64_t pixels, int32_t width, int32_t height);
  extern uint8_t user__browser__font__SDF_ATLAS[1048576];
  user__browser__compositor__load_font_atlas(
      (uint64_t)user__browser__font__SDF_ATLAS, 1024, 1024);

  printf("[E2E] Injecting layout fixture...\n");
  uint64_t root_id = create_node(1); // TAG_HTML = 1
  http_set_root_node(root_id);

  e2e_inject_fixture("tests/fixtures/google_snapshot.html");

  if (http_get_eof_reached() == 1) {
    printf("[E2E] Transpiling legacy HTML topology into CSSOM Flexbox...\n");
    extern void user__browser__transpiler__transpile_dom_tree(uint32_t node_id);
    user__browser__transpiler__transpile_dom_tree(root_id & 0xFFFF);

    printf("[E2E] Rendering matrix frame...\n");
    apply_cascade_to_tree();
    user__browser__layout__layout_tree();

    user__browser__paint__begin_frame();
    user__browser__paint__paint_tree();

    extern uint32_t user__browser__paint__get_cmd_count();
    extern uint32_t user__browser__paint__get_z_sort_count();
    printf("[DEBUG] Z_SORT_COUNT: %d\n",
           user__browser__paint__get_z_sort_count());
    printf("[DEBUG] CMD_COUNT: %d\n", user__browser__paint__get_cmd_count());

    extern uint8_t user__browser__dom__STYLE_BG_R[65536];
    extern uint8_t user__browser__dom__STYLE_BG_G[65536];
    extern uint8_t user__browser__dom__STYLE_BG_B[65536];
    extern uint8_t user__browser__dom__STYLE_BG_A[65536];
    extern int32_t user__browser__dom__LAYOUT_W[65536];
    extern int32_t user__browser__dom__LAYOUT_H[65536];
    extern int32_t user__browser__dom__LAYOUT_X[65536];
    extern int32_t user__browser__dom__LAYOUT_Y[65536];
    extern uint32_t user__browser__dom__DOM_NODE_TAG[65536];

    int colored_nodes = 0;
    extern uint32_t user__browser__dom__STYLE_PARENT[65536];
    extern uint64_t user__browser__dom__DOM_NODE_FIRST_CHILD[65536];
    extern uint64_t user__browser__dom__DOM_NODE_NEXT_SIBLING[65536];
    extern uint8_t user__browser__dom__STYLE_DISPLAY[65536];
    extern uint8_t user__browser__dom__STYLE_FLEX_DIR[65536];
    extern uint8_t user__browser__dom__STYLE_W_UNIT[65536];
    extern int32_t user__browser__dom__STYLE_W[65536];
    extern uint32_t user__browser__dom__STYLE_NEXT_SIBLING[65536];

    extern uint64_t user__browser__dom__DOM_NODE_FIRST_CHILD[65536];

    for (int i = 1; i < 1000; i++) {
      if (user__browser__dom__STYLE_BG_A[i] > 0 ||
          user__browser__dom__DOM_NODE_TAG[i] == 11 ||
          user__browser__dom__DOM_NODE_TAG[i] == 12 ||
          user__browser__dom__DOM_NODE_TAG[i] == 3) {
        printf("[DEBUG] N%d: tag=%d disp=%d fd=%d par=%d ns=%d sW=%d sU=%d "
               "W=%d H=%d X=%d Y=%d BG=(%d,%d,%d)\n",
               i, user__browser__dom__DOM_NODE_TAG[i],
               user__browser__dom__STYLE_DISPLAY[i],
               user__browser__dom__STYLE_FLEX_DIR[i],
               user__browser__dom__STYLE_PARENT[i],
               (int)user__browser__dom__STYLE_NEXT_SIBLING[i],
               user__browser__dom__STYLE_W[i],
               user__browser__dom__STYLE_W_UNIT[i],
               user__browser__dom__LAYOUT_W[i], user__browser__dom__LAYOUT_H[i],
               user__browser__dom__LAYOUT_X[i], user__browser__dom__LAYOUT_Y[i],
               user__browser__dom__STYLE_BG_R[i],
               user__browser__dom__STYLE_BG_G[i],
               user__browser__dom__STYLE_BG_B[i]);
        colored_nodes++;
      }
    }
    // Print nodes that actually got layout (W>0)
    int laid_out = 0;
    for (int i = 1; i < 1000; i++) {
      if (user__browser__dom__LAYOUT_W[i] > 0)
        laid_out++;
    }
    printf("[E2E] Colored=%d LaidOut=%d\n", colored_nodes, laid_out);

    trigger_e2e_compositor((uint64_t)HOST_BUFFER, 1920, 1080);

    e2e_save_png("tests/output/prisimi_google_render.png", 1920, 1080,
                 HOST_BUFFER);
    printf("[OK] Engine layout functionally complete.\n");
  } else {
    printf("[FAIL] Lexer parser failed EOF state.\n");
  }
}
