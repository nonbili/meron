package main

import (
	"regexp"
	"runtime"

	options "meron/internal/desktopruntime"
)

// opaqueHexColor is the only title bar color form the native layers accept.
var opaqueHexColor = regexp.MustCompile(`^#[0-9a-fA-F]{6}$`)

// roundedWindowCorners: on Linux the window is translucent so the frontend can
// round the bottom corners to match the ones GTK draws on the title bar
// (GTK 3 clips neither the webview nor its own frame to them).
var roundedWindowCorners = runtime.GOOS == "linux"

// setWindowAppearance tells the native window chrome which appearance the
// frontend is painting, so the parts of the window the webview does not draw —
// the title bar above all — match the theme picked in settings instead of
// staying light under a dark theme.
//
// setNativeWindowDark applies the platform's native appearance.
// setNativeTitlebarColors then paints the
// title bar in the side nav's colors where the platform allows it.
func (a *App) setWindowAppearance(payload map[string]any) (any, error) {
	dark, _ := payload["dark"].(bool)
	setNativeWindowDark(dark)
	bg, _ := payload["titlebar"].(string)
	fg, _ := payload["titlebarText"].(string)
	setNativeTitlebarColors(bg, fg)
	return map[string]any{"ok": true}, nil
}

// windowBackgroundColour is what shows through where the page is transparent:
// nothing on Linux, so the rounded corners show the desktop; the Wails default
// (white) elsewhere.
func windowBackgroundColour() *options.RGBA {
	if roundedWindowCorners {
		return &options.RGBA{R: 0, G: 0, B: 0, A: 0}
	}
	return nil
}
