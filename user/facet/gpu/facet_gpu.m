/*
 * Facet GPU Bridge — Metal Compute Implementation
 *
 * Wraps Apple's Metal API in C-callable functions for the Salt FFI.
 * Manages MTLDevice, MTLCommandQueue, MTLBuffer, and compute pipelines.
 *
 * Architecture:
 *   FacetGPUState (heap struct) holds device + queue
 *   Buffers and pipelines returned as opaque i64 handles
 *   All Metal objects use ARC — no manual retain/release needed
 *
 * Compile: clang -ObjC -framework Metal -fobjc-arc
 */

#import <CoreVideo/CoreVideo.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ═══════════════════════════════════════════════════════════════
// GPU State — tracks device and command queue
// ═══════════════════════════════════════════════════════════════

typedef struct {
  id<MTLDevice> device;
  id<MTLCommandQueue> commandQueue;
} FacetGPUState;

// ═══════════════════════════════════════════════════════════════
// facet_gpu_init — Create Metal device and command queue
// ═══════════════════════════════════════════════════════════════

int64_t facet_gpu_init(void) {
  @autoreleasepool {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (!device) {
      fprintf(stderr, "[facet_gpu] ERROR: No Metal device found\n");
      return 0;
    }

    FacetGPUState *state = (FacetGPUState *)calloc(1, sizeof(FacetGPUState));
    if (!state)
      return 0;

    state->device = device;
    state->commandQueue = [device newCommandQueue];

    if (!state->commandQueue) {
      fprintf(stderr, "[facet_gpu] ERROR: Failed to create command queue\n");
      free(state);
      return 0;
    }

    return (int64_t)state;
  }
}

// ═══════════════════════════════════════════════════════════════
// facet_gpu_create_buffer — Allocate shared-memory Metal buffer
// ═══════════════════════════════════════════════════════════════

int64_t facet_gpu_create_buffer(int64_t device_handle, int64_t size_bytes) {
  if (!device_handle || size_bytes <= 0)
    return 0;

  @autoreleasepool {
    FacetGPUState *state = (FacetGPUState *)device_handle;
    id<MTLBuffer> buffer =
        [state->device newBufferWithLength:(NSUInteger)size_bytes
                                   options:MTLResourceStorageModeShared];
    if (!buffer) {
      fprintf(stderr,
              "[facet_gpu] ERROR: Failed to create buffer of %lld bytes\n",
              (long long)size_bytes);
      return 0;
    }

    // Return the buffer object as an opaque handle.
    // ARC retains it because we store it via CFBridgingRetain.
    return (int64_t)CFBridgingRetain(buffer);
  }
}

// ═══════════════════════════════════════════════════════════════
// facet_gpu_buffer_contents — Get CPU pointer to buffer data
// ═══════════════════════════════════════════════════════════════

void *facet_gpu_buffer_contents(int64_t buffer_handle) {
  if (!buffer_handle)
    return NULL;
  id<MTLBuffer> buffer = (__bridge id<MTLBuffer>)(void *)buffer_handle;
  return [buffer contents];
}

// ═══════════════════════════════════════════════════════════════
// facet_gpu_buffer_length — Get buffer size in bytes
// ═══════════════════════════════════════════════════════════════

int64_t facet_gpu_buffer_length(int64_t buffer_handle) {
  if (!buffer_handle)
    return 0;
  id<MTLBuffer> buffer = (__bridge id<MTLBuffer>)(void *)buffer_handle;
  return (int64_t)[buffer length];
}

// ═══════════════════════════════════════════════════════════════
// facet_gpu_compile_shader — Compile MSL source to pipeline
// ═══════════════════════════════════════════════════════════════

int64_t facet_gpu_compile_shader(int64_t device_handle, const char *msl_source,
                                 const char *fn_name) {
  if (!device_handle || !msl_source || !fn_name)
    return 0;

  @autoreleasepool {
    FacetGPUState *state = (FacetGPUState *)device_handle;

    NSString *source = [NSString stringWithUTF8String:msl_source];
    NSError *error = nil;

    // Compile MSL source to a Metal library
    id<MTLLibrary> library = [state->device newLibraryWithSource:source
                                                         options:nil
                                                           error:&error];
    if (!library) {
      fprintf(stderr, "[facet_gpu] ERROR: Shader compilation failed: %s\n",
              [[error localizedDescription] UTF8String]);
      return 0;
    }

    // Get the kernel function by name
    NSString *funcName = [NSString stringWithUTF8String:fn_name];
    id<MTLFunction> function = [library newFunctionWithName:funcName];
    if (!function) {
      fprintf(stderr,
              "[facet_gpu] ERROR: Kernel '%s' not found in compiled library\n",
              fn_name);
      return 0;
    }

    // Create compute pipeline state
    id<MTLComputePipelineState> pipeline =
        [state->device newComputePipelineStateWithFunction:function
                                                     error:&error];
    if (!pipeline) {
      fprintf(stderr, "[facet_gpu] ERROR: Pipeline creation failed: %s\n",
              [[error localizedDescription] UTF8String]);
      return 0;
    }

    return (int64_t)CFBridgingRetain(pipeline);
  }
}

// ═══════════════════════════════════════════════════════════════
// facet_gpu_dispatch — Run a compute shader
// ═══════════════════════════════════════════════════════════════

