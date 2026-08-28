//go:build linux

package main

import (
	"context"
	"strings"
	"testing"
)

// Raising a window on Linux takes an unmap/map cycle: mutter ignores a plain
// show and a gtk_window_present from an unfocused process, on Wayland and on
// X11 alike, so a notification or tray click would leave the window wherever it
// was. Guarding the remap on WindowIsMinimised — which is right on Windows,
// where unminimising un-maximises — silently broke that, so pin the sequence.
func TestShowAndRaiseRemapsWindow(t *testing.T) {
	restore := stubWindowCalls(t)
	defer restore()

	showAndRaiseMainWindow(context.Background())

	if got := strings.Join(windowCalls, ","); got != "hide,show,unminimise" {
		t.Fatalf("window calls = %q, want hide,show,unminimise", got)
	}
	if windowIsMinimisedCalls != 0 {
		t.Fatalf("raise consulted WindowIsMinimised %d times; the remap must not be conditional", windowIsMinimisedCalls)
	}
}
