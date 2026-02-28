// =============================================================================
// Basalt WASM Bridge Runtime — Brutalist 6-Export API
// =============================================================================
//
// The flat-memory pipeline. JavaScript is the I/O layer; WASM is the math
// sandbox. Minimize boundary crossings at all costs.
//
// WASM Exports (6 total):
//   basalt_alloc(bytes)                  → void*    Allocate for model data
//   basalt_init(model_ptr, size)         → i32      Parse headers, alloc state
//   basalt_ingest_prompt(tokens_ptr, n)  → void     Bulk prefill (1 crossing)
//   basalt_generate_next()               → i64      Forward + sample → token
//   basalt_get_config(param_id)          → i64      Unified config getter
//   basalt_free()                        → void     Burn the context down
//
// =============================================================================

#include <stddef.h>
#include <stdint.h>

// ── Salt-compiled externs (linked from basalt.o) ─────────────────────────────

extern void *main__basalt_engine_init(uint8_t *model_data, int64_t model_size);
extern void main__basalt_engine_ingest_prompt(void *es_ptr, int64_t *tokens,
                                              int64_t count);
extern int64_t main__basalt_engine_generate_step(void *es_ptr);
extern void main__basalt_engine_free(void *es_ptr);
extern int64_t main__basalt_engine_get_config(void *es_ptr, int64_t param_id);

// ── WASM memory management (bump allocator) ─────────────────────────────────

extern unsigned char __heap_base;
static unsigned char *heap_ptr = 0;

void *malloc(int64_t size) {
  if (!heap_ptr)
    heap_ptr = &__heap_base;
  uintptr_t addr = ((uintptr_t)heap_ptr + 15) & ~(uintptr_t)15;
  heap_ptr = (unsigned char *)(addr + size);
  unsigned char *p = (unsigned char *)addr;
  for (int64_t i = 0; i < size; i++)
    p[i] = 0;
  return (void *)addr;
}

void free(void *ptr) { (void)ptr; }

// ── memcpy / memset shims ───────────────────────────────────────────────────

void *memcpy(void *dest, const void *src, unsigned long n) {
  unsigned char *d = (unsigned char *)dest;
  const unsigned char *s = (const unsigned char *)src;
  for (unsigned long i = 0; i < n; i++)
    d[i] = s[i];
  return dest;
}

void *memset(void *dest, int val, unsigned long n) {
  unsigned char *d = (unsigned char *)dest;
  for (unsigned long i = 0; i < n; i++)
    d[i] = (unsigned char)val;
  return dest;
}

// ── Salt contract violation handler ─────────────────────────────────────────

void __salt_contract_violation(void) { /* no-op in WASM */ }

// ── Engine state (C-side storage — only engine pointer remains) ─────────────

static void *g_engine_ptr = NULL;

// ── Prompt token bridge (legacy — kept for native run_inference) ────────────

#define MAX_PROMPT_TOKENS 8192

static int64_t g_prompt_tokens[MAX_PROMPT_TOKENS];
static int64_t g_prompt_count = 0;

int64_t salt_get_prompt_count(void) { return g_prompt_count; }
int64_t salt_get_prompt_token(int64_t idx) {
  if (idx < 0 || idx >= g_prompt_count)
    return 0;
  return g_prompt_tokens[idx];
}

// ── Clock shim ──────────────────────────────────────────────────────────────

int64_t salt_clock_now(void) { return 0; }

// ── argc/argv shims ─────────────────────────────────────────────────────────

int32_t salt_get_argc(void) { return 0; }
void *salt_get_argv(int32_t idx) { return NULL; }

// ── mmap/open/close shims ───────────────────────────────────────────────────

int32_t open(const void *path, int32_t flags) { return -1; }
void *mmap(void *addr, uint64_t len, int32_t prot, int32_t flags, int32_t fd,
           int64_t offset) {
  return (void *)-1;
}
int32_t close(int32_t fd) { return -1; }

// ── Math shims (no libc in freestanding WASM) ───────────────────────────────

float sqrtf(float x) { return __builtin_sqrtf(x); }
float fabsf(float x) { return __builtin_fabsf(x); }

float expf(float x) {
  if (x > 88.0f)
    return 3.4028235e+38f;
  if (x < -88.0f)
    return 0.0f;
  float t = x * 1.4426950408889634f;
  int32_t i = (int32_t)t;
  if (t < 0 && t != (float)i)
    i--;
  float f = t - (float)i;
  float p = 1.0f + f * (0.6931472f +
                        f * (0.2402265f + f * (0.0554953f + f * 0.0096761f)));
  union {
    float fl;
    int32_t iv;
  } u;
  u.iv = (i + 127) << 23;
  return p * u.fl;
}

float sinf(float x) {
  const float PI = 3.14159265358979323846f;
  const float TWO_PI = 6.28318530717958647692f;
  x = x - TWO_PI * (int32_t)(x / TWO_PI);
  if (x > PI)
    x -= TWO_PI;
  if (x < -PI)
    x += TWO_PI;
  float x2 = x * x;
  return x * (1.0f - x2 * (1.0f / 6.0f - x2 * (1.0f / 120.0f - x2 / 5040.0f)));
}

float cosf(float x) { return sinf(x + 1.5707963267948966f); }

