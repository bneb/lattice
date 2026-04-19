// Sprint 6 Form Submission Test Bridge
// Extends Sprint 5 bridge with URL construction and navigation capture.

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ── Sprint 5 forwarding (focus + text mutation) ──
extern void dom_handle_click_focus(uint32_t node_idx);
extern uint32_t dom_get_active_focus(void);
extern uint32_t dom_get_active_cursor(void);
extern void ext_dom_mutate_text(uint32_t node_idx, uint8_t char_code, uint8_t is_backspace);
extern uint32_t find_form_ancestor(uint32_t node_idx);

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

uint32_t test_find_form_ancestor(uint32_t node_idx) {
    return find_form_ancestor(node_idx);
}

// ── Sprint 6: Navigation capture ──
static char last_nav_url[4096];
static uint32_t last_nav_url_len = 0;

// Override sys_browser_navigate to capture the URL instead of navigating
void sys_browser_navigate(uint64_t ptr, uint32_t len) {
    if (len > 4095) len = 4095;
    memcpy(last_nav_url, (const char*)(uintptr_t)ptr, len);
    last_nav_url[len] = '\0';
    last_nav_url_len = len;
    printf("  [NAV] Captured navigation URL: %s (%u bytes)\n", last_nav_url, len);
}

// ── Sprint 6: URL construction wrapper ──
extern void construct_search_url_and_navigate(
    uint64_t action_ptr, uint32_t action_len,
    uint64_t name_ptr, uint32_t name_len,
    uint64_t value_ptr, uint32_t value_len);

uint64_t test_construct_and_get_url(
    uint64_t action_ptr, uint32_t action_len,
    uint64_t name_ptr, uint32_t name_len,
    uint64_t value_ptr, uint32_t value_len
) {
    construct_search_url_and_navigate(
        action_ptr, action_len,
        name_ptr, name_len,
        value_ptr, value_len);
    return (uint64_t)(uintptr_t)last_nav_url;
}

uint64_t test_get_last_nav_url_ptr(void) {
    return (uint64_t)(uintptr_t)last_nav_url;
}

uint32_t test_get_last_nav_url_len(void) {
    return last_nav_url_len;
}
