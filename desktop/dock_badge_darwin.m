#import <AppKit/AppKit.h>

// Sets the badge on the app's Dock icon; an empty label removes it. The string
// is copied before returning (Go frees it); AppKit is touched on the main thread.
void setDockBadgeLabel(const char *label) {
    @autoreleasepool {
        NSString *text = label && label[0] ? [NSString stringWithUTF8String:label] : nil;
        dispatch_async(dispatch_get_main_queue(), ^{
            [[NSApp dockTile] setBadgeLabel:text];
        });
    }
}
