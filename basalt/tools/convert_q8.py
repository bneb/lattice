#!/usr/bin/env python3
"""
Basalt f32 → q8_0 Model Converter
==================================
Converts a llama2.c-format f32 model to Basalt q8_0 format.

Input:  7-int header (28 bytes) + f32 weights
Output: 8-int header (32 bytes) + mixed f32/q8_0 weights

q8_0 block format (34 bytes per 32 floats):
  - 2 bytes: f16 scale factor (max_abs / 127)
  - 32 bytes: int8 quantized values

Usage:
  python3 convert_q8.py stories15M.bin stories15M_q8.bin
"""

import struct
import sys
import numpy as np
from pathlib import Path


def quantize_q8_block(floats: np.ndarray) -> bytes:
    """Quantize 32 f32 values to a single q8_0 block (34 bytes)."""
    assert len(floats) == 32
    max_abs = np.max(np.abs(floats))
    if max_abs == 0:
        return b'\x00\x00' + b'\x00' * 32

    scale = max_abs / 127.0
    # Quantize: round(value / scale), clamp to [-128, 127]
    quantized = np.clip(np.round(floats / scale), -128, 127).astype(np.int8)

    # Convert scale to f16 bytes (little-endian)
    scale_f16 = np.float16(scale)
    scale_bytes = scale_f16.tobytes()

    return scale_bytes + quantized.tobytes()


def quantize_tensor_q8(data: np.ndarray) -> bytes:
    """Quantize an entire f32 tensor to q8_0 blocks."""
    flat = data.flatten()
    assert len(flat) % 32 == 0, f"Tensor size {len(flat)} not multiple of 32"
    n_blocks = len(flat) // 32
    blocks = []
    for i in range(n_blocks):
        block = flat[i * 32 : (i + 1) * 32]
        blocks.append(quantize_q8_block(block))
    return b''.join(blocks)


