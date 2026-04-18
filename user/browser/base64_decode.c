// ============================================================================
// Google Unblock: Native Base64 Data URI Decoder
// ============================================================================
// Zero-dependency C decoder for data:image/... Base64 URIs.
// Google embeds its logo and icons as inline Base64 to bypass HTTP roundtrips.
// This decoder feeds raw pixel bytes directly to Metal IOSurface textures,
// bypassing the net.salt IPC pipeline entirely.
// ============================================================================

#include <stdint.h>
#include <string.h>

// RFC 4648 §4 decoding table — maps ASCII byte to 6-bit value (64 = invalid)
static const uint8_t B64_DECODE_TABLE[256] = {
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64, // 0x00-0x0F
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64, // 0x10-0x1F
    64,64,64,64,64,64,64,64,64,64,64,62,64,64,64,63, // 0x20-0x2F (+, /)
    52,53,54,55,56,57,58,59,60,61,64,64,64, 0,64,64, // 0x30-0x3F (0-9, =)
    64, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14, // 0x40-0x4F (A-O)
    15,16,17,18,19,20,21,22,23,24,25,64,64,64,64,64, // 0x50-0x5F (P-Z)
    64,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40, // 0x60-0x6F (a-o)
    41,42,43,44,45,46,47,48,49,50,51,64,64,64,64,64, // 0x70-0x7F (p-z)
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64, // 0x80-0xFF
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,
    64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,
};

// Decode a Base64 payload into raw bytes.
// Returns the number of decoded bytes written to dst, or -1 on error.
// Silently skips whitespace (CR, LF, space, tab) for robustness.
int32_t base64_decode(const uint8_t *src, uint32_t src_len,
                      uint8_t *dst, uint32_t dst_max) {
    if (!src || !dst || dst_max == 0) return -1;

    uint32_t out = 0;
    uint32_t accum = 0;
    int bits = 0;

    for (uint32_t i = 0; i < src_len; i++) {
        uint8_t ch = src[i];

        // Skip whitespace
        if (ch == ' ' || ch == '\n' || ch == '\r' || ch == '\t') continue;

        // Padding '=' signals end
        if (ch == '=') break;

        uint8_t val = B64_DECODE_TABLE[ch];
        if (val == 64) continue; // Skip invalid characters gracefully

        accum = (accum << 6) | val;
        bits += 6;

        if (bits >= 8) {
            bits -= 8;
            if (out >= dst_max) return -1; // Output buffer overflow
            dst[out++] = (uint8_t)((accum >> bits) & 0xFF);
        }
    }

    return (int32_t)out;
}

// Detect if a byte string is a data: URI.
// Returns 1 if the string starts with "data:", 0 otherwise.
int32_t is_data_uri(const uint8_t *src, uint32_t len) {
    if (!src || len < 5) return 0;
    return (src[0] == 'd' && src[1] == 'a' && src[2] == 't' &&
            src[3] == 'a' && src[4] == ':') ? 1 : 0;
}

// Detect if a data URI is specifically a data:image/ URI.
// Returns 1 if it matches "data:image/", 0 otherwise.
int32_t is_data_image_uri(const uint8_t *src, uint32_t len) {
    if (!src || len < 11) return 0;
    // "data:image/"
    return (src[0] == 'd' && src[1] == 'a' && src[2] == 't' &&
            src[3] == 'a' && src[4] == ':' && src[5] == 'i' &&
            src[6] == 'm' && src[7] == 'a' && src[8] == 'g' &&
            src[9] == 'e' && src[10] == '/') ? 1 : 0;
}

// Find the byte offset where the raw Base64 payload begins.
// Scans for ";base64," and returns the index after the comma.
// Returns -1 if the ";base64," marker is not found.
int32_t data_uri_get_payload_offset(const uint8_t *src, uint32_t len) {
    if (!src || len < 13) return -1; // "data:x;base64," minimum

    // Scan for ";base64,"
    for (uint32_t i = 5; i + 7 < len; i++) {
        if (src[i]   == ';' &&
            src[i+1] == 'b' &&
            src[i+2] == 'a' &&
            src[i+3] == 's' &&
            src[i+4] == 'e' &&
            src[i+5] == '6' &&
            src[i+6] == '4' &&
            src[i+7] == ',') {
            return (int32_t)(i + 8);
        }
    }
    return -1;
}

// Convenience: decode a full data:image/...;base64,XXXX URI in one call.
// Writes decoded bytes to dst. Returns decoded byte count or -1 on error.
int32_t decode_data_uri(const uint8_t *src, uint32_t src_len,
                        uint8_t *dst, uint32_t dst_max) {
    int32_t offset = data_uri_get_payload_offset(src, src_len);
    if (offset < 0) return -1;
    return base64_decode(src + offset, src_len - (uint32_t)offset, dst, dst_max);
}
