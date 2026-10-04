package main

import (
	"context"
	"testing"
	"time"
)

func stubStartupReveal(t *testing.T) <-chan string {
	t.Helper()
	reveal, force := startupWindowReveal, startupWindowForceReveal
	calls := make(chan string, 16)
	startupWindowReveal = func(context.Context) { calls <- "reveal" }
	startupWindowForceReveal = func(context.Context) { calls <- "force" }
	t.Cleanup(func() {
		startupWindowReveal, startupWindowForceReveal = reveal, force
	})
	return calls
}

func expectStartupReveal(t *testing.T, calls <-chan string, want string) {
	t.Helper()
	select {
	case got := <-calls:
		if got != want {
			t.Fatalf("startup call = %q, want %q", got, want)
		}
	case <-time.After(time.Second):
		t.Fatalf("startup call %q never arrived", want)
	}
}

func TestStartupFallbackWithoutFrontend(t *testing.T) {
	calls := stubStartupReveal(t)
	a := &App{ctx: context.Background()}
	timer := a.armStartupWindowFallback(time.Millisecond)
	defer timer.Stop()
	expectStartupReveal(t, calls, "reveal")
	expectStartupReveal(t, calls, "force")

	// A late/repeated frontend call must not reopen a window hidden later.
	a.revealWindow(false)
	a.revealWindow(false)
	if len(calls) != 0 {
		t.Fatal("late frontend readiness revealed the window again")
	}
}

func TestStartupFallbackForcesPendingMaximise(t *testing.T) {
	calls := stubStartupReveal(t)
	a := &App{ctx: context.Background()}
	if _, err := a.invoke("window.ready", nil); err != nil {
		t.Fatal(err)
	}
	expectStartupReveal(t, calls, "reveal")

	// The frontend already consumed the once guard, but Linux may still be
	// transparent while waiting for an unresponsive window manager.
	timer := a.armStartupWindowFallback(time.Millisecond)
	defer timer.Stop()
	expectStartupReveal(t, calls, "force")
	if len(calls) != 0 {
		t.Fatal("fallback repeated the normal reveal")
	}
}

func TestExplicitShowForcesStartupReveal(t *testing.T) {
	calls := stubStartupReveal(t)
	defer stubWindowCalls(t)()
	a := &App{ctx: context.Background()}
	a.showMainWindow()
	expectStartupReveal(t, calls, "reveal")
	expectStartupReveal(t, calls, "force")
	if len(windowCalls) == 0 {
		t.Fatal("explicit Show did not show and raise the window")
	}

	a.showMainWindow()
	expectStartupReveal(t, calls, "force")
	if len(calls) != 0 {
		t.Fatal("explicit Show repeated the normal reveal")
	}
}

func TestStartupFallbackSkipsClosedOrHiddenWindow(t *testing.T) {
	for _, state := range []string{"cancelled", "quitting", "hidden"} {
		t.Run(state, func(t *testing.T) {
			calls := stubStartupReveal(t)
			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()
			a := &App{ctx: ctx}
			switch state {
			case "cancelled":
				cancel()
			case "quitting":
				a.quitting.Store(true)
			case "hidden":
				a.windowHidden.Store(true)
			}
			timer := a.armStartupWindowFallback(time.Millisecond)
			defer timer.Stop()
			select {
			case call := <-calls:
				t.Fatalf("%s window received startup call %q", state, call)
			case <-time.After(20 * time.Millisecond):
			}
		})
	}
}
