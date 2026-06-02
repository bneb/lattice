#include <stdint.h>
#include <stdlib.h>

long long kernel__ecs__ecs_epoch__EPOCH_MARKS[1024] = {0};

static uint64_t global_epoch = 0;
static uint64_t core_epoch[16] = {0};
static uint64_t core_in_epoch[16] = {0};

void ebr_enter_epoch(uint64_t cpu) {
    core_epoch[cpu] = global_epoch;
    core_in_epoch[cpu] = 1;
}

void ebr_exit_epoch(uint64_t cpu) {
    core_in_epoch[cpu] = 0;
}

void ebr_advance_epoch(void) {
    global_epoch++;
}

uint64_t ebr_get_global_epoch(void) {
    return global_epoch;
}

uint64_t ebr_get_core_epoch(uint64_t cpu) {
    return core_epoch[cpu];
}

uint64_t ebr_get_core_in_epoch(uint64_t cpu) {
    return core_in_epoch[cpu];
}

void flush_frame(int32_t width, int32_t height) {}

void ebr_reclaim(uint64_t cpu) {}
