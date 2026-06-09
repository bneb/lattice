import re

with open("README.md", "r") as f:
    content = f.read()

# Replace the title and badges
title_regex = r"# Salt \+ KeuOS\n\n\*\*A Microkernel for High-Performance Distributed Workloads,\*\*\n\*\*built in a systems language with embedded formal verification\.\*\*\n\nSalt is an ahead-of-time compiled systems language that combines the performance of C with compile-time safety through an embedded Z3 theorem prover\. KeuOS is a microkernel operating system written entirely in Salt, achieving unikernel-level latency while maintaining hardware-enforced Ring 0 / Ring 3 isolation\.\n\nTogether, they form a single system where the language's core capabilities \(formal verification and MLIR-based lowering\) become the operating system's capabilities: zero-trap IPC, proof-carrying descriptors, and cache-line-deterministic data planes\.\n\n\[\!\[Benchmarks\]\(https://img\.shields\.io/badge/Performance-C_Parity_Achieved-brightgreen\?style=flat-square\)\]\(benchmarks/BENCHMARKS\.md\)"
title_replacement = """# Salt + KeuOS

**An experimental systems language with MLIR lowering and Z3 embedded formal verification.**

Salt is an ahead-of-time compiled toy systems language exploring the intersection of MLIR lowering and Z3-based safety. KeuOS is a proof-of-concept microkernel written in Salt.

[![Experimental](https://img.shields.io/badge/Status-Experimental-orange?style=flat-square)]()"""
content = re.sub(title_regex, title_replacement, content, flags=re.DOTALL)

with open("README.md", "w") as f:
    f.write(content)

