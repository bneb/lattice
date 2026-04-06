# Salt Dialect Definition

**The Mission:** The "High-Level Assembly" of the KeuOS ecosystem.

## Overview
This directory defines the `!salt` MLIR dialect. It acts as the bridge between the rich semantics of the Salt AST and the raw LLVM IR.

## Components

| File | Role |
|------|------|
| [`SaltOps.td`](./SaltOps.td) | **TableGen Definitions.** Defines ops like `salt.yield`, `salt.alloca`. Note: `salt.verify` has been superseded by Z3 Proof-or-Panic in `salt-front` — contracts now lower to standard MLIR (`scf.if`). |
| [`SaltOps.cpp`](./SaltOps.cpp) | **Op Logic.** C++ implementation of verifiers and canonicalizers. |
| [`SaltDialect.td`](./SaltDialect.td) | **Dialect Registration.** Defines the `salt` namespace. |
