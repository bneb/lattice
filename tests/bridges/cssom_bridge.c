#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern int32_t js_init_quickjs();
extern int32_t js_eval_buffer(const uint64_t code_ptr, uint32_t len);
extern void user__browser__css__init_css_defaults();
extern void set_max_test_frames(uint64_t frames);
extern void run_loop();

// DOM creation & layout
extern uint64_t ext_salt_create_node(uint32_t tag);
extern void js_dom_append_child(uint32_t parent_idx, uint32_t child_idx);
extern void dom_set_id(uint32_t idx, uint64_t ptr, uint32_t len);

extern uint8_t user__browser__dom__DIRTY_PAINT[65536];
extern uint8_t user__browser__dom__DIRTY_LAYOUT[65536];
extern int32_t user__browser__dom__STYLE_W[65536];
extern uint8_t user__browser__dom__STYLE_BG_R[65536];

extern void dom_clear_dirty_flags();
extern void css_lex_stylesheet(uint64_t ptr, uint32_t len);

int32_t c_bridge_cssom_e2e_test() {
    printf("\n--- Epic 53: The Dynamic Cascade E2E Test ---\n");
    int pass = 0;
    
    printf("[CSSOM-E2E] Phase 0: Initialize engine...\n");
    airlock_init_allocator();
    init_arrays();
    extern uint64_t user__os__ipc_ring__IPC_BUFFER_PTR;
    user__os__ipc_ring__IPC_BUFFER_PTR = (uint64_t)malloc(65536);
    if (js_init_quickjs() < 0) {
        printf("  [FAIL] QuickJS init failed.\n");
        return 1;
    }
    user__browser__css__init_css_defaults();
    
    set_max_test_frames(1);
    
    printf("[CSSOM-E2E] Phase 1: Creating DOM and CSSOM rules...\n");
    uint64_t root_id = create_node(1); // HTML
    uint32_t root_idx = (uint32_t)(root_id & 0xFFFF);
    
    uint64_t target_id = create_node(4); // DIV
    uint32_t target_idx = (uint32_t)(target_id & 0xFFFF);
    js_dom_append_child(root_idx, target_idx);
    
    const char* target_id_str = "hero";
    dom_set_id(target_idx, (uint64_t)target_id_str, (uint32_t)strlen(target_id_str));
    
    // Lex global rules
    const char* css_src = 
        ".highlight { background-color: #ff0000; }\n"
        ".expanded { width: 500px; }\n";
    css_lex_stylesheet((uint64_t)css_src, (uint32_t)strlen(css_src));
    
    // Fast-forward initial layout to clear flags
    extern uint8_t user__browser__http_lexer__HTTP_EOF_REACHED;
    user__browser__http_lexer__HTTP_EOF_REACHED = 1;
    run_loop(); 
    dom_clear_dirty_flags();
    
    printf("[CSSOM-E2E] Phase 2: Injecting aesthetic mutation...\n");
    const char* highlight_js = "document.getElementById('hero').className = 'highlight';";
    if (js_eval_buffer((uint64_t)highlight_js, (uint32_t)strlen(highlight_js)) != 0) {
        printf("  [FAIL] JS failed to run\n");
        return 1;
    }
    
    // Tick to apply the style
    run_loop();
    
    // Check dirty flags
    if (user__browser__dom__DIRTY_PAINT[target_idx] == 1 && user__browser__dom__DIRTY_LAYOUT[target_idx] == 0) {
        printf("  [PASS] Aesthetic mutation triggered DIRTY_PAINT without DIRTY_LAYOUT\n");
        pass++;
    } else {
        printf("  [FAIL] Flags incorrect: paint=%d layout=%d\n", user__browser__dom__DIRTY_PAINT[target_idx], user__browser__dom__DIRTY_LAYOUT[target_idx]);
        return 1;
    }
    // Check effect
    if (user__browser__dom__STYLE_BG_R[target_idx] == 255) {
        printf("  [PASS] Background color applied (#ff0000)\n");
        pass++;
    } else {
        printf("  [FAIL] Background color not applied\n");
        return 1; // 2 failures max so no issue
    }
    
    dom_clear_dirty_flags();
    
    printf("[CSSOM-E2E] Phase 3: Injecting geometric mutation...\n");
    const char* expand_js = "document.getElementById('hero').className = 'highlight expanded';";
    if (js_eval_buffer((uint64_t)expand_js, (uint32_t)strlen(expand_js)) != 0) {
        printf("  [FAIL] JS failed to run\n");
        return 1;
    }
    
    run_loop();
    
    extern int32_t user__browser__dom__LAYOUT_W[65536];
    extern uint8_t user__browser__dom__DIRTY_LAYOUT[65536];
    
    if (user__browser__dom__LAYOUT_W[target_idx] == 500) {
        printf("  [PASS] Geometric mutation triggered LAYOUT Engine\n");
        pass++;
    } else {
        printf("  [FAIL] LAYOUT Engine was not triggered properly, got %d\n", user__browser__dom__LAYOUT_W[target_idx]);
        return 1;
    }
    
    if (user__browser__dom__STYLE_W[target_idx] == 500) {
        printf("  [PASS] Width rule applied (500px)\n");
        pass++;
    } else {
        printf("  [FAIL] Width was not 500px, got: %d\n", user__browser__dom__STYLE_W[target_idx]);
        return 1;
    }
    
    printf("\n[CSSOM-E2E] === RESULTS: %d PASS, 0 FAIL ===\n", pass);
    printf("[OK] Epic 53: The Dynamic Cascade is live.\n");
    return 0;
}
