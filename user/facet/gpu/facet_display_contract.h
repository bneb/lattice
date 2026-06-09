// =============================================================================
// KeuOS Facet Display Contract (Platform-Agnostic C-ABI)
// =============================================================================
//
// The Salt engine (compositor.salt) communicates with the display hardware
// exclusively through these C-linkage symbols. Each target platform provides
// its own implementation:
//
//   macOS:    facet_gpu.m + facet_window.m       (Metal + Cocoa)
//   Linux:    facet_gpu_vulkan.c + facet_window_wayland.c  (Vulkan + Wayland/DRM)
//   KeuOS:    facet_gpu_uefi.c + facet_window_uefi.c       (GOP Framebuffer + AVX-512)
//
// The LLVM linker resolves these symbols at compile time based on the target
// triple. The Salt code never references platform-specific types or headers.
// =============================================================================

#ifndef FACET_DISPLAY_CONTRACT_H
#define FACET_DISPLAY_CONTRACT_H

#include <stdint.h>

// --- Window Lifecycle ---
// Initialize a physical window/surface at the given pixel dimensions.
void facet_window_init(int width, int height);

// Acquire the next presentable surface from the swapchain.
// Returns an opaque handle (CAMetalDrawable* on macOS, VkImage on Linux, 
// framebuffer addr on KeuOS). Returns NULL/0 if unavailable.
void* facet_window_next_drawable(void);

// Drain the OS event queue (mouse, keyboard, scroll, close).
// Must be called once per frame from the keuos main loop.
void facet_window_pump_events(void);

// Returns accumulated scroll wheel delta since last call, then resets to 0.
float facet_window_get_scroll_delta(void);

// --- GPU Pipeline ---
// Initialize the GPU device, compile shaders, create pipeline states.
// Called lazily on first use.
void facet_gpu_compositor_init(void);

// Returns the GPU device handle (used by window bridge for CAMetalLayer setup).
// Platform-specific return type cast to void* across the C boundary.
void* facet_gpu_get_device(void);

// Upload a single-channel (R8) font atlas texture to the GPU.
void facet_gpu_load_font_atlas(uint8_t* pixels, int width, int height);

// Render an array of RenderPrimitive structs to the given drawable surface.
// native_drawable: opaque handle from facet_window_next_drawable()
// rects: pointer to flat array of 48-byte RenderPrimitive structs
// width, height: viewport dimensions in pixels
// rect_count: number of primitives to render
// scroll_y: vertical scroll offset in pixels
void facet_gpu_rasterize_primitives(void* native_drawable, void* rects, 
                                     int width, int height, int rect_count,
                                     float scroll_y);

// Legacy edge rasterizer (CPU-based vector path rendering)
void facet_gpu_rasterize_edges(uint8_t* canvas, void* edges,
                                int width, int height, int edge_count,
                                uint8_t r, uint8_t g, uint8_t b, uint8_t a);

#endif // FACET_DISPLAY_CONTRACT_H
