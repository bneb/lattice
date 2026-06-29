# Math, ML & Specialized Modules

## `std.math` — Mathematics

Compiler intrinsics lowered to LLVM native opcodes. Available in both `f32` and `f64` variants.

```salt
use std.math
```

### f32 functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `expf` | `(f32) -> f32` | e^x |
| `logf` | `(f32) -> f32` | Natural logarithm |
| `powf` | `(f32, f32) -> f32` | x^y |
| `sqrtf` | `(f32) -> f32` | Square root |
| `sinf` | `(f32) -> f32` | Sine |
| `cosf` | `(f32) -> f32` | Cosine |
| `fabsf` | `(f32) -> f32` | Absolute value |
| `floorf` | `(f32) -> f32` | Round down |
| `ceilf` | `(f32) -> f32` | Round up |

### f64 functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `exp` | `(f64) -> f64` | e^x |
| `log` | `(f64) -> f64` | Natural logarithm |
| `pow` | `(f64, f64) -> f64` | x^y |
| `sqrt` | `(f64) -> f64` | Square root |
| `sin` | `(f64) -> f64` | Sine |
| `cos` | `(f64) -> f64` | Cosine |
| `fabs` | `(f64) -> f64` | Absolute value |
| `floor` | `(f64) -> f64` | Round down |
| `ceil` | `(f64) -> f64` | Round up |

### Bit manipulation (u64)

| Function | Signature | Description |
|----------|-----------|-------------|
| `ctz_u64` | `(u64) -> u64` | Count trailing zeros |
| `clz_u64` | `(u64) -> u64` | Count leading zeros |
| `popcount_u64` | `(u64) -> u64` | Population count (set bits) |

## `std.simd` — Portable SIMD

Swiss-Table Group abstraction and SIMD vector types for cache-friendly data structures.

```salt
use std.simd
```

### Group — Swiss-Table probing (8 bytes at a time)

| Method | Signature | Description |
|--------|-----------|-------------|
| `load` | `(Ptr<i8>) -> Group` | Load 8 control bytes from memory |
| `match_tag` | `(self, i8) -> u64` | Bitmask with high bit set for matches |
| `first_empty` | `(self) -> i64` | Byte offset (0-7) of first EMPTY slot, or -1 |
| `has_empty` | `(self) -> bool` | True if any EMPTY exists in this group |
| `first_match` | `(self, i8) -> i64` | Byte offset of first match, or -1 |
| `width` | `() -> i64` | Group width in bytes (always 8) |

### u64x2 — 2-lane 64-bit vector

| Method | Signature | Description |
|--------|-----------|-------------|
| `splat` | `(u64) -> u64x2` | Broadcast value to both lanes |
| `new` | `(u64, u64) -> u64x2` | Create from two values |
| `xor` | `(&self, u64x2) -> u64x2` | Lane-wise XOR |
| `mul` | `(&self, u64x2) -> u64x2` | Lane-wise multiply |
| `extract_lo` | `(&self) -> u64` | Extract lane 0 |
| `extract_hi` | `(&self) -> u64` | Extract lane 1 |
| `reduce_xor` | `(&self) -> u64` | Horizontal XOR reduction |

### Prefetch intrinsics

| Function | Signature | Description |
|----------|-----------|-------------|
| `prefetch_read` | `(Ptr<i8>) -> ()` | Prefetch for read (high locality) |
| `prefetch_read_once` | `(Ptr<i8>) -> ()` | Prefetch for read (low locality) |
| `prefetch_write` | `(Ptr<i8>) -> ()` | Prefetch for write |

### Branch prediction hints

| Function | Signature | Description |
|----------|-----------|-------------|
| `unlikely` | `(bool) -> bool` | Mark condition as unlikely (cold path) |
| `likely` | `(bool) -> bool` | Mark condition as likely (hot path) |

## `std.linalg` — Linear Algebra

Shape-safe tensor operations.

