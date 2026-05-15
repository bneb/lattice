# KeuOS User-Space Security & Logic Audit Report

**Date:** 2024-05-23
**Auditor:** Gemini CLI Autonomous Agent
**Scope:** `user/` directory (Lattice Monorepo)
**Status:** CRITICAL ISSUES FOUND

---

## 1. Memory Safety & Buffer Overflows

### 1.1 SPSC Ring Buffer Overflow (Wrap-around Logic)
*   **File:** `user/lib/ring.salt`
*   **Lines:** 84-90 (in `push`), 140-146 (in `pop`)
*   **Issue:** The `push` and `pop` operations use modulo arithmetic to calculate the `next_head` and `new_tail`, but the actual memory copy loop is linear and does not handle the wrap-around case.
*   **Impact:** If a write or read crosses the physical boundary of the ring (i.e., `head + len > capacity`), it will perform an out-of-bounds write/read, leading to memory corruption or crashes.
*   **Recommendation:** Split the copy into two segments (head to end-of-buffer, and start-of-buffer to remaining length) or use a logic that handles wrap-around per byte.

### 1.2 GPU Command Buffer Overflow
*   **File:** `user/browser/compositor.salt`
*   **Lines:** 330-360 (`trigger_hardware_compositor`), 400-430 (`flush_frame`)
*   **Issue:** Both functions iterate over `count` (the number of paint primitives) and write to a fixed-size `GPU_RECT_BUF` without any bounds checking.
*   **Impact:** A malicious or overly complex webpage with thousands of DOM nodes can cause `count` to exceed the GPU buffer size, resulting in a buffer overflow that overwrites adjacent memory in the browser process.

### 1.3 WebSocket Message Truncation/Overflow
*   **File:** `user/browser/main.salt`
*   **Lines:** 180-195 (`pump_websocket_frames`)
*   **Issue:** The code extracts WebSocket payloads into a fixed-size `net.WS_TEMP_DECODE` buffer (8192 bytes). If `payload_len > 8192`, the loop continues but only writes the first 8192 bytes. The code then dispatches the message with the truncated length, potentially leading to garbled data or logic errors in the JS engine.
*   **Impact:** Data integrity loss for large WebSocket messages.

---

## 2. Concurrency & Race Conditions

### 2.1 Non-Atomic SPSC Ring Updates (Inter-Process)
*   **File:** `user/os/ipc_ring.salt`, `user/lib/ring.salt`, `user/lib/socket.salt`
*   **Issue:** These files implement SPSC rings for inter-process communication (IPC) via shared memory. However, they lack hardware memory barriers (`sfence`, `lfence`, `mfence`) or the use of atomic/volatile primitives to ensure that the data written to the buffer is visible to the consumer *before* the head/tail pointers are updated.
*   **Impact:** On weakly-ordered architectures (or due to compiler reordering), a consumer might read "old" or garbage data because the pointer update was visible before the data write.

### 2.2 Global State Race Condition in NetD
*   **File:** `user/netd/router.salt`
*   **Lines:** 75-85 (`bind_stream`)
*   **Issue:** The `bind_stream` function modifies global arrays (`PORT_STATES`, `PORT_STREAMS`, etc.) without any mutex or lock.
*   **Impact:** If multiple processes attempt to bind ports simultaneously, or if NetD is multithreaded, two processes could successfully "claim" the same port if they both pass the `PORT_UNBOUND` check before the first one sets `PORT_BOUND`.

### 2.3 Partial Write Corruption in IPC
*   **File:** `user/os/ipc_ring.salt`
*   **Lines:** 85-105 (`ipc_ring_push_bytes`)
*   **Issue:** The function ignores the return value of `ipc_push_byte`. If the ring buffer becomes full mid-message, it continues to "push" (and fail), effectively dropping bytes from the middle of a framed message.
*   **Impact:** Corrupts the IPC protocol stream, leading to desynchronization and potential exploitation if the parser misinterprets the resulting garbled frame.

---

## 3. Security Vulnerabilities

### 3.1 Predictable File Descriptors
*   **File:** `user/lib/socket.salt`
*   **Line:** 190
*   **Issue:** FDs are generated using `port % 256`.
*   **Impact:** This is highly predictable and allows for easy FD-guessing attacks or denial-of-service by pre-emptively exhausting specific FD slots.

### 3.2 Unsafe CLI Argument Parsing
*   **File:** `user/browser/main.salt`
*   **Lines:** 250-290 (`sys_parse_cli_url_ptr`, `sys_parse_cli_ipc_fd`)
*   **Issue:** The parser iterates until it finds a NULL terminator (`while *((ptr) as &u8) != 0`) without checking the bounds of the `argv` array or the length of the strings provided by the kernel.
*   **Impact:** If a malicious process spawns the browser with malformed `argv`, it can cause an out-of-bounds read or a hang.

---

## 4. Architectural Code Smells & Bugs

### 4.1 Debug Artifact in Production Rendering
*   **File:** `user/browser/compositor.salt`
*   **Line:** 437
*   **Issue:** `*((buf_ptr) as &mut f32) = 55.0;`
*   **Fact:** This line unconditionally overwrites the X-coordinate of the first rendering primitive in every frame with the constant `55.0`.
*   **Impact:** Visual glitch where the first element on every page is shifted to X=55, regardless of CSS. This was likely a diagnostic tool left in the codebase.

### 4.2 Busy-Wait in Browser Main Thread
*   **File:** `user/browser/net.salt`
*   **Lines:** 185-190
*   **Issue:** Performs a tight `while` loop with a decrementing counter (10,000,000) to wait for a shared memory signal from another process.
*   **Impact:** Freezes the browser UI for the duration of the wait. If the other process is slow or hung, the browser becomes unresponsive. This should use a proper event/interrupt-driven notification.

### 4.3 Broken Wrap-around Logic in Socket Library
*   **File:** `user/lib/socket.salt`
*   **Lines:** 45-55 (`spsc_available`)
*   **Issue:** The logic `cap - tail + head` is only correct if `head` and `tail` are already modulo `cap`. However, the push/pop functions increment them linearly without modulo.
*   **Impact:** Once `head` or `tail` exceeds `cap`, the `spsc_available` function will return incorrect values, breaking flow control.
