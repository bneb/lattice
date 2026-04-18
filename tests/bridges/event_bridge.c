#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

// External Engine Initializers
extern void airlock_init_allocator();
extern void init_arrays();
extern int32_t js_init_quickjs();
extern int32_t js_eval_buffer(const char* code_ptr, uint32_t len);
extern int32_t js_execute_pending_jobs();

// DOM creation
extern uint64_t create_node(uint32_t tag);
extern void js_dom_append_child(uint32_t parent_idx, uint32_t child_idx);

// Layout
extern void user__browser__css__init_css_defaults();
extern void user__browser__layout__layout_tree();

// Hit-test & event routing
extern uint32_t dom_hit_test(uint32_t node_idx, int32_t target_x, int32_t target_y);
extern void sys_on_mouse_click(int32_t x, int32_t y);
extern uint32_t dom_get_active_focus();

// DOM SoA arrays (direct access for test setup)
extern int32_t user__browser__dom__LAYOUT_X[65536];
extern int32_t user__browser__dom__LAYOUT_Y[65536];
extern int32_t user__browser__dom__LAYOUT_W[65536];
extern int32_t user__browser__dom__LAYOUT_H[65536];
extern int32_t user__browser__dom__STYLE_W[65536];
extern int32_t user__browser__dom__STYLE_H[65536];
extern uint8_t user__browser__dom__STYLE_DISPLAY[65536];
extern uint32_t user__browser__dom__DOM_NODE_TAG[65536];
extern uint32_t user__browser__dom__STYLE_PARENT[65536];
extern uint64_t user__browser__dom__DOM_NODE_COUNT;

// String interning for element IDs
extern void dom_set_id(uint32_t idx, uint64_t ptr, uint32_t len);

// ============================================================================
// Epic 50: Event Routing Matrix E2E Test
// ============================================================================

