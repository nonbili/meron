package main

import (
	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// setWindowAppearance tells the native window chrome which appearance the
// frontend is painting, so the parts of the window the webview does not draw —
// the title bar above all — match the theme picked in settings instead of
// staying light under a dark theme.
//
// The wails runtime's theme calls only do something on Windows (the DWM dark
// caption); Linux and macOS are handled by setNativeWindowDark, whose
// implementation is per platform.
func (a *App) setWindowAppearance(payload map[string]any) (any, error) {
	dark, _ := payload["dark"].(bool)
	if a.ctx != nil {
		if dark {
			wailsRuntime.WindowSetDarkTheme(a.ctx)
		} else {
			wailsRuntime.WindowSetLightTheme(a.ctx)
		}
	}
	setNativeWindowDark(dark)
	return map[string]any{"ok": true}, nil
}
