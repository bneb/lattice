#!/usr/bin/env python3
"""Generate exact byte arrays for the E2E JIT tier test."""

# Script 1: JIT benchmark
script1 = """let fib = (n) => {
    if (n <= 1) return 1;
    return fib(n - 1) + fib(n - 2);
};
let t0 = performance.now();
for (let i = 0; i < 1000; i++) {
    fib(15);
    if (i == 100) {
        print("[JIT] Warmup complete...");
    }
}
let t1 = performance.now();
print("[JIT] Benchmark Complete.");"""

# Script 2: GC verification
script2 = """let c0 = getFreeNodeCount();
{
  let n = document.createElement("div");
  print("[GC] Node created: " + n.tagName);
}
gc();
let c1 = getFreeNodeCount();
if (c1 > c0) {
  print("VERIFICATION SUCCESS");
} else {
  print("VERIFICATION FAILURE");
}
"""

# Filename
filename = "jit.js\x00\x00"

def to_salt_array(name, data):
    b = data.encode('utf-8') if isinstance(data, str) else data
    size = len(b)
    # Format as Salt array literal with 16 bytes per line
    lines = []
    for i in range(0, size, 16):
        chunk = b[i:i+16]
        lines.append("        " + ", ".join(str(x) for x in chunk))
    body = ",\n".join(lines)
    return f"    let {name}: [u8; {size}] = [\n{body}\n    ];"

print("// === Script 1 (JIT Benchmark) ===")
print(f"// Length: {len(script1.encode('utf-8'))}")
print(to_salt_array("script", script1))
print()
print("// === Filename ===")
print(f"// Length: {len(filename.encode('utf-8'))}")
print(to_salt_array("filename", filename))
print()
print("// === Script 2 (GC Verification) ===")
print(f"// Length: {len(script2.encode('utf-8'))}")
print(to_salt_array("gc_script", script2))