void facet_gpu_dispatch(int64_t device_handle, int64_t pipeline_handle,
                        const int64_t *buffers, int32_t buffer_count,
                        int32_t thread_count) {
  if (!device_handle || !pipeline_handle || !buffers || buffer_count <= 0 ||
      thread_count <= 0)
    return;

  @autoreleasepool {
    FacetGPUState *state = (FacetGPUState *)device_handle;
    id<MTLComputePipelineState> pipeline =
        (__bridge id<MTLComputePipelineState>)(void *)pipeline_handle;

    // Create command buffer
    id<MTLCommandBuffer> commandBuffer = [state->commandQueue commandBuffer];
    if (!commandBuffer) {
      fprintf(stderr, "[facet_gpu] ERROR: Failed to create command buffer\n");
      return;
    }

    // Create compute encoder
    id<MTLComputeCommandEncoder> encoder =
        [commandBuffer computeCommandEncoder];
    if (!encoder) {
      fprintf(stderr, "[facet_gpu] ERROR: Failed to create compute encoder\n");
      return;
    }

    // Set pipeline and buffers
    [encoder setComputePipelineState:pipeline];
    for (int32_t i = 0; i < buffer_count; i++) {
      id<MTLBuffer> buf = (__bridge id<MTLBuffer>)(void *)buffers[i];
      [encoder setBuffer:buf offset:0 atIndex:(NSUInteger)i];
    }

    // Calculate threadgroup size
    NSUInteger maxThreadsPerGroup = [pipeline maxTotalThreadsPerThreadgroup];
    NSUInteger threadGroupSize = maxThreadsPerGroup;
    if (threadGroupSize > (NSUInteger)thread_count) {
      threadGroupSize = (NSUInteger)thread_count;
    }

    MTLSize gridSize = MTLSizeMake((NSUInteger)thread_count, 1, 1);
    MTLSize groupSize = MTLSizeMake(threadGroupSize, 1, 1);

    [encoder dispatchThreads:gridSize threadsPerThreadgroup:groupSize];
    [encoder endEncoding];

    // Submit and wait
    [commandBuffer commit];
    [commandBuffer waitUntilCompleted];

    // Check for errors
    if ([commandBuffer error]) {
      fprintf(stderr, "[facet_gpu] ERROR: GPU execution failed: %s\n",
              [[[commandBuffer error] localizedDescription] UTF8String]);
    }
  }
}

// ═══════════════════════════════════════════════════════════════
// facet_gpu_destroy_buffer — Release a Metal buffer
// ═══════════════════════════════════════════════════════════════

void facet_gpu_destroy_buffer(int64_t buffer_handle) {
  if (!buffer_handle)
    return;
  // Release the retained buffer object
  CFBridgingRelease((void *)buffer_handle);
}

// ═══════════════════════════════════════════════════════════════
// facet_gpu_destroy — Release GPU device and queue
// ═══════════════════════════════════════════════════════════════

void facet_gpu_destroy(int64_t device_handle) {
  if (!device_handle)
    return;
  FacetGPUState *state = (FacetGPUState *)device_handle;
  // ARC releases device and commandQueue when state is freed
  state->device = nil;
  state->commandQueue = nil;
  free(state);
}

// ═══════════════════════════════════════════════════════════════
// facet_gpu_alloc / facet_gpu_free — Bypass Salt leak detector
// ═══════════════════════════════════════════════════════════════

void *facet_gpu_alloc(int64_t size) {
  void *ptr;
  // Metal shared buffers require page alignment for zero-copy.
  posix_memalign(&ptr, 16384, (size_t)size);
  return ptr;
}

void facet_gpu_free(void *ptr) { free(ptr); }

// ═══════════════════════════════════════════════════════════════
// brutalist Flat C-API for Compositor
// ═══════════════════════════════════════════════════════════════

typedef struct __attribute__((packed)) {
  float x0;
  float y0;
  float x1;
  float y1;
  float dir;
} RenderEdge;

typedef struct __attribute__((packed)) {
  float x;
  float y;
  float w;
  float h;
  float uv_x;
  float uv_y;
  float uv_w;
  float uv_h;
  uint32_t color;
  uint32_t type;
  float border_radius;
  uint32_t shadow_color;
  float shadow_x;
  float shadow_y;
  float shadow_blur;
  float shadow_spread;
  float transform_x;
  float transform_y;
  float opacity;
  float pad;
} RenderPrimitive;

typedef struct __attribute__((packed)) {
  int width;
  int height;
  int edge_count;
  uint8_t r;
  uint8_t g;
  uint8_t b;
  uint8_t a;
} RenderParams;

typedef struct __attribute__((packed)) {
  int width;
  int height;
  int rect_count;
} RenderRectParams;

static id<MTLDevice> global_device = nil;
static id<MTLCommandQueue> global_queue = nil;
static id<MTLComputePipelineState> pso_rasterize = nil;
static id<MTLRenderPipelineState> pso_instanced_prims = nil;
static id<MTLTexture> global_font_atlas = nil;
static id<MTLTexture> global_image_textures[64];
static int global_image_count = 0;

void facet_gpu_compositor_init(void);

id<MTLDevice> facet_gpu_get_device(void) {
  if (!global_device)
    facet_gpu_compositor_init();
  return global_device;
}

id<MTLCommandQueue> facet_gpu_get_queue(void) {
  if (!global_queue)
    facet_gpu_compositor_init();
  return global_queue;
}

