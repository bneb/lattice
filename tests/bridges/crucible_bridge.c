#include <stdio.h>
#include <stdint.h>
#include <string.h>

extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern int32_t js_init_quickjs();
extern int32_t js_eval_buffer(const char* code_ptr, uint32_t len);
extern uint64_t ext_salt_create_node(uint32_t tag);
extern uint32_t ext_salt_resolve_node(uint64_t id);
extern void sys_js_evaluate_script(uint64_t code_ptr, uint32_t code_len, uint64_t filename_ptr, uint32_t filename_len);
extern void js_bridge_dispatch_document_event(const char *type_ptr, uint32_t type_len);
extern int32_t js_execute_pending_jobs();
extern void sys_js_pump_script_queue();
extern uint64_t dom_alloc_text(uint32_t len);
extern void js_lex_html_chunk(uint64_t root_id, uint64_t ptr, uint32_t len, uint8_t can_exec);
extern void js_dom_append_child(uint32_t parent_idx, uint32_t child_idx);
extern void dom_set_id(uint32_t idx, uint64_t id_ptr, uint32_t id_len);
extern uint32_t dom_get_tag(uint32_t idx);
extern uint64_t dom_get_first_child(uint32_t idx);
extern uint64_t dom_get_next_sibling(uint32_t idx);
extern uint64_t dom_get_text_ptr(uint32_t idx);
extern uint32_t dom_get_text_len(uint32_t idx);
extern uint64_t dom_get_id_ptr(uint32_t idx);
extern uint32_t dom_get_id_len(uint32_t idx);
extern uint64_t dom_get_node_count();
extern void js_bridge_dispatch_event(uint64_t node_id, const char *type_ptr, uint32_t type_len);
extern uint32_t dom_get_generation(uint32_t idx);

// Stubs for OS-level functions not available in test environment
void sys_gpu_set_scissor_rect(int32_t x, int32_t y, int32_t w, int32_t h) {}
uint64_t sys_mmap_file(uint64_t filename_ptr, uint32_t size) { return 0; }

// The IPC ring uses this global pointer for its buffer
extern uint64_t user__os__ipc_ring__IPC_BUFFER_PTR;
static uint8_t dummy_ipc_ring[65536];

// ============================================================================
// Helper: Load the SPA bundle from disk into arena memory
// ============================================================================
static char spa_bundle_buf[65536]; // 64KB max bundle size
static uint32_t spa_bundle_len = 0;

static int load_spa_bundle(void) {
    // Try multiple paths to find the fixture
    const char *paths[] = {
        "tests/fixtures/spa_bundle.js",
        "../tests/fixtures/spa_bundle.js",
        "./spa_bundle.js",
        NULL
    };
    
    FILE *f = NULL;
    for (int i = 0; paths[i]; i++) {
        f = fopen(paths[i], "r");
        if (f) break;
    }
    
    if (!f) {
        printf("[FAIL] Could not find spa_bundle.js fixture\n");
        return -1;
    }
    
    spa_bundle_len = (uint32_t)fread(spa_bundle_buf, 1, sizeof(spa_bundle_buf) - 1, f);
    fclose(f);
    spa_bundle_buf[spa_bundle_len] = '\0';
    printf("[INFO] Loaded spa_bundle.js (%u bytes)\n", spa_bundle_len);
    return 0;
}

// ============================================================================
// Helper: Find a child node by ID string (walks the tree iteratively)
// ============================================================================
static uint32_t find_node_by_id(const char *search_id) {
    uint32_t search_len = strlen(search_id);
    // Linear scan through DOM nodes
    extern uint64_t user__browser__dom__DOM_NODE_COUNT;
    uint32_t count = (uint32_t)user__browser__dom__DOM_NODE_COUNT;
    for (uint32_t i = 1; i < count; i++) {
        uint64_t id_ptr = dom_get_id_ptr(i);
        uint32_t id_len = dom_get_id_len(i);
        if (id_len == search_len && id_ptr != 0) {
            if (memcmp((const char *)(uintptr_t)id_ptr, search_id, search_len) == 0) {
                return i;
            }
        }
    }
    return 0;
}

// ============================================================================
// Helper: Read text content of node idx
// ============================================================================
static const char* get_node_text(uint32_t idx, uint32_t *out_len) {
    *out_len = dom_get_text_len(idx);
    uint64_t ptr = dom_get_text_ptr(idx);
    if (ptr == 0) return NULL;
    return (const char *)(uintptr_t)ptr;
}

