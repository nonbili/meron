#import <AppKit/AppKit.h>
#import <objc/runtime.h>

// fyne.io/systray owns the status item and exports its delegate as `owner`.
// Its pinned version stores the item in the `statusItem` ivar. Check that
// ivar before using it so a future library change cannot crash the app.
extern id owner;

@interface MeronTrayUnreadBadge : NSView
@end

@implementation MeronTrayUnreadBadge
- (NSView *)hitTest:(NSPoint)point {
    // Keep both left and right clicks on the status button.
    return nil;
}

- (void)drawRect:(NSRect)dirtyRect {
    NSButton *button = (NSButton *)self.superview;
    NSRect imageRect = [button.cell imageRectForBounds:button.bounds];
    // Match the Linux symbolic badge: centre (14, 14), radius 2.125 in
    // a 16px icon. AppKit's image rectangle has its origin at the bottom.
    NSPoint centre = NSMakePoint(NSMinX(imageRect) + NSWidth(imageRect) * 0.875,
                                NSMinY(imageRect) + NSHeight(imageRect) * 0.125);
    CGFloat outerRadius = NSWidth(imageRect) * (2.625 / 16);
    CGFloat radius = NSWidth(imageRect) * (2.125 / 16);
    [[NSColor whiteColor] setFill];
    [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(centre.x - outerRadius,
        centre.y - outerRadius, outerRadius * 2, outerRadius * 2)] fill];
    [[NSColor colorWithSRGBRed:1 green:59.0/255 blue:48.0/255 alpha:1] setFill];
    [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(centre.x - radius,
        centre.y - radius, radius * 2, radius * 2)] fill];
}
@end

static MeronTrayUnreadBadge *meronTrayUnreadBadge;

void setTrayUnreadBadge(int unread) {
    dispatch_async(dispatch_get_main_queue(), ^{
        Ivar itemIvar = class_getInstanceVariable([owner class], "statusItem");
        if (owner == nil || itemIvar == NULL) {
            NSLog(@"Meron: systray status item unavailable for unread badge");
            return;
        }
        NSStatusItem *item = object_getIvar(owner, itemIvar);
        NSStatusBarButton *button = item.button;
        if (button == nil) {
            return;
        }
        if (meronTrayUnreadBadge == nil && unread) {
            meronTrayUnreadBadge = [[MeronTrayUnreadBadge alloc] initWithFrame:button.bounds];
            meronTrayUnreadBadge.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
            [button addSubview:meronTrayUnreadBadge];
        }
        meronTrayUnreadBadge.hidden = !unread;
        [meronTrayUnreadBadge setNeedsDisplay:YES];
    });
}