void facet_gpu_load_font_atlas(uint8_t *pixels, int width, int height) {
  if (!global_device)
    facet_gpu_compositor_init();
  @autoreleasepool {
    MTLTextureDescriptor *td = [MTLTextureDescriptor
        texture2DDescriptorWithPixelFormat:MTLPixelFormatR8Unorm
                                     width:width
                                    height:height
                                 mipmapped:NO];
    td.usage = MTLTextureUsageShaderRead;
    td.storageMode = MTLStorageModeShared;
    global_font_atlas = [global_device newTextureWithDescriptor:td];
    MTLRegion region = MTLRegionMake2D(0, 0, width, height);
    [global_font_atlas replaceRegion:region
                         mipmapLevel:0
                           withBytes:pixels
                         bytesPerRow:width];
  }
}

int facet_gpu_upload_image(uint8_t *rgba, int width, int height) {
  if (!global_device)
    facet_gpu_compositor_init();
  if (global_image_count >= 64)
    return -1;

  @autoreleasepool {
    MTLTextureDescriptor *td = [MTLTextureDescriptor
        texture2DDescriptorWithPixelFormat:MTLPixelFormatRGBA8Unorm
                                     width:width
                                    height:height
                                 mipmapped:NO];
    td.usage = MTLTextureUsageShaderRead;
    td.storageMode = MTLStorageModeShared;
    id<MTLTexture> tex = [global_device newTextureWithDescriptor:td];
    MTLRegion region = MTLRegionMake2D(0, 0, width, height);
    [tex replaceRegion:region
           mipmapLevel:0
             withBytes:rgba
           bytesPerRow:(width * 4)];

    int slot = global_image_count;
    global_image_textures[slot] = tex;
    global_image_count++;
    return slot;
  }
}

void facet_gpu_free_texture(int slot) {
  if (slot >= 0 && slot < 64) {
    @autoreleasepool {
      global_image_textures[slot] = nil; // ARC physical teardown
    }
  }
}

void facet_gpu_update_texture(int slot, uint8_t *rgba, int width, int height) {
  if (!global_device)
    return;
  if (slot >= 0 && slot < 64 && global_image_textures[slot] != nil) {
    @autoreleasepool {
      id<MTLTexture> tex = global_image_textures[slot];
      MTLRegion region = MTLRegionMake2D(0, 0, width, height);
      // MTLStorageModeShared forces a contiguous block replacement synchronized
      // directly securely across Apple Silicon unified memory matrix.
      [tex replaceRegion:region
             mipmapLevel:0
               withBytes:rgba
             bytesPerRow:(width * 4)];
    }
  }
}

