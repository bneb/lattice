// tests/bridges/selectors_diff_bridge.c
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char selector_buffer[1024];
static unsigned int selector_len = 0;

__attribute__((constructor)) static void load_selector(void) {
  const char *sel = getenv("SELECTOR_STRING");
  if (!sel)
    return;
  strncpy(selector_buffer, sel, sizeof(selector_buffer));
  selector_len = strlen(selector_buffer);
}

unsigned long long ext_get_mock_selector_ptr(void) {
  return (unsigned long long)selector_buffer;
}

unsigned int ext_get_mock_selector_len(void) { return selector_len; }

void sys_log_info(unsigned long long msg_ptr, unsigned int msg_len) {
  uint8_t *msg = (uint8_t *)msg_ptr;
  fwrite(msg, 1, msg_len, stdout);
  fflush(stdout);
}

void sys_print_u32(unsigned int u) { printf("%u", u); }
