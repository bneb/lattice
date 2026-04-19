// Sprint 5 Keyboard Test Bridge
// Thin C wrappers around dom.salt functions to bypass Salt cross-module
// global resolution issues with ACTIVE_FOCUS_NODE.

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

extern void dom_handle_click_focus(uint32_t node_idx);
extern uint32_t dom_get_active_focus(void);
extern uint32_t dom_get_active_cursor(void);
extern void ext_dom_mutate_text(uint32_t node_idx, uint8_t char_code, uint8_t is_backspace);

void test_click_focus(uint32_t node_idx) {
    dom_handle_click_focus(node_idx);
}

uint32_t test_get_focus(void) {
    return dom_get_active_focus();
}

void test_mutate_text(uint32_t node_idx, uint8_t ch, uint8_t is_backspace) {
    ext_dom_mutate_text(node_idx, ch, is_backspace);
}

uint32_t test_get_cursor(void) {
    return dom_get_active_cursor();
}

extern uint32_t find_form_ancestor(uint32_t node_idx);

uint32_t test_find_form_ancestor(uint32_t node_idx) {
    return find_form_ancestor(node_idx);
}
