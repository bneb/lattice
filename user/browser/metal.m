/*
 * metal.m — IOSurface Renderer Bridge (Epic 68)
 *
 * Creates an IOSurface backed by a Metal texture that the renderer
 * draws into. The IOSurfaceID is pushed over the IPC ring so the
 * Main Process can composite it into a CoreAnimation layer.
 *
 * Architecture:
 *   1. sys_gpu_init_iosurface() — called at renderer boot
 *   2. facet_gpu.m reads the global render target via
 * sys_gpu_get_iosurface_texture()
 *   3. sys_gpu_commit_iosurface() — called after each frame
 */

#import <CoreVideo/CoreVideo.h>
#import <IOSurface/IOSurface.h>
#import <Metal/Metal.h>
#include <stdio.h>
#include <stdlib.h>

// ═════════════════════════════════════════════════════════════
// IOSurface State
// ═════════════════════════════════════════════════════════════

static IOSurfaceRef sharedSurface = NULL;
static id<MTLTexture> renderTargetTexture = nil;
static uint32_t currentSurfaceID = 0;
static int iosurface_mode_active = 0;

extern void sys_ipc_send_r2m_command(uint32_t cmd_type, uint64_t arg1);
extern id<MTLDevice> facet_gpu_get_device(void);
extern id<MTLCommandQueue> facet_gpu_get_queue(void);
extern void facet_gpu_compositor_init(void);

#define CMD_NEW_FRAME 1

// ═════════════════════════════════════════════════════════════
// sys_gpu_init_iosurface — Create shared surface for IPC compositing
// ═════════════════════════════════════════════════════════════

void sys_gpu_init_iosurface(int width, int height) {
  // Ensure the GPU compositor is ready (shares device/queue with facet_gpu.m)
  facet_gpu_compositor_init();
  id<MTLDevice> device = facet_gpu_get_device();

  if (sharedSurface) {
    CFRelease(sharedSurface);
    sharedSurface = NULL;
    renderTargetTexture = nil;
  }

  NSDictionary *properties = @{
    (id)kIOSurfaceWidth : @(width),
    (id)kIOSurfaceHeight : @(height),
    (id)kIOSurfaceBytesPerElement : @(4),
    (id)kIOSurfacePixelFormat : @(kCVPixelFormatType_32BGRA),
    (id)kIOSurfaceIsGlobal : @(YES)
  };

  sharedSurface = IOSurfaceCreate((CFDictionaryRef)properties);
  if (!sharedSurface) {
    fprintf(stderr, "[IOSurface] FATAL: Failed to create IOSurface %dx%d\n",
            width, height);
    return;
  }

  currentSurfaceID = IOSurfaceGetID(sharedSurface);

  // Bind the IOSurface to a Metal Texture for the render pass
  MTLTextureDescriptor *desc = [MTLTextureDescriptor
      texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                   width:width
                                  height:height
                               mipmapped:NO];
  desc.usage = MTLTextureUsageRenderTarget | MTLTextureUsageShaderRead;

  renderTargetTexture = [device newTextureWithDescriptor:desc
                                               iosurface:sharedSurface
                                                   plane:0];

  if (!renderTargetTexture) {
    fprintf(stderr,
            "[IOSurface] FATAL: Failed to bind Metal texture to IOSurface\n");
    return;
  }

  iosurface_mode_active = 1;

  fprintf(stderr, "[IOSurface] Created %dx%d surface (ID=%u)\n", width, height,
          currentSurfaceID);

  // Announce surface to Main Process
  sys_ipc_send_r2m_command(CMD_NEW_FRAME, currentSurfaceID);
}

// ═════════════════════════════════════════════════════════════
// Query API for facet_gpu.m
// ═════════════════════════════════════════════════════════════

int sys_gpu_is_iosurface_mode(void) { return iosurface_mode_active; }

id<MTLTexture> sys_gpu_get_iosurface_texture(void) {
  return renderTargetTexture;
}

// ═════════════════════════════════════════════════════════════
// sys_gpu_commit_iosurface — Signal Main Process after frame
// ═════════════════════════════════════════════════════════════

void sys_gpu_commit_iosurface(void) {
  if (!iosurface_mode_active)
    return;
  sys_ipc_send_r2m_command(CMD_NEW_FRAME, currentSurfaceID);
}

// ═════════════════════════════════════════════════════════════
// sys_gpu_rasterize_iosurface — Render to IOSurface texture
// Called from Salt's compositor.flush_frame()
// ═════════════════════════════════════════════════════════════

extern void facet_gpu_rasterize_to_texture(id<MTLTexture> target, void *rects,
                                           int width, int height,
                                           int param_count, float scroll_y);

void sys_gpu_rasterize_iosurface(void *rects, int width, int height,
                                 int rect_count, float scroll_y) {
  if (!iosurface_mode_active || !renderTargetTexture)
    return;
  facet_gpu_rasterize_to_texture(renderTargetTexture, rects, width, height,
                                 rect_count, scroll_y);
}

// ═════════════════════════════════════════════════════════════
// Epic 88: Hardware VSync Loop (Compositor Thread)
// ═════════════════════════════════════════════════════════════

extern void ext_anim_drain_queue(double current_time_ms);
extern void ext_compositor_tick(double current_time_ms);
extern void *compositor_get_rect_buf_ptr(void);
extern int compositor_get_rect_count(void);
extern float compositor_get_scroll_y(void);

static CVDisplayLinkRef displayLink;

static CVReturn sys_display_link_callback(CVDisplayLinkRef displayLink,
                                          const CVTimeStamp *inNow,
                                          const CVTimeStamp *inOutputTime,
                                          CVOptionFlags flagsIn,
                                          CVOptionFlags *flagsOut,
                                          void *displayLinkContext) {
  // Current time in milliseconds for the animation engine
  double current_time_ms =
      ((double)inOutputTime->videoTime / (double)inOutputTime->videoTimeScale) *
      1000.0;

  // 1. Hand off pending animations from JS thread (lock-free)
  ext_anim_drain_queue(current_time_ms);

  // 2. Compute interpolation matrix (Newton-Raphson Bezier)
  ext_compositor_tick(current_time_ms);

  // 3. Independent GPU Pack & Rasterize (only if there's content to draw)
  void *rects = compositor_get_rect_buf_ptr();
  int count = compositor_get_rect_count();
  float scroll = compositor_get_scroll_y();

  if (count > 0) {
    // We assume 1920x1080 for multi-process IOSurface buffers
    sys_gpu_rasterize_iosurface(rects, 1920, 1080, count, scroll);

    // 4. Signal readiness to host process
    sys_gpu_commit_iosurface();
  }

  return kCVReturnSuccess;
}

void sys_init_vsync(void) {
  if (displayLink)
    return;

  CVDisplayLinkCreateWithActiveCGDisplays(&displayLink);
  CVDisplayLinkSetOutputCallback(displayLink, &sys_display_link_callback, NULL);
  CVDisplayLinkStart(displayLink);
  printf("[Compositor] Hardware VSync Matrix Active (60Hz DisplayLink)\n");
}
