//go:build (!linux && !darwin) || (linux && bindings)

package main

// Windows needs nothing here: the wails runtime's theme call already switches
// the DWM caption. Any other platform has no native chrome to tint.
func setNativeWindowDark(dark bool) {}
