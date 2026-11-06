package main

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestLoadWindowStateDefaultsToMaximised(t *testing.T) {
	path := filepath.Join(t.TempDir(), "window.json")
	want := windowState{Width: defaultWindowWidth, Height: defaultWindowHeight, Maximised: true}
	if got := loadWindowState(path); got != want {
		t.Fatalf("missing file: got %+v, want %+v", got, want)
	}
	if err := os.WriteFile(path, []byte("not json"), 0o600); err != nil {
		t.Fatal(err)
	}
	if got := loadWindowState(path); got != want {
		t.Fatalf("corrupt file: got %+v, want %+v", got, want)
	}
}

func TestLoadWindowStateRoundTrip(t *testing.T) {
	path := filepath.Join(t.TempDir(), "window.json")
	saved := windowState{Width: 900, Height: 700, Maximised: false}
	if err := saveWindowState(path, saved); err != nil {
		t.Fatal(err)
	}
	if got := loadWindowState(path); got != saved {
		t.Fatalf("got %+v, want %+v", got, saved)
	}
}

func TestLoadWindowStateIgnoresTinySize(t *testing.T) {
	path := filepath.Join(t.TempDir(), "window.json")
	if err := saveWindowState(path, windowState{Width: 10, Height: 10}); err != nil {
		t.Fatal(err)
	}
	want := windowState{Width: defaultWindowWidth, Height: defaultWindowHeight, Maximised: false}
	if got := loadWindowState(path); got != want {
		t.Fatalf("got %+v, want %+v", got, want)
	}
}

type fakeWindow struct {
	width, height int
	maximised     bool
	normal        bool
	tiled         bool
	// desktopFrame: the desktop draws the frame (KDE Plasma, X11), not GTK.
	desktopFrame bool
}

func TestShouldQuitPreservesStateSavedBeforeWindowDestruction(t *testing.T) {
	w := &fakeWindow{width: 1200, height: 800, maximised: true}
	stubWindowState(t, w)
	app := newWindowStateApp(t)
	app.closeToTray.Store(false)
	if app.beforeClose(app.ctx) {
		t.Fatal("close should proceed")
	}
	w.width, w.height, w.maximised = 0, 0, false
	if !app.shouldQuit() {
		t.Fatal("quit should proceed")
	}
	if saved := loadWindowState(app.windowStatePath); !saved.Maximised {
		t.Fatalf("destroyed window overwrote saved state: %+v", saved)
	}
}

func stubWindowState(t *testing.T, w *fakeWindow) {
	t.Helper()
	size, isMaximised, isNormal, isTiled, drawsFrame := windowGetSize, windowIsMaximised, windowIsNormal, windowIsTiled, windowDrawsFrame
	windowGetSize = func(context.Context) (int, int) { return w.width, w.height }
	windowIsMaximised = func(context.Context) bool { return w.maximised }
	windowIsNormal = func(context.Context) bool { return w.normal }
	windowIsTiled = func() bool { return w.tiled }
	windowDrawsFrame = func() bool { return !w.desktopFrame }
	t.Cleanup(func() {
		windowGetSize, windowIsMaximised, windowIsNormal, windowIsTiled, windowDrawsFrame = size, isMaximised, isNormal, isTiled, drawsFrame
	})
}

func newWindowStateApp(t *testing.T) *App {
	t.Helper()
	path := filepath.Join(t.TempDir(), "window.json")
	app := &App{ctx: context.Background(), windowStatePath: path}
	app.window = loadWindowState(path)
	app.windowSaved = app.window
	app.closeToTray.Store(true)
	return app
}

