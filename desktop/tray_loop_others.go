//go:build !windows

package main

import "fyne.io/systray"

// trayLoop hands the tray to systray's external-loop mode: on macOS the caller
// has already put us on the main thread, and the Linux backend talks to the
// StatusNotifier host over D-Bus, so neither needs a thread of its own.
func trayLoop(ready, exit func()) (start, stop func()) {
	return systray.RunWithExternalLoop(ready, exit)
}
