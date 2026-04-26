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

// ============================================================================
// Phase 21: Typographic Measurement Bridge (CoreText)
// ============================================================================
// Returns the physical bounding box for a UTF-8 string at a given font size.
// Called by Salt layout solver for intrinsic sizing of text nodes.

static float g_measured_text_w = 0.0f;
static float g_measured_text_h = 0.0f;

// Unconstrained measurement: returns natural single-line width
void ext_measure_text_node(const char* text, uint32_t text_len, float font_size, float* out_width, float* out_height) {
    if (!text || text_len == 0 || font_size <= 0.0f) {
        *out_width = 0.0f;
        *out_height = 0.0f;
        return;
    }
    
    NSString *string = [[NSString alloc] initWithBytes:text
                                                length:text_len
                                              encoding:NSUTF8StringEncoding];
    if (!string || string.length == 0) {
        *out_width = 0.0f;
        *out_height = 0.0f;
        return;
    }
    
    CTFontRef font = CTFontCreateUIFontForLanguage(kCTFontUIFontSystem, (CGFloat)font_size, NULL);
    NSDictionary *attributes = @{ (id)kCTFontAttributeName: (__bridge id)font };
    NSAttributedString *attrString = [[NSAttributedString alloc] initWithString:string
                                                                    attributes:attributes];
    
    CTFramesetterRef framesetter = CTFramesetterCreateWithAttributedString(
        (CFAttributedStringRef)attrString);
    
    CGSize targetSize = CGSizeMake(CGFLOAT_MAX, CGFLOAT_MAX);
    CGSize fitSize = CTFramesetterSuggestFrameSizeWithConstraints(
        framesetter, CFRangeMake(0, 0), NULL, targetSize, NULL);
    
    *out_width = (float)ceil(fitSize.width);
    *out_height = (float)ceil(fitSize.height);
    
    CFRelease(framesetter);
    CFRelease(font);
}

// Salt FFI entry: measures text and caches results in globals
void ext_measure_text_cached(uint64_t text_ptr, uint32_t text_len, float font_size) {
    g_measured_text_w = 0.0f;
    g_measured_text_h = 0.0f;
    
    if (!text_ptr || text_len == 0 || font_size <= 0.0f) return;
    
    ext_measure_text_node((const char*)text_ptr, text_len, font_size,
                          &g_measured_text_w, &g_measured_text_h);
}

// Constrained-width measurement: respects max_width for word wrapping
void ext_measure_text_constrained(uint64_t text_ptr, uint32_t text_len, float font_size, float max_width) {
    g_measured_text_w = 0.0f;
    g_measured_text_h = 0.0f;
    if (!text_ptr || text_len == 0 || font_size <= 0.0f) {
        return;
    }
    
    NSString *string = [[NSString alloc] initWithBytes:(const char*)text_ptr
                                                length:text_len
                                              encoding:NSUTF8StringEncoding];
    if (!string || string.length == 0) return;
    
    CTFontRef font = CTFontCreateUIFontForLanguage(kCTFontUIFontSystem, (CGFloat)font_size, NULL);
    NSDictionary *attributes = @{ (id)kCTFontAttributeName: (__bridge id)font };
    NSAttributedString *attrString = [[NSAttributedString alloc] initWithString:string
                                                                    attributes:attributes];
    
    CTFramesetterRef framesetter = CTFramesetterCreateWithAttributedString(
        (CFAttributedStringRef)attrString);
    
    // Constrain width to max_width for multiline word wrapping
    CGSize targetSize = CGSizeMake((CGFloat)max_width, CGFLOAT_MAX);
    CGSize fitSize = CTFramesetterSuggestFrameSizeWithConstraints(
        framesetter, CFRangeMake(0, 0), NULL, targetSize, NULL);
    
    g_measured_text_w = (float)ceil(fitSize.width);
    g_measured_text_h = (float)ceil(fitSize.height);
    
    CFRelease(framesetter);
    CFRelease(font);
}

// Getter functions for Salt to read cached measurements
float ext_get_measured_text_w(void) { return g_measured_text_w; }
float ext_get_measured_text_h(void) { return g_measured_text_h; }

