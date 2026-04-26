// Stubs for main.salt functions referenced by app_main.salt / net.salt in test
// builds
#include <stdint.h>

void sys_browser_navigate(uint64_t url_ptr, uint32_t url_len) {}
void sys_js_pump_script_queue(void) {}
void set_frame_count(uint64_t count) {}
uint64_t get_frame_count(void) { return 0; }
void set_dom_content_loaded_fired(uint32_t val) {}
uint32_t get_dom_content_loaded_fired(void) { return 0; }
uint32_t get_max_test_frames(void) { return 10; }
void pump_websocket_frames(void) {}
void init_glyphs(void) {}
