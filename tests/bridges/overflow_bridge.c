#include <stdio.h>
#include <stdint.h>
#include <string.h>

extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern int32_t js_init_quickjs();
extern int32_t js_eval_buffer(const uint64_t code_ptr, uint32_t len);

void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {
    if (x == -999999 || y == -999999) return; // ignore initial infinite clip
    printf("[GPU] SCISSOR PUSH: X=%d Y=%d W=%d H=%d\n", x, y, w, h);
    fflush(stdout);
}

int c_bridge_overflow_e2e_test() {
    printf("[C] QuickJS evaluating overflow payload...\n");
    airlock_init_allocator();
    init_arrays();
    js_init_quickjs();
    
    // The structural payload
    const char *script = 
        "let c = document.createElement('div'); c.id = 'viewport'; "
        "c.style.width = '100'; c.style.height = '100'; c.style.overflow = 'hidden'; "
        
        "let m = document.createElement('div'); m.id = 'content'; "
        "m.style.width = '100'; m.style.height = '500'; "
        "c.appendChild(m); ";
    js_eval_buffer((uint64_t)script, strlen(script));
    return 0;
}

extern void dom_set_layout_scroll_y(uint32_t idx, float val);
int c_bridge_overflow_scroll(uint32_t node_idx) {
    dom_set_layout_scroll_y(node_idx, 50.0f);
    return 0;
}

int c_bridge_print_y(int32_t y) {
    printf("[E2E] Content Y is: %d\n", y);
    fflush(stdout);
    return 0;
}
