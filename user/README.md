# User Space

Ring 3 applications that run on KeuOS.

## Overview
KeuOS user space is built using the same language and primitives (`Region`, `Channel`) as the kernel, but runs with restricted privileges.

## Components

| Directory | Role | Status |
|-----------|------|--------|
| [`facet/`](./facet) | **The Compositor.** A resolution-independent vector UI server. | Planning |
| [`grit/`](./grit) | **The Shell.** A structural, object-oriented shell/REPL. | Planning |

## Formal Verification
Salt allows userspace code to be formally verified using Z3-backed constraints (`requires` and `ensures`). This guarantees memory safety, absence of integer overflow, and precise bounds checking at compile-time without runtime overhead.

- Read the [Userspace Formal Verification Guide](./VERIFICATION.md) to get started.
- See the [Verified Math Example](./examples/verified_math.salt) for code samples.
