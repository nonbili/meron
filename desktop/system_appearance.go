package main

import (
	"sync"
	"time"

	wailsRuntime "meron/internal/desktopruntime"
)

// The OS light/dark preference, for the "Match system" theme setting.
//
// The webview's prefers-color-scheme cannot answer this: setWindowAppearance
// pins the native appearance to the app's own theme (NSApp on macOS, GTK's
// prefer-dark on Linux), and the webview then reports that pin back instead of
// the system's choice. So each platform reads the preference from its source
// directly and watches it for changes (see system_appearance_<os>.go).

var (
	systemAppearanceMu    sync.Mutex
	systemAppearanceKnown bool
	systemAppearanceDark  bool

	// Closed once the platform's first reading is in, or has failed, so the
	// boot query can wait for it instead of answering "unknown" and letting the
	// window paint the wrong appearance for a moment.
	systemAppearanceReady     = make(chan struct{})
	systemAppearanceReadyOnce sync.Once
)

// systemAppearanceSettled marks the first reading done, whatever its outcome.
func systemAppearanceSettled() {
	systemAppearanceReadyOnce.Do(func() { close(systemAppearanceReady) })
}

// systemAppearance answers "system.appearance": {dark: bool}, or {dark: null}
// while the platform has not reported a preference (the frontend then falls
// back to prefers-color-scheme).
func (a *App) systemAppearance() (any, error) {
	select {
	case <-systemAppearanceReady:
	case <-time.After(2 * time.Second):
	}
	systemAppearanceMu.Lock()
	defer systemAppearanceMu.Unlock()
	if !systemAppearanceKnown {
		return map[string]any{"dark": nil}, nil
	}
	return map[string]any{"dark": systemAppearanceDark}, nil
}

// setSystemAppearance records the platform's preference and, when it changed,
// tells the frontend so a theme that follows the system repaints.
func (a *App) setSystemAppearance(dark bool) {
	systemAppearanceMu.Lock()
	changed := !systemAppearanceKnown || systemAppearanceDark != dark
	systemAppearanceKnown = true
	systemAppearanceDark = dark
	systemAppearanceMu.Unlock()
	systemAppearanceSettled()
	if changed && a.ctx != nil {
		wailsRuntime.EventsEmit(a.ctx, "system.appearance", map[string]any{"dark": dark})
	}
}
