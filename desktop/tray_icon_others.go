//go:build !linux

package main

import "runtime"

// macOS draws a template image black or white to match the menu bar itself.
// Windows keeps the colour icon: the setting isn't offered there.
func applyPlatformTrayIcon(unread, monochrome bool) error {
	if monochrome && runtime.GOOS == "darwin" {
		traySetTemplateIcon(trayMonoIcon(unread), trayIcon(unread))
		return nil
	}
	traySetIcon(trayIcon(unread))
	return nil
}
