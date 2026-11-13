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

func TestBackgroundRequested(t *testing.T) {
	for _, test := range []struct {
		name string
		args []string
		want bool
	}{
		{name: "normal"},
		{name: "background", args: []string{"--background"}, want: true},
		{name: "with mailto", args: []string{"--background", "mailto:hello@example.com"}},
		{name: "filename", args: []string{"/tmp/--background.eml"}},
		{name: "after separator", args: []string{"--", "--background"}},
	} {
		t.Run(test.name, func(t *testing.T) {
			if got := backgroundRequested(test.args); got != test.want {
				t.Fatalf("backgroundRequested(%q) = %v, want %v", test.args, got, test.want)
			}
		})
	}
}

func TestBackgroundStartupStaysHiddenUntilNormalLaunch(t *testing.T) {
	calls := stubStartupReveal(t)
	defer stubWindowCalls(t)()
	a := &App{ctx: context.Background(), startInBackground: true}
	a.windowHidden.Store(true)

	// Frontend readiness and the safety timeout must leave background sync
	// running without opening the main window.
	if _, err := a.invoke("window.ready", nil); err != nil {
		t.Fatal(err)
	}
	timer := a.armStartupWindowFallback(time.Millisecond)
	defer timer.Stop()
	select {
	case call := <-calls:
		t.Fatalf("background startup received %q", call)
	case <-time.After(20 * time.Millisecond):
	}
	a.HandleSecondInstanceLaunch([]string{"--background"})
	if len(calls) != 0 || len(windowCalls) != 0 || !a.windowHidden.Load() {
		t.Fatal("second background launch opened the main window")
	}

	a.HandleSecondInstanceLaunch(nil)
	expectStartupReveal(t, calls, "reveal")
	expectStartupReveal(t, calls, "force")
	if len(windowCalls) == 0 || a.windowHidden.Load() {
		t.Fatal("normal launch did not show the background window")
	}
	if _, err := a.invoke("window.ready", nil); err != nil {
		t.Fatal(err)
	}
	if len(calls) != 0 {
		t.Fatal("late readiness repeated the startup reveal")
	}
}

func TestBackgroundShowArmsMaximiseBeforeMapping(t *testing.T) {
	stubStartupReveal(t)
	defer stubWindowCalls(t)()
	a := &App{ctx: context.Background(), startInBackground: true}
	a.window.Maximised = true
	a.windowHidden.Store(true)
	original := startupWindowMaximiseOnShow
	t.Cleanup(func() { startupWindowMaximiseOnShow = original })
	armed := false
	startupWindowMaximiseOnShow = func(_ context.Context, maximised bool) {
		if !maximised || !a.windowHidden.Load() || len(windowCalls) != 0 {
			t.Fatal("maximisation must be armed while the saved window is still hidden")
		}
		armed = true
	}
	a.showMainWindow()
	if !armed || len(windowCalls) == 0 {
		t.Fatal("background window was not armed and shown")
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