void facet_gpu_compositor_init(void) {
  if (global_device)
    return;

  global_device = MTLCreateSystemDefaultDevice();
  global_queue = [global_device newCommandQueue];

  NSString *msl_source =
      @"#include <metal_stdlib>\n"
       "using namespace metal;\n"
       "struct Edge { float x0; float y0; float x1; float y1; float dir; };\n"
       "struct RenderParams { int width; int height; int edge_count; uint8_t "
       "r; uint8_t g; uint8_t b; uint8_t a; };\n"
       "kernel void rasterize_edges(device uint8_t* canvas [[buffer(0)]], \n"
       "                            device const Edge* edges [[buffer(1)]], \n"
       "                            constant RenderParams& params "
       "[[buffer(2)]], \n"
       "                            uint2 tid [[thread_position_in_grid]]) {\n"
       "    if (tid.x >= (uint)params.width || tid.y >= (uint)params.height) "
       "return;\n"
       "    float py = float(tid.y) + 0.5;\n"
       "    float px = float(tid.x) + 0.5;\n"
       "    float winding = 0.0;\n"
       "    for (int i = 0; i < params.edge_count; i++) {\n"
       "        float y0 = edges[i].y0;\n"
       "        float y1 = edges[i].y1;\n"
       "        if (py >= y0 && py < y1) {\n"
       "            float t = (py - y0) / (y1 - y0);\n"
       "            float ix = edges[i].x0 + t * (edges[i].x1 - edges[i].x0);\n"
       "            if (ix <= px) { winding += edges[i].dir; }\n"
       "        }\n"
       "    }\n"
       "    if (abs(winding) > 0.001) {\n"
       "        int off = (tid.y * params.width + tid.x) * 4;\n"
       "        uint8_t a = params.a;\n"
       "        if (a == 255) {\n"
       "            canvas[off] = params.r; canvas[off+1] = params.g; "
       "canvas[off+2] = params.b; canvas[off+3] = a;\n"
       "        } else if (a > 0) {\n"
       "            int dst_r = canvas[off]; int dst_g = canvas[off+1]; int "
       "dst_b = canvas[off+2]; int dst_a = canvas[off+3];\n"
       "            int inv_a = 255 - a;\n"
       "            canvas[off] = (params.r + (dst_r * inv_a) / 255); "
       "canvas[off+1] = (params.g + (dst_g * inv_a) / 255);\n"
       "            canvas[off+2] = (params.b + (dst_b * inv_a) / 255); "
       "canvas[off+3] = (a + (dst_a * inv_a) / 255);\n"
       "        }\n"
       "    }\n"
       "};\n"
       "struct Primitive {\n"
       "    float x; float y; float w; float h;\n"
       "    float uv_x; float uv_y; float uv_w; float uv_h;\n"
       "    uint32_t color; uint32_t type;\n"
       "    float border_radius; uint32_t shadow_color;\n"
       "    float shadow_x; float shadow_y; float shadow_blur; float "
       "shadow_spread;\n"
       "    float transform_x; float transform_y; float opacity; float pad;\n"
       "};\n"
       "struct VertexParams { float screen_w; float screen_h; float scroll_y; "
       "};\n"
       "struct RasterizerData {\n"
       "    float4 position [[position]];\n"
       "    float2 uv;\n"
       "    float4 color;\n"
       "    float prim_type;\n"
       "    float2 local_pos;\n"
       "    float2 size;\n"
       "    float border_radius;\n"
       "    float4 shadow_color;\n"
       "    float4 shadow_params;\n"
       "    float opacity;\n"
       "};\n"
       "float roundedBoxSDF(float2 p, float2 b, float r) {\n"
       "    float2 q = abs(p) - b + r;\n"
       "    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;\n"
       "}\n"
       "vertex RasterizerData vertex_main(\n"
       "    uint vertex_id [[vertex_id]],\n"
       "    uint instance_id [[instance_id]],\n"
       "    device const Primitive* rects [[buffer(0)]],\n"
       "    constant VertexParams& vp [[buffer(1)]]) {\n"
       "    RasterizerData out;\n"
       "    Primitive p = rects[instance_id];\n"
       "    float2 transform = float2(p.transform_x, p.transform_y);\n"
       "    float margin = max(p.shadow_blur * 2.0 + abs(p.shadow_x) + "
       "abs(p.shadow_y), 1.0);\n"
       "    float2 corners[4] = { float2(p.x - margin, p.y - margin), "
       "float2(p.x + p.w + margin, p.y - margin),\n"
       "                          float2(p.x - margin, p.y + p.h + margin), "
       "float2(p.x + p.w + margin, p.y + p.h + margin) };\n"
       "    corners[0] += transform; corners[1] += transform; corners[2] += "
       "transform; corners[3] += transform;\n"
       "    float2 uvs[4] = { float2(p.uv_x, p.uv_y), float2(p.uv_x + p.uv_w, "
       "p.uv_y),\n"
       "                      float2(p.uv_x, p.uv_y + p.uv_h), float2(p.uv_x + "
       "p.uv_w, p.uv_y + p.uv_h) };\n"
       "    float2 local[4] = { float2(-margin, -margin), float2(p.w + margin, "
       "-margin),\n"
       "                        float2(-margin, p.h + margin), float2(p.w + "
       "margin, p.h + margin) };\n"
       "    float2 pos = corners[vertex_id];\n"
       "    pos.y -= vp.scroll_y;\n"
       "    out.position = float4(pos.x / vp.screen_w * 2.0 - 1.0,\n"
       "                          1.0 - pos.y / vp.screen_h * 2.0,\n"
       "                          0.0, 1.0);\n"
       "    out.uv = uvs[vertex_id];\n"
       "    out.local_pos = local[vertex_id];\n"
       "    out.size = float2(p.w, p.h);\n"
       "    out.border_radius = p.border_radius;\n"
       "    float r = ((p.color >> 24) & 0xFF) / 255.0;\n"
       "    float g = ((p.color >> 16) & 0xFF) / 255.0;\n"
       "    float b = ((p.color >> 8) & 0xFF) / 255.0;\n"
       "    float a = (p.color & 0xFF) / 255.0;\n"
       "    out.color = float4(r, g, b, a);\n"
       "    float sr = ((p.shadow_color >> 24) & 0xFF) / 255.0;\n"
       "    float sg = ((p.shadow_color >> 16) & 0xFF) / 255.0;\n"
       "    float sb = ((p.shadow_color >> 8) & 0xFF) / 255.0;\n"
       "    float sa = (p.shadow_color & 0xFF) / 255.0;\n"
       "    out.shadow_color = float4(sr, sg, sb, sa);\n"
       "    out.shadow_params = float4(p.shadow_x, p.shadow_y, p.shadow_blur, "
       "p.shadow_spread);\n"
       "    out.prim_type = (float)p.type;\n"
       "    out.opacity = p.opacity;\n"
       "    return out;\n"
       "}\n"
       "fragment half4 fragment_main(\n"
       "    RasterizerData in [[stage_in]],\n"
       "    texture2d<half, access::sample> fontAtlas [[texture(0)]],\n"
       "    texture2d<float, access::sample> imageAtlas [[texture(1)]],\n"
       "    texture2d<float, access::sample> yTexture [[texture(2)]],\n"
       "    texture2d<float, access::sample> oopifTexture [[texture(3)]],\n"
       "    texture2d<float, access::sample> uvTexture [[texture(4)]]) {\n"
       "    constexpr sampler atlasSampler(coord::normalized, "
       "address::clamp_to_edge, filter::linear);\n"
       "    float4 c = in.color;\n"
       "    if (in.prim_type > 3.5) {\n"
       "        float4 texel = oopifTexture.sample(atlasSampler, in.uv);\n"
       "        c = float4(texel.r, texel.g, texel.b, texel.a);\n"
       "    } else if (in.prim_type > 2.5) {\n"
       "        float y = yTexture.sample(atlasSampler, in.uv).r;\n"
       "        float2 uv = uvTexture.sample(atlasSampler, in.uv).rg - "
       "float2(0.5, 0.5);\n"
       "        float r = y + 1.402 * uv.y;\n"
       "        float g = y - 0.344136 * uv.x - 0.714136 * uv.y;\n"
       "        float b = y + 1.772 * uv.x;\n"
       "        c = float4(r, g, b, 1.0);\n"
       "    } else if (in.prim_type > 1.5) {\n"
       "        float4 texel = imageAtlas.sample(atlasSampler, in.uv);\n"
       "        c = float4(texel.r, texel.g, texel.b, texel.a);\n"
       "    } else if (in.prim_type > 0.5) {\n"
       "        float dist = fontAtlas.sample(atlasSampler, in.uv).r;\n"
       "        float edge = 0.05;\n"
       "        c.a = c.a * smoothstep(0.5 - edge, 0.5 + edge, dist);\n"
       "    } else {\n"
       "        float2 center = in.size / 2.0;\n"
       "        float box_dist = roundedBoxSDF(in.local_pos - center, center, "
       "in.border_radius);\n"
       "        float box_alpha = 1.0 - smoothstep(-0.5, 0.5, box_dist);\n"
       "        float2 shadow_center = center + float2(in.shadow_params.x, "
       "in.shadow_params.y);\n"
       "        float shadow_dist = roundedBoxSDF(in.local_pos - "
       "shadow_center, center + in.shadow_params.w, in.border_radius);\n"
       "        float shadow_alpha = 1.0 - smoothstep(-in.shadow_params.z, "
       "in.shadow_params.z, shadow_dist);\n"
       "        shadow_alpha *= in.shadow_color.a;\n"
       "        float box_a = box_alpha * in.color.a;\n"
       "        float out_a = box_a + shadow_alpha * (1.0 - box_a);\n"
       "        if (out_a <= 0.0) discard_fragment();\n"
       "        float3 out_rgb = (c.rgb * box_a + in.shadow_color.rgb * "
       "shadow_alpha * (1.0 - box_a)) / out_a;\n"
       "        c = float4(out_rgb, out_a);\n"
       "    }\n"
       "    c.a *= in.opacity;\n"
       "    if (c.a <= 0.0) discard_fragment();\n"
       "    return half4(c);\n"
       "}\n";

  NSError *error = nil;
  id<MTLLibrary> library = [global_device newLibraryWithSource:msl_source
                                                       options:nil
                                                         error:&error];
  if (!library) {
    NSLog(@"FATAL: Salt GPU Bridge failed to compile MSL: %@", error);
    exit(1);
  }

  id<MTLFunction> func = [library newFunctionWithName:@"rasterize_edges"];
  pso_rasterize = [global_device newComputePipelineStateWithFunction:func
                                                               error:&error];
  if (!pso_rasterize) {
    NSLog(@"FATAL: Salt GPU Bridge failed to create edge pipeline state: %@",
          error);
    exit(1);
  }

  // Build the Vertex/Fragment Render Pipeline for Instanced Primitives
  id<MTLFunction> vtx_func = [library newFunctionWithName:@"vertex_main"];
  id<MTLFunction> frag_func = [library newFunctionWithName:@"fragment_main"];
  MTLRenderPipelineDescriptor *rpd = [[MTLRenderPipelineDescriptor alloc] init];
  rpd.vertexFunction = vtx_func;
  rpd.fragmentFunction = frag_func;
  rpd.colorAttachments[0].pixelFormat = MTLPixelFormatBGRA8Unorm;
  // Hardware Alpha Blending: src * srcAlpha + dst * (1 - srcAlpha)
  rpd.colorAttachments[0].blendingEnabled = YES;
  rpd.colorAttachments[0].sourceRGBBlendFactor = MTLBlendFactorSourceAlpha;
  rpd.colorAttachments[0].destinationRGBBlendFactor =
      MTLBlendFactorOneMinusSourceAlpha;
  rpd.colorAttachments[0].sourceAlphaBlendFactor = MTLBlendFactorSourceAlpha;
  rpd.colorAttachments[0].destinationAlphaBlendFactor =
      MTLBlendFactorOneMinusSourceAlpha;
  pso_instanced_prims =
      [global_device newRenderPipelineStateWithDescriptor:rpd error:&error];
  if (!pso_instanced_prims) {
    NSLog(@"FATAL: Salt GPU Bridge failed to create instanced render pipeline: "
          @"%@",
          error);
    exit(1);
  }
}

