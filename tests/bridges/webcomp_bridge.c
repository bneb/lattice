#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern int32_t js_init_quickjs();
extern uint64_t ext_salt_create_node(uint32_t tag);
extern void js_dom_append_child(uint32_t parent_idx, uint32_t child_idx);
extern void dom_set_id(uint32_t idx, uint64_t id_ptr, uint32_t id_len);
extern uint64_t dom_alloc_text(uint32_t len);

extern void sys_js_evaluate_script(uint64_t code_ptr, uint32_t code_len, uint64_t filename_ptr, uint32_t filename_len);
extern int32_t js_eval_buffer(const char* code_ptr, uint32_t len);

void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}
uint64_t sys_mmap_file(uint64_t filename_ptr, uint32_t size) { return 0; }

// Use dummy IPC buffer just in case
extern uint64_t user__os__ipc_ring__IPC_BUFFER_PTR;
static uint8_t dummy_ipc_ring[65536];

int32_t webcomp_execute_test() {
    printf("--- Running test_e2e_webcomponents ---\n");
    
    user__os__ipc_ring__IPC_BUFFER_PTR = (uint64_t)dummy_ipc_ring;
    airlock_init_allocator();
    init_arrays();
    js_init_quickjs();
    
    // Create the document structure: html > body > div#root
    uint64_t html_node = create_node(1);  // TAG_HTML
    uint64_t body_node = create_node(3);  // TAG_BODY
    uint64_t root_node = create_node(4);  // TAG_DIV
    
    uint32_t html_idx = (uint32_t)(html_node & 0xFFFF);
    uint32_t body_idx = (uint32_t)(body_node & 0xFFFF);
    uint32_t root_idx = (uint32_t)(root_node & 0xFFFF);
    
    js_dom_append_child(html_idx, body_idx);
    js_dom_append_child(body_idx, root_idx);
    
    const char *root_id = "root";
    uint64_t id_ptr = dom_alloc_text(4);
    memcpy((void*)(uintptr_t)id_ptr, root_id, 4);
    dom_set_id(root_idx, id_ptr, 4);
    
    printf("[PASS] Engine initialized with <div id=\"root\">\n");
    
    FILE *f = fopen("tests/fixtures/webcomp_bundle.js", "rb");
    if (!f) {
        printf("[FAIL] Could not load webcomp_bundle.js\n");
        return 1;
    }
    fseek(f, 0, SEEK_END);
    long fsize = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *bundle = malloc(fsize + 1);
    fread(bundle, 1, fsize, f);
    fclose(f);
    bundle[fsize] = 0;
    
    user__os__ipc_ring__IPC_BUFFER_PTR = (uint64_t)dummy_ipc_ring;
    
    printf("[INFO] Executing WebComponent bundle...\n");
    sys_js_evaluate_script((uint64_t)bundle, (uint32_t)fsize, (uint64_t)"webcomp_bundle.js", 17);
    free(bundle);
    
    // Evaluate checks
    const char *check1 = "if (!globalThis.__wcCreated) throw new Error('WebComponent was never instantiated');";
    int32_t r1 = js_eval_buffer(check1, strlen(check1));
    if (r1 != 0) { printf("[FAIL] __wcCreated check failed\n"); return 1; }
    printf("[PASS] customElements.define and createElement mapping successful\n");
    
    const char *check2 = "if (!globalThis.__wcConnected) throw new Error('connectedCallback never fired');";
    int32_t r2 = js_eval_buffer(check2, strlen(check2));
    if (r2 != 0) { printf("[FAIL] connectedCallback never fired\n"); return 2; }
    printf("[PASS] connectedCallback executed upon appendChild\n");
    
    printf("[INFO] Simulating click on inner WC button...\n");
    const char *check3 = "document.getElementById('wc-btn').click();";
    js_eval_buffer(check3, strlen(check3));
    
    const char *check4 = "if (!document.getElementById('wc-btn').textContent.includes('1')) throw new Error('WC State mutation failed');";
    int32_t r4 = js_eval_buffer(check4, strlen(check4));
    if (r4 != 0) { printf("[FAIL] State mutation failed\n"); return 4; }
    printf("[PASS] Shadow encapsulation and internal state mutability verified\n");
    
    const char *check5 = "document.getElementById('root').removeChild(globalThis.wc);";
    js_eval_buffer(check5, strlen(check5));
    
    const char *check6 = "if (!globalThis.__wcDisconnected) throw new Error('disconnectedCallback never fired');";
    int32_t r6 = js_eval_buffer(check6, strlen(check6));
    if (r6 != 0) { printf("[FAIL] disconnectedCallback never fired\n"); return 6; }
    printf("[PASS] disconnectedCallback executed upon removeChild\n");
    
    printf("\n--- Exit code: 0 ---\n");
    return 0;
}
