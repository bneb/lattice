#import <CoreText/CoreText.h>
#include <hb.h>
#include <hb-coretext.h>

hb_font_t *hb_system_font = NULL;
hb_buffer_t *hb_global_buffer = NULL;

void sys_typography_init() {
    // 1. Load System UI Font via CoreText
    CTFontRef ct_font = CTFontCreateUIFontForLanguage(kCTFontUIFontSystem, 16.0, NULL);
    
    // 2. Bridge to HarfBuzz
    hb_system_font = hb_coretext_font_create(ct_font);
    
    // 3. Initialize Global Reusable Shaping Buffer
    hb_global_buffer = hb_buffer_create();
    
    CFRelease(ct_font);
}

// Struct to pack results natively for Salt
typedef struct {
    uint32_t glyph_id;
    float x_advance;
    float y_advance;
    float x_offset;
    float y_offset;
} ShapedGlyph;

uint32_t sys_shape_text(const char* text, uint32_t len, ShapedGlyph* out_buffer, uint32_t max_glyphs) {
    hb_buffer_clear_contents(hb_global_buffer);
    hb_buffer_add_utf8(hb_global_buffer, text, len, 0, len);
    
    // HarfBuzz auto-guesses direction (LTR/RTL) and language script
    hb_buffer_guess_segment_properties(hb_global_buffer);
    
    hb_shape(hb_system_font, hb_global_buffer, NULL, 0);
    
    uint32_t glyph_count;
    hb_glyph_info_t *glyph_info = hb_buffer_get_glyph_infos(hb_global_buffer, &glyph_count);
    hb_glyph_position_t *glyph_pos = hb_buffer_get_glyph_positions(hb_global_buffer, &glyph_count);
    
    uint32_t write_count = glyph_count < max_glyphs ? glyph_count : max_glyphs;
    
    // Pack into the SoA-friendly C-struct for Salt
    for (uint32_t i = 0; i < write_count; i++) {
        out_buffer[i].glyph_id = glyph_info[i].codepoint;
        out_buffer[i].x_advance = glyph_pos[i].x_advance / 64.0f; // HarfBuzz uses 26.6 fractional pixels
        out_buffer[i].y_advance = glyph_pos[i].y_advance / 64.0f;
        out_buffer[i].x_offset  = glyph_pos[i].x_offset / 64.0f;
        out_buffer[i].y_offset  = glyph_pos[i].y_offset / 64.0f;
    }
    
    return write_count;
}
