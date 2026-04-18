#include <stdint.h>
#include <string.h>
#include "sha1.c"

static const char base64_table[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

void base64_encode(const uint8_t *data, size_t input_length, char *encoded_data) {
    size_t output_length = 4 * ((input_length + 2) / 3);
    for (size_t i = 0, j = 0; i < input_length;) {
        uint32_t octet_a = i < input_length ? (unsigned char)data[i++] : 0;
        uint32_t octet_b = i < input_length ? (unsigned char)data[i++] : 0;
        uint32_t octet_c = i < input_length ? (unsigned char)data[i++] : 0;

        uint32_t triple = (octet_a << 0x10) + (octet_b << 0x08) + octet_c;

        encoded_data[j++] = base64_table[(triple >> 3 * 6) & 0x3F];
        encoded_data[j++] = base64_table[(triple >> 2 * 6) & 0x3F];
        encoded_data[j++] = base64_table[(triple >> 1 * 6) & 0x3F];
        encoded_data[j++] = base64_table[(triple >> 0 * 6) & 0x3F];
    }
    
    for (size_t i = 0; i < (3 - input_length % 3) % 3; i++) {
        encoded_data[output_length - 1 - i] = '=';
    }
    encoded_data[output_length] = '\0';
}

void generate_ws_accept_key(const char* key, uint32_t len, char* out) {
    char combined[128];
    const char magic_string[] = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    memcpy(combined, key, len);
    memcpy(combined + len, magic_string, 36);
    
    SHA1_CTX ctx;
    uint8_t hash[20];
    SHA1Init(&ctx);
    SHA1Update(&ctx, (const uint8_t*)combined, len + 36);
    SHA1Final(hash, &ctx);
    
    base64_encode(hash, 20, out);
}
