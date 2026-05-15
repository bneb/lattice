#include <stdint.h>
#include <stdio.h>

// Salt Globals (mangled names)
extern uint32_t user__browser__dom__ACTIVE_HOVER_NODE_IDX;
extern uint32_t user__browser__dom__ACTIVE_FOCUS_NODE_IDX;

// Core engine logic we want to test
extern uint32_t sys_hit_test(float x, float y, uint32_t root);

void test_sys_on_mouse_move(int32_t x, int32_t y) {
    uint32_t root = 1;
    uint32_t hit = sys_hit_test((float)x, (float)y, root);
    user__browser__dom__ACTIVE_HOVER_NODE_IDX = hit;
}

uint32_t test_get_hovered_node(void) {
    return user__browser__dom__ACTIVE_HOVER_NODE_IDX;
}

void test_set_focused_node(uint32_t node) {
    user__browser__dom__ACTIVE_FOCUS_NODE_IDX = node;
}

// Stubs for common symbols needed by browser modules
__attribute__((weak)) void css_arena_inc_count(void) {}
__attribute__((weak)) void css_arena_set_hash(uint32_t slot, uint32_t hash) {}
__attribute__((weak)) void ext_engine_process_mouse_down(float x, float y) {}
__attribute__((weak)) uint32_t hash_string(uint64_t ptr, uint32_t len) { return 0; }
__attribute__((weak)) void sys_print_float(double f) { printf("DEBUG: %f\n", f); }

int sprint9_polish_test_dummy(void) { return 0; }
void ext_net_navigate(uint64_t url_ptr, uint32_t url_len) {
    char* url = (char*)(uintptr_t)url_ptr;
    printf("[BRIDGE] Navigating to: %.*s\n", url_len, url);
}
