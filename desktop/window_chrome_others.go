//go:build (!linux && !windows) || bindings

package main

// macOS always hides its title bar (main.go); elsewhere there is no frame to
// keep without one.
const integratedTitlebarSupported = false

// See window_chrome_linux.go.
const framelessTitlebar = false

var titlebarSwitchesLive = func() bool { return false }

func installWindowChrome(integrated bool) {}

func setNativeTitlebarIntegrated(integrated bool) {}

func closeNativeWindow() {}

func nativeActivateWithToken(token string) {}

func nativeChromeSettings() (layout, doubleClick string) { return "", "" }

func nativeWindowTiled() bool { return false }

func nativeDrawsFrame() bool { return false }
