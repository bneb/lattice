#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

// Dummy implementations for linker
__attribute__((weak)) void css_arena_inc_count(void) {}
__attribute__((weak)) void css_arena_set_hash(void) {}
__attribute__((weak)) void ext_engine_process_key_down(void) {}
__attribute__((weak)) void ext_engine_process_mouse_down(void) {}
__attribute__((weak)) void ext_salt_paint_inject_dom_pointers(void) {}
__attribute__((weak)) uint32_t hash_string(uint64_t ptr, uint32_t len) { return 0; }
__attribute__((weak)) void user__browser__compositor__load_font_atlas(uint64_t pixels, int32_t width, int32_t height) {}

int sprint8_search_e2e_test_dummy(void) { return 0; }
void ext_net_navigate(uint64_t url_ptr, uint32_t url_len) {
    char* url = (char*)(uintptr_t)url_ptr;
    printf("[BRIDGE] Navigating to: %.*s\n", url_len, url);
}