float powf(float base, float e) {
  if (base <= 0.0f)
    return 0.0f;
  union {
    float fl;
    int32_t iv;
  } u;
  u.fl = base;
  float exp_v = (float)((u.iv >> 23) - 127);
  u.iv = (u.iv & 0x007FFFFF) | 0x3F800000;
  float m = u.fl;
  float ln_m = (m - 1.0f) * (2.0f - 0.5f * (m - 1.0f));
  float ln_base = exp_v * 0.6931472f + ln_m;
  return expf(e * ln_base);
}

// ── I/O shims ───────────────────────────────────────────────────────────────

#ifdef __wasm__
__attribute__((import_module("env"), import_name("log_status"))) extern void
js_log_status(const char *msg, int32_t len);
#endif

int64_t write(int32_t fd, const void *buf, int64_t count) {
#ifdef __wasm__
  if (fd == 2 && count > 0) {
    js_log_status((const char *)buf, (int32_t)count);
  }
#endif
  return count;
}

// putchar — Salt's println! uses this for newline
int32_t putchar(int32_t c) {
  char ch = (char)c;
  write(2, &ch, 1);
  return c;
}

// ── Salt println runtime ────────────────────────────────────────────────────

void __salt_print_literal(const char *str, int64_t len) { write(2, str, len); }

static void print_i64(int64_t val) {
  char buf[21];
  int pos = 20;
  int neg = 0;
  buf[pos] = 0;
  if (val < 0) {
    neg = 1;
    val = -val;
  }
  if (val == 0) {
    buf[--pos] = '0';
  }
  while (val > 0) {
    buf[--pos] = '0' + (int)(val % 10);
    val /= 10;
  }
  if (neg)
    buf[--pos] = '-';
  write(2, buf + pos, 20 - pos);
}

void __salt_print_i64(int64_t val) { print_i64(val); }
void __salt_print_u64(int64_t val) { print_i64(val); }
void __salt_print_f64(double val) {
  if (val < 0) {
    write(2, "-", 1);
    val = -val;
  }
  print_i64((int64_t)val);
}
void __salt_print_bool(int8_t val) {
  if (val)
    write(2, "true", 4);
  else
    write(2, "false", 5);
}
void __salt_print_ptr(int64_t val) {
  write(2, "0x", 2);
  char hex[17];
  for (int i = 15; i >= 0; i--) {
    int d = (int)(val & 0xF);
    hex[i] = d < 10 ? '0' + d : 'a' + d - 10;
    val >>= 4;
  }
  write(2, hex, 16);
}

// =============================================================================
// WASM Exports — The Brutalist 6
// =============================================================================

#define WASM_EXPORT __attribute__((visibility("default")))

// 1. Allocate WASM linear memory for model ArrayBuffer
WASM_EXPORT void *basalt_alloc(int64_t bytes) { return malloc(bytes); }

// 2. Init engine from model data. Returns 0 on success, -1 on failure.
WASM_EXPORT int32_t basalt_init(void *model_data, int64_t model_size) {
  g_prompt_count = 0;
  g_engine_ptr = main__basalt_engine_init((uint8_t *)model_data, model_size);
  return g_engine_ptr ? 0 : -1;
}

// 3. Bulk prompt ingest. JS writes Int64Array into WASM memory, passes
// ptr+count.
//    Salt runs the entire prefill loop internally (1 boundary crossing).
WASM_EXPORT void basalt_ingest_prompt(int64_t *tokens_ptr, int64_t count) {
  main__basalt_engine_ingest_prompt(g_engine_ptr, tokens_ptr, count);
}

// 4. One forward pass + sample. Returns token ID, or -1 on EOS/error.
WASM_EXPORT int64_t basalt_generate_next(void) {
  return main__basalt_engine_generate_step(g_engine_ptr);
}

// 5. Unified config getter. param_id: 0=dim, 1=hidden_dim, 2=n_layers,
//    3=n_heads, 4=n_kv_heads, 5=vocab_size, 6=seq_len. Returns -1 for invalid.
WASM_EXPORT int64_t basalt_get_config(int64_t param_id) {
  return main__basalt_engine_get_config(g_engine_ptr, param_id);
}

// 6. Burn it down.
WASM_EXPORT void basalt_free(void) {
  main__basalt_engine_free(g_engine_ptr);
  g_engine_ptr = NULL;
  g_prompt_count = 0;
}

// 7. Reset context (multi-turn chat). Zeros KV cache, resets position.
//    Keeps loaded model weights — no re-init needed.
extern void main__basalt_engine_reset(void *);
WASM_EXPORT void basalt_reset(void) { main__basalt_engine_reset(g_engine_ptr); }

// 8. Set sampling parameters: temperature, top-p, and PRNG seed.
//    temperature_bits and topp_bits are f32 values reinterpreted as i32.
//    Call BEFORE generate_next. seed=0 keeps current PRNG state.
extern void main__basalt_engine_set_sampling(void *, float, float, int64_t);
WASM_EXPORT void basalt_set_sampling(int32_t temperature_bits,
                                     int32_t topp_bits, int64_t seed) {
  union {
    int32_t i;
    float f;
  } t, p;
  t.i = temperature_bits;
  p.i = topp_bits;
  main__basalt_engine_set_sampling(g_engine_ptr, t.f, p.f, seed);
}
