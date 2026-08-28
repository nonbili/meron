//go:build !linux

package main

import (
	"context"

	"github.com/wailsapp/wails/v2/pkg/options"
)

// startWindowState maximises the window as it opens. Windows and macOS keep a
// sane restore geometry when a window starts maximised, so nothing else is
// needed there.
func startWindowState() options.WindowStartState {
	return options.Maximised
}

func maximiseOnDomReady(ctx context.Context) {}
