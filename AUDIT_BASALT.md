# Basalt Audit Report

**Date:** 2024-05-24
**Auditor:** Gemini CLI
**Scope:** `basalt/src/`

## 1. Critical Memory Leaks

### `basalt/src/sampler.salt` (lines 101-102)
**Issue:** The `sample_topp` function allocates `prob_buf` and `idx_buf` scratch buffers on every call but never frees them.
**Risk:** Since sampling is performed for every generated token, this causes a massive memory leak (approx. 384KB per token for a 32k vocab). A 1000-token generation will leak ~384MB of RAM.
**Fix:** Add `free(prob_buf as Ptr<u8>)` and `free(idx_buf as Ptr<u8>)` before the return statements.

### `basalt/src/main.salt` (lines 268-272)
**Issue:** `basalt_engine_free` frees the `EngineState` and `RunState` but neglects to free the RoPE frequency buffers (`freq_cis_real`, `freq_cis_imag`) allocated in `basalt_engine_init`.
**Risk:** Repeatedly initializing and freeing the engine (e.g., in a long-running server or app) will eventually exhaust system memory.

### `basalt/src/main.salt` (line 55)
**Issue:** In `build_freq_cis`, if the second `malloc` fails (`imag_ptr`), the function returns early without freeing the first successful allocation (`real_ptr`).
**Risk:** Leak on OOM condition.

---

## 2. Logic & Correctness Bugs

### `basalt/src/model_loader.salt` (lines 86-87)
**Issue:** In the f32 weight mapping, `freq_cis_real` and `freq_cis_imag` are both assigned to the same `w_ptr` without any offset or increment between them.
**Risk:** The RoPE rotation in `transformer.salt` will use identical values for real and imaginary components, fundamentally breaking the attention mechanism and resulting in gibberish output.

### `basalt/src/sampler.salt` (line 135)
**Issue:** The "Break simulation" in `sample_topp` is non-functional: `for skip in 0..0 {}`. 
**Risk:** The `last_idx` will not stop at the top-p threshold; it will continue to increment until the end of the `n0` loop. This effectively disables Top-P truncation, making it behave like standard sampling from the entire filtered set.

### `basalt/src/model_loader.salt` (line 42)
**Issue:** `is_model_q8` checks `header[7] == 1`. For legacy llama2.c models with 7-int headers (28 bytes), `header[7]` reads the first 4 bytes of the weights table.
**Risk:** False-positive or false-negative quantization detection depending on the first float value of the embedding table. This will cause a crash or garbage output when the wrong kernels are selected.

---

## 3. Performance & Efficiency

### `basalt/src/tokenizer.salt` (line 216)
**Issue:** `bpe_encode_segment` performs a `malloc(1)` and `free` for *every single byte* in the input segment during the initialization phase.
**Risk:** Severe performance degradation during prompt processing. Heap fragmentation.

### `basalt/src/tokenizer.salt` (line 288)
**Issue:** The prompt pre-scan in `bpe_encode` uses nested loops over `text_len`, resulting in $O(N^2)$ complexity relative to the prompt length.
**Risk:** Long prompts will hang the engine for seconds or minutes.

### `basalt/src/main.salt` (line 108)
**Issue:** `mmap_file` hardcodes the mapping length to `1024 * 1024 * 1024` (1GB).
**Risk:** Models larger than 1GB (e.g., Llama-2-7B f32 is 28GB, q8 is 7GB) cannot be loaded.

---

## 4. Stability & Safety

### `basalt/src/tokenizer.salt` (line 212)
**Issue:** `bpe_encode_segment` returns early with an empty `return;` on malloc failure, but the function signature declares an `i64` return type.
**Risk:** Undefined behavior or compiler-dependent garbage return values.

### `basalt/src/transformer.salt` (line 103)
**Issue:** `kw_dim` calculation `(cfg.dim * cfg.n_kv_heads) / cfg.n_heads` relies on integer division.
**Risk:** If `n_heads` does not perfectly divide the product, activation buffer offsets will be misaligned, leading to memory corruption during KV caching.
