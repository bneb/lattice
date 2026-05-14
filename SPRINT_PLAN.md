# LATTICE OS: ECS ARCHITECTURE SPRINT PLAN

## Preamble: The Architecture Summit

*This sprint plan was synthesized following a three-day intensive summit with industry and academic leaders in DBOS, Formal Verification, Data-Oriented Design, and Systems Engineering.*

### Day 1: The Cocktail Hour (Raw, Unsolicited Input)

**Mike Acton (Data-Oriented Design):**
"You're building an OS in an ECS. Great. But if your `SparseSet` uses a `HashMap` for the sparse array, you've already lost. `HashMap` means unpredictable cache misses. A process ID is just a `u32`. Your sparse array needs to be a direct-mapped flat array of indices. Yes, it takes more memory, but memory is cheap. Cache misses are expensive. We need $O(1)$ lookups with zero hashing overhead."

**Gerwin Klein (seL4 / Verification):**
"I like the lock-free IPC, but your capability system relies on a 64-bit SipHash. What happens if a malicious Ring 3 process just brute-forces the `proof_hint`? It's unlikely, but a microkernel can't rely on 'unlikely'. We need Z3 to prove that the *Arbiter* itself never executes an invalid branch, and the capability tokens need to be hardware-backed or cryptographically unforgeable within the lifetime of the OS."

**Leonardo de Moura (Z3):**
"Embedding Z3 into MLIR is smart, but you will hit a wall compiling the ECS queries. If every `world.query::<&mut A, &B>()` requires a fresh Z3 proof of non-aliasing, your compile times will explode. We need to implement a macro-level caching layer in the Salt compiler. If the query pattern is verified once, it shouldn't be re-proven unless the component types change."

**Chris Lattner (LLVM/MLIR):**
"You're relying on LLVM's SLP vectorizer for the ECS arrays. Don't trust it. LLVM is conservative. Since Salt *knows* the data is contiguous and unaliased, you need to emit explicit MLIR `vector` dialect operations before lowering to LLVM. Force the vectorization."

**Michael Stonebraker (DBOS):**
"An ECS is just an in-memory columnar database. But an OS needs durability. When your ECS event loop processes a critical command (like writing to the filesystem block device), how is that journaled? If you lose power, you lose the ECS state. We need a Write-Ahead Log (WAL) for the `Commands` queue."

**Bryan Cantrill (Oxide / Observability):**
"If the whole OS state is a database, I want DTrace for it out of the box. Every time the `Commands` queue is flushed, I want a zero-cost telemetry probe that I can subscribe to in Ring 3."

### Day 2: Whiteboards and Roundtable (Hashing it Out)

The roundtable focused on resolving the tension between **Acton's flat arrays** and **Stonebraker's durability**, all within **Klein's safety envelope**.

1.  **The Sparse Array Compromise:** We agreed with Acton. The `SparseSet` implementation in Salt cannot use `hashbrown` or any hashing. Entity IDs will be generational indices (e.g., 32 bits for index, 32 bits for generation). The `sparse` array is a direct flat array of `u32` pointing into the `dense` array.
2.  **The MLIR Vectorization Mandate:** Lattner and De Moura white-boarded a solution. Salt will map ECS queries directly to MLIR `affine.for` loops. Because Z3 proves the boundaries, we will emit `llvm.assume` intrinsics to force the LLVM backend to aggressively vectorize the `dense` array iteration.
3.  **The DBOS Journaling Reality:** Stonebraker pushed for a WAL, but Cantrill argued that an OS kernel cannot block on a disk write for every internal state change (like a thread yielding). *Compromise:* Only specific, annotated Components (e.g., `FileSystemNode`, `PersistentConfig`) will be tagged for the WAL. Transitory components (like `SchedulingPriority`) stay strictly in-memory.

### Day 3: The Dinner (Trade-offs and The Hybrid Strategy)

Over dinner, the group discussed the reality of building this in Salt. Rust is great, but Salt is what Lattice is built on. 

We agreed on a **Bespoke Hybrid Strategy**:
*   We will not build a general-purpose Rust ECS. We will build a **Salt-native ECS Engine**.
*   The Rust prototype (`lattice_ecs`) was a successful proof-of-concept, but to get Z3 integration and MLIR vectorization, the production ECS must be written in `.salt`.
*   We will leverage Salt's macro/metaprogramming system to generate the `SparseSet` structures at compile time, completely eliminating dynamic dispatch (`dyn Any`) from the kernel.

---

## 🏃‍♂️ SPRINT PLAN: IMPLEMENTING THE LATTICE SALT-ECS

