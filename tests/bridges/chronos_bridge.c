#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>

// Salt Exports
extern int32_t js_eval_buffer(uint64_t code_ptr, uint32_t len);
extern void update_timers_mock(uint64_t new_ms);
extern void set_use_mock_time(uint8_t use_mock);

// Main Run Loop step wrapper
extern void run_loop();

// Timer variables mapped
extern uint64_t user__browser__timers__MOCK_TIME_MS;
extern uint8_t user__browser__timers__USE_MOCK_TIME;

// Engine Initializers
extern void ext_salt_airlock_init_allocator();
extern void ext_salt_init_arrays();
extern int32_t js_init_quickjs();
extern void user__browser__css__init_css_defaults();
extern void set_max_test_frames(uint64_t frames);

int32_t c_bridge_chronos_e2e_test() {
    printf("\n--- Epic 52: The Chronos Matrix E2E Test ---\n");
    int pass = 0;
    
    printf("[CHRONOS-E2E] Phase 0: Initialize engine...\n");
    airlock_init_allocator();
    init_arrays();
    extern uint64_t user__os__ipc_ring__IPC_BUFFER_PTR;
    user__os__ipc_ring__IPC_BUFFER_PTR = (uint64_t)malloc(65536);
    int32_t init_result = js_init_quickjs();
    if (init_result < 0) {
        printf("[FAIL] QuickJS init failed.\n");
        return 1;
    }
    user__browser__css__init_css_defaults();
    
    // Crucial: clamp run_loop to exactly 1 tick so we can manually advance clock
    set_max_test_frames(1);
    
    printf("[CHRONOS-E2E] Phase 1: Mocking time...\n");
    user__browser__timers__USE_MOCK_TIME = 1;
    user__browser__timers__MOCK_TIME_MS = 1000; // Start at t=1000ms
    
    printf("[CHRONOS-E2E] Phase 2: Injecting JS script...\n");
    const char* script = 
        "var _e52_frames = 0;\n"
        "function tick() { _e52_frames++; requestAnimationFrame(tick); }\n"
        "requestAnimationFrame(tick);\n"
        "var _e52_timedOut = false;\n"
        "var _e52_intervalCount = 0;\n"
        "setTimeout(function() { _e52_timedOut = true; }, 100);\n"
        "var _e52_intId = setInterval(function() { _e52_intervalCount++; }, 30);\n";
        
    int res = js_eval_buffer((uint64_t)script, strlen(script));
    if (res != 0) {
        printf("  [FAIL] JS eval failed\n");
        return 1;
    }
    printf("  [PASS] JS eval succeeded\n");
    pass++;
    
    printf("[CHRONOS-E2E] Phase 3: Entering main loop (5 frames at 16ms)...\n");
    for (int i = 0; i < 5; i++) {
        user__browser__timers__MOCK_TIME_MS += 16;
        run_loop();
    }
    
    // Check state using evaluation
    const char* check1 = 
        "if (_e52_frames !== 5) throw new Error('Expected 5 frames, got ' + _e52_frames);\n"
        "if (_e52_timedOut !== false) throw new Error('Expected timeout false');\n"
        "if (_e52_intervalCount !== 2) throw new Error('Expected interval 2, got ' + _e52_intervalCount);\n"; // 30ms and 60ms
    res = js_eval_buffer((uint64_t)check1, strlen(check1));
    if (res != 0) {
        printf("  [FAIL] Phase 3 Verification Failed\n");
        return 1;
    }
    printf("  [PASS] Frames=5, TimedOut=false, Interval=2\n");
    pass++;
    
    printf("[CHRONOS-E2E] Phase 4: Fast-forward time (+100ms) and clear interval...\n");
    const char* clear_script = "clearInterval(_e52_intId);\n";
    js_eval_buffer((uint64_t)clear_script, strlen(clear_script));
    
    user__browser__timers__MOCK_TIME_MS += 100;
    run_loop(); // 6th frame
    
    const char* check2 = 
        "if (_e52_frames !== 6) throw new Error('Expected 6 frames, got ' + _e52_frames);\n"
        "if (_e52_timedOut !== true) throw new Error('Expected timeout true');\n"
        "if (_e52_intervalCount !== 2) throw new Error('Interval should have been cleared');\n";
    res = js_eval_buffer((uint64_t)check2, strlen(check2));
    if (res != 0) {
        printf("  [FAIL] Phase 4 Verification Failed\n");
        return 1;
    }
    printf("  [PASS] Frames=6, TimedOut=true, Interval=2 (cleared!)\n");
    pass++;
    
    printf("\n[CHRONOS-E2E] === RESULTS: %d PASS, 0 FAIL ===\n", pass);
    printf("[OK] Epic 52 Completed. Single Page Applications unlocked!\n");
    
    return 0;
}
