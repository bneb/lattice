// tests/bridges/http_lexer_diff_bridge.c
// Provides mock HTTP response payload for the differential test.
// Reads from HTTP_TEST_FILE env var.

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char http_buffer[1048576]; // 1MB max
static unsigned int http_len = 0;

__attribute__((constructor)) static void load_http(void) {
  const char *path = getenv("HTTP_TEST_FILE");
  if (!path)
    return;
  FILE *f = fopen(path, "rb");
  if (!f) {
    fprintf(stderr, "[BRIDGE] Failed to open %s\n", path);
    return;
  }
  http_len = (unsigned int)fread(http_buffer, 1, sizeof(http_buffer), f);
  fclose(f);
}

unsigned long long ext_get_mock_http_ptr(void) {
  return (unsigned long long)http_buffer;
}

unsigned int ext_get_mock_http_len(void) { return http_len; }

// Stubs
void sys_log_info(unsigned long long msg_ptr, unsigned int msg_len) {
  uint8_t *msg = (uint8_t *)msg_ptr;
  fwrite(msg, 1, msg_len, stdout);
  fflush(stdout);
}

void sys_log_net(unsigned long long msg_ptr, unsigned int msg_len) {
  // Discard net logging in test mode
}

void sys_print_float(double f) { printf("%g", f); }
void sys_print_u32(unsigned int u) { printf("%u", u); }
void sys_print_i32(int i) { printf("%d", i); }

// Stubs for DOM operations the HTTP lexer calls
void js_lex_html_chunk(unsigned long long root, unsigned long long str,
                       unsigned int len, unsigned char can_exec) {}
unsigned long long dom_alloc_text(unsigned int len) { return 0; }

// Runtime stubs
void airlock_init_allocator() {}
void init_arrays() {}