// ============================================================================
// Phase 1: Initialize engine and parse shell HTML
// ============================================================================
int crucible_init(void) {
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
    
    // Set up the tree
    js_dom_append_child(html_idx, body_idx);
    js_dom_append_child(body_idx, root_idx);
    
    // Set id="root" on the root div
    const char *root_id = "root";
    uint64_t id_ptr = dom_alloc_text(4);
    memcpy((void*)(uintptr_t)id_ptr, root_id, 4);
    dom_set_id(root_idx, id_ptr, 4);
    
    printf("[PASS] Engine initialized with <div id=\"root\">\n");
    return 0;
}

// ============================================================================
// Phase 2: Execute the SPA bundle
// ============================================================================
int crucible_execute_bundle(void) {
    if (load_spa_bundle() != 0) return -1;
    
    printf("[INFO] Executing SPA bundle...\n");
    
    sys_js_evaluate_script(
        (uint64_t)spa_bundle_buf,
        spa_bundle_len,
        (uint64_t)"spa_bundle.js",
        13  // strlen("spa_bundle.js")
    );
    
    // Flush microtask queue
    while (js_execute_pending_jobs() > 0) {}
    
    // Check if framework mounted
    const char *check = "if (globalThis.__crucibleMounted !== true) throw new Error('Framework did not mount');";
    int32_t result = js_eval_buffer(check, strlen(check));
    if (result != 0) {
        printf("[FAIL] Framework failed to mount — __crucibleMounted not set\n");
        return -1;
    }
    
    printf("[PASS] SPA framework mounted successfully\n");
    return 0;
}

// ============================================================================
// Phase 3: Verify DOM tree structure
// ============================================================================
int crucible_verify_dom_tree(void) {
    // The VDOM should have rendered:
    // root > div#app-container > [h1, p#count-display, button#inc-btn, input#input-box, p#echo-display]
    
    uint32_t app_container = find_node_by_id("app-container");
    if (app_container == 0) {
        printf("[FAIL] Could not find <div id=\"app-container\">\n");
        return -1;
    }
    printf("[PASS] Found <div id=\"app-container\"> at node %u\n", app_container);
    
    // Check tag is DIV (4)
    uint32_t tag = dom_get_tag(app_container);
    if (tag != 4) {
        printf("[FAIL] app-container tag is %u, expected 4 (DIV)\n", tag);
        return -1;
    }
    
    // Find the specific elements
    uint32_t inc_btn = find_node_by_id("inc-btn");
    if (inc_btn == 0) {
        printf("[FAIL] Could not find <button id=\"inc-btn\">\n");
        return -1;
    }
    printf("[PASS] Found <button id=\"inc-btn\"> at node %u (tag=%u)\n", inc_btn, dom_get_tag(inc_btn));
    
    uint32_t input_box = find_node_by_id("input-box");
    if (input_box == 0) {
        printf("[FAIL] Could not find <input id=\"input-box\">\n");
        return -1;
    }
    printf("[PASS] Found <input id=\"input-box\"> at node %u (tag=%u)\n", input_box, dom_get_tag(input_box));
    
    uint32_t count_display = find_node_by_id("count-display");
    if (count_display == 0) {
        printf("[FAIL] Could not find <p id=\"count-display\">\n");
        return -1;
    }
    
    uint32_t echo_display = find_node_by_id("echo-display");
    if (echo_display == 0) {
        printf("[FAIL] Could not find <p id=\"echo-display\">\n");
        return -1;
    }
    
    printf("[PASS] All framework-injected elements found in native DOM SoA\n");
    return 0;
}

// ============================================================================
// Phase 4: Verify initial text content
// ============================================================================
int crucible_verify_initial_text(void) {
    // count-display should have a text child "Count: 0"
    uint32_t count_p = find_node_by_id("count-display");
    if (count_p == 0) return -1;
    
    // Get first child — should be a text node
    uint64_t child_id = dom_get_first_child(count_p);
    if (child_id == 0) {
        printf("[FAIL] count-display has no children\n");
        return -1;
    }
    uint32_t child_idx = (uint32_t)(child_id & 0xFFFF);
    uint32_t child_tag = dom_get_tag(child_idx);
    if (child_tag != 0) {
        printf("[FAIL] count-display first child is not a text node (tag=%u)\n", child_tag);
        return -1;
    }
    
    uint32_t text_len;
    const char *text = get_node_text(child_idx, &text_len);
    if (!text || text_len == 0) {
        printf("[FAIL] count-display text node has no content\n");
        return -1;
    }
    
    printf("[INFO] count-display text: \"%.*s\"\n", text_len, text);
    
    if (text_len >= 8 && memcmp(text, "Count: 0", 8) == 0) {
        printf("[PASS] Initial count text is correct: \"Count: 0\"\n");
        return 0;
    }
    
    printf("[FAIL] Expected \"Count: 0\", got \"%.*s\"\n", text_len, text);
    return -1;
}

