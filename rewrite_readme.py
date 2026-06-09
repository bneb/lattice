import re

with open("README.md", "r") as f:
    content = f.read()

# Replace the "Performance" paragraph
perf_regex = r"\*\*Salt achieves exact performance parity with C\*\* across all equivalently-optimized benchmarks.*?LLVM\."
perf_replacement = """**Salt targets performance within 10% of highly optimized C**. Where Salt shines is not in magically beating C, but in achieving C-like performance while maintaining **Zero-Cost Abstraction**: Salt provides formally verified safety, rich generics, and arena memory without paying any runtime penalty. The Z3 proofs discharge at compile time, the arenas free in O(1), and the MLIR backend optimizes precisely like LLVM."""
content = re.sub(perf_regex, perf_replacement, content, flags=re.DOTALL)

# Replace the LETTUCE case study
lettuce_regex = r"\| \*\*Throughput\*\* \| \*\*234,000 ops/sec\*\* \| 115,000 ops/sec \|\n\| \*\*Source\*\* \| 567 lines \| ~100,000 lines \|\n\| \*\*Memory model\*\* \| Arena \+ Swiss-table \| jemalloc \+ dict \|\n\n2× Redis throughput at 0\.6% of the code size\. \[Architecture →\]\(lettuce/\)"
lettuce_replacement = """| **Source** | 567 lines | ~100,000 lines |
| **Memory model** | Arena + Swiss-table | jemalloc + dict |

An experimental proof-of-concept showing how to build a safe, fast data store using Arenas and Z3 verification without lifetime annotations. [Architecture →](lettuce/)"""
content = re.sub(lettuce_regex, lettuce_replacement, content, flags=re.DOTALL)

# Replace the Basalt case study to be more grounded
basalt_regex = r"\| \*\*tok/s\*\* \(stories15M, M4\) \| \*\*~870\*\* \| ~877 \|"
basalt_replacement = """| **Performance** | Targets C-parity | Baseline |"""
content = re.sub(basalt_regex, basalt_replacement, content, flags=re.DOTALL)

# Replace Facet case study
facet_regex = r"\| \*\*Per frame\*\* \(512×512 tiger\) \| 2,186 μs \| 2,214 μs \|\n\| \*\*Throughput\*\* \| 457 fps \| 451 fps \|\n\nSalt's MLIR codegen matches `clang -O3` on a real rendering pipeline with ~160 cubic Bézier curves\. \[Architecture →\]\(user/facet/\)"
facet_replacement = """| **Performance** | Targets C-parity | Baseline |

Salt's MLIR codegen aims to match `clang -O3` on a real rendering pipeline with ~160 cubic Bézier curves. [Architecture →](user/facet/)"""
content = re.sub(facet_regex, facet_replacement, content, flags=re.DOTALL)

# Replace "Design Pillars" intro in README
pillars_regex = r"Most operating systems are written in C.*?The Three Pillars"
pillars_replacement = """Salt is an experimental systems language that replaces traditional runtime checks and manual memory management with **compile-time proofs** and **arena-based allocation**. 

### The Three Pillars

#### 1. Fast Enough (Targeting within 10% of C)
Salt relies on MLIR to lower code into highly optimized native machine code. It does not aim to magically beat C, but rather to achieve performance within 10% of highly optimized C code, allowing systems to be fast without sacrificing safety.

#### 2. Supremely Ergonomic
Salt avoids the cognitive overhead of lifetime annotations and complex borrow checkers. Memory is managed via Arena allocators, allowing developers to write high-performance code with a simple mental model.

#### 3. Formally Verified (Z3)
Salt uses an embedded Z3 theorem prover to verify array bounds, alignment, and custom preconditions at compile time.

### KeuOS Architecture"""
content = re.sub(pillars_regex, pillars_replacement, content, flags=re.DOTALL)

# Remove the old Pillars A, B, C text since it's replaced above.
old_pillars_regex = r"#### 🔥 Pillar A: Zero-Trap Data Plane.*?---\n\n## Approach"
old_pillars_replacement = """---\n\n## Approach"""
content = re.sub(old_pillars_regex, old_pillars_replacement, content, flags=re.DOTALL)


with open("README.md", "w") as f:
    f.write(content)

