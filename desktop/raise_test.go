package main

import (
	"context"
	"testing"
)

var (
	windowCalls            []string
	windowIsMinimisedCalls int
	windowMinimised        bool
)

// stubWindowCalls records the window moves showAndRaiseMainWindow makes instead
// of driving a real window, and returns a func restoring the wails runtime.
func stubWindowCalls(t *testing.T) func() {
	t.Helper()
	show, hide := windowShow, windowHide
	unminimise, isMinimised := windowUnminimise, windowIsMinimised
	windowCalls = nil
	windowIsMinimisedCalls = 0
	windowShow = func(context.Context) { windowCalls = append(windowCalls, "show") }
	windowHide = func(context.Context) { windowCalls = append(windowCalls, "hide") }
	windowUnminimise = func(context.Context) { windowCalls = append(windowCalls, "unminimise") }
	windowIsMinimised = func(context.Context) bool {
		windowIsMinimisedCalls++
		return windowMinimised
	}
	return func() {
		windowShow, windowHide = show, hide
		windowUnminimise, windowIsMinimised = unminimise, isMinimised
		windowMinimised = false
	}
}
