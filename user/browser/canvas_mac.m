#import <Foundation/Foundation.h>
#import <IOSurface/IOSurface.h>
#import <CoreGraphics/CoreGraphics.h>
#import <CoreVideo/CoreVideo.h>

// Max 256 active canvases to respect zero-allocation bounds
CGContextRef active_contexts[256];
uint32_t active_surface_ids[256];

extern void sys_invalidate_paint(void);

uint32_t sys_canvas_create_backing_store(uint32_t node_id, uint32_t width, uint32_t height) {
    NSDictionary *props = @{
        (id)kIOSurfaceWidth: @(width),
        (id)kIOSurfaceHeight: @(height),
        (id)kIOSurfaceBytesPerElement: @(4),
        (id)kIOSurfacePixelFormat: @(kCVPixelFormatType_32BGRA)
    };
    
    IOSurfaceRef surface = IOSurfaceCreate((CFDictionaryRef)props);
    IOSurfaceLock(surface, 0, NULL);
    
    CGColorSpaceRef colorSpace = CGColorSpaceCreateDeviceRGB();
    CGContextRef ctx = CGBitmapContextCreate(
        IOSurfaceGetBaseAddress(surface),
        width, height, 8,
        IOSurfaceGetBytesPerRow(surface),
        colorSpace,
        kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Host
    );
    
    IOSurfaceUnlock(surface, 0, NULL);
    CGColorSpaceRelease(colorSpace);
    
    uint32_t surface_id = IOSurfaceGetID(surface);
    
    // Map node_id (modulo 256) to context array for fast O(1) lookup
    uint32_t slot = node_id % 256;
    
    // Release previous context if it exists in this slot
    if (active_contexts[slot]) {
        CGContextRelease(active_contexts[slot]);
    }
    
    active_contexts[slot] = ctx;
    active_surface_ids[slot] = surface_id;
    
    return surface_id;
}

void sys_canvas_set_fill_color(uint32_t node_id, float r, float g, float b, float a) {
    CGContextRef ctx = active_contexts[node_id % 256];
    if (ctx) CGContextSetRGBFillColor(ctx, r, g, b, a);
}

void sys_canvas_fill_rect(uint32_t node_id, float x, float y, float w, float h) {
    CGContextRef ctx = active_contexts[node_id % 256];
    if (ctx) {
        // CoreGraphics has an inverted Y-axis compared to the W3C Canvas spec.
        // We must apply an affine transform to flip the coordinate space.
        CGContextSaveGState(ctx);
        CGContextTranslateCTM(ctx, 0, CGBitmapContextGetHeight(ctx));
        CGContextScaleCTM(ctx, 1.0, -1.0);
        
        CGContextFillRect(ctx, CGRectMake(x, y, w, h));
        CGContextRestoreGState(ctx);
        
        // Notify the Salt W3C run-loop that this texture requires repainting
        sys_invalidate_paint();
    }
}
