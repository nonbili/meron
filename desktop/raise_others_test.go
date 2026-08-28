//go:build !linux

package main

import (
	"context"
	"strings"
	"testing"
)

// Off Linux, showing already lifts the window, and unminimising a window that
// was hidden while maximised is SW_RESTORE — it would come back un-maximised.
func TestShowAndRaiseOnlyUnminimisesWhenMinimised(t *testing.T) {
	restore := stubWindowCalls(t)
	defer restore()

	showAndRaiseMainWindow(context.Background())
	if got := strings.Join(windowCalls, ","); got != "show" {
		t.Fatalf("window calls = %q, want show", got)
	}

	windowCalls = nil
	windowMinimised = true
	showAndRaiseMainWindow(context.Background())
	if got := strings.Join(windowCalls, ","); got != "show,unminimise" {
		t.Fatalf("minimised window calls = %q, want show,unminimise", got)
	}
}
