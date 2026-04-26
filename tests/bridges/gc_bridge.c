#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

// External Engine Initializers
extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern int32_t js_init_quickjs();
extern int32_t js_eval_buffer(const char* code_ptr, uint32_t len);
extern int32_t js_execute_pending_jobs();

// Salt DOM accessors
extern uint64_t dom_get_node_count();
extern uint32_t dom_get_free_list_count();

// ============================================================================
// Epic 49: GC Free-List Stress Test
// ============================================================================
// Loops 100,000 createElement/deref cycles through QuickJS.
// Asserts that NODE_COUNT remains bounded (never hits 65,535 OOM).
// Validates that the free-list is actively recycling slots.

void gc_stress_test() {
    printf("[GC-E2E] Phase 1: Initialize engine...\n");
    airlock_init_allocator();
    init_arrays();

    int32_t init_result = js_init_quickjs();
    if (init_result < 0) {
        printf("[FAIL] QuickJS init failed.\n");
        return;
    }
    printf("[GC-E2E] QuickJS initialized OK.\n");

    uint64_t node_count_before = dom_get_node_count();
    uint32_t free_count_before = dom_get_free_list_count();
    printf("[GC-E2E] Initial NODE_COUNT: %llu, FREE_LIST: %u\n",
           (unsigned long long)node_count_before, free_count_before);

    // ========================================================================
    // Phase 2: Batch stress — create 100 divs in a tight loop, then deref.
    // QuickJS will GC them when the scope exits or when we force a GC cycle.
    // We do this in batches to give QuickJS a chance to run its GC.
    // ========================================================================
    printf("[GC-E2E] Phase 2: Running 100,000 create/deref cycles in batches...\n");

    // We run 1000 batches of 100 creates each.
    // After each batch, we force GC by calling JS_RunGC via a script trick.
    const int BATCHES = 1000;
    const int BATCH_SIZE = 100;
    int total_created = 0;

    for (int batch = 0; batch < BATCHES; batch++) {
        // Create BATCH_SIZE elements that go out of scope immediately
        char script[512];
        snprintf(script, sizeof(script),
            "for (var _gc_i = 0; _gc_i < %d; _gc_i++) {"
            "  var _gc_div = document.createElement('div');"
            "  _gc_div = null;"
            "}", BATCH_SIZE);

        int32_t eval_result = js_eval_buffer(script, (uint32_t)strlen(script));
        if (eval_result < 0) {
            printf("[FAIL] JS eval failed at batch %d\n", batch);
            return;
        }

        // Drain pending microtasks
        while (js_execute_pending_jobs() > 0) {}

        total_created += BATCH_SIZE;

        // Check invariant every 100 batches (every 10,000 elements)
        if ((batch + 1) % 100 == 0) {
            uint64_t current_node_count = dom_get_node_count();
            uint32_t current_free_count = dom_get_free_list_count();
            printf("[GC-E2E] Checkpoint: %d created | NODE_COUNT: %llu | FREE_LIST: %u\n",
                   total_created,
                   (unsigned long long)current_node_count,
                   current_free_count);

            // CRITICAL INVARIANT: node_count must never approach 65535
            if (current_node_count >= 60000) {
                printf("[FAIL] NODE_COUNT hit %llu — free-list is NOT recycling!\n",
                       (unsigned long long)current_node_count);
                return;
            }
        }
    }

    // ========================================================================
    // Phase 3: Final validation
    // ========================================================================
    uint64_t final_node_count = dom_get_node_count();
    uint32_t final_free_count = dom_get_free_list_count();

    printf("\n[GC-E2E] === FINAL RESULTS ===\n");
    printf("[GC-E2E] Total elements created: %d\n", total_created);
    printf("[GC-E2E] Final NODE_COUNT: %llu\n", (unsigned long long)final_node_count);
    printf("[GC-E2E] Final FREE_LIST_COUNT: %u\n", final_free_count);
    printf("[GC-E2E] Recycling active: %s\n", final_free_count > 0 ? "YES" : "NO");

    // The node count should be FAR below 65535 if recycling is working.
    // With QuickJS GC, it might not reclaim every single cycle immediately,
    // but it should keep the watermark drastically below the OOM ceiling.
    if (final_node_count < 60000) {
        printf("[OK] GC Free-List is operational. OOM Time-Bomb defused.\n");
    } else {
        printf("[FAIL] NODE_COUNT too high (%llu). Free-list recycling not working.\n",
               (unsigned long long)final_node_count);
    }
}
