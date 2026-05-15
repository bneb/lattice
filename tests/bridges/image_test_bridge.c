// Test bridge for image decoding verification
// Bakes a 4x4 PNG and exposes decode + pixel readback to Salt

#include <stdint.h>
#include <stdio.h>

extern uint8_t* facet_image_decode(const uint8_t* bytes, int len, int* out_w, int* out_h);
extern void facet_image_free(uint8_t* pixels);

// 4x4 PNG: rows 0-1=red(255,0,0), rows 2-3=blue(0,0,255)
// 78 bytes
static const unsigned char test_png_4x4[] = { 0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04, 0x08, 0x06, 0x00, 0x00, 0x00, 0xa9, 0xf1, 0x9e, 0x7e, 0x00, 0x00, 0x00, 0x15, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0x19, 0x33, 0x60, 0x08, 0xa0, 0xf1, 0x31, 0x05, 0x00, 0x2c, 0x50, 0x1f, 0xe1, 0xf5, 0x0c, 0x6f, 0x2d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82 };
static const int test_png_4x4_len = 78;

static uint8_t* decoded_pixels = NULL;
static int decoded_w = 0;
static int decoded_h = 0;

// Decode the baked test PNG. Returns 0 on success.
// Dummy implementations for linker
void css_arena_inc_count(void) {}
void css_arena_set_hash(void) {}
void ext_engine_process_key_down(void) {}
void ext_engine_process_mouse_down(void) {}
void ext_salt_paint_inject_dom_pointers(void) {}
uint32_t hash_string(uint64_t ptr, uint32_t len) { return 0; }
void user__browser__compositor__load_font_atlas(uint64_t pixels, int32_t width, int32_t height) {}

int facet_test_decode_baked_png(void) {
    decoded_pixels = facet_image_decode(test_png_4x4, test_png_4x4_len, &decoded_w, &decoded_h);
    if (!decoded_pixels) return -1;
    printf("Decoded: %dx%d\n", decoded_w, decoded_h);
    return 0;
}

int facet_test_get_decoded_width(void) { return decoded_w; }
int facet_test_get_decoded_height(void) { return decoded_h; }

// Read a specific pixel channel. channel: 0=R, 1=G, 2=B, 3=A
int facet_test_get_pixel(int x, int y, int channel) {
    if (!decoded_pixels || x < 0 || y < 0 || x >= decoded_w || y >= decoded_h) return -1;
    int idx = (y * decoded_w + x) * 4 + channel;
    return decoded_pixels[idx];
}

void facet_test_free_decoded(void) {
    if (decoded_pixels) {
        facet_image_free(decoded_pixels);
        decoded_pixels = NULL;
    }
}