// A resize followed by a maximise inside the save delay, then a quit: the size
// the resize left is what unmaximising goes back to next launch.
func TestResizeThenQuickMaximiseThenQuitKeepsResizedSize(t *testing.T) {
	w := &fakeWindow{width: 1000, height: 600, normal: true}
	stubWindowState(t, w)
	app := newWindowStateApp(t)

	app.windowResized()
	*w = fakeWindow{width: 1920, height: 1080, maximised: true}
	app.windowResized()
	app.quitting.Store(true)
	if app.beforeClose(app.ctx) {
		t.Fatal("quit was prevented")
	}

	want := windowState{Width: 1000, Height: 600, Maximised: true}
	if got := loadWindowState(app.windowStatePath); got != want {
		t.Fatalf("got %+v, want %+v", got, want)
	}
}

func TestWindowResizedSavesOnceSettled(t *testing.T) {
	delay := windowSaveDelay
	windowSaveDelay = 20 * time.Millisecond
	t.Cleanup(func() { windowSaveDelay = delay })
	stubWindowState(t, &fakeWindow{width: 1000, height: 600, normal: true})
	app := newWindowStateApp(t)

	app.windowResized()
	if _, err := os.Stat(app.windowStatePath); !os.IsNotExist(err) {
		t.Fatalf("saved before settling: %v", err)
	}
	deadline := time.Now().Add(2 * time.Second)
	want := windowState{Width: 1000, Height: 600}
	for loadWindowState(app.windowStatePath) != want {
		if time.Now().After(deadline) {
			t.Fatalf("not saved after settling: got %+v", loadWindowState(app.windowStatePath))
		}
		time.Sleep(5 * time.Millisecond)
	}
}

func TestRememberWindowStateSkipsMinimisedAndHidden(t *testing.T) {
	w := &fakeWindow{width: 1000, height: 600}
	stubWindowState(t, w)
	app := newWindowStateApp(t)

	app.rememberWindowState(app.ctx)
	if _, err := os.Stat(app.windowStatePath); !os.IsNotExist(err) {
		t.Fatalf("minimised window was saved: %v", err)
	}

	w.normal = true
	app.windowHidden.Store(true)
	app.rememberWindowState(app.ctx)
	if _, err := os.Stat(app.windowStatePath); !os.IsNotExist(err) {
		t.Fatalf("hidden window was saved: %v", err)
	}
}

func TestBeforeClose(t *testing.T) {
	if hideOnCloseNatively {
		t.Skip("the close button hides natively on this platform")
	}
	defer stubWindowCalls(t)()
	stubWindowState(t, &fakeWindow{width: 1000, height: 600, normal: true})

	app := newWindowStateApp(t)
	if !app.beforeClose(app.ctx) {
		t.Fatal("close-to-tray: close was not prevented")
	}
	if len(windowCalls) != 1 || windowCalls[0] != "hide" || !app.windowHidden.Load() {
		t.Fatalf("close-to-tray: window calls %v, hidden %v", windowCalls, app.windowHidden.Load())
	}

	app = newWindowStateApp(t)
	app.closeToTray.Store(false)
	if app.beforeClose(app.ctx) {
		t.Fatal("close-to-tray off: close was prevented")
	}

	app = newWindowStateApp(t)
	app.quitting.Store(true)
	if app.beforeClose(app.ctx) {
		t.Fatal("explicit quit: close was prevented")
	}
	if got := loadWindowState(app.windowStatePath); got.Maximised || got.Width != 1000 {
		t.Fatalf("explicit quit: state not saved, got %+v", got)
	}
}