```salt
use std.linalg
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `matmul` | `(Tensor<f32, [M,N]>, Tensor<f32, [N,P]>) -> Tensor<f32, [M,P]>` | Matrix multiply |
| `fma_update` | `(&mut Tensor<f32, [R,C]>, f32, Tensor<f32, [R,K]>, Tensor<f32, [K,C]>) -> ()` | Fused multiply-add update: self += scale * (A @ B) |

## `std.nn` — Neural Network Operations

In-place element-wise operations on `Ptr<f32>` buffers. All activations operate on f32 (not f64).

```salt
use std.nn
```

### Activations (`std.nn.activations`)

| Function | Signature | Description |
|----------|-----------|-------------|
| `relu` | `(Ptr<f32>, i64) -> ()` | ReLU: dst[i] = max(0, dst[i]), in-place |
| `relu_grad` | `(f32) -> f32` | ReLU gradient: 1.0 if x > 0 else 0.0 |
| `sigmoid` | `(Ptr<f32>, i64) -> ()` | Sigmoid: 1/(1+e^(-x)), in-place |
| `tanh_activation` | `(Ptr<f32>, i64) -> ()` | Tanh: (e^(2x)-1)/(e^(2x)+1), in-place |

### Ops (`std.nn.ops`)

| Function | Signature | Description |
|----------|-----------|-------------|
| `add_bias` | `(Ptr<f32>, i64, Ptr<f32>) -> ()` | dst[i] += bias[i] |
| `zeros` | `(Ptr<f32>, i64) -> ()` | Zero-fill: dst[i] = 0.0 |
| `scale` | `(Ptr<f32>, i64, f32) -> ()` | dst[i] *= factor |
| `argmax` | `(Ptr<f32>, i64) -> i64` | Return index of maximum element |

### Loss (`std.nn.loss`)

| Function | Signature | Description |
|----------|-----------|-------------|
| `softmax_cross_entropy_grad` | `(Ptr<f32>, Ptr<f32>, i64, i64) -> ()` | Fused softmax + cross-entropy gradient |

## `std.autograd` — Automatic Differentiation

Reverse-mode autodiff for training neural networks.

```salt
use std.autograd
```

## `std.crypto` — TLS Bridge

BearSSL FFI bridge for TLS connections.

```salt
use std.crypto.tls
```

Delegates to BearSSL (`vendor/bearssl/`) for TLS 1.2 handshake, certificate validation, and encrypted transport. The Salt API provides a simplified wrapper around the C implementation.

## `std.encoding` — Data Encoding

Base64 and hex encoding/decoding.

```salt
use std.encoding.encoding
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `base64_encode` | `(Ptr<u8>, i64, Ptr<u8>) -> i64` | Encode bytes to base64 |
| `base64_encoded_len` | `(i64) -> i64` | Compute base64 output length for input |
| `hex_encode` | `(Ptr<u8>, i64, Ptr<u8>) -> i64` | Encode bytes to hex (lowercase) |
| `hex_encoded_len` | `(i64) -> i64` | Compute hex output length for input |

## `std.json` — JSON Parsing & Writing

Zero-copy JSON parser and streaming writer.

```salt
use std.json.json.{JsonParser, JsonWriter, JsonArray, JsonObject}
```

**Parsing:**
```salt
use std.json.json.JsonParser, JsonArray, JSON_NUMBER

let mut p = JsonParser::new("42" as Ptr<u8>, 2);
let val = p.parse_value();  // JsonValue { type_tag: JSON_NUMBER, num_val: 42.0 }

// Parse an array
let mut p = JsonParser::new("[1, true, null]" as Ptr<u8>, 15);
let mut arr = JsonArray::new();
p.parse_array(&mut arr);     // arr.len == 3
let first = arr.num_vals[0]; // 1.0
```

**Writing:**
```salt
use std.json.json.JsonWriter

let mut w = JsonWriter::new(buf, 4096);
w.write_object_start();
w.write_key("x" as Ptr<u8>, 1);
w.write_i64(42);
w.write_object_end();  // {"x":42}
```

## `std.process` — Subprocess Execution

```salt
use std.process.Command

let status = Command::new("/bin/echo")
    .arg1("hello")
    .execute();
// status = exit code (0 = success)
```

## `std.path` — Path Manipulation

```salt
use std.path
```

## `std.random` — Random Numbers

```salt
use std.random
```

## `std.time` — Clock & Timing

```salt
use std.time
```

## `std.env` — Environment Variables

```salt
use std.env
```

## `std.args` — Command-Line Arguments

```salt
use std.args
```
