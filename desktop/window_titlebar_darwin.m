#import <AppKit/AppKit.h>

// Keep in sync with TitleBar.tsx's h-10. AppKit uses logical points, matching
// the webview's CSS pixels even on Retina displays.
static const CGFloat MeronTitlebarHeight = 40;

@interface MeronTitlebarObserver : NSObject {
    NSWindow *window;
    BOOL pending;
}
- (instancetype)initWithWindow:(NSWindow *)target;
- (void)scheduleAlignment:(NSNotification *)note;
- (void)align;
- (void)windowClosed:(NSNotification *)note;
@end

@implementation MeronTitlebarObserver
- (instancetype)initWithWindow:(NSWindow *)target {
    self = [super init];
    if (self) {
        window = target;
        NSNotificationCenter *center = [NSNotificationCenter defaultCenter];
        for (NSString *name in @[NSWindowDidResizeNotification,
                                 NSWindowDidBecomeKeyNotification,
                                 NSWindowDidDeminiaturizeNotification,
                                 NSWindowDidExitFullScreenNotification]) {
            [center addObserver:self selector:@selector(scheduleAlignment:)
                           name:name object:target];
        }
        [center addObserver:self selector:@selector(windowClosed:)
                       name:NSWindowWillCloseNotification object:target];
    }
    return self;
}

- (void)scheduleAlignment:(NSNotification *)note {
    if (pending) return;
    pending = YES;
    // AppKit may lay out the native title bar during the notification. Apply
    // our geometry after that layout has completed.
    dispatch_async(dispatch_get_main_queue(), ^{
        pending = NO;
        [self align];
    });
}

- (void)align {
    // Fullscreen uses AppKit's own title bar; restore our layout on exit.
    if (!window || (window.styleMask & NSWindowStyleMaskFullScreen)) return;
    NSButton *close = [window standardWindowButton:NSWindowCloseButton];
    NSView *container = close.superview.superview;
    if (!container || !container.superview) return;

    // Grow the native container as well as moving the buttons so their lower
    // halves remain inside its hit-test area. Preserve native horizontal spacing.
    NSRect frame = container.frame;
    frame.size.height = MeronTitlebarHeight;
    frame.origin.y = NSMaxY(container.superview.bounds) - MeronTitlebarHeight;
    if (!NSEqualRects(container.frame, frame)) [container setFrame:frame];
    for (NSNumber *kind in @[@(NSWindowCloseButton),
                            @(NSWindowMiniaturizeButton),
                            @(NSWindowZoomButton)]) {
        NSButton *button = [window standardWindowButton:(NSWindowButton)kind.integerValue];
        if (!button) continue;
        NSRect buttonFrame = button.frame;
        buttonFrame.origin.y = (MeronTitlebarHeight - NSHeight(buttonFrame)) / 2;
        if (!NSEqualRects(button.frame, buttonFrame)) [button setFrame:buttonFrame];
    }
}

- (void)windowClosed:(NSNotification *)note {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    window = nil;
}
@end

static MeronTitlebarObserver *meronTitlebarObserver = nil;

void alignTrafficLights(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (meronTitlebarObserver) {
            [meronTitlebarObserver scheduleAlignment:nil];
            return;
        }
        NSWindow *target = NSApp.mainWindow ?: NSApp.keyWindow;
        if (!target) {
            for (NSWindow *candidate in NSApp.windows) {
                if (candidate.styleMask & NSWindowStyleMaskFullSizeContentView) {
                    target = candidate;
                    break;
                }
            }
        }
        if (!target) return;
        meronTitlebarObserver = [[MeronTitlebarObserver alloc] initWithWindow:target];
        [meronTitlebarObserver align];
    });
}