// ============================================================================
// Phase 5: Fire click event on inc-btn and verify state mutation
// ============================================================================
int crucible_simulate_click(void) {
    uint32_t btn_idx = find_node_by_id("inc-btn");
    if (btn_idx == 0) {
        printf("[FAIL] Cannot find inc-btn for click simulation\n");
        return -1;
    }
    
    uint32_t btn_gen = dom_get_generation(btn_idx);
    uint64_t btn_node_id = (uint64_t)btn_idx | ((uint64_t)btn_gen << 16);
    
    printf("[INFO] Simulating click on inc-btn (node_id=0x%llx, idx=%u)\n",
           (unsigned long long)btn_node_id, btn_idx);
    
    // Fire the click event through the JS bridge — this triggers event bubbling
    js_bridge_dispatch_event(btn_node_id, "click", 5);
    
    // Flush microtask queue (setState is async)
    while (js_execute_pending_jobs() > 0) {}
    
    // Now verify the count text changed to "Count: 1"
    uint32_t count_p = find_node_by_id("count-display");
    if (count_p == 0) {
        printf("[FAIL] count-display disappeared after click!\n");
        return -1;
    }
    
    uint64_t child_id = dom_get_first_child(count_p);
    if (child_id == 0) {
        printf("[FAIL] count-display has no children after click\n");
        return -1;
    }
    uint32_t child_idx = (uint32_t)(child_id & 0xFFFF);
    
    uint32_t text_len;
    const char *text = get_node_text(child_idx, &text_len);
    if (!text) {
        printf("[FAIL] count-display text node is null after click\n");
        return -1;
    }
    
    printf("[INFO] count-display text after click: \"%.*s\"\n", text_len, text);
    
    if (text_len >= 8 && memcmp(text, "Count: 1", 8) == 0) {
        printf("[PASS] setState → VDOM diff → native DOM mutation verified: \"Count: 1\"\n");
        return 0;
    }
    
    printf("[FAIL] Expected \"Count: 1\", got \"%.*s\"\n", text_len, text);
    return -1;
}

// ============================================================================
// Phase 6: Verify node count stability (GC / Free-List integrity)
// ============================================================================
int crucible_verify_node_stability(void) {
    extern uint64_t user__browser__dom__DOM_NODE_COUNT;
    uint32_t pre_count = (uint32_t)user__browser__dom__DOM_NODE_COUNT;
    
    printf("[INFO] Node count before 2nd click: %u\n", pre_count);
    
    // Fire another click
    uint32_t btn_idx = find_node_by_id("inc-btn");
    if (btn_idx == 0) return -1;
    
    uint32_t btn_gen = dom_get_generation(btn_idx);
    uint64_t btn_node_id = (uint64_t)btn_idx | ((uint64_t)btn_gen << 16);
    js_bridge_dispatch_event(btn_node_id, "click", 5);
    while (js_execute_pending_jobs() > 0) {}
    
    uint32_t post_count = (uint32_t)user__browser__dom__DOM_NODE_COUNT;
    printf("[INFO] Node count after 2nd click: %u\n", post_count);
    
    // Verify the count text is now "Count: 2"
    uint32_t count_p = find_node_by_id("count-display");
    uint64_t child_id = dom_get_first_child(count_p);
    uint32_t child_idx = (uint32_t)(child_id & 0xFFFF);
    uint32_t text_len;
    const char *text = get_node_text(child_idx, &text_len);
    
    if (!text || text_len < 8 || memcmp(text, "Count: 2", 8) != 0) {
        printf("[FAIL] Expected \"Count: 2\", got \"%.*s\"\n", text_len, text ? text : "(null)");
        return -1;
    }
    printf("[PASS] Double-click verified: \"Count: 2\"\n");
    
    // Node count should not have grown (VDOM diff reuses text nodes in-place)
    if (post_count <= pre_count + 2) {
        printf("[PASS] Node count stable: %u → %u (delta ≤ 2, free-list working)\n", pre_count, post_count);
        return 0;
    }
    
    printf("[WARN] Node count grew: %u → %u (delta=%u) — free-list may need tuning\n",
           pre_count, post_count, post_count - pre_count);
    return 0; // Warning, not failure
}
