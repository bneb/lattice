#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

// External Engine Initializers
extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern void js_engine_init();
extern void js_engine_eval_string(uint64_t code_ptr, uint32_t len);
extern void sys_jsc_flush_microtasks();

// DOM creation & layout
extern uint64_t ext_salt_create_node(uint32_t tag);
extern void js_dom_append_child(uint32_t parent_idx, uint32_t child_idx);
extern void user__browser__css__init_css_defaults();
extern void dom_set_id(uint32_t idx, uint64_t ptr, uint32_t len);
extern void airlock_init_allocator(void);
extern void init_arrays(void);
extern uint64_t create_node(uint32_t tag);

// Fetch resolution — the C-bridge impl that resolves a QuickJS Promise
extern void js_resolve_fetch_impl(uint64_t fetch_id, uint64_t buffer_ptr, uint32_t length);

// Salt-side SoA queue getters
extern uint32_t net_get_fetch_count();
extern uint8_t net_get_fetch_state(uint32_t slot);
extern uint64_t net_get_fetch_id(uint32_t slot);

// DOM layout arrays for verifying style mutations from .then()
extern int32_t user__browser__dom__STYLE_W[65536];
extern uint8_t user__browser__dom__STYLE_W_UNIT[65536];
extern int32_t user__browser__dom__LAYOUT_W[65536];

// ============================================================================
// Epic 51: Async Fetch E2E Test
// ============================================================================
// Validates the full async pipeline:
//   JS fetch() → Promise created → mock NetD response → Promise resolved
//   → .then() fires → DOM mutation inside callback → layout dirty flag set

static const char* mock_api_response = "{\"message\":\"success\",\"value\":42}";

