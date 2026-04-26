#include <stdio.h>
#include <stdint.h>
#include <string.h>

extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern int32_t js_init_quickjs();
extern int32_t js_eval_buffer(const uint64_t code_ptr, uint32_t len);

int c_bridge_positioning_e2e_test() {
    printf("[C] QuickJS evaluating positioning payload...\n");
    airlock_init_allocator();
    init_arrays();
    js_init_quickjs();
    
    // The structural payload
    const char *script = 
        "let c = document.createElement('div'); c.id = 'c'; "
        "c.style.position = 'relative'; c.style.top = '50'; c.style.left = '50'; "
        "c.style.width = '500'; c.style.height = '500'; "
        
        "let b = document.createElement('button'); b.id = 'b'; "
        "b.style.width = '100'; b.style.height = '50'; "
        "c.appendChild(b); "
        
        "let m = document.createElement('div'); m.id = 'm'; "
        "m.style.position = 'absolute'; m.style.top = '10'; m.style.left = '10'; "
        "m.style.zIndex = '999'; m.style.width = '200'; m.style.height = '200'; "
        "c.appendChild(m); ";
    js_eval_buffer((uint64_t)script, strlen(script));
    return 0;
}