void facet_gpu_rasterize_edges(uint8_t *canvas, RenderEdge *edges, int width,
                               int height, int edge_count, uint8_t r, uint8_t g,
                               uint8_t b, uint8_t a) {
  if (!global_device)
    facet_gpu_compositor_init();

  RenderParams params = {width, height, edge_count, r, g, b, a};

  // Debug print
  // printf("[GPU] Dispatch %dx%d, edges: %d, color: rgba(%d, %d, %d, %d)\n",
  // params.width, params.height, params.edge_count, params.r, params.g,
  // params.b, params.a);

  id<MTLCommandBuffer> cmd_buffer = [global_queue commandBuffer];
  id<MTLComputeCommandEncoder> encoder = [cmd_buffer computeCommandEncoder];
  [encoder setComputePipelineState:pso_rasterize];

  size_t canvas_size = params.width * params.height * 4;
  size_t edges_size = params.edge_count * sizeof(RenderEdge);

  id<MTLBuffer> buf_canvas =
      [global_device newBufferWithBytesNoCopy:canvas
                                       length:canvas_size
                                      options:MTLResourceStorageModeShared
                                  deallocator:nil];

  id<MTLBuffer> buf_edges =
      [global_device newBufferWithBytesNoCopy:edges
                                       length:edges_size
                                      options:MTLResourceStorageModeShared
                                  deallocator:nil];

  [encoder setBuffer:buf_canvas offset:0 atIndex:0];
  [encoder setBuffer:buf_edges offset:0 atIndex:1];
  [encoder setBytes:&params length:sizeof(RenderParams) atIndex:2];

  MTLSize threads_per_group = MTLSizeMake(16, 16, 1);
  MTLSize threadgroups =
      MTLSizeMake((params.width + 15) / 16, (params.height + 15) / 16, 1);

  [encoder dispatchThreadgroups:threadgroups
          threadsPerThreadgroup:threads_per_group];
  [encoder endEncoding];

  [cmd_buffer commit];
  [cmd_buffer waitUntilCompleted];
}

