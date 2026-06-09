// =============================================================================
// KeuOS Image Decoder Bridge (Platform-Agnostic)
// =============================================================================
// Wraps stb_image.h to provide C-ABI functions callable from Salt.
// Pure C — no OS dependencies. Compiles on macOS, Linux, KeuOS.
// =============================================================================

#define STB_IMAGE_IMPLEMENTATION
#define STBI_ONLY_PNG
#define STBI_ONLY_JPEG
#define STBI_ONLY_BMP
#define STBI_NO_STDIO  // No file I/O — we feed raw bytes from the Airlock
#include "stb_image.h"

#include <stdint.h>
#include <stddef.h>

// Decode raw image bytes (PNG/JPEG/BMP) into RGBA pixels.
// Returns pointer to RGBA pixel buffer, or NULL on failure.
// Writes width/height to out_w/out_h.
uint8_t* facet_image_decode(const uint8_t* bytes, int len, int* out_w, int* out_h) {
    int channels;
    // Force 4 channels (RGBA) regardless of source format
    uint8_t* pixels = stbi_load_from_memory(bytes, len, out_w, out_h, &channels, 4);
    return pixels;
}

// Free decoded pixel buffer
void facet_image_free(uint8_t* pixels) {
    if (pixels) stbi_image_free(pixels);
}