def convert(input_path: str, output_path: str):
    data = np.fromfile(input_path, dtype=np.uint8)
    print(f"Input: {input_path} ({len(data):,} bytes)")

    # Parse 7-int header
    header = struct.unpack('7i', data[:28].tobytes())
    dim, hidden_dim, n_layers, n_heads, n_kv_heads, raw_vocab, seq_len = header

    # Handle shared weights (negative vocab_size)
    shared_weights = raw_vocab < 0
    vocab_size = abs(raw_vocab)

    head_size = dim // n_heads
    kv_dim = (dim * n_kv_heads) // n_heads

    print(f"Config: dim={dim}, hidden_dim={hidden_dim}, n_layers={n_layers}, "
          f"n_heads={n_heads}, n_kv_heads={n_kv_heads}, vocab_size={vocab_size}, seq_len={seq_len}")
    print(f"Shared weights: {shared_weights}")

    # Validate dim % 32
    assert dim % 32 == 0, f"dim={dim} not multiple of 32 — cannot quantize"
    assert hidden_dim % 32 == 0, f"hidden_dim={hidden_dim} not multiple of 32 — cannot quantize"

    # Read weights as f32 (skip 28-byte header)
    weights = np.frombuffer(data[28:].tobytes(), dtype=np.float32)
    offset = 0

    def read_f32(count: int) -> np.ndarray:
        nonlocal offset
        result = weights[offset : offset + count].copy()
        offset += count
        return result

    # 1. Token embedding (vocab_size × dim)
    token_embedding = read_f32(vocab_size * dim)

    # 2. rms_att_weight (n_layers × dim) — stays f32
    rms_att_weight = read_f32(n_layers * dim)

    # 3. wq (n_layers × dim × dim)
    wq = read_f32(n_layers * dim * (n_heads * head_size))

    # 4. wk (n_layers × kv_dim × dim)
    wk = read_f32(n_layers * dim * (n_kv_heads * head_size))

    # 5. wv (n_layers × kv_dim × dim)
    wv = read_f32(n_layers * dim * (n_kv_heads * head_size))

    # 6. wo (n_layers × dim × dim)
    wo = read_f32(n_layers * (n_heads * head_size) * dim)

    # 7. rms_ffn_weight (n_layers × dim) — stays f32
    rms_ffn_weight = read_f32(n_layers * dim)

    # 8. w1 (n_layers × hidden_dim × dim)
    w1 = read_f32(n_layers * dim * hidden_dim)

    # 9. w2 (n_layers × dim × hidden_dim)
    w2 = read_f32(n_layers * hidden_dim * dim)

    # 10. w3 (n_layers × hidden_dim × dim)
    w3 = read_f32(n_layers * dim * hidden_dim)

    # 11. rms_final_weight (dim) — stays f32
    rms_final_weight = read_f32(dim)

    print(f"Read {offset:,} floats ({offset * 4:,} bytes)")

    # Compute sizes
    def q8_size(n_floats: int) -> int:
        return (n_floats // 32) * 34

    f32_size = lambda n: n * 4

    # Write output
    with open(output_path, 'wb') as f:
        # 8-int header (32 bytes)
        # Preserve the original raw_vocab (which may be negative for shared weights)
        new_header = struct.pack('8i',
            dim, hidden_dim, n_layers, n_heads, n_kv_heads,
            raw_vocab,  # preserve shared_weights flag
            seq_len,
            1  # quant_type = q8_0
        )
        f.write(new_header)
        written = 32

        # 1. Token embedding — q8_0
        emb_q8 = quantize_tensor_q8(token_embedding)
        f.write(emb_q8)
        written += len(emb_q8)
        print(f"  token_embedding: {len(token_embedding)*4:>10,} → {len(emb_q8):>10,} bytes ({len(token_embedding)*4/len(emb_q8):.1f}x)")

        # 2. rms_att_weight — f32
        rms_att_bytes = rms_att_weight.tobytes()
        f.write(rms_att_bytes)
        written += len(rms_att_bytes)

        # 3. wq — q8_0
        wq_q8 = quantize_tensor_q8(wq)
        f.write(wq_q8)
        written += len(wq_q8)
        print(f"  wq:              {len(wq)*4:>10,} → {len(wq_q8):>10,} bytes ({len(wq)*4/len(wq_q8):.1f}x)")

        # 4. wk — q8_0
        wk_q8 = quantize_tensor_q8(wk)
        f.write(wk_q8)
        written += len(wk_q8)

        # 5. wv — q8_0
        wv_q8 = quantize_tensor_q8(wv)
        f.write(wv_q8)
        written += len(wv_q8)

        # 6. wo — q8_0
        wo_q8 = quantize_tensor_q8(wo)
        f.write(wo_q8)
        written += len(wo_q8)

        # 7. rms_ffn_weight — f32
        rms_ffn_bytes = rms_ffn_weight.tobytes()
        f.write(rms_ffn_bytes)
        written += len(rms_ffn_bytes)

        # 8. w1 — q8_0
        w1_q8 = quantize_tensor_q8(w1)
        f.write(w1_q8)
        written += len(w1_q8)

        # 9. w2 — q8_0
        w2_q8 = quantize_tensor_q8(w2)
        f.write(w2_q8)
        written += len(w2_q8)

        # 10. w3 — q8_0
        w3_q8 = quantize_tensor_q8(w3)
        f.write(w3_q8)
        written += len(w3_q8)

        # 11. rms_final_weight — f32
        rms_final_bytes = rms_final_weight.tobytes()
        f.write(rms_final_bytes)
        written += len(rms_final_bytes)

    # Summary
    original_size = len(data)
    ratio = original_size / written
    print(f"\nOutput: {output_path} ({written:,} bytes)")
    print(f"Compression: {original_size:,} → {written:,} bytes ({ratio:.2f}x)")


if __name__ == '__main__':
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <input.bin> <output_q8.bin>")
        sys.exit(1)
    convert(sys.argv[1], sys.argv[2])
