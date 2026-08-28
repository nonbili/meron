//go:build !linux

package main

import "context"

// showAndRaiseMainWindow brings the window to the front, focused.
//
// WindowShow already lifts a minimised window here, and unminimising on top of
// that is not a no-op: it is SW_RESTORE, which drops a maximised window back to
// its restored size. Restoring a window hidden while maximised should bring it
// back maximised, so only unminimise when it really is minimised.
func showAndRaiseMainWindow(ctx context.Context) {
	windowShow(ctx)
	if windowIsMinimised(ctx) {
		windowUnminimise(ctx)
	}
}
