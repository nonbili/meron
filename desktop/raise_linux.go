//go:build linux

package main

import (
	"context"
	"sync"
)

var (
	activationTokenMu sync.Mutex
	// The activation token the notification server handed over with the click
	// being handled, consumed by the next showAndRaiseMainWindow.
	pendingActivationToken string
)

// setPendingActivationToken records the token a notification click came with,
// for the showAndRaiseMainWindow that click triggers.
func setPendingActivationToken(token string) {
	activationTokenMu.Lock()
	pendingActivationToken = token
	activationTokenMu.Unlock()
}

func takePendingActivationToken() string {
	activationTokenMu.Lock()
	defer activationTokenMu.Unlock()
	token := pendingActivationToken
	pendingActivationToken = ""
	return token
}

// Indirected so tests can observe the sequence without a GTK window.
var windowActivateWithToken = nativeActivateWithToken

// showAndRaiseMainWindow brings the window to the front, focused.
//
// The Wails runtime maps onto three GTK calls here: WindowShow is
// gtk_widget_show, WindowHide is gtk_widget_hide, WindowUnminimise is
// gtk_window_present. Neither show nor present raises a window that is merely
// behind another one: mutter refuses activation requests from a process that
// does not hold the focus and has no activation token to pass, on Wayland and
// on X11 alike.
//
// A notification click usually does come with a token (the server's
// ActivationToken signal, or the portal's platform data), and presenting with
// it is a real activation: mutter switches to the window's workspace and
// focuses it there. Without one (a tray click, or a server that sends none),
// what mutter does honour is a freshly mapped window, so unmapping and mapping
// again is the only route back to the front. That lands the window on the
// current workspace rather than its own, which is why the token is preferred.
// GTK reapplies the maximised state across the remap, so a maximised window
// returns maximised.
func showAndRaiseMainWindow(ctx context.Context) {
	if token := takePendingActivationToken(); token != "" {
		// A hidden (tray) window has to be mapped first; for a visible one
		// this is a no-op.
		windowShow(ctx)
		windowActivateWithToken(token)
		return
	}
	windowHide(ctx)
	windowShow(ctx)
	windowUnminimise(ctx)
}
