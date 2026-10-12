//go:build darwin

package main

import "fyne.io/systray"

var traySetTemplateIconWithBadge = systray.SetTemplateIconWithBadge

func applyPlatformTrayIcon(unread, monochrome bool) error {
	if monochrome {
		// Template only the base: AppKit would recolour a dot in this image
		// too. The badge is a separate view over the status item's button.
		traySetTemplateIconWithBadge(trayIconMonoPNG, unread)
	} else {
		traySetIcon(trayIcon(unread))
	}
	return nil
}
