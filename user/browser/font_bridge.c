// ============================================================================
// Epic 35: Typography Matrix SDF Baker
// ============================================================================
#define STB_TRUETYPE_IMPLEMENTATION
#include "../../vendor/stb/font_data.h"
#include "../../vendor/stb/stb_truetype.h"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

extern uint8_t user__browser__font__SDF_ATLAS[1048576]; // 1024x1024
extern void init_glyphs();

typedef struct {
  float atlas_x;
  float atlas_y;
  float width;
  float height;
  float advance;
  float bearing_x;
  float bearing_y;
  float ascent;
  float descent;
  float line_gap;
} GlyphMetrics;

extern GlyphMetrics user__browser__font__GLYPH_METRICS[256];

float KERN_TABLE_LOCAL[65536] = {0.0f};

float _get_kerning_offset_c(uint32_t char1, uint32_t char2) {
  uint32_t idx = (char1 * 256) + char2;
  if (idx >= 65536)
    return 0.0f;
  return KERN_TABLE_LOCAL[idx];
}

void init_kerning_atlas(stbtt_fontinfo *font, float scale) {
  for (int i = 32; i < 256; i++) {
    for (int j = 32; j < 256; j++) {
      int kern_advance = stbtt_GetCodepointKernAdvance(font, i, j);
      int idx = (i * 256) + j;
      KERN_TABLE_LOCAL[idx] = (float)kern_advance * scale;
    }
  }
}

void bake_sdf_atlas() {
  stbtt_fontinfo font;
  if (!stbtt_InitFont(&font, vendor_stb_Roboto_Regular_ttf, 0)) {
    printf("[FONT BRIDGE] FAILED TO INIT TTF!\n");
    return; // Failed to init
  }
  printf("[FONT BRIDGE] TTF INIT SUCCESS!\n");

  // Atlas packing variables
  int pack_x = 0;
  int pack_y = 0;
  int max_row_height = 0;

  // Scale for rendering (we will bake at a high res for crisp SDF logic)
  // 64px is a decent size for baking SDFs
  float scale = stbtt_ScaleForPixelHeight(&font, 64.0f);
  int ascent, descent, lineGap;
  stbtt_GetFontVMetrics(&font, &ascent, &descent, &lineGap);

  user__browser__font__GLYPH_METRICS[0].ascent = (float)ascent * scale;
  user__browser__font__GLYPH_METRICS[0].descent = (float)descent * scale;
  user__browser__font__GLYPH_METRICS[0].line_gap = (float)lineGap * scale;
  
  printf("[FONT BRIDGE] Computed ascent: %f, descent: %f, line_gap: %f\n", 
         user__browser__font__GLYPH_METRICS[0].ascent, 
         user__browser__font__GLYPH_METRICS[0].descent, 
         user__browser__font__GLYPH_METRICS[0].line_gap);
  printf("[FONT BRIDGE] PTR: %p, sizeof(GlyphMetrics)=%lu\n", (void*)&user__browser__font__GLYPH_METRICS[0], sizeof(GlyphMetrics));

  init_kerning_atlas(&font, scale);

  // ASCII 32 to 126
  for (int codepoint = 32; codepoint < 256; codepoint++) {
    int advance, lsb;
    stbtt_GetCodepointHMetrics(&font, codepoint, &advance, &lsb);

    int glyph_index = stbtt_FindGlyphIndex(&font, codepoint);

    int w, h, xoff, yoff;
    // Make SDF: scale, padding, onedge_value, pixel_dist_scale
    // 5 padding, 128 onedge, 32 dist scale
    unsigned char *sdf = stbtt_GetGlyphSDF(&font, scale, glyph_index, 5, 128,
                                           32.0f, &w, &h, &xoff, &yoff);

    // Wrap atlas row if needed
    if (pack_x + w > 1024) {
      pack_y += max_row_height + 2;
      pack_x = 0;
      max_row_height = 0;
    }

    if (sdf && w > 0 && h > 0) {
      // Copy SDF into global atlas
      for (int r = 0; r < h; r++) {
        for (int c = 0; c < w; c++) {
          int dst_idx = ((pack_y + r) * 1024) + (pack_x + c);
          if (dst_idx < 1048576) {
            user__browser__font__SDF_ATLAS[dst_idx] = sdf[r * w + c];
          }
        }
      }
      stbtt_FreeSDF(sdf, 0);
    }

    // Record metrics (normalized to 1.0 = 64px) for simple math in Salt
    user__browser__font__GLYPH_METRICS[codepoint].atlas_x =
        (float)pack_x / 1024.0f;
    user__browser__font__GLYPH_METRICS[codepoint].atlas_y =
        (float)pack_y / 1024.0f;
    user__browser__font__GLYPH_METRICS[codepoint].width = (float)w / 1024.0f;
    user__browser__font__GLYPH_METRICS[codepoint].height = (float)h / 1024.0f;
    user__browser__font__GLYPH_METRICS[codepoint].advance = advance * scale;
    user__browser__font__GLYPH_METRICS[codepoint].bearing_x =
        xoff; // includes padding from SDF
    user__browser__font__GLYPH_METRICS[codepoint].bearing_y = yoff;

    pack_x += w + 2;
    if (h > max_row_height)
      max_row_height = h;
  }
}

void* font_bridge_get_metrics_ptr() {
    return &user__browser__font__GLYPH_METRICS[0];
}

void font_bridge_sync_metrics(uint64_t ptr) {
    uint32_t *arr = (uint32_t *)ptr;
    float asc = user__browser__font__GLYPH_METRICS[0].ascent;
    float dsc = user__browser__font__GLYPH_METRICS[0].descent;
    float lg = user__browser__font__GLYPH_METRICS[0].line_gap;
    
    // Copy the IEEE 754 bits directly
    memcpy(&arr[0], &asc, 4);
    memcpy(&arr[1], &dsc, 4);
    memcpy(&arr[2], &lg, 4);
}
