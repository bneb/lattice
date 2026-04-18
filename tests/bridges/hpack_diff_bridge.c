// tests/bridges/hpack_diff_bridge.c
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char hpack_buffer[65536];
static unsigned int hpack_len = 0;

__attribute__((constructor)) static void load_hpack(void) {
  const char *path = getenv("HPACK_TEST_FILE");
  if (!path)
    return;
  FILE *f = fopen(path, "rb");
  if (!f)
    return;
  hpack_len = (unsigned int)fread(hpack_buffer, 1, sizeof(hpack_buffer), f);
  fclose(f);
}

unsigned long long ext_get_mock_hpack_ptr(void) {
  return (unsigned long long)hpack_buffer;
}

unsigned int ext_get_mock_hpack_len(void) { return hpack_len; }

unsigned long long ext_hpack_get_static_key(unsigned int index) {
  if (index == 2)
    return (unsigned long long)":method";
  if (index == 4)
    return (unsigned long long)":path";
  if (index == 7)
    return (unsigned long long)":scheme";
  if (index == 8)
    return (unsigned long long)":status";
  return 0;
}

unsigned long long ext_hpack_get_static_val(unsigned int index) {
  if (index == 2)
    return (unsigned long long)"GET";
  if (index == 4)
    return (unsigned long long)"/";
  if (index == 7)
    return (unsigned long long)"https";
  if (index == 8)
    return (unsigned long long)"200";
  return 0;
}

void sys_log_info(unsigned long long msg_ptr, unsigned int msg_len) {
  uint8_t *msg = (uint8_t *)msg_ptr;
  fwrite(msg, 1, msg_len, stdout);
  fflush(stdout);
}

// Runtime stubs
void airlock_init_allocator() {}
void init_arrays() {}
void sys_print_u32(unsigned int u) { printf("%u", u); }
