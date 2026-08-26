//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c
#cgo LDFLAGS: -framework AppKit

void setAppAppearanceDark(int dark);
*/
import "C"

// setNativeWindowDark pins NSApp's appearance to the app's own theme.
//
// The title bar is hidden here (see main.go), but the appearance still drives
// the traffic lights, the native context menus, sheets and scroll bars, which
// would otherwise follow the system while the window paints the opposite theme.
func setNativeWindowDark(dark bool) {
	value := C.int(0)
	if dark {
		value = 1
	}
	C.setAppAppearanceDark(value)
}
