//go:build linux

package main

import "os"

// Linux shells take an app's icon from its desktop entry, for the launcher and
// for the running windows alike, and Meron writes that entry itself. Flatpak
// and Snap install their own, outside the sandbox, so there is nothing to
// change there; dev runs install no entry at all.
func appIconStyleSupported() bool {
	return os.Getenv("FLATPAK_ID") == "" && os.Getenv("SNAP_NAME") == "" && !isWailsDevLaunch()
}

// platformSetAppIcon points the desktop entry at a PNG, filed under name; nil
// restores the shipped icon.
func platformSetAppIcon(name string, png []byte) {
	installDesktopEntry(name, png)
}
