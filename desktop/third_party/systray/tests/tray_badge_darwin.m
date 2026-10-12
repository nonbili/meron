// Run test-tray-badge-darwin.sh on macOS. Compile the actual native tray
// implementation so this catches integration failures hidden by Go stubs.
#include "../systray_darwin.m"
#include <assert.h>

static int readyCount;
void systray_ready(void) { readyCount++; }
void systray_on_exit(void) {}
void systray_left_click(void) {}
void systray_right_click(void) {}
void systray_menu_item_selected(int menu_id) {}
void systray_menu_will_open(void) {}

int main(int argc, const char **argv) {
  @autoreleasepool {
    assert(argc == 2);
    [NSApplication sharedApplication];
    NSData *icon = [NSData dataWithContentsOfFile:@(argv[1])];
    assert(icon.length > 0);

    // A request arriving before nativeStart must survive item creation.
    setIconWithBadge(icon.bytes, (int)icon.length, true, true);
    assert(owner == nil);
    nativeStart();
    assert(readyCount == 1);
    NSStatusItem *item = [owner valueForKey:@"statusItem"];
    SystrayUnreadBadge *badge = [owner valueForKey:@"unreadBadge"];
    assert(item.button.image.isTemplate);
    assert(badge != nil && !badge.hidden);
    assert([badge hitTest:NSMakePoint(0, 0)] == nil);

    // The gap is transparent, not an opaque white ring. Read from the
    // real native image, so both template masking and Retina scaling run.
    NSBitmapImageRep *pixels = [[NSBitmapImageRep alloc] initWithData:item.button.image.TIFFRepresentation];
    NSInteger x = (NSInteger)(pixels.pixelsWide * 0.875);
    NSInteger y = (NSInteger)(pixels.pixelsHigh * 0.875);
    assert([[pixels colorAtX:x y:y] alphaComponent] < 0.1);

    setIconWithBadge(icon.bytes, (int)icon.length, true, false);
    assert(badge.hidden);
    setIconWithBadge(icon.bytes, (int)icon.length, true, true);
    assert([owner valueForKey:@"unreadBadge"] == badge && !badge.hidden);
    setIcon(icon.bytes, (int)icon.length, false);
    assert(badge.hidden && !item.button.image.isTemplate);

    // Confirm the badge paints a red pixel rather than a template/white dot.
    setIconWithBadge(icon.bytes, (int)icon.length, true, true);
    [item.button layoutSubtreeIfNeeded];
    pixels = [badge bitmapImageRepForCachingDisplayInRect:badge.bounds];
    [badge cacheDisplayInRect:badge.bounds toBitmapImageRep:pixels];
    BOOL foundRed = NO;
    for (NSInteger py = 0; py < pixels.pixelsHigh; py++) {
      for (NSInteger px = 0; px < pixels.pixelsWide; px++) {
        NSColor *colour = [[pixels colorAtX:px y:py] colorUsingColorSpace:[NSColorSpace sRGBColorSpace]];
        if (colour.alphaComponent > 0.9 && colour.redComponent > 0.9 && colour.greenComponent < 0.4) {
          foundRed = YES;
        }
      }
    }
    assert(foundRed);
    [[NSStatusBar systemStatusBar] removeStatusItem:item];
  }
  return 0;
}
