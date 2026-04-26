#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern int32_t js_init_quickjs();
extern int32_t js_eval_buffer(const uint64_t code_ptr, uint32_t len);

int c_bridge_reconciliation_e2e_test() {
    airlock_init_allocator();
    init_arrays();
    js_init_quickjs();
    
    // 1. Initial State provided by the prompt: <ul id="list"><li id="a">A</li><li id="c">C</li></ul>
    const char *init_script = 
        "let list = document.createElement('ul'); list.id = 'list'; "
        "let a = document.createElement('li'); a.id = 'a'; a.textContent = 'A'; "
        "list.appendChild(a); "
        "let c = document.createElement('li'); c.id = 'c'; c.textContent = 'C'; "
        "list.appendChild(c); ";
        
    js_eval_buffer((uint64_t)init_script, strlen(init_script));
    
    // 2. The React/Preact Reconciliation Simulation
    const char *diff_script = 
        "let list2 = document.getElementById('list'); "
        "let c2 = document.getElementById('c'); "
        "let b = document.createElement('li'); b.id = 'b'; b.textContent = 'B'; "
        "list2.insertBefore(b, c2); "
        "let a2 = document.getElementById('a'); "
        "list2.removeChild(a2); ";
        
    js_eval_buffer((uint64_t)diff_script, strlen(diff_script));
    
    return 0;
}
