package main

import (
	"runtime"

	"fyne.io/systray"
)

// trayLoop runs systray on one dedicated OS thread.
//
// Win32 queues a window's messages to the thread that created it, and only that
// thread's GetMessage ever sees them. systray.RunWithExternalLoop creates the
// tray window on the caller's goroutine but then pumps messages from a freshly
// spawned one, which the Go scheduler is free to put on a different OS thread —
// so the icon appears (Shell_NotifyIcon has no thread affinity) while every
// click on it is dispatched to a queue nobody reads. That is why neither the
// left-click handler nor the right-click menu did anything.
//
// systray.Run does the window creation and the message loop back to back, so
// locking the goroutine to its thread for the duration keeps both on the same
// one. It blocks until Quit, hence the goroutine.
func trayLoop(ready, exit func()) (start, stop func()) {
	start = func() {
		go func() {
			runtime.LockOSThread()
			defer runtime.UnlockOSThread()
			systray.Run(ready, exit)
		}()
	}
	return start, systray.Quit
}
