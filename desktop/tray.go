package main

import (
	"context"
	"runtime"
	"sync"

	"fyne.io/systray"
	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
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

	start, stop := trayLoop(a.trayReady, func() {
		a.logf("tray stopped")
	})
	a.trayStop = stop
	start()
}

func (a *App) stopTray() {
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
	systray.SetIcon(trayIcon(a.currentTrayUnread()))
	if runtime.GOOS != "darwin" {
		// On macOS the title renders as text beside the menu bar icon.
		systray.SetTitle("Meron")
	}
	systray.SetTooltip("Meron")
	systray.SetOnTapped(func() {
		a.showMainWindow()
	})

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
	showAndRaiseMainWindow(ctx)
}

func (a *App) hideMainWindow() {
	ctx := a.runtimeContext()
	if ctx == nil {
		return
	}
	wailsRuntime.WindowHide(ctx)
}

func (a *App) quitFromTray() {
	ctx := a.runtimeContext()
	if ctx == nil {
		return
	}
	wailsRuntime.Quit(ctx)
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

func (a *App) setTrayUnread(unread bool) {
	a.trayMu.Lock()
	if a.trayHasUnread == unread {
		a.trayMu.Unlock()
		return
	}
	a.trayHasUnread = unread
	a.trayMu.Unlock()

	systray.SetIcon(trayIcon(unread))
}

func (a *App) currentTrayUnread() bool {
	a.trayMu.Lock()
	defer a.trayMu.Unlock()
	return a.trayHasUnread
}

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
