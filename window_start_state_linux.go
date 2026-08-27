//go:build linux

package main

import (
	"context"
	"sync"

	"github.com/wailsapp/wails/v2/pkg/options"
	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// startWindowState leaves the window in its normal state on Linux.
//
// Maximising through WindowStartState happens in the same main loop turn that
// maps the window, so the window manager never sees an unmaximised geometry and
// has nothing sensible to restore to: unmaximising later (a double click on the
// title bar) drops the window at the monitor origin, with its title bar under
// the GNOME top bar. Mapping normal first and maximising from DomReady gives the
// window manager a real restore geometry.
func startWindowState() options.WindowStartState {
	return options.Normal
}

var maximiseOnce sync.Once

// maximiseOnDomReady maximises the window once the page is up, which is the
// earliest point where the window is guaranteed to be mapped.
//
// DomReady fires on every completed navigation, not only the first one - an
// error boundary reload, the OAuth flow, a dev server reload - so the maximise
// is guarded: a window the user has since unmaximised has to stay that way.
func maximiseOnDomReady(ctx context.Context) {
	maximiseOnce.Do(func() {
		wailsRuntime.WindowMaximise(ctx)
	})
}