// The frontend rounds the window's corners only while it floats: square when
// maximised, fullscreen or tiled, and never where the platform doesn't ask for it.
func TestWindowResizedReportsRoundedCorners(t *testing.T) {
	rounded := roundedWindowCorners
	t.Cleanup(func() { roundedWindowCorners = rounded })
	w := &fakeWindow{width: 1000, height: 600, normal: true}
	stubWindowState(t, w)
	app := newWindowStateApp(t)

	cases := []struct {
		name     string
		platform bool
		window   fakeWindow
		want     bool
	}{
		{"floating", true, fakeWindow{width: 1000, height: 600, normal: true}, true},
		{"maximised", true, fakeWindow{width: 1920, height: 1080, maximised: true}, false},
		{"fullscreen", true, fakeWindow{width: 1920, height: 1080}, false},
		{"tiled", true, fakeWindow{width: 960, height: 1080, normal: true, tiled: true}, false},
		{"desktop frame", true, fakeWindow{width: 1000, height: 600, normal: true, desktopFrame: true}, false},
		{"other platform", false, fakeWindow{width: 1000, height: 600, normal: true}, false},
	}
	for _, c := range cases {
		roundedWindowCorners = c.platform
		*w = c.window
		result, err := app.windowResized()
		if err != nil {
			t.Fatal(err)
		}
		if got := result.(map[string]any)["rounded"]; got != c.want {
			t.Errorf("%s: rounded = %v, want %v", c.name, got, c.want)
		}
	}
}

func stubAppQuit(t *testing.T) *int {
	t.Helper()
	quits := 0
	orig := appQuit
	appQuit = func(context.Context) { quits++ }
	t.Cleanup(func() { appQuit = orig })
	return &quits
}

// Ctrl+Q does what the close button does: hide to the tray with close-to-tray
// on, quit with it off. macOS keeps its native ⌘Q, which always quits.
func TestRequestQuit(t *testing.T) {
	defer stubWindowCalls(t)()
	stubWindowState(t, &fakeWindow{width: 1000, height: 600, normal: true})

	app := newWindowStateApp(t)
	quits := stubAppQuit(t)
	app.requestQuit()
	if hideOnCloseNatively {
		if *quits != 1 || len(windowCalls) != 0 {
			t.Fatalf("macOS: quits %d, window calls %v", *quits, windowCalls)
		}
	} else if *quits != 0 || len(windowCalls) != 1 || windowCalls[0] != "hide" || !app.windowHidden.Load() {
		t.Fatalf("close-to-tray: quits %d, window calls %v, hidden %v", *quits, windowCalls, app.windowHidden.Load())
	}

	windowCalls = nil
	app = newWindowStateApp(t)
	app.closeToTray.Store(false)
	quits = stubAppQuit(t)
	app.requestQuit()
	if *quits != 1 || !app.quitting.Load() || len(windowCalls) != 0 {
		t.Fatalf("close-to-tray off: quits %d, quitting %v, window calls %v", *quits, app.quitting.Load(), windowCalls)
	}
}

func TestMainWindowCloseQuitsWithDocumentWindowsOpen(t *testing.T) {
	stubWindowState(t, &fakeWindow{width: 1000, height: 600, normal: true})
	app := newWindowStateApp(t)
	app.closeToTray.Store(false)
	app.emlReaders = map[string]*emlReader{"reader": {window: &fakeEmlWindow{}}}
	oldQuit := appQuit
	defer func() { appQuit = oldQuit }()
	quit := make(chan struct{}, 1)
	appQuit = func(context.Context) { quit <- struct{}{} }
	app.mainWindowCloseRequested()
	select {
	case <-quit:
	case <-time.After(time.Second):
		t.Fatal("closing main window did not quit while a reader was open")
	}
	if !app.quitting.Load() || !app.closing.Load() {
		t.Fatal("quit did not preserve the main window lifecycle state")
	}
}

func TestMainWindowCloseToTrayDoesNotQuitWithDocumentWindowsOpen(t *testing.T) {
	defer stubWindowCalls(t)()
	stubWindowState(t, &fakeWindow{width: 1000, height: 600, normal: true})
	app := newWindowStateApp(t)
	app.emlReaders = map[string]*emlReader{"reader": {window: &fakeEmlWindow{}}}
	app.mainWindowCloseRequested()
	if !app.windowHidden.Load() || app.closing.Load() || app.quitting.Load() {
		t.Fatal("close-to-tray did not preserve the application")
	}
	if len(windowCalls) != 1 || windowCalls[0] != "hide" {
		t.Fatalf("window calls: %v", windowCalls)
	}
}
