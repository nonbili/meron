//go:build linux && !bindings

package main

// A file of its own: a cgo file with //export may only declare in its preamble,
// and window_chrome_linux.go defines its C there.

import "C"

import wailsRuntime "meron/internal/desktopruntime"

//export goWindowStateChanged
func goWindowStateChanged() {
	if globalApp == nil || globalApp.ctx == nil {
		return
	}
	// Called on the GTK thread; the event emit must not block it.
	go wailsRuntime.EventsEmit(globalApp.ctx, "window.stateChanged")
}

//export goWindowTitlebarUnavailable
func goWindowTitlebarUnavailable() {
	// GTK kept the native frame after a missed startup hook. Report the actual
	// mode to React while preserving the saved custom-titlebar preference.
	integratedTitlebar.Store(false)
}
