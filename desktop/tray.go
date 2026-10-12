package main

import (
	"context"
	"fmt"
	"runtime"
	"sync"

	"fyne.io/systray"
	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// First-run default; saved style is restored natively before publication and
// supplied to the frontend by tray.getStyle.
var trayMonochromeDefault = runtime.GOOS != "windows"

// Routed through package vars so tests can see which icon was set.
var (
	traySetIcon           = systray.SetIcon
	trayApplyPlatformIcon = applyPlatformTrayIcon
	trayIconNeedsRepair   = platformTrayIconNeedsRepair
	trayOnTapped          = systray.SetOnTapped
)

var (
	trayMenuMu   sync.Mutex
	trayShowItem *systray.MenuItem
	trayHideItem *systray.MenuItem
	trayQuitItem *systray.MenuItem
)

func (a *App) startTrayPhysical() {
	if a.trayStop != nil {
		return
	}

	// Establish click semantics before the Linux ItemIsMenu snapshot.
	trayOnTapped(a.showMainWindow)
	if runtime.GOOS == "linux" {
		// The tray item is published while trayReady already runs, from what
		// was set before: a title or icon name set in between is lost. So set
		// them up front; trayReady sets them again for the other platforms.
		a.applyTrayIcon()
		a.waitForTrayIcon()
		systray.SetTitle(appTitle())
		systray.SetTooltip(appTitle())
	}
	start, stop := trayLoop(a.trayReady, func() {
		a.logf("tray stopped")
	})
	a.trayStop = stop
	start()
}

func (a *App) stopTray() {
	a.tray.stop()
	if a.trayStop == nil {
		return
	}

	a.trayStopOnce.Do(func() {
		defer func() {
			if recovered := recover(); recovered != nil {
				a.logf("tray stop recovered: %v", recovered)
			}
		}()
		a.trayStop()
	})
}

func (a *App) trayReady() {
	// Linux already has the pre-publication icon; other native backends need
	// their first write after creating the status item/window.
	a.requestTrayIcon(runtime.GOOS != "linux")
	if runtime.GOOS != "darwin" {
		// On macOS the title renders as text beside the menu bar icon.
		systray.SetTitle(appTitle())
	}
	systray.SetTooltip(appTitle())

	// Held across the reads and the AddMenuItem calls: the frontend pushes its
	// localized labels while this goroutine is starting, and a setNativeLabels
	// landing in between would find the items still nil and skip them, leaving
	// the English defaults installed here for the rest of the session.
	trayMenuMu.Lock()
	labels := a.currentNativeLabels()
	show := systray.AddMenuItem(labels.trayShow, labels.trayShow)
	hide := systray.AddMenuItem(labels.trayHide, labels.trayHideTooltip)
	systray.AddSeparator()
	quit := systray.AddMenuItem(labels.trayQuit, labels.trayQuit)
	trayShowItem, trayHideItem, trayQuitItem = show, hide, quit
	trayMenuMu.Unlock()

	go a.handleTrayClicks(show.ClickedCh, a.showMainWindow)
	go a.handleTrayClicks(hide.ClickedCh, a.hideMainWindow)
	go a.handleTrayClicks(quit.ClickedCh, a.quitFromTray)
}

func (a *App) handleTrayClicks(clicked <-chan struct{}, fn func()) {
	for range clicked {
		fn()
	}
}

// Window moves go through these vars rather than the wails runtime directly so
// showAndRaiseMainWindow, whose correct call sequence differs per platform and
// is easy to break from another platform's point of view, can be tested.
var (
	windowShow        = wailsRuntime.WindowShow
	windowHide        = wailsRuntime.WindowHide
	windowUnminimise  = wailsRuntime.WindowUnminimise
	windowIsMinimised = wailsRuntime.WindowIsMinimised
)

func (a *App) showMainWindow() {
	ctx := a.runtimeContext()
	if ctx == nil {
		return
	}
	a.windowHidden.Store(false)
	a.revealWindow(true)
	showAndRaiseMainWindow(ctx)
}

func (a *App) hideMainWindow() {
	ctx := a.runtimeContext()
	if ctx == nil {
		return
	}
	a.rememberWindowState(ctx)
	a.windowHidden.Store(true)
	windowHide(ctx)
}

func (a *App) quitFromTray() {
	a.quit()
}

func (a *App) runtimeContext() context.Context {
	if a.ctx == nil {
		a.logf("tray action ignored: runtime context is not ready")
		return nil
	}
	return a.ctx
}

func (a *App) traySetUnread(payload map[string]any) (any, error) {
	unread := false
	switch value := payload["unread"].(type) {
	case bool:
		unread = value
	case float64:
		unread = value > 0
	case int:
		unread = value > 0
	}
	a.setTrayUnread(unread)
	return map[string]any{"ok": true}, nil
}

func (a *App) setTrayUnread(unread bool) { a.trayState().setUnread(unread) }

func (a *App) traySetStyle(payload map[string]any) (any, error) {
	monochrome, ok := payload["monochrome"].(bool)
	if !ok {
		return nil, fmt.Errorf("tray.setStyle: monochrome must be a boolean")
	}
	a.trayStyleMu.Lock()
	defer a.trayStyleMu.Unlock()
	// Apply the live choice even if native storage is temporarily read-only.
	a.trayState().setMonochrome(monochrome)
	persisted := true
	if a.trayStylePath != "" {
		if err := saveTrayStyle(a.trayStylePath, monochrome); err != nil {
			persisted = false
			a.logf("tray: cannot save style (live choice still applied): %v", err)
		}
	}
	return map[string]any{"ok": true, "persisted": persisted}, nil
}

func (a *App) applyTrayIcon()             { a.requestTrayIcon(true) }
func (a *App) requestTrayIcon(force bool) { a.trayState().request(force) }

// Linux pre-publication waiting is outside the renderer's state lock.
func (a *App) waitForTrayIcon() { a.trayState().wait() }

func trayIcon(unread bool) []byte {
	if runtime.GOOS == "windows" {
		if unread {
			return trayIconUnreadICO
		}
		return trayIconICO
	}
	if unread {
		return trayIconUnreadPNG
	}
	return trayIconPNG
}