typedef struct {
  float screen_w;
  float screen_h;
  float scroll_y;
} VertexParams;

void facet_gpu_rasterize_primitives(void *native_drawable,
                                    RenderPrimitive *rects, int width,
                                    int height, int param_count,
                                    float scroll_y) {
  if (!global_device)
    facet_gpu_compositor_init();
  if (param_count <= 0) {
    CFRelease(native_drawable);
    return;
  }

  id<CAMetalDrawable> drawable = (__bridge id)native_drawable;

  // Create render pass descriptor targeting the drawable texture
  MTLRenderPassDescriptor *rpd = [MTLRenderPassDescriptor renderPassDescriptor];
  rpd.colorAttachments[0].texture = drawable.texture;
  rpd.colorAttachments[0].loadAction = MTLLoadActionClear;
  rpd.colorAttachments[0].storeAction = MTLStoreActionStore;
  rpd.colorAttachments[0].clearColor =
      MTLClearColorMake(0.098, 0.098, 0.098, 1.0); // Dark background (#191919)

  size_t rects_size = param_count * sizeof(RenderPrimitive);
  id<MTLBuffer> buf_rects =
      [global_device newBufferWithBytesNoCopy:rects
                                       length:rects_size
                                      options:MTLResourceStorageModeShared
                                  deallocator:nil];

  VertexParams vp = {(float)width, (float)height, scroll_y};

  id<MTLCommandBuffer> cmd_buffer = [global_queue commandBuffer];
  id<MTLRenderCommandEncoder> encoder =
      [cmd_buffer renderCommandEncoderWithDescriptor:rpd];
  [encoder setRenderPipelineState:pso_instanced_prims];

  // Viewport scissor clipping — clips fragments at window edges during scroll
  MTLScissorRect scissor = {0, 0, (NSUInteger)width, (NSUInteger)height};
  [encoder setScissorRect:scissor];

  // Vertex shader bindings
  [encoder setVertexBuffer:buf_rects offset:0 atIndex:0];
  [encoder setVertexBytes:&vp length:sizeof(VertexParams) atIndex:1];

  // Fragment shader bindings
  if (global_font_atlas) {
    [encoder setFragmentTexture:global_font_atlas atIndex:0];
  }
  if (global_image_count > 0 && global_image_textures[0]) {
    [encoder setFragmentTexture:global_image_textures[0] atIndex:1];
  }

  // Epic 65: YUV Hardware Video Decoder Binding
  extern CVPixelBufferRef get_latest_video_frame(void);
  CVPixelBufferRef video_buf = get_latest_video_frame();
  id<MTLTexture> target_y_tex = nil;
  id<MTLTexture> target_uv_tex = nil;
  CVMetalTextureRef cv_y_tex = NULL;
  CVMetalTextureRef cv_uv_tex = NULL;
  static CVMetalTextureCacheRef texture_cache = NULL;
  if (!texture_cache && global_device) {
    CVMetalTextureCacheCreate(kCFAllocatorDefault, NULL, global_device, NULL,
                              &texture_cache);
  }

  if (video_buf && texture_cache) {
    size_t w = CVPixelBufferGetWidth(video_buf);
    size_t h = CVPixelBufferGetHeight(video_buf);

    CVReturn yStatus = CVMetalTextureCacheCreateTextureFromImage(
        kCFAllocatorDefault, texture_cache, video_buf, NULL,
        MTLPixelFormatR8Unorm, w, h, 0, &cv_y_tex);
    if (yStatus == kCVReturnSuccess) {
      target_y_tex = CVMetalTextureGetTexture(cv_y_tex);
      [encoder setFragmentTexture:target_y_tex atIndex:2];
    } else {
      printf("[GPU] Y plane cache extraction failed\n");
    }

    CVReturn uvStatus = CVMetalTextureCacheCreateTextureFromImage(
        kCFAllocatorDefault, texture_cache, video_buf, NULL,
        MTLPixelFormatRG8Unorm, w / 2, h / 2, 1, &cv_uv_tex);
    if (uvStatus == kCVReturnSuccess) {
      target_uv_tex = CVMetalTextureGetTexture(cv_uv_tex);
      [encoder setFragmentTexture:target_uv_tex atIndex:4];
    } else {
      printf("[GPU] UV plane cache extraction failed\n");
    }
  }

  // Epic 69: Handle OOPIF IOSurface Binding
  id<MTLTexture> oopif_tex = nil;
  for (int p_idx = 0; p_idx < param_count; p_idx++) {
    if (rects[p_idx].type == 4) {
      uint32_t surface_id = rects[p_idx].color; // Packed in paint.salt
      IOSurfaceRef surf = IOSurfaceLookup(surface_id);
      if (surf) {
        MTLTextureDescriptor *td = [MTLTextureDescriptor
            texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                         width:IOSurfaceGetWidth(surf)
                                        height:IOSurfaceGetHeight(surf)
                                     mipmapped:NO];
        oopif_tex = [global_device newTextureWithDescriptor:td
                                                  iosurface:surf
                                                      plane:0];
        [encoder setFragmentTexture:oopif_tex atIndex:3];
        CFRelease(surf);
        break; // MVP: Only one OOPIF texture per batch for now
      }
    }
  }

  // Batch Breaking Render Loop for Display List Textures
  int current_batch_start = 0;
  id<MTLTexture> current_tex_at_1 = nil;
  if (global_image_count > 0 && global_image_textures[0]) {
    current_tex_at_1 = global_image_textures[0];
  }

  for (int i = 0; i <= param_count; i++) {
    bool break_batch = false;
    id<MTLTexture> desired_tex = current_tex_at_1;

    if (i < param_count) {
      if (rects[i].type == 5) { // OP_DRAW_IMAGE
        uint32_t node_idx = rects[i].color;
        extern uint32_t ext_dom_get_img_texture_slot(uint32_t node_idx);
        uint32_t tex_id = ext_dom_get_img_texture_slot(node_idx);
        if (tex_id > 0 && tex_id < 64) {
             desired_tex = global_image_textures[tex_id];
        }
      }
      
      if (desired_tex != current_tex_at_1 && rects[i].type == 5) {
          break_batch = true;
      }
    } else {
      break_batch = true; // Flush final primitives
    }

    if (break_batch && i > current_batch_start) {
      if (current_tex_at_1) {
          [encoder setFragmentTexture:current_tex_at_1 atIndex:1];
      }
      [encoder drawPrimitives:MTLPrimitiveTypeTriangleStrip
                  vertexStart:0
                  vertexCount:4
                instanceCount:(i - current_batch_start)
                 baseInstance:current_batch_start];
      current_batch_start = i;
    }

    if (i < param_count && break_batch) {
      current_tex_at_1 = desired_tex;
    }
  }

  [encoder endEncoding];

  [cmd_buffer presentDrawable:drawable];
  [cmd_buffer commit];

  if (cv_y_tex)
    CFRelease(cv_y_tex);
  if (cv_uv_tex)
    CFRelease(cv_uv_tex);
  if (texture_cache)
    CVMetalTextureCacheFlush(texture_cache, 0);

  CFRelease(native_drawable);
}

