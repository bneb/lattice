# Math, ML & Specialized Modules

## `std.math` — Vectorized Mathematics

SIMD-accelerated transcendentals with NEON register mapping.

```salt
use std.math
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `sin` | `(f64) -> f64` | Sine |
| `cos` | `(f64) -> f64` | Cosine |
| `tan` | `(f64) -> f64` | Tangent |
| `exp` | `(f64) -> f64` | e^x |
| `log` | `(f64) -> f64` | Natural logarithm |
| `sqrt` | `(f64) -> f64` | Square root |
| `pow` | `(f64, f64) -> f64` | x^y |
| `abs` | `(f64) -> f64` | Absolute value |
| `floor` | `(f64) -> f64` | Round down |
| `ceil` | `(f64) -> f64` | Round up |
| `round` | `(f64) -> f64` | Round to nearest |

## `std.simd` — Portable SIMD

NEON/AVX vector operations using `f32x4` and `i32x4` types.

```salt
use std.simd
```

| Type | Width | Description |
|------|-------|-------------|
| `f32x4` | 128-bit | 4 × f32 (NEON v4sf) |
| `i32x4` | 128-bit | 4 × i32 (NEON v4si) |

| Operation | Signature | Description |
|-----------|-----------|-------------|
| `vector_fma` | `(f32x4, f32x4, f32x4) -> f32x4` | Fused multiply-add (a*b + c) |
| `vector_load` | `(Ptr<f32>) -> f32x4` | Load 4 floats |
| `vector_store` | `(Ptr<f32>, f32x4) -> ()` | Store 4 floats |
| `vector_reduce_add` | `(f32x4) -> f32` | Horizontal sum |
| `vector_broadcast` | `(f32) -> f32x4` | Replicate scalar to all lanes |

## `std.linalg` — Linear Algebra

Tensor operations with `linalg.matmul` lowering to AMX on Apple Silicon.

```salt
use std.linalg
```

| Type/Method | Signature | Description |
|-------------|-----------|-------------|
| `Tensor::new` | `(Ptr<f32>, Shape, Stride) -> Tensor` | Create tensor view |
| `matmul` | `(&Tensor, &Tensor) -> Tensor` | Matrix multiply (`A @ B`) |
| `transpose` | `(&Tensor) -> Tensor` | Transpose tensor |
| `relu` | `(&Tensor) -> Tensor` | ReLU activation |
| `softmax` | `(&Tensor) -> Tensor` | Softmax along last dimension |

**Usage with `@` operator:**
```salt
use std.linalg.Tensor

let output = weights @ input;  // matmul, compiles to linalg.matmul → AMX
```

## `std.nn` — Neural Network Operations

Standard activation and loss functions.

```salt
use std.nn
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `relu` | `(f64) -> f64` | ReLU: max(0, x) |
| `sigmoid` | `(f64) -> f64` | Sigmoid: 1/(1+e^(-x)) |
| `softmax` | `(Ptr<f64>, i64) -> ()` | Softmax in-place |
| `cross_entropy` | `(Ptr<f64>, Ptr<f64>, i64) -> f64` | Cross-entropy loss |
| `mse` | `(Ptr<f64>, Ptr<f64>, i64) -> f64` | Mean squared error |

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

## `std.regex` — Regular Expressions

Regex engine backed by QuickJS's `libregexp`.

```salt
use std.regex.regex
```

## `std.encoding` — Data Encoding

Base64 and hex encoding/decoding.

```salt
use std.encoding.encoding
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `base64_encode` | `(Ptr<u8>, i64, Ptr<u8>) -> i64` | Encode bytes to base64 |
| `base64_decode` | `(Ptr<u8>, i64, Ptr<u8>) -> i64` | Decode base64 to bytes |
| `hex_encode` | `(Ptr<u8>, i64, Ptr<u8>) -> i64` | Encode bytes to hex |
| `hex_decode` | `(Ptr<u8>, i64, Ptr<u8>) -> i64` | Decode hex to bytes |

## `std.json` — JSON Parsing & Writing

Zero-copy JSON parser and streaming writer.

```salt
use std.json.json.{JsonParser, JsonWriter, JsonArray, JsonObject}
```

**Type tags:**
| Constant | Value | Meaning |
|----------|-------|---------|
| `JSON_NUMBER` | — | Numeric value |
| `JSON_STRING` | — | String value |
| `JSON_BOOL` | — | Boolean value |
| `JSON_NULL` | — | null |

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

## `std.fs` — File System Operations

```salt
use std.fs
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `exists` | `(StringView) -> bool` | Check if path exists |
| `is_file` | `(StringView) -> bool` | Check if path is a regular file |
| `is_dir` | `(StringView) -> bool` | Check if path is a directory |
| `remove` | `(StringView) -> Result<()>` | Delete file |
| `rename` | `(StringView, StringView) -> Result<()>` | Rename/move file |

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
