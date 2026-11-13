#import <AppKit/AppKit.h>

// Replaces the running app's icon, or restores the bundle icon when there is no
// data. The bytes are copied before returning (Go owns them); AppKit is touched
// on the main thread.
void setDockIconPNG(const void *data, int length) {
    @autoreleasepool {
        NSData *png = data && length > 0 ? [NSData dataWithBytes:data length:(NSUInteger)length] : nil;
        dispatch_async(dispatch_get_main_queue(), ^{
            NSImage *image = png ? [[NSImage alloc] initWithData:png] : nil;
            [NSApp setApplicationIconImage:image];
        });
    }
}
