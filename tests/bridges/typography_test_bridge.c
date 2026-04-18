// Stubs for main.salt functions referenced by app_main.salt in test builds
#include <stdint.h>

// Font metrics struct matching font.salt GlyphMetrics
typedef struct {
  float atlas_x, atlas_y, width, height, advance, bearing_x, bearing_y;
  float ascent, descent, line_gap;
} GlyphMetrics_Test;
extern GlyphMetrics_Test user__browser__font__GLYPH_METRICS[];

// Expose font line height for test validation (same formula as layout.salt line
// 430)
float get_font_line_height(float font_size) {
  float scale = font_size / 64.0f;
  float ascent = user__browser__font__GLYPH_METRICS[0].ascent;
  float descent = user__browser__font__GLYPH_METRICS[0].descent;
  float gap = user__browser__font__GLYPH_METRICS[0].line_gap;
  return (ascent * scale) - (descent * scale) + (gap * scale);
}

void sys_browser_navigate(uint64_t url_ptr, uint32_t url_len) {}
void sys_js_pump_script_queue(void) {}
void set_frame_count(uint64_t count) {}
uint64_t get_frame_count(void) { return 0; }
void set_dom_content_loaded_fired(uint32_t val) {}
uint32_t get_dom_content_loaded_fired(void) { return 0; }
uint32_t get_max_test_frames(void) { return 10; }
uint32_t check_any_layout_dirty(void) { return 0; }
void pump_websocket_frames(void) {}
