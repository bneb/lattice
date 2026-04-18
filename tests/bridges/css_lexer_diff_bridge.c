// tests/bridges/css_lexer_diff_bridge.c
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char css_buffer[1048576]; // 1MB max
static unsigned int css_len = 0;

__attribute__((constructor)) static void load_css(void) {
  const char *path = getenv("CSS_TEST_FILE");
  FILE *f = NULL;
  if (path) {
    f = fopen(path, "rb");
    if (!f) {
      fprintf(stderr, "[BRIDGE] Failed to open %s\n", path);
      return;
    }
  } else {
    // Fallback to a default if no env var
    return;
  }
  css_len = (unsigned int)fread(css_buffer, 1, sizeof(css_buffer), f);
  if (path && f)
    fclose(f);
}

unsigned long long ext_get_mock_css_ptr(void) {
  return (unsigned long long)css_buffer;
}

unsigned int ext_get_mock_css_len(void) { return css_len; }

// Stubs for runtime
void sys_log_info(unsigned long long msg_ptr, unsigned int msg_len) {
  uint8_t *msg = (uint8_t *)msg_ptr;
  fwrite(msg, 1, msg_len, stdout);
  fflush(stdout);
}

void sys_print_float(double f) { printf("%g", f); }

void sys_print_u32(unsigned int u) { printf("%u", u); }
void sys_print_i32(int i) { printf("%d", i); }

// Dummy for allocator
void airlock_init_allocator() {}
void init_arrays() {}
