#import <AppKit/AppKit.h>

// Pins the process appearance to the theme the frontend paints. Set on the main
// thread: NSApp's appearance is UI state, and AppKit repaints every window from
// it as soon as it changes.
void setAppAppearanceDark(int dark) {
    dispatch_async(dispatch_get_main_queue(), ^{
        NSAppearanceName name = dark ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua;
        [NSApp setAppearance:[NSAppearance appearanceNamed:name]];
    });
}
