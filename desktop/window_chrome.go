package main

import (
	"runtime"
	"sync/atomic"

	wailsRuntime "meron/internal/desktopruntime"
)

// titlebarSystem is windowState.Titlebar for someone who turned the
// integrated title bar off. Anything else is the default: integrated (no GTK
// title bar; the page's own takes its place and draws the window controls)
// on supported platforms.
const titlebarSystem = "system"

// integratedTitlebar is whether the integrated title bar is in effect now.
var integratedTitlebar atomic.Bool

// integratedTitlebarActive is whether the integrated title bar is actually
// drawn. Frame ownership affects rounded corners, not title bar support.
func integratedTitlebarActive() bool {
	return integratedTitlebarSupported && integratedTitlebar.Load()
}

// Window controls go through these vars so the commands can be tested.
var (
	windowMinimise       = wailsRuntime.WindowMinimise
	windowToggleMaximise = wailsRuntime.WindowToggleMaximise
	windowClose          = closeNativeWindow
	windowIsTiled        = nativeWindowTiled
	windowDrawsFrame     = nativeDrawsFrame
	applyNativeTitlebar  = setNativeTitlebarIntegrated
	readChromeSettings   = nativeChromeSettings
)

// windowChrome tells the frontend how to draw the title bar: whether it is
// integrated now, whether that is the saved choice (the two differ until a
// restart where the switch can't apply live), the platform whose controls to
// draw, and the desktop's button layout and double-click action, which it
// follows rather than offering its own settings.
func (a *App) windowChrome() (any, error) {
	layout, doubleClick := readChromeSettings()
	supported := integratedTitlebarSupported
	a.windowMu.Lock()
	wanted := a.window.Titlebar != titlebarSystem
	a.windowMu.Unlock()
	return map[string]any{
		"supported":   supported,
		"integrated":  supported && integratedTitlebar.Load(),
		"wanted":      supported && wanted,
		"platform":    runtime.GOOS,
		"layout":      layout,
		"doubleClick": doubleClick,
	}, nil
}

// windowSetTitlebar switches between the system and the integrated title bar
// and remembers the choice in window.json, which is read before the window is
// created on the next launch. Where the switch can't apply to the live window
// (Windows and Linux desktops with native server-side decorations), that launch
// is when it takes effect.
func (a *App) windowSetTitlebar(payload map[string]any) (any, error) {
	wanted, _ := payload["integrated"].(bool)
	integrated := wanted && integratedTitlebarSupported
	a.windowMu.Lock()
	a.window.Titlebar = ""
	if !wanted {
		a.window.Titlebar = titlebarSystem
	}
	a.windowMu.Unlock()
	a.flushWindowState()
	if titlebarSwitchesLive() && integratedTitlebar.Swap(integrated) != integrated {
		applyNativeTitlebar(integrated)
		refreshNativeTitlebarCss()
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) windowControl(command string) (any, error) {
	ctx := a.runtimeContext()
	switch command {
	case "window.minimise":
		if ctx != nil {
			windowMinimise(ctx)
		}
	case "window.toggleMaximise":
		if ctx != nil {
			windowToggleMaximise(ctx)
		}
	case "window.close":
		windowClose()
	}
	return map[string]any{"ok": true}, nil
}
