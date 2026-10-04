package main

import "time"

const startupWindowRevealDelay = 2 * time.Second

// Indirection lets the startup failure paths be tested without a native window.
var (
	startupWindowReveal      = revealStartupWindow
	startupWindowForceReveal = forceRevealStartupWindow
)

func (a *App) armStartupWindowFallback(delay time.Duration) *time.Timer {
	return time.AfterFunc(delay, func() {
		if a.ctx.Err() != nil || a.quitting.Load() || a.windowHidden.Load() {
			return
		}
		a.revealWindow(true)
	})
}

func (a *App) revealWindow(force bool) {
	a.windowReadyOnce.Do(func() { startupWindowReveal(a.ctx) })
	if force {
		// Linux can still be waiting for the window manager's maximise reply
		// after accepting window.ready. A timeout or explicit Show bypasses
		// that wait, even if the frontend already consumed windowReadyOnce.
		startupWindowForceReveal(a.ctx)
	}
}
