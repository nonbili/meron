#import <AppKit/AppKit.h>

@interface MeronAttachmentDragSource : NSObject <NSDraggingSource>
@end

@implementation MeronAttachmentDragSource
- (NSDragOperation)draggingSession:(NSDraggingSession *)session
 sourceOperationMaskForDraggingContext:(NSDraggingContext)context {
    return NSDragOperationCopy;
}
@end

int startAttachmentDrag(const char *path) {
    @autoreleasepool {
        NSWindow *window = NSApp.keyWindow;
        if (window.contentView == nil || (NSEvent.pressedMouseButtons & 1) == 0) return 0;

        NSString *filename = [NSString stringWithUTF8String:path];
        if (filename == nil) return 0;
        NSURL *url = [NSURL fileURLWithPath:filename];
        NSDraggingItem *item = [[NSDraggingItem alloc] initWithPasteboardWriter:url];
        NSPoint location = window.mouseLocationOutsideOfEventStream;
        NSPoint origin = [window.contentView convertPoint:location fromView:nil];
        NSImage *icon = [NSWorkspace.sharedWorkspace iconForFile:filename];
        [item setDraggingFrame:NSMakeRect(origin.x, origin.y - 32, 32, 32) contents:icon];

        NSEvent *event = [NSEvent mouseEventWithType:NSEventTypeLeftMouseDragged
            location:location modifierFlags:0 timestamp:NSProcessInfo.processInfo.systemUptime
            windowNumber:window.windowNumber context:nil eventNumber:0 clickCount:1 pressure:1];
        static MeronAttachmentDragSource *source;
        if (source == nil) source = [[MeronAttachmentDragSource alloc] init];
        NSDraggingSession *session = [window.contentView beginDraggingSessionWithItems:@[item] event:event source:source];
        [item release];
        return session != nil;
    }
}
