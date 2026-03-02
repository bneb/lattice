# Lattice Roadmap

This roadmap outlines the progression of Lattice from a research kernel to a secure infrastructure for edge AI agents.

---

## Phase 1: The Sandbox *(Current)*

Our immediate goal is to allow userspace exploration and ABI fuzzing without stability guarantees.

- **Onboarding** — Establish a frictionless build process for Linux and macOS.
- **Verification** — Provide documentation on how userspace programs are formally verified via Z3.
- **Test Suites** — Build out a robust set of Ring 3 test cases.

> [!NOTE]
> ABI Status: **Level 0, Experimental.** System calls may change between commits.

---

## Phase 2: Service Orchestration *(Medium Term)*

We will move toward running non-trivial services in Ring 3 with a solidified core ABI.

- **IPC Formalization** — Finalize the SPSC ring buffer contract for userspace process communication.
- **Memory Allocation** — Implement basic allocation wrappers like `user.alloc` for `sys_brk`.
- **Service Porting** — Run a read-only version of the Lettuce state engine as a standalone userspace process.

---

## Phase 3: The AI Appliance *(Long Term)*

This phase focuses on realizing the end-to-end agent runtime vision.

- **Basalt Integration** — Port the Basalt reasoning engine into Ring 3.
- **Full Pipeline** — Run the complete NetD, Basalt, and Lettuce pipeline entirely as verified services.

---

## Phase 4: Open Ecosystem *(Future)*

General-purpose application development on a stable, formally verified foundation.

- **Standard Library** — Release a comprehensive `salt-std` for userspace.
- **Community Services** — Open the platform for a wider variety of community-submitted applications.
