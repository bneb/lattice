#include <stdint.h>

void *kernel__ecs__events__NETWORK_EVENTS = 0;
void *kernel__ecs__events__PROCESS_EXIT_EVENTS = 0;
void *kernel__ecs__events__TIMER_EVENTS = 0;

int64_t volatile_read_i64(uint64_t addr) { return *(volatile int64_t *)addr; }
void volatile_write_i64(uint64_t addr, int64_t val) {
  *(volatile int64_t *)addr = val;
}

void flush_frame(int32_t width, int32_t height) {}