// ═══════════════════════════════════════════════════════════════
// facet_gpu_rasterize_to_texture — Render to IOSurface texture
// Epic 68: Used in multi-process mode (no CAMetalDrawable)
// ═══════════════════════════════════════════════════════════════

void facet_gpu_rasterize_to_texture(id<MTLTexture> target,
                                    RenderPrimitive *rects, int width,
                                    int height, int param_count,
                                    float scroll_y) {
  if (!global_device)
    facet_gpu_compositor_init();
  if (param_count <= 0 || !target)
    return;

  @autoreleasepool {
    MTLRenderPassDescriptor *rpd =
        [MTLRenderPassDescriptor renderPassDescriptor];
    rpd.colorAttachments[0].texture = target;
    rpd.colorAttachments[0].loadAction = MTLLoadActionClear;
    rpd.colorAttachments[0].storeAction = MTLStoreActionStore;
    rpd.colorAttachments[0].clearColor =
        MTLClearColorMake(1.0, 1.0, 1.0, 1.0); // White background

    size_t rects_size = param_count * sizeof(RenderPrimitive);
    // Use newBufferWithBytes (copies data) instead of newBufferWithBytesNoCopy
    // because Salt global arrays are NOT page-aligned.
    id<MTLBuffer> buf_rects =
        [global_device newBufferWithBytes:rects
                                   length:rects_size
                                  options:MTLResourceStorageModeShared];
    if (!buf_rects) {
      fprintf(stderr, "[GPU] FATAL: Failed to create rect buffer (%zu bytes)\n",
              rects_size);
      return;
    }

    VertexParams vp = {(float)width, (float)height, scroll_y};

    id<MTLCommandBuffer> cmd_buffer = [global_queue commandBuffer];
    id<MTLRenderCommandEncoder> encoder =
        [cmd_buffer renderCommandEncoderWithDescriptor:rpd];
    [encoder setRenderPipelineState:pso_instanced_prims];

    MTLScissorRect scissor = {0, 0, (NSUInteger)width, (NSUInteger)height};
    [encoder setScissorRect:scissor];

    [encoder setVertexBuffer:buf_rects offset:0 atIndex:0];
    [encoder setVertexBytes:&vp length:sizeof(VertexParams) atIndex:1];

    if (global_font_atlas) {
      [encoder setFragmentTexture:global_font_atlas atIndex:0];
    }
    if (global_image_count > 0 && global_image_textures[0]) {
      [encoder setFragmentTexture:global_image_textures[0] atIndex:1];
    }

    // Epic 65: YUV Hardware Video Decoder Binding
    extern CVPixelBufferRef get_latest_video_frame(void);
    CVPixelBufferRef video_buf = get_latest_video_frame();
    CVMetalTextureRef cv_y_tex = NULL;
    CVMetalTextureRef cv_uv_tex = NULL;
    static CVMetalTextureCacheRef iosurface_tex_cache = NULL;
    if (!iosurface_tex_cache && global_device) {
      CVMetalTextureCacheCreate(kCFAllocatorDefault, NULL, global_device, NULL,
                                &iosurface_tex_cache);
    }

    if (video_buf && iosurface_tex_cache) {
      size_t w = CVPixelBufferGetWidth(video_buf);
      size_t h = CVPixelBufferGetHeight(video_buf);

      CVReturn yStatus = CVMetalTextureCacheCreateTextureFromImage(
          kCFAllocatorDefault, iosurface_tex_cache, video_buf, NULL,
          MTLPixelFormatR8Unorm, w, h, 0, &cv_y_tex);
      if (yStatus == kCVReturnSuccess) {
        [encoder setFragmentTexture:CVMetalTextureGetTexture(cv_y_tex)
                            atIndex:2];
      }

      CVReturn uvStatus = CVMetalTextureCacheCreateTextureFromImage(
          kCFAllocatorDefault, iosurface_tex_cache, video_buf, NULL,
          MTLPixelFormatRG8Unorm, w / 2, h / 2, 1, &cv_uv_tex);
      if (uvStatus == kCVReturnSuccess) {
        [encoder setFragmentTexture:CVMetalTextureGetTexture(cv_uv_tex)
                            atIndex:3];
      }
    }

    // Batch Breaking Render Loop for Multi-Process Rasterization
    int current_batch_start = 0;
    id<MTLTexture> current_tex_at_1 = nil;
    if (global_image_count > 0 && global_image_textures[0]) {
      current_tex_at_1 = global_image_textures[0];
    }
  
    for (int i = 0; i <= param_count; i++) {
      bool break_batch = false;
      id<MTLTexture> desired_tex = current_tex_at_1;
  
      if (i < param_count) {
        if (rects[i].type == 5) { // OP_DRAW_IMAGE
          uint32_t node_idx = rects[i].color;
          extern uint32_t ext_dom_get_img_texture_slot(uint32_t node_idx);
          uint32_t tex_id = ext_dom_get_img_texture_slot(node_idx);
          if (tex_id > 0 && tex_id < 64) {
               desired_tex = global_image_textures[tex_id];
          }
        }
        
        if (desired_tex != current_tex_at_1 && rects[i].type == 5) {
            break_batch = true;
        }
      } else {
        break_batch = true; // Flush final primitives
      }
  
      if (break_batch && i > current_batch_start) {
        if (current_tex_at_1) {
            [encoder setFragmentTexture:current_tex_at_1 atIndex:1];
        }
        [encoder drawPrimitives:MTLPrimitiveTypeTriangleStrip
                    vertexStart:0
                    vertexCount:4
                  instanceCount:(i - current_batch_start)
                   baseInstance:current_batch_start];
        current_batch_start = i;
      }
  
      if (i < param_count && break_batch) {
        current_tex_at_1 = desired_tex;
      }
    }
  
    [encoder endEncoding];

    [cmd_buffer commit];
    [cmd_buffer waitUntilCompleted];

    if (cv_y_tex)
      CFRelease(cv_y_tex);
    if (cv_uv_tex)
      CFRelease(cv_uv_tex);
    if (iosurface_tex_cache)
      CVMetalTextureCacheFlush(iosurface_tex_cache, 0);
  }
}