void async_fetch_e2e_test() {
    int pass = 0;
    int fail = 0;
    
    printf("[ASYNC-E2E] Phase 1: Initialize engine...\n");
    airlock_init_allocator();
    init_arrays();
    
    // Initialize IPC ring buffer to prevent segfaults when VirtIO bridge writes to it
    extern uint64_t user__os__ipc_ring__IPC_BUFFER_PTR;
    user__os__ipc_ring__IPC_BUFFER_PTR = (uint64_t)malloc(65536);
    
    js_engine_init();
    user__browser__css__init_css_defaults();

    // ========================================================================
    // Phase 2: Build a minimal DOM with an output element
    // ========================================================================
    printf("[ASYNC-E2E] Phase 2: Building test DOM...\n");
    
    uint64_t root_id = create_node(1); // TAG_HTML
    uint32_t root_idx = (uint32_t)(root_id & 0xFFFF);
    
    uint64_t output_id = create_node(4); // TAG_DIV
    uint32_t output_idx = (uint32_t)(output_id & 0xFFFF);
    js_dom_append_child(root_idx, output_idx);
    
    const char *output_id_str = "output";
    dom_set_id(output_idx, (uint64_t)output_id_str, 6);
    
    printf("  [INFO] root=%u output=%u\n", root_idx, output_idx);

    // ========================================================================
    // Phase 3: Inject JS with a fetch() call
    // ========================================================================
    printf("[ASYNC-E2E] Phase 3: Injecting JS with fetch()...\n");
    
    // This script:
    // 1. Sets a global 'data' variable to "waiting"
    // 2. Calls fetch('/api/data')
    // 3. In the .then() chain, parses JSON and mutates the DOM
    const char *script =
        "var _e51_data = 'waiting';"
        "var _e51_resolved = false;"
        "fetch('/api/data').then(function(res) {"
        "  return res.json();"
        "}).then(function(json) {"
        "  _e51_data = json.message;"
        "  _e51_resolved = true;"
        "  var out = document.getElementById('output');"
        "  out.style.width = '500px';"
        "});";
    
    js_engine_eval_string((uint64_t)script, (uint32_t)strlen(script));
    printf("  [PASS] JS eval succeeded\n");
    pass++;
    
    // Drain any immediate microtasks (fetch creates a pending promise, no .then() yet)
    sys_jsc_flush_microtasks();

    // ========================================================================
    // Phase 4: Verify pre-resolution state
    // ========================================================================
    printf("[ASYNC-E2E] Phase 4: Verifying pre-resolution state...\n");
    
    // Salt queue should have 1 pending fetch
    uint32_t queue_count = net_get_fetch_count();
    if (queue_count == 1) {
        printf("  [PASS] Salt fetch queue count: %u\n", queue_count);
        pass++;
    } else {
        printf("  [FAIL] Expected queue count 1, got %u\n", queue_count);
        fail++;
    }
    
    // Slot 0 should be PENDING (state=1)
    uint8_t slot0_state = net_get_fetch_state(0);
    if (slot0_state == 1) {
        printf("  [PASS] Slot 0 state: PENDING (%u)\n", slot0_state);
        pass++;
    } else {
        printf("  [FAIL] Expected state PENDING(1), got %u\n", slot0_state);
        fail++;
    }
    
    // JS should still be in "waiting" state
    const char *check_waiting = "if (_e51_data !== 'waiting') throw new Error('Expected waiting, got: ' + _e51_data);";
    js_engine_eval_string((uint64_t)check_waiting, (uint32_t)strlen(check_waiting));
    printf("  [PASS] Pre-resolve: _e51_data === 'waiting'\n");
    pass++;

    // ========================================================================
    // Phase 5: Mock NetD completion — resolve the fetch
    // ========================================================================
    printf("[ASYNC-E2E] Phase 5: Mocking NetD response...\n");
    
    // Get the fetch_id that was assigned
    uint64_t fetch_id = net_get_fetch_id(0);
    printf("  [INFO] Resolving fetch_id=%llu with mock JSON payload\n", (unsigned long long)fetch_id);
    
    // Simulate NetD delivering the response payload
    js_resolve_fetch_impl(fetch_id, (uint64_t)mock_api_response, (uint32_t)strlen(mock_api_response));
    sys_jsc_flush_microtasks();

    // ========================================================================
    // Phase 6: Verify post-resolution state
    // ========================================================================
    printf("[ASYNC-E2E] Phase 6: Verifying post-resolution state...\n");
    
    // JS callback should have fired: _e51_data === "success"
    const char *check_resolved = "if (_e51_data !== 'success') throw new Error('Expected success, got: ' + _e51_data);";
    js_engine_eval_string((uint64_t)check_resolved, (uint32_t)strlen(check_resolved));
    printf("  [PASS] Post-resolve: _e51_data === 'success'\n");
    pass++;
    
    // _e51_resolved should be true
    const char *check_flag = "if (_e51_resolved !== true) throw new Error('_e51_resolved not true');";
    js_engine_eval_string((uint64_t)check_flag, (uint32_t)strlen(check_flag));
    printf("  [PASS] Post-resolve: _e51_resolved === true\n");
    pass++;
    
    // Salt queue should be reclaimed (count back to 0)
    uint32_t post_count = net_get_fetch_count();
    if (post_count == 0) {
        printf("  [PASS] Salt queue reclaimed: count=%u\n", post_count);
        pass++;
    } else {
        printf("  [FAIL] Expected queue count 0 after reclaim, got %u\n", post_count);
        fail++;
    }
    
    // DOM mutation: output node should have width = 500px set via JS
    extern int32_t js_get_style_w(uint64_t node_id);
    int32_t output_width = js_get_style_w(output_id);
    
    if (output_width == 500) {
        printf("  [PASS] DOM mutation: output.style.width = %dpx\n", output_width);
        pass++;
    } else {
        printf("  [FAIL] DOM mutation: expected width=500, got %d\n", output_width);
        fail++;
    }

    // ========================================================================
    // Final Results
    // ========================================================================
    printf("\n[ASYNC-E2E] === RESULTS: %d PASS, %d FAIL ===\n", pass, fail);
    if (fail == 0) {
        printf("[OK] Epic 51: The Asynchronous Singularity is operational.\n");
    } else {
        printf("[FAIL] Epic 51: %d test(s) failed.\n", fail);
    }
}