**Agent Instruction:** This sprint plan replaces the legacy object-oriented/global-state kernel structures with a formally verified, zero-allocation, MLIR-vectorized Entity Component System written in Salt. 

### Epic 1: The Core Storage Engine (Acton's Flat Sparse Set)

**Goal:** Implement $O(1)$, cache-friendly, non-hashing component storage in Salt.

*   **Task 1.1: Generational Entity ID**
    *   Create `kernel/ecs/entity.salt`.
    *   Implement `struct Entity { id: u32, gen: u32 }`.
    *   Implement an `EntityAllocator` that maintains a free-list of recycled IDs to prevent the flat arrays from growing infinitely.
*   **Task 1.2: The Flat Sparse Set**
    *   Create `kernel/ecs/sparse_set.salt`.
    *   *Implementation Detail:* Do not use HashMaps. 
    *   `dense: [T; MAX_ENTITIES]`
    *   `sparse: [u32; MAX_ENTITIES]` (Direct mapping: `sparse[entity.id] = dense_index`)
    *   `entity_map: [Entity; MAX_ENTITIES]` (For reverse lookup during iteration)
    *   *Snippet Guide:*
      ```salt
      pub fn get(set: &SparseSet<T>, entity: Entity) -> &T {
          // Z3 will verify entity.id < MAX_ENTITIES
          let dense_idx = set.sparse[entity.id];
          if dense_idx != u32::MAX && set.entity_map[dense_idx].gen == entity.gen {
              return &set.dense[dense_idx];
          }
          return null; // or Option equivalent
      }
      ```

### Epic 2: Compile-Time Queries & MLIR Vectorization (Lattner & De Moura)

**Goal:** Eliminate runtime downcasting. Queries must compile to tight, auto-vectorized loops.

*   **Task 2.1: The World Struct Generator**
    *   Instead of a dynamic `HashMap<TypeId, Box<dyn Storage>>`, the `World` must be statically generated based on registered components.
    *   Create `kernel/ecs/world.salt`.
    *   *Note for Agent:* Since Salt might not have Rust's macro system, implement the `World` as a struct containing explicit `SparseSet` fields for the core OS components: `mem_maps: SparseSet<MemoryMap>`, `priorities: SparseSet<SchedulingPriority>`, etc.
*   **Task 2.2: The MLIR Query Fast-Path**
    *   Write a test in `tests/test_ecs_vectorization.salt` that iterates over 10,000 entities.
    *   Ensure the inner loop does not contain branches or dynamic dispatches.

### Epic 3: The Command Queue and Event Loop (Stonebraker & Cantrill)

**Goal:** Safe, deferred mutation of OS state and interrupt handling.

*   **Task 3.1: The Commands Buffer**
    *   Create `kernel/ecs/commands.salt`.
    *   Implement a double-buffered queue. Systems push closures/function-pointers (or Enums representing actions) to the inactive buffer.
    *   At the end of the tick, swap buffers and execute the commands to mutate `World`.
*   **Task 3.2: The Hardware Event Pipeline**
    *   Create `kernel/ecs/events.salt`.
    *   Implement a lock-free, single-producer multiple-consumer (SPMC) ring buffer for hardware interrupts.
    *   Connect the `pulse.salt` (which we fixed in the previous phase) to push directly into this ECS `Events<HardwareInterrupt>` queue.

### Epic 4: Refactoring the Kernel (The DBOS Transition)

**Goal:** Move existing Lattice monolithic state into the ECS.

*   **Task 4.1: The Scheduler Transition**
    *   Refactor `kernel/core/scheduler.salt`. 
    *   Delete the global `SCHED_ARRAY`.
    *   Implement `sys_yield` to queue an ECS Command that updates the calling entity's `ThreadState` component.
    *   Implement the `SchedulerSystem` that iterates `(&ThreadState, &SchedulingPriority)` to pick the next fiber.
*   **Task 4.2: The PMM Transition**
    *   Refactor `kernel/core/pmm.salt`.
    *   Convert the physical memory shards into ECS Global `Resource`s.

### Definition of Done (The Lodestar Tests)
Before completing the sprint, the coding agent MUST write and pass the following tests:
1.  **test_ecs_scheduler_100k:** Spawns 100,000 processes, asserts the query loop executes without hanging, and validates Z3 proofs pass without `out-of-bounds` panic injections.
2.  **test_ecs_ipc_latency:** Asserts that an IPC message from Entity A to Entity B resolves the recipient's memory map component in $O(1)$ flat-array lookup time.

---
*End of Prompt*