void event_routing_e2e_test() {
    int pass = 0;
    int fail = 0;
    
    printf("[EVENT-E2E] Phase 1: Initialize engine...\n");
    airlock_init_allocator();
    init_arrays();
    int32_t init_result = js_init_quickjs();
    if (init_result < 0) {
        printf("[FAIL] QuickJS init failed.\n");
        return;
    }
    user__browser__css__init_css_defaults();

    // ========================================================================
    // Phase 2: Build a minimal DOM tree manually
    //
    //   root(1)
    //     └─ body(2)
    //          ├─ container(3) — DIV 400x300 at (100,100)
    //          │    ├─ btn(4) — BUTTON 120x40 at (200,200)
    //          │    └─ other(5) — DIV 120x40 at (200,260)
    //          └─ outside(6) — DIV 100x50 at (800,800)
    // ========================================================================
    printf("[EVENT-E2E] Phase 2: Building test DOM...\n");

    // Create root HTML node (gets index 1 from bump allocator)
    uint64_t root_id = create_node(1); // TAG_HTML
    uint32_t root_idx = (uint32_t)(root_id & 0xFFFF);

    uint64_t body_id = create_node(3); // TAG_BODY
    uint32_t body_idx = (uint32_t)(body_id & 0xFFFF);
    js_dom_append_child(root_idx, body_idx);

    uint64_t container_id = create_node(4); // TAG_DIV
    uint32_t container_idx = (uint32_t)(container_id & 0xFFFF);
    js_dom_append_child(body_idx, container_idx);

    uint64_t btn_id = create_node(20); // TAG_BUTTON
    uint32_t btn_idx = (uint32_t)(btn_id & 0xFFFF);
    js_dom_append_child(container_idx, btn_idx);
    // Set a known ID for getElementById
    const char *btn_id_str = "btn";
    dom_set_id(btn_idx, (uint64_t)btn_id_str, 3);

    uint64_t other_id = create_node(4); // TAG_DIV
    uint32_t other_idx = (uint32_t)(other_id & 0xFFFF);
    js_dom_append_child(container_idx, other_idx);

    uint64_t outside_id = create_node(4); // TAG_DIV
    uint32_t outside_idx = (uint32_t)(outside_id & 0xFFFF);
    js_dom_append_child(body_idx, outside_idx);

    printf("  [INFO] Node indices: root=%u body=%u container=%u btn=%u other=%u outside=%u\n",
           root_idx, body_idx, container_idx, btn_idx, other_idx, outside_idx);

    // ========================================================================
    // Phase 3: Set layout geometry manually (bypass layout engine for precision)
    // ========================================================================
    printf("[EVENT-E2E] Phase 3: Setting layout geometry...\n");

    // Root
    user__browser__dom__LAYOUT_X[root_idx] = 0;
    user__browser__dom__LAYOUT_Y[root_idx] = 0;
    user__browser__dom__LAYOUT_W[root_idx] = 1920;
    user__browser__dom__LAYOUT_H[root_idx] = 1080;
    user__browser__dom__STYLE_DISPLAY[root_idx] = 1;

    // Body
    user__browser__dom__LAYOUT_X[body_idx] = 0;
    user__browser__dom__LAYOUT_Y[body_idx] = 0;
    user__browser__dom__LAYOUT_W[body_idx] = 1920;
    user__browser__dom__LAYOUT_H[body_idx] = 1080;
    user__browser__dom__STYLE_DISPLAY[body_idx] = 1;

    // Container
    user__browser__dom__LAYOUT_X[container_idx] = 100;
    user__browser__dom__LAYOUT_Y[container_idx] = 100;
    user__browser__dom__LAYOUT_W[container_idx] = 400;
    user__browser__dom__LAYOUT_H[container_idx] = 300;
    user__browser__dom__STYLE_DISPLAY[container_idx] = 1;

    // Button (inside container)
    user__browser__dom__LAYOUT_X[btn_idx] = 200;
    user__browser__dom__LAYOUT_Y[btn_idx] = 200;
    user__browser__dom__LAYOUT_W[btn_idx] = 120;
    user__browser__dom__LAYOUT_H[btn_idx] = 40;
    user__browser__dom__STYLE_DISPLAY[btn_idx] = 1;

    // Other div (inside container, below button)
    user__browser__dom__LAYOUT_X[other_idx] = 200;
    user__browser__dom__LAYOUT_Y[other_idx] = 260;
    user__browser__dom__LAYOUT_W[other_idx] = 120;
    user__browser__dom__LAYOUT_H[other_idx] = 40;
    user__browser__dom__STYLE_DISPLAY[other_idx] = 1;

    // Outside div
    user__browser__dom__LAYOUT_X[outside_idx] = 800;
    user__browser__dom__LAYOUT_Y[outside_idx] = 800;
    user__browser__dom__LAYOUT_W[outside_idx] = 100;
    user__browser__dom__LAYOUT_H[outside_idx] = 50;
    user__browser__dom__STYLE_DISPLAY[outside_idx] = 1;

    // ========================================================================
    // Phase 4: Test hit-testing (pure geometric queries)
    // ========================================================================
    printf("[EVENT-E2E] Phase 4: Testing hit-test raycast...\n");

    // Test 1: Click dead center of button
    uint32_t hit1 = dom_hit_test(root_idx, 260, 220);
    if (hit1 == btn_idx) {
        printf("  [PASS] Hit-test: center of button → node %u\n", hit1);
        pass++;
    } else {
        printf("  [FAIL] Hit-test: expected %u, got %u\n", btn_idx, hit1);
        fail++;
    }

    // Test 2: Click inside container but outside button
    uint32_t hit2 = dom_hit_test(root_idx, 110, 110);
    if (hit2 == container_idx) {
        printf("  [PASS] Hit-test: container margin → node %u\n", hit2);
        pass++;
    } else {
        printf("  [FAIL] Hit-test: expected %u (container), got %u\n", container_idx, hit2);
        fail++;
    }

    // Test 3: Click outside everything (but inside root/body)
    uint32_t hit3 = dom_hit_test(root_idx, 1900, 50);
    if (hit3 == body_idx || hit3 == root_idx) {
        printf("  [PASS] Hit-test: empty area → node %u\n", hit3);
        pass++;
    } else {
        printf("  [FAIL] Hit-test: expected body(%u) or root(%u), got %u\n", body_idx, root_idx, hit3);
        fail++;
    }

    // Test 4: Click on "other" div (sibling below button)
    uint32_t hit4 = dom_hit_test(root_idx, 260, 280);
    if (hit4 == other_idx) {
        printf("  [PASS] Hit-test: other div → node %u\n", hit4);
        pass++;
    } else {
        printf("  [FAIL] Hit-test: expected %u (other), got %u\n", other_idx, hit4);
        fail++;
    }

    // ========================================================================
    // Phase 5: Test QuickJS event dispatch via sys_on_mouse_click
    // ========================================================================
    printf("[EVENT-E2E] Phase 5: Testing JS event dispatch...\n");

    // Register a click handler on the button via JS
    const char *setup_script =
        "var _e50_clicked = false;"
        "var _e50_btn = document.getElementById('btn');"
        "_e50_btn.addEventListener('click', function() {"
        "  _e50_clicked = true;"
        "});";
    js_eval_buffer(setup_script, (uint32_t)strlen(setup_script));
    while (js_execute_pending_jobs() > 0) {}

    // Verify initial state
    const char *check_before = "if (_e50_clicked !== false) throw new Error('Pre-click should be false');";
    int32_t pre_result = js_eval_buffer(check_before, (uint32_t)strlen(check_before));
    if (pre_result == 0) {
        printf("  [PASS] Pre-click state: _e50_clicked === false\n");
        pass++;
    } else {
        printf("  [FAIL] Pre-click state check failed\n");
        fail++;
    }

    // Fire a click at the button's center coordinates
    printf("  [INFO] Firing sys_on_mouse_click(%d, %d) targeting button node %u\n", 260, 220, btn_idx);
    sys_on_mouse_click(260, 220);

    // Verify the JS callback was triggered
    const char *check_after = "if (_e50_clicked !== true) throw new Error('Post-click should be true, got: ' + _e50_clicked);";
    int32_t post_result = js_eval_buffer(check_after, (uint32_t)strlen(check_after));
    if (post_result == 0) {
        printf("  [PASS] Post-click state: _e50_clicked === true\n");
        pass++;
    } else {
        printf("  [FAIL] Post-click state check failed — JS callback was NOT triggered\n");
        fail++;
    }

    // ========================================================================
    // Phase 6: Test focus matrix
    // ========================================================================
    printf("[EVENT-E2E] Phase 6: Testing focus matrix...\n");

    // Button should have focus after click
    uint32_t focus_after_btn = dom_get_active_focus();
    if (focus_after_btn == btn_idx) {
        printf("  [PASS] Focus after button click: node %u\n", focus_after_btn);
        pass++;
    } else {
        printf("  [FAIL] Focus: expected %u, got %u\n", btn_idx, focus_after_btn);
        fail++;
    }

    // Click outside on a non-interactive div — should blur
    sys_on_mouse_click(850, 825);
    uint32_t focus_after_blur = dom_get_active_focus();
    if (focus_after_blur == 0) {
        printf("  [PASS] Focus blur after clicking non-interactive: %u\n", focus_after_blur);
        pass++;
    } else {
        printf("  [FAIL] Focus should be 0 after blur, got %u\n", focus_after_blur);
        fail++;
    }

    // ========================================================================
    // Final Results
    // ========================================================================
    printf("\n[EVENT-E2E] === RESULTS: %d PASS, %d FAIL ===\n", pass, fail);
    if (fail == 0) {
        printf("[OK] Epic 50: Event Routing Matrix is operational.\n");
    } else {
        printf("[FAIL] Epic 50: %d test(s) failed.\n", fail);
    }
}
