# Test Suite

**The Mission:** Ensure the compiler and standard library strictly adhere to the KeuOS Invariants.

## Structure

| Directory | Role |
|-----------|------|
| `*.salt` (root) | **Integration tests.** End-to-end compiler tests covering hashmap, arena, combinators, file I/O, println, f-strings, iterators, proof witnesses, etc. |
| [`unit/`](./unit) | **Unit tests.** Focused tests for individual language features (20 test files). |
| [`regression/`](./regression) | **Bug fixes.** Tests reproducing specific, fixed bugs to prevent regression. |
| [`cases/`](./cases) | **Test cases.** Grouped test scenarios. |
| [`keuos/`](./keuos) | **KeuOS runtime tests.** Tests for the keuos compilation model. |
| [`v5_isolation/`](./v5_isolation) | **V5 isolation tests.** Module isolation and linking tests. |
| [`v6_vector/`](./v6_vector) | **V6 vector tests.** SIMD and vectorization tests. |
| [`snapshots/`](./snapshots) | **Golden outputs.** Validated compiler output for "known good" states. |

## Run Tests
```bash
cd salt-front && cargo test
```

Individual Salt test files can be compiled and run directly:
```bash
./salt-front/target/release/salt-front tests/sanity.salt -o test_bin && ./test_bin
```
