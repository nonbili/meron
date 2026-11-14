//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c
#cgo LDFLAGS: -framework AppKit

void setDockIconPNG(const void *data, int length);
*/
import "C"
import (
	"unsafe"

	"meron/internal/appicon"
)

// The Dock, the app switcher and Mission Control draw the running app's icon,
// which AppKit lets the app replace. Finder and Launchpad read the bundle and
// keep the shipped icon, as does the Dock once Meron has quit.
func appIconStyleSupported() bool { return true }

// platformSetAppIcon draws a PNG as the running app's icon; nil restores the
// bundle's own.
func platformSetAppIcon(_ appicon.Variant, png []byte) {
	if len(png) == 0 {
		C.setDockIconPNG(nil, 0)
		return
	}
	C.setDockIconPNG(unsafe.Pointer(&png[0]), C.int(len(png)))
}
