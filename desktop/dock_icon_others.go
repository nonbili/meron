//go:build !darwin && !linux

package main

import "meron/internal/appicon"

// Windows reads the taskbar and Start menu icon from the executable, which a
// running app cannot replace, so no choice is offered there.
func appIconStyleSupported() bool { return false }

func platformSetAppIcon(appicon.Variant, []byte) {}
