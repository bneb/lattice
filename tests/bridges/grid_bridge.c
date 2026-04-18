#include <stdio.h>
#include <stdint.h>
#include <string.h>

extern void airlock_init_allocator();
extern void init_arrays();
extern int32_t js_init_quickjs();
extern int32_t js_eval_buffer(const uint64_t code_ptr, uint32_t len);

void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}

int c_bridge_grid_e2e_test() {
    airlock_init_allocator();
    init_arrays();
    js_init_quickjs();
    
    // Boot the engine with a Grid
    const char *script = 
        "let root = document.createElement('div'); "
        "root.id = 'app'; "
        "root.style.display = 'grid'; "
        "root.style.gridTemplateColumns = '200px 1fr'; "
        "root.style.width = '1000'; "
        
        "let sidebar = document.createElement('div'); "
        "sidebar.id = 'sidebar'; "
        "sidebar.style.gridColumnStart = 1; "
        "root.appendChild(sidebar); "
        
        "let main = document.createElement('div'); "
        "main.id = 'main'; "
        "main.style.gridColumnStart = 2; "
        "root.appendChild(main); "
        
        "document.body = root;";
        
    js_eval_buffer((uint64_t)script, strlen(script));
    return 0;
}
