#include <stdint.h>
#include <string.h>
#include <stdio.h>

__attribute__((weak)) uint8_t check_any_layout_dirty() { return 0; }
__attribute__((weak)) void ext_flush_frame() {}
__attribute__((weak)) void ext_tls_write_bytes(uint64_t a, uint32_t b) {}
__attribute__((weak)) uint8_t get_dom_content_loaded_fired() { return 0; }
__attribute__((weak)) uint64_t get_frame_count() { return 0; }
__attribute__((weak)) uint64_t get_max_test_frames() { return 0; }
__attribute__((weak)) void pump_websocket_frames() {}
__attribute__((weak)) void set_dom_content_loaded_fired(uint8_t a) {}
__attribute__((weak)) void set_frame_count(uint64_t a) {}
__attribute__((weak)) void sys_browser_navigate(uint64_t a, uint32_t b) {}
__attribute__((weak)) void sys_js_pump_script_queue() {}

// Externs from Salt DOM
extern void init_arrays();
extern uint64_t create_node(int32_t tag);

// Externs from HTML Lexer
extern void js_lex_html_chunk(uint64_t root_id, uint64_t ptr, uint32_t len, uint8_t can_execute);

// Access arrays directly
extern uint32_t resolve_node(uint64_t id);
extern uint64_t dom_get_first_child(uint32_t idx);
extern uint64_t dom_get_next_sibling(uint32_t idx);
extern uint32_t dom_get_tag(uint32_t idx);

/*
Tag IDs Reference:
DIV = 3
SPAN = 4
P = 5
META = 97
*/

int verify_tree_1() {
    init_arrays();
    uint64_t root = create_node(3); // DIV root
    
    // Valid nesting: <div><span><p></p></span></div>
    const char *html = "<div><span><p></p></span></div>";
    js_lex_html_chunk(root, (uint64_t)html, strlen(html), 0);
    
    uint32_t r_idx = resolve_node(root);
    uint64_t div_id = dom_get_first_child(r_idx);
    if (!div_id) { printf("[Test 1] Missing outer DIV\n"); return 1; }
    
    uint32_t div_idx = resolve_node(div_id);
    uint64_t span_id = dom_get_first_child(div_idx);
    if (!span_id) { printf("[Test 1] Missing SPAN inside DIV\n"); return 1; }
    
    uint32_t span_idx = resolve_node(span_id);
    uint64_t p_id = dom_get_first_child(span_idx);
    if (!p_id) { printf("[Test 1] Missing P inside SPAN\n"); return 1; }
    
    printf("✅ Tree Test 1 Passed (Valid Nesting)\n");
    return 0;
}

int verify_tree_2() {
    init_arrays();
    uint64_t root = create_node(3); // DIV root
    
    // Mismatched closing tag: <div><span></div>
    // The div should not be closed by the span closing tag. Wait, span is not closed.
    // The </div> tag doesn't match the current parent (span).
    // The unwinding logic should pop both span and div.
    const char *html = "<div><span></div><p></p>";
    js_lex_html_chunk(root, (uint64_t)html, strlen(html), 0);
    
    uint32_t r_idx = resolve_node(root);
    uint64_t div_id = dom_get_first_child(r_idx);
    if (!div_id) { printf("[Test 2] Missing outer DIV\n"); return 1; }
    
    uint32_t div_idx = resolve_node(div_id);
    uint64_t span_id = dom_get_first_child(div_idx);
    if (!span_id) { printf("[Test 2] Missing SPAN inside DIV\n"); return 1; }
    
    uint64_t p_id = dom_get_next_sibling(div_idx);
    if (!p_id) { 
        printf("[Test 2] Missing P after DIV. It was incorrectly nested inside because the DIV didn't close properly.\n"); 
        return 1; 
    }
    
    printf("✅ Tree Test 2 Passed (Mismatched Closing Tag)\n");
    return 0;
}

int verify_tree_3() {
    init_arrays();
    uint64_t root = create_node(3); // DIV root
    
    // Stray closing tag: <div></p><span></span></div>
    const char *html = "<div></p><span></span></div><p></p>";
    js_lex_html_chunk(root, (uint64_t)html, strlen(html), 0);
    
    uint32_t r_idx = resolve_node(root);
    uint64_t div_id = dom_get_first_child(r_idx);
    if (!div_id) { printf("[Test 3] Missing outer DIV\n"); return 1; }
    
    uint32_t div_idx = resolve_node(div_id);
    uint64_t span_id = dom_get_first_child(div_idx);
    if (!span_id) { 
        // If it popped on </p>, span would be a sibling to div.
        printf("[Test 3] Missing SPAN inside DIV. Stray </p> incorrectly popped the tree.\n"); 
        return 1; 
    }
    
    uint64_t p_id = dom_get_next_sibling(div_idx);
    if (!p_id) { 
        printf("[Test 3] Missing P after DIV.\n"); 
        return 1; 
    }
    
    printf("✅ Tree Test 3 Passed (Stray Closing Tag Ignored)\n");
    return 0;
}

int c_lexer_tree_test() {
    printf("--- Epic: HTML Lexer Tree Parsing Edge Cases ---\n");
    
    if (verify_tree_1() != 0) return 1;
    if (verify_tree_2() != 0) return 1;
    if (verify_tree_3() != 0) return 1;
    
    return 0;
}
