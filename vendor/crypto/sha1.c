#include <stdint.h>
#include <string.h>

#define SHA1_K0 0x5a827999
#define SHA1_K20 0x6ed9eba1
#define SHA1_K40 0x8f1bbcdc
#define SHA1_K60 0xca62c1d6

typedef struct {
    uint32_t state[5];
    uint32_t count[2];
    uint8_t buffer[64];
} SHA1_CTX;

static void SHA1Transform(uint32_t state[5], const uint8_t buffer[64]) {
    uint32_t a, b, c, d, e, t, w[80];
    int i;
    for (i = 0; i < 16; i++) {
        w[i] = ((uint32_t)buffer[i*4] << 24) | ((uint32_t)buffer[i*4+1] << 16) | ((uint32_t)buffer[i*4+2] << 8) | ((uint32_t)buffer[i*4+3]);
    }
    for (i = 16; i < 80; i++) {
        t = w[i-3] ^ w[i-8] ^ w[i-14] ^ w[i-16];
        w[i] = (t << 1) | (t >> 31);
    }
    a = state[0]; b = state[1]; c = state[2]; d = state[3]; e = state[4];

    for (i = 0; i < 20; i++) {
        t = ((a << 5) | (a >> 27)) + ((b & c) | ((~b) & d)) + e + w[i] + SHA1_K0;
        e = d; d = c; c = (b << 30) | (b >> 2); b = a; a = t;
    }
    for (i = 20; i < 40; i++) {
        t = ((a << 5) | (a >> 27)) + (b ^ c ^ d) + e + w[i] + SHA1_K20;
        e = d; d = c; c = (b << 30) | (b >> 2); b = a; a = t;
    }
    for (i = 40; i < 60; i++) {
        t = ((a << 5) | (a >> 27)) + ((b & c) | (b & d) | (c & d)) + e + w[i] + SHA1_K40;
        e = d; d = c; c = (b << 30) | (b >> 2); b = a; a = t;
    }
    for (i = 60; i < 80; i++) {
        t = ((a << 5) | (a >> 27)) + (b ^ c ^ d) + e + w[i] + SHA1_K60;
        e = d; d = c; c = (b << 30) | (b >> 2); b = a; a = t;
    }

    state[0] += a; state[1] += b; state[2] += c; state[3] += d; state[4] += e;
}

void SHA1Init(SHA1_CTX *context) {
    context->state[0] = 0x67452301;
    context->state[1] = 0xefcdab89;
    context->state[2] = 0x98badcfe;
    context->state[3] = 0x10325476;
    context->state[4] = 0xc3d2e1f0;
    context->count[0] = context->count[1] = 0;
}

void SHA1Update(SHA1_CTX *context, const uint8_t *data, uint32_t len) {
    uint32_t i, j;
    j = context->count[0];
    if ((context->count[0] += len << 3) < j) context->count[1]++;
    context->count[1] += (len >> 29);
    j = (j >> 3) & 63;
    if ((j + len) > 63) {
        memcpy(&context->buffer[j], data, (i = 64 - j));
        SHA1Transform(context->state, context->buffer);
        for (; i + 63 < len; i += 64) {
            SHA1Transform(context->state, &data[i]);
        }
        j = 0;
    } else {
        i = 0;
    }
    memcpy(&context->buffer[j], &data[i], len - i);
}

void SHA1Final(uint8_t digest[20], SHA1_CTX *context) {
    uint32_t i;
    uint8_t finalcount[8];
    uint8_t c;
    for (i = 0; i < 8; i++) {
        finalcount[i] = (uint8_t)((context->count[(i >= 4 ? 0 : 1)] >> ((3 - (i & 3)) * 8)) & 255);
    }
    c = 0200;
    SHA1Update(context, &c, 1);
    while ((context->count[0] & 504) != 448) {
        c = 0000;
        SHA1Update(context, &c, 1);
    }
    SHA1Update(context, finalcount, 8); // append len
    for (i = 0; i < 20; i++) {
        digest[i] = (uint8_t)((context->state[i >> 2] >> ((3 - (i & 3)) * 8)) & 255);
    }
    memset(context, 0, sizeof(*context));
    memset(&finalcount, 0, sizeof(finalcount));
}
