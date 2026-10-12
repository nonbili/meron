//go:build !linux && !darwin

package main

// Windows keeps the colour icon: the setting isn't offered there.
func applyPlatformTrayIcon(unread, monochrome bool) error {
	traySetIcon(trayIcon(unread))
	return nil
}
