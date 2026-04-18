// tests/bridges/js_lexer_diff_bridge.c
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char js_buffer[65536];
static unsigned int js_len = 0;

__attribute__((constructor)) static void load_js(void) {
  const char *path = getenv("JS_TEST_FILE");
  if (!path) {
    fprintf(stderr, "[BRIDGE-JS] No JS_TEST_FILE env var set\n");
    return;
  }
  FILE *f = fopen(path, "rb");
  if (!f) {
    fprintf(stderr, "[BRIDGE-JS] Failed to open %s\n", path);
    return;
  }
  js_len = (unsigned int)fread(js_buffer, 1, sizeof(js_buffer), f);
  fclose(f);
  fprintf(stderr, "[BRIDGE-JS] Loaded %u bytes from %s\n", js_len, path);
}

unsigned long long ext_get_mock_js_ptr(void) {
  fprintf(stderr, "[BRIDGE-JS] ext_get_mock_js_ptr called, returning %p\n",
          js_buffer);
  return (unsigned long long)js_buffer;
}

unsigned long long ext_get_mock_js_len(void) {
  fprintf(stderr, "[BRIDGE-JS] ext_get_mock_js_len called, returning %llu\n",
          (unsigned long long)js_len);
  return (unsigned long long)js_len;
}

void sys_log_info(unsigned long long msg_ptr, unsigned int msg_len) {
  uint8_t *msg = (uint8_t *)msg_ptr;
  fwrite(msg, 1, msg_len, stdout);
  fflush(stdout);
}

void sys_print_float(double f) { printf("%g", f); }
void airlock_init_allocator() {}
void init_arrays() {}
void sys_print_u32(unsigned int u) { printf("%u", u); }