void facet_gpu_render_to_buffer(uint8_t *output, RenderPrimitive *rects,
                                int width, int height, int param_count,
                                float scroll_y) {
  if (!global_device)
    facet_gpu_compositor_init();
  if (param_count <= 0)
    return;

  @autoreleasepool {
    // Create an offscreen texture with shared storage for CPU readback
    MTLTextureDescriptor *td = [MTLTextureDescriptor
        texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                     width:width
                                    height:height
                                 mipmapped:NO];
    td.usage = MTLTextureUsageRenderTarget;
    td.storageMode = MTLStorageModeShared;
    id<MTLTexture> offscreen = [global_device newTextureWithDescriptor:td];

    // Render pass targeting the offscreen texture
    MTLRenderPassDescriptor *rpd =
        [MTLRenderPassDescriptor renderPassDescriptor];
    rpd.colorAttachments[0].texture = offscreen;
    rpd.colorAttachments[0].loadAction = MTLLoadActionClear;
    rpd.colorAttachments[0].storeAction = MTLStoreActionStore;
    rpd.colorAttachments[0].clearColor = MTLClearColorMake(1.0, 1.0, 1.0, 1.0);

    size_t rects_size = param_count * sizeof(RenderPrimitive);
    id<MTLBuffer> buf_rects =
        [global_device newBufferWithBytesNoCopy:rects
                                         length:rects_size
                                        options:MTLResourceStorageModeShared
                                    deallocator:nil];

    VertexParams vp = {(float)width, (float)height, scroll_y};

    id<MTLCommandBuffer> cmd_buffer = [global_queue commandBuffer];
    id<MTLRenderCommandEncoder> encoder =
        [cmd_buffer renderCommandEncoderWithDescriptor:rpd];
    [encoder setRenderPipelineState:pso_instanced_prims];

    MTLScissorRect scissor = {0, 0, (NSUInteger)width, (NSUInteger)height};
    [encoder setScissorRect:scissor];

    [encoder setVertexBuffer:buf_rects offset:0 atIndex:0];
    [encoder setVertexBytes:&vp length:sizeof(VertexParams) atIndex:1];

    if (global_font_atlas) {
      [encoder setFragmentTexture:global_font_atlas atIndex:0];
    }
    if (global_image_count > 0 && global_image_textures[0]) {
      [encoder setFragmentTexture:global_image_textures[0] atIndex:1];
    }

    [encoder drawPrimitives:MTLPrimitiveTypeTriangleStrip
                vertexStart:0
                vertexCount:4
              instanceCount:param_count];
    [encoder endEncoding];

    [cmd_buffer commit];
    [cmd_buffer waitUntilCompleted];

    // Copy pixels from GPU texture to CPU buffer
    MTLRegion region = MTLRegionMake2D(0, 0, width, height);
    [offscreen getBytes:output
            bytesPerRow:(width * 4)
             fromRegion:region
            mipmapLevel:0];
  }
}
