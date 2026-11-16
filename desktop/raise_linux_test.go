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

// With an activation token the window is presented in place: a remap would
// pull it onto the current workspace instead of switching to its own.
func TestShowAndRaiseUsesActivationToken(t *testing.T) {
	restore := stubWindowCalls(t)
	defer restore()
	activate := windowActivateWithToken
	defer func() { windowActivateWithToken = activate }()
	var activatedWith string
	windowActivateWithToken = func(token string) {
		activatedWith = token
		windowCalls = append(windowCalls, "activate")
	}

	setPendingActivationToken("click-token")
	showAndRaiseMainWindow(context.Background())

	if got := strings.Join(windowCalls, ","); got != "show,activate" {
		t.Fatalf("window calls = %q, want show,activate", got)
	}
	if activatedWith != "click-token" {
		t.Fatalf("activated with %q, want click-token", activatedWith)
	}

	// The token is single-use: the next raise (a tray click) remaps again.
	windowCalls = nil
	showAndRaiseMainWindow(context.Background())
	if got := strings.Join(windowCalls, ","); got != "hide,show,unminimise" {
		t.Fatalf("window calls after token use = %q, want hide,show,unminimise", got)
	}
}
