#import <Cocoa/Cocoa.h>
#import <QuartzCore/CAMetalLayer.h>

static NSWindow* global_window = nil;
static CAMetalLayer* global_metal_layer = nil;

extern id<MTLDevice> facet_gpu_get_device(void); // Defined securely in facet_gpu.m 
extern void ext_salt_update_viewport(float w, float h); // Salt FFI Boundary

@interface KeuOSWindowDelegate : NSObject <NSWindowDelegate>
@end

@implementation KeuOSWindowDelegate
- (BOOL)windowShouldClose:(id)sender {
    [NSApp terminate:nil];
    return YES;
}

- (void)windowDidResize:(NSNotification *)notification {
    NSWindow *window = notification.object;
    CGSize size = window.contentView.frame.size;
    ext_salt_update_viewport((float)size.width, (float)size.height);
}
@end

void facet_window_init(int width, int height) {
    if (global_window) return;
    
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        
        NSRect frame = NSMakeRect(0, 0, width, height);
        NSUInteger style = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable;
        
        global_window = [[NSWindow alloc] initWithContentRect:frame
                                                    styleMask:style
                                                      backing:NSBackingStoreBuffered
                                                        defer:NO];
        [global_window setTitle:@"KeuOS SPA Environment"];
        [global_window center];
        
        KeuOSWindowDelegate* delegate = [[KeuOSWindowDelegate alloc] init];
        [global_window setDelegate:delegate];
        
        id<MTLDevice> device = facet_gpu_get_device();
        
        NSView* contentView = [global_window contentView];
        global_metal_layer = [CAMetalLayer layer];
        global_metal_layer.device = device;
        global_metal_layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
        global_metal_layer.framebufferOnly = YES;
        global_metal_layer.frame = contentView.bounds;
        global_metal_layer.contentsScale = [[NSScreen mainScreen] backingScaleFactor];
        
        [contentView setLayer:global_metal_layer];
        [contentView setWantsLayer:YES];
        
        // Initial viewport sync
        ext_salt_update_viewport((float)width, (float)height);
        
        [global_window makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
        [NSApp finishLaunching];
    }
}

void* facet_window_next_drawable(void) {
    if (!global_metal_layer) return NULL;
    @autoreleasepool {
        id<CAMetalDrawable> drawable = [global_metal_layer nextDrawable];
        if (drawable) {
            CFRetain((__bridge CFTypeRef)drawable);
        }
        return (__bridge void*)drawable;
    }
}

static float scroll_delta_y = 0.0f;
static uint8_t keyboard_buffer[256];
static uint32_t keyboard_buffer_len = 0;

void facet_window_pump_events(void) {
    @autoreleasepool {
        NSEvent *event;
        while ((event = [NSApp nextEventMatchingMask:NSEventMaskAny 
                                           untilDate:[NSDate distantPast] 
                                              inMode:NSDefaultRunLoopMode 
                                             dequeue:YES])) {
            if ([event type] == NSEventTypeScrollWheel) {
                scroll_delta_y += (float)[event deltaY];
            } else if ([event type] == NSEventTypeKeyDown) {
                unsigned short keyCode = [event keyCode];
                if (keyCode == 123 && keyboard_buffer_len < 256) {
                    keyboard_buffer[keyboard_buffer_len++] = 28;
                } else if (keyCode == 124 && keyboard_buffer_len < 256) {
                    keyboard_buffer[keyboard_buffer_len++] = 29;
                } else {
                    NSString* chars = [event characters];
                    if (chars && [chars length] > 0) {
                        const char* utf8 = [chars UTF8String];
                        if (utf8) {
                            for (int i = 0; utf8[i] != '\0' && keyboard_buffer_len < 256; i++) {
                                keyboard_buffer[keyboard_buffer_len] = (uint8_t)utf8[i];
                                keyboard_buffer_len++;
                            }
                        }
                    }
                }
            }
            [NSApp sendEvent:event];
        }
    }
}

uint32_t facet_window_drain_keyboard(uint8_t* target) {
    uint32_t count = keyboard_buffer_len;
    for (uint32_t i = 0; i < count; i++) {
        target[i] = keyboard_buffer[i];
    }
    keyboard_buffer_len = 0;
    return count;
}

float facet_window_get_scroll_delta(void) {
    float d = scroll_delta_y;
    scroll_delta_y = 0.0f;
    return d;
}
