package main

import (
	"context"
	"runtime"

	"fyne.io/systray"
	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

func (a *App) startTrayPhysical() {
	if a.trayStop != nil {
		return
	}

	start, stop := systray.RunWithExternalLoop(a.trayReady, func() {
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

	show := systray.AddMenuItem("Show Meron", "Show Meron")
	hide := systray.AddMenuItem("Hide to Tray", "Hide Meron to the system tray")
	systray.AddSeparator()
	quit := systray.AddMenuItem("Quit Meron", "Quit Meron")

	go a.handleTrayClicks(show.ClickedCh, a.showMainWindow)
	go a.handleTrayClicks(hide.ClickedCh, a.hideMainWindow)
	go a.handleTrayClicks(quit.ClickedCh, a.quitFromTray)
}

func (a *App) handleTrayClicks(clicked <-chan struct{}, fn func()) {
	for range clicked {
		fn()
	}
}

func (a *App) showMainWindow() {
	ctx := a.runtimeContext()
	if ctx == nil {
		return
	}
	wailsRuntime.WindowShow(ctx)
	wailsRuntime.WindowUnminimise(ctx)
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
