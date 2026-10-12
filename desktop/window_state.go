package main

import (
	"context"
	"encoding/json"
	"os"
	"os/signal"
	"path/filepath"
	"runtime"
	"syscall"
	"time"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

const (
	defaultWindowWidth  = 1200
	defaultWindowHeight = 800
	minWindowWidth      = 480
	minWindowHeight     = 360
)

// hideOnCloseNatively is true where the close button hides the window by
// platform convention and quitting is its own command (⌘Q on macOS), so the
// close-to-tray setting does not apply there.
var hideOnCloseNatively = runtime.GOOS == "darwin"

// windowState is the main window geometry carried from one launch to the
// next. Width and Height are the unmaximised size, kept while the window is
// maximised so that unmaximising after a restart goes back to it. Position is
// left to the window manager: Wayland does not let clients place themselves,
// and elsewhere a remembered position can land on a monitor that is gone.
//
// It lives in its own file rather than the core's prefs because the window is
// created before the core is started.
type windowState struct {
	Width     int  `json:"width"`
	Height    int  `json:"height"`
	Maximised bool `json:"maximised"`
	// Titlebar is "system" once the integrated title bar (Linux) is turned
	// off, else empty for the default.
	Titlebar string `json:"titlebar,omitempty"`
}

// Window queries go through these vars so rememberWindowState can be tested.
var (
	windowGetSize     = wailsRuntime.WindowGetSize
	windowIsMaximised = wailsRuntime.WindowIsMaximised
	windowIsNormal    = wailsRuntime.WindowIsNormal
)

func windowStatePath() string {
	return filepath.Join(appConfigDir(), "window.json")
}

// loadWindowState reads the saved geometry, falling back to a maximised
// window at the default size on first launch or when the file is unusable.
func loadWindowState(path string) windowState {
	state := windowState{Width: defaultWindowWidth, Height: defaultWindowHeight, Maximised: true}
	data, err := os.ReadFile(path)
	if err != nil {
		return state
	}
	var saved windowState
	if err := json.Unmarshal(data, &saved); err != nil {
		return state
	}
	state.Maximised = saved.Maximised
	if saved.Titlebar == titlebarSystem {
		state.Titlebar = titlebarSystem
	}
	if saved.Width >= minWindowWidth && saved.Height >= minWindowHeight {
		state.Width, state.Height = saved.Width, saved.Height
	}
	return state
}

func saveWindowState(path string, state windowState) error {
	data, err := json.Marshal(state)
	if err != nil {
		return err
	}
	return writeFileAtomic(path, data, 0o600, 0o700)
}

// windowSaveDelay is how long a resize-driven save waits for the window to
// stop changing, so dragging an edge does not write the file every frame.
var windowSaveDelay = 500 * time.Millisecond

// sampleWindowState reads the window's current geometry into memory. It runs
// on every resize, so the unmaximised size is on record before a maximise hides
// it, and before the window is hidden or the app quits. A hidden window was
// sampled when it was hidden, and a minimised or fullscreen one says nothing
// about the size to come back to, so both keep what was recorded before.
func (a *App) sampleWindowState(ctx context.Context) {
	if a.windowHidden.Load() {
		return
	}
	maximised := windowIsMaximised(ctx)
	normal := !maximised && windowIsNormal(ctx)
	var width, height int
	if normal {
		width, height = windowGetSize(ctx)
	}
	a.windowMu.Lock()
	defer a.windowMu.Unlock()
	switch {
	case maximised:
		a.window.Maximised = true
	case normal:
		a.window.Maximised = false
		if width >= minWindowWidth && height >= minWindowHeight {
			a.window.Width, a.window.Height = width, height
		}
	}
}

// windowResized samples the window as it changes and saves once it settles.
func (a *App) windowResized() (any, error) {
	ctx := a.runtimeContext()
	if ctx == nil {
		return map[string]any{"ok": true}, nil
	}
	a.sampleWindowState(ctx)
	normal := windowIsNormal(ctx)
	// A tiled (half-screen) window is square like a maximised one, and so is a
	// window whose frame the desktop draws (see gtkFrame in window_chrome_linux.go).
	rounded := roundedWindowCorners && normal && !windowIsTiled() && windowDrawsFrame()
	a.windowMu.Lock()
	defer a.windowMu.Unlock()
	if a.window != a.windowSaved {
		if a.windowSaveTimer == nil {
			a.windowSaveTimer = time.AfterFunc(windowSaveDelay, a.flushWindowState)
		} else {
			a.windowSaveTimer.Reset(windowSaveDelay)
		}
	}
	return map[string]any{"ok": true, "rounded": rounded, "maximised": windowIsMaximised(ctx)}, nil
}

// rememberWindowState samples the window and saves it straight away, for
// when it is about to be hidden or the app is about to quit.
func (a *App) rememberWindowState(ctx context.Context) {
	a.sampleWindowState(ctx)
	a.flushWindowState()
}

func (a *App) flushWindowState() {
	a.windowMu.Lock()
	defer a.windowMu.Unlock()
	if a.windowSaveTimer != nil {
		a.windowSaveTimer.Stop()
	}
	if a.window == a.windowSaved {
		return
	}
	if err := saveWindowState(a.windowStatePath, a.window); err != nil {
		a.logf("window state save failed: %v", err)
		return
	}
	a.windowSaved = a.window
}

// beforeClose runs for the close button and for every quit. With
// close-to-tray on, the close button only hides the window; the tray's Quit
// and the updater mark the quit as intended first, so they still go through.
func (a *App) beforeClose(ctx context.Context) (prevent bool) {
	if !hideOnCloseNatively && !a.quitting.Load() && a.closeToTray.Load() {
		a.hideMainWindow()
		return true
	}
	// A signal delivers two quits (ours and Wails' own); only the first may
	// stop the main loop.
	if a.closing.Swap(true) {
		return true
	}
	a.rememberWindowState(ctx)
	return false
}

// appQuit is a var so tests can tell a quit from a hide without a running app.
var appQuit = wailsRuntime.Quit

// quit exits the app, bypassing close-to-tray.
func (a *App) quit() {
	ctx := a.runtimeContext()
	if ctx == nil {
		return
	}
	a.quitting.Store(true)
	appQuit(ctx)
}

// requestQuit is the Ctrl+Q shortcut (Linux/Windows; macOS has a native ⌘Q).
// It exits like the tray's Quit, except that close-to-tray turns it into
// hiding the window, the same as the close button.
func (a *App) requestQuit() {
	if !hideOnCloseNatively && a.closeToTray.Load() {
		a.hideMainWindow()
		return
	}
	a.quit()
}

// quitOnSignal makes SIGTERM and SIGINT quit for real. Wails turns them into
// the same quit request as the close button, which close-to-tray would
// otherwise answer by hiding the window.
func (a *App) quitOnSignal() {
	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGTERM, os.Interrupt)
	go func() {
		for range signals {
			a.quit()
		}
	}()
}

func (a *App) windowSetCloseToTray(payload map[string]any) (any, error) {
	enabled, ok := payload["enabled"].(bool)
	if ok {
		a.closeToTray.Store(enabled)
	}
	return map[string]any{"ok": true}, nil
}
