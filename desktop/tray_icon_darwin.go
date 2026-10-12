//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c
#cgo LDFLAGS: -framework AppKit

void setTrayUnreadBadge(int unread);
*/
import "C"

var traySetUnreadBadge = func(unread bool) {
	value := C.int(0)
	if unread {
		value = 1
	}
	C.setTrayUnreadBadge(value)
}

func applyPlatformTrayIcon(unread, monochrome bool) error {
	if monochrome {
		// Template only the base: AppKit would recolour a dot in this image
		// too. The badge is a separate view over the status item's button.
		traySetTemplateIcon(trayIconMonoPNG, trayIcon(false))
	} else {
		traySetIcon(trayIcon(unread))
	}
	traySetUnreadBadge(monochrome && unread)
	return nil
}
